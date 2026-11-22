/*
 * sdrkit: the typed interface of the stack family "avdata", version 1.2. MIT licence.
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

#ifndef SDRKIT_AVDATA_H
#define SDRKIT_AVDATA_H

#include "../sdrkit.h"
#include "tones.h"

// The version of the family that this header describes.
#define SDRKIT_AVDATA_VERSION_MAJOR 1
#define SDRKIT_AVDATA_VERSION_MINOR 2

// The stack "acars": its id, and the keys of its event types.
#define SDRKIT_STACK_ACARS "acars"
#define SDRKIT_EVENT_ACARS_MESSAGE "acars.message"

// The stack "vdl2": its id, and the keys of its event types.
#define SDRKIT_STACK_VDL2 "vdl2"
#define SDRKIT_EVENT_VDL2_MESSAGE "vdl2.message"

// The stack "vdl4": its id, and the keys of its event types.
#define SDRKIT_STACK_VDL4 "vdl4"
#define SDRKIT_EVENT_VDL4_MESSAGE "vdl4.message"

// The stack "selcal": its id, and the keys of its event types.
#define SDRKIT_STACK_SELCAL "selcal"

// The stack "airband": its id, and the keys of its event types.
#define SDRKIT_STACK_AIRBAND "airband"
#define SDRKIT_EVENT_AIRBAND_ACTIVITY "airband.activity"

// The stack "hfdl": its id, and the keys of its event types.
#define SDRKIT_STACK_HFDL "hfdl"
#define SDRKIT_EVENT_HFDL_MESSAGE "hfdl.message"

// The stack "vdl3": its id, and the keys of its event types.
#define SDRKIT_STACK_VDL3 "vdl3"

// The stack "gbas_vdb": its id, and the keys of its event types.
#define SDRKIT_STACK_GBAS_VDB "gbas_vdb"

// The stack "ldacs": its id, and the keys of its event types.
#define SDRKIT_STACK_LDACS "ldacs"

// The stack "aeromacs": its id, and the keys of its event types.
#define SDRKIT_STACK_AEROMACS "aeromacs"

// The options of the setting "demod" of the stack "selcal".
typedef enum sdrkit_selcal_demod {
  // Auto
  SDRKIT_SELCAL_DEMOD_AUTO = 0,
  // FM
  SDRKIT_SELCAL_DEMOD_FM = 1,
  // USB
  SDRKIT_SELCAL_DEMOD_USB = 2,
  // LSB
  SDRKIT_SELCAL_DEMOD_LSB = 3,
  // AM
  SDRKIT_SELCAL_DEMOD_AM = 4,
} sdrkit_selcal_demod;

// The options of the setting "tx_spacing" of the stack "airband".
typedef enum sdrkit_airband_tx_spacing {
  // 8.33 kHz
  SDRKIT_AIRBAND_TX_SPACING_8_33 = 0,
  // 25 kHz
  SDRKIT_AIRBAND_TX_SPACING_25_KHZ = 1,
} sdrkit_airband_tx_spacing;

// The options of the setting "rs_bit_order" of the stack "vdl3".
typedef enum sdrkit_vdl3_rs_bit_order {
  // Least significant bit first
  SDRKIT_VDL3_RS_BIT_ORDER_LSB_FIRST = 0,
  // Most significant bit first
  SDRKIT_VDL3_RS_BIT_ORDER_MSB_FIRST = 1,
} sdrkit_vdl3_rs_bit_order;

// The result fields of ACARS.
typedef struct sdrkit_acars_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good blocks.
  int64_t frames_ok;
  // Bad blocks.
  int64_t frames_bad;
  // Good blocks that needed a repair.
  int64_t blocks_repaired;
  // Frame starts (SYN SYN SOH) seen.
  int64_t frames_started;
  // Messages with a position.
  int64_t positions;
  // Channels being decoded now.
  int64_t channels_active;
  // Aircraft in the table.
  int64_t aircraft;
  // Messages in the list.
  int64_t messages;
  // True if `good_pct` holds a value.
  bool has_good_pct;
  // Good blocks, percent of all blocks. No value before a block. Unit: %.
  double good_pct;
  // What is watched: the channel list, or the 25 kHz grid.
  sdrkit_string watching;
} sdrkit_acars_fields;

// A row of the messages. The row key is the place in the log.
typedef struct sdrkit_acars_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time. Unit: s.
  double signal_s;
  // The channel. A frequency in Hz.
  double frequency;
  // The level of the carrier. Unit: dB.
  double level_db;
  // True if `mode` holds a value.
  bool has_mode;
  // The mode character.
  sdrkit_string mode;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration.
  sdrkit_string registration;
  // True if `aircraft` holds a value.
  bool has_aircraft;
  // The row of the aircraft in the aircraft table.
  sdrkit_string aircraft;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id of a downlink.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label.
  sdrkit_string label;
  // True if `label_meaning` holds a value.
  bool has_label_meaning;
  // The meaning of the label.
  sdrkit_string label_meaning;
  // True if `block_id` holds a value.
  bool has_block_id;
  // The block id of the first block.
  sdrkit_string block_id;
  // True if `msn` holds a value.
  bool has_msn;
  // The message sequence number of a downlink.
  sdrkit_string msn;
  // Aircraft to ground.
  bool downlink;
  // True if `ack` holds a value.
  bool has_ack;
  // The technical acknowledgement: `NAK`, or the block id acknowledged.
  sdrkit_string ack;
  // True if `text` holds a value.
  bool has_text;
  // The printable text.
  sdrkit_string text;
  // True if `reading` holds a value.
  bool has_reading;
  // The reading of the fixed format.
  sdrkit_string reading;
  // True if `reading_kind` holds a value.
  bool has_reading_kind;
  // The kind of the reading.
  sdrkit_string reading_kind;
  // True if `reading_category` holds a value.
  bool has_reading_category;
  // The category of the reading.
  sdrkit_string reading_category;
  // True if `cpdlc_imi` holds a value.
  bool has_cpdlc_imi;
  // The CPDLC envelope: AT1, CR1, CC1 or DR1.
  sdrkit_string cpdlc_imi;
  // True if `cpdlc_ground` holds a value.
  bool has_cpdlc_ground;
  // The CPDLC ground facility.
  sdrkit_string cpdlc_ground;
  // True if `cpdlc_aircraft` holds a value.
  bool has_cpdlc_aircraft;
  // The aircraft named in the CPDLC envelope.
  sdrkit_string cpdlc_aircraft;
  // True if `cpdlc_crc_ok` holds a value.
  bool has_cpdlc_crc_ok;
  // The CPDLC envelope check passed.
  bool cpdlc_crc_ok;
  // True if `cpdlc_parsed` holds a value.
  bool has_cpdlc_parsed;
  // The CPDLC message body decoded completely.
  bool cpdlc_parsed;
  // True if `cpdlc_text` holds a value.
  bool has_cpdlc_text;
  // The decoded CPDLC message.
  sdrkit_string cpdlc_text;
  // Blocks joined.
  int64_t blocks;
  // Bits the repair changed.
  int64_t bits_fixed;
  // True if `position` holds a value.
  bool has_position;
  // The position the message gives.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude the message gives. Unit: ft.
  double alt_ft;
} sdrkit_acars_message_row;

// A row of the aircraft. The row key is the registration.
typedef struct sdrkit_acars_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `address` holds a value.
  bool has_address;
  // The 24-bit address, six hex digits.
  sdrkit_string address;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration.
  sdrkit_string registration;
  // True if `country` holds a value.
  bool has_country;
  // The State the address is allocated to.
  sdrkit_string country;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label of the last message.
  sdrkit_string label;
  // Messages of the aircraft.
  int64_t messages;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude of the last position. Unit: ft.
  double alt_ft;
  // True if `track_deg` holds a value.
  bool has_track_deg;
  // The track. Unit: deg.
  double track_deg;
  // True if `gs_kt` holds a value.
  bool has_gs_kt;
  // The ground speed. Unit: kt.
  double gs_kt;
  // True if `vs_fpm` holds a value.
  bool has_vs_fpm;
  // The vertical rate. Unit: ft/min.
  double vs_fpm;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the aircraft was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // The channel of the last message. A frequency in Hz.
  double frequency;
  // The level of the last downlink. Unit: dB.
  double level_db;
} sdrkit_acars_aircraft_row;

// A row of the channels. The row key is the frequency in hertz.
typedef struct sdrkit_acars_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel. A frequency in Hz.
  double frequency;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level now. No value without a carrier. Unit: dB.
  double level_db;
  // The signal to noise ratio in 7 kHz. Unit: dB.
  double snr_db;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // A decoder is on the channel.
  bool active;
  // Good blocks.
  int64_t good;
  // Bad blocks.
  int64_t bad;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The wall-clock time of the last good block, when known. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_acars_channel_row;

// The result fields of VDL Mode 2.
typedef struct sdrkit_vdl2_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good frames.
  int64_t frames_ok;
  // Bursts that failed.
  int64_t bursts_bad;
  // Octets the Reed-Solomon code corrected.
  int64_t octets_corrected;
  // Headers that needed their FEC.
  int64_t headers_fixed;
  // Frames with a bad FCS.
  int64_t fcs_bad;
  // Frames with a position.
  int64_t positions;
  // True if `tuning_error` holds a value.
  bool has_tuning_error;
  // How far the radio is off the channel grid. No value before the first report of the channels. A frequency in Hz.
  double tuning_error;
  // Aircraft in the table.
  int64_t aircraft;
  // Ground stations in the table.
  int64_t ground_stations;
  // Frames in the list.
  int64_t messages;
} sdrkit_vdl2_fields;

// A row of the frames. The row key is the place in the log.
typedef struct sdrkit_vdl2_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time. Unit: s.
  double signal_s;
  // The channel. A frequency in Hz.
  double frequency;
  // The level of the burst. Unit: dB.
  double level_db;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // The source address, six hex digits.
  sdrkit_string from;
  // The type of the source address.
  int64_t from_type;
  // The destination address, six hex digits.
  sdrkit_string to;
  // The type of the destination address.
  int64_t to_type;
  // True if `frame_kind` holds a value.
  bool has_frame_kind;
  // I, S or U.
  sdrkit_string frame_kind;
  // True if `frame` holds a value.
  bool has_frame;
  // The name of the frame.
  sdrkit_string frame;
  // The C/R bit of the source.
  bool response;
  // True if `protocol` holds a value.
  bool has_protocol;
  // The deepest layer read.
  sdrkit_string protocol;
  // True if `summary` holds a value.
  bool has_summary;
  // One line.
  sdrkit_string summary;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration of an ACARS message.
  sdrkit_string registration;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id of an ACARS message.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label of an ACARS message.
  sdrkit_string label;
  // True if `text` holds a value.
  bool has_text;
  // The text of an ACARS message.
  sdrkit_string text;
  // The ACARS check sequence passed.
  bool acars_ok;
  // True if `blocks` holds a value.
  bool has_blocks;
  // Blocks joined into the ACARS message.
  int64_t blocks;
  // True if `reading_kind` holds a value.
  bool has_reading_kind;
  // The kind of the reading.
  sdrkit_string reading_kind;
  // True if `reading_category` holds a value.
  bool has_reading_category;
  // The category of the reading.
  sdrkit_string reading_category;
  // True if `cpdlc_imi` holds a value.
  bool has_cpdlc_imi;
  // The CPDLC envelope: AT1, CR1, CC1 or DR1.
  sdrkit_string cpdlc_imi;
  // True if `cpdlc_ground` holds a value.
  bool has_cpdlc_ground;
  // The CPDLC ground facility.
  sdrkit_string cpdlc_ground;
  // True if `cpdlc_aircraft` holds a value.
  bool has_cpdlc_aircraft;
  // The aircraft named in the CPDLC envelope.
  sdrkit_string cpdlc_aircraft;
  // True if `cpdlc_crc_ok` holds a value.
  bool has_cpdlc_crc_ok;
  // The CPDLC envelope check passed.
  bool cpdlc_crc_ok;
  // True if `cpdlc_parsed` holds a value.
  bool has_cpdlc_parsed;
  // The CPDLC message body decoded completely.
  bool cpdlc_parsed;
  // True if `cpdlc_text` holds a value.
  bool has_cpdlc_text;
  // The decoded CPDLC message.
  sdrkit_string cpdlc_text;
  // True if `position` holds a value.
  bool has_position;
  // The position the frame gives.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude the frame gives. Unit: ft.
  double alt_ft;
  // True if `detail` holds a value.
  bool has_detail;
  // Every layer, one line each, joined by " | ".
  sdrkit_string detail;
} sdrkit_vdl2_message_row;

// A row of the aircraft. The row key is the address.
typedef struct sdrkit_vdl2_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `address` holds a value.
  bool has_address;
  // The 24-bit address, six hex digits.
  sdrkit_string address;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration.
  sdrkit_string registration;
  // True if `country` holds a value.
  bool has_country;
  // The State the address is allocated to.
  sdrkit_string country;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label of the last message.
  sdrkit_string label;
  // Messages of the aircraft.
  int64_t messages;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude of the last position. Unit: ft.
  double alt_ft;
  // True if `track_deg` holds a value.
  bool has_track_deg;
  // The track. Unit: deg.
  double track_deg;
  // True if `gs_kt` holds a value.
  bool has_gs_kt;
  // The ground speed. Unit: kt.
  double gs_kt;
  // True if `vs_fpm` holds a value.
  bool has_vs_fpm;
  // The vertical rate. Unit: ft/min.
  double vs_fpm;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the aircraft was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // Frames to or from the aircraft.
  int64_t count;
  // The channel of the last frame. A frequency in Hz.
  double frequency;
  // True if `ground_station` holds a value.
  bool has_ground_station;
  // The ground station the aircraft last talked to.
  sdrkit_string ground_station;
  // True if `estimate` holds a value.
  bool has_estimate;
  // The position now, by dead reckoning from the last fix.
  sdrkit_position estimate;
  // The points of the trail.
  int64_t trail_points;
} sdrkit_vdl2_aircraft_row;

// A row of the ground stations. The row key is the address.
typedef struct sdrkit_vdl2_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address, six hex digits.
  sdrkit_string address;
  // The kind of the station.
  sdrkit_string kind;
  // Frames the station sent.
  int64_t count;
  // The channel. A frequency in Hz.
  double frequency;
  // The level of its last frame. Unit: dB.
  double level_db;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the station was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `airports` holds a value.
  bool has_airports;
  // The airports the station covers.
  sdrkit_string airports;
  // True if `position` holds a value.
  bool has_position;
  // The position from its GSIF.
  sdrkit_position position;
} sdrkit_vdl2_station_row;

// A row of the channels heard now. The row key is the frequency in hertz.
typedef struct sdrkit_vdl2_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel. A frequency in Hz.
  double frequency;
  // The channel is inside the band of the input.
  bool in_band;
  // A good burst in the last 2 s.
  bool active;
  // The level of the channel. Unit: dB.
  double level_db;
  // The signal to noise ratio of the channel. Unit: dB.
  double snr_db;
  // Bursts found.
  int64_t found;
  // Good bursts.
  int64_t good;
  // Bad bursts.
  int64_t bad;
  // Frames.
  int64_t frames;
  // The offset of the last burst. A frequency in Hz.
  double offset_hz;
} sdrkit_vdl2_channel_row;

// The result fields of VDL Mode 4.
typedef struct sdrkit_vdl4_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a good FCS.
  int64_t frames;
  // Bursts that failed on the channels now heard.
  int64_t bad;
  // Aircraft in the table.
  int64_t aircraft;
  // Ground stations in the table.
  int64_t ground_stations;
} sdrkit_vdl4_fields;

// A row of the messages. The row key is the place in the log.
typedef struct sdrkit_vdl4_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time. Unit: s.
  double signal_s;
  // The channel. A frequency in Hz.
  double frequency;
  // The level of the burst. Unit: dB.
  double level_db;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // The address of the sender, six hex digits.
  sdrkit_string address;
  // The kind of the address.
  sdrkit_string address_kind;
  // True if `message` holds a value.
  bool has_message;
  // The name of the message.
  sdrkit_string message;
  // True if `summary` holds a value.
  bool has_summary;
  // One line.
  sdrkit_string summary;
  // True if `dls_text` holds a value.
  bool has_dls_text;
  // The text of the DLS.
  sdrkit_string dls_text;
  // True if `position` holds a value.
  bool has_position;
  // The position the message gives.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude the message gives. Unit: ft.
  double alt_ft;
} sdrkit_vdl4_message_row;

// A row of the aircraft. The row key is the address.
typedef struct sdrkit_vdl4_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `address` holds a value.
  bool has_address;
  // The 24-bit address, six hex digits.
  sdrkit_string address;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration.
  sdrkit_string registration;
  // True if `country` holds a value.
  bool has_country;
  // The State the address is allocated to.
  sdrkit_string country;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label of the last message.
  sdrkit_string label;
  // Messages of the aircraft.
  int64_t messages;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude of the last position. Unit: ft.
  double alt_ft;
  // True if `track_deg` holds a value.
  bool has_track_deg;
  // The track. Unit: deg.
  double track_deg;
  // True if `gs_kt` holds a value.
  bool has_gs_kt;
  // The ground speed. Unit: kt.
  double gs_kt;
  // True if `vs_fpm` holds a value.
  bool has_vs_fpm;
  // The vertical rate. Unit: ft/min.
  double vs_fpm;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the aircraft was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // Messages heard from it on the link.
  int64_t count;
  // The channel of the last burst. A frequency in Hz.
  double frequency;
  // The points of the trail.
  int64_t trail_points;
} sdrkit_vdl4_aircraft_row;

// A row of the ground stations. The row key is the address.
typedef struct sdrkit_vdl4_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address, six hex digits.
  sdrkit_string address;
  // Messages of the station.
  int64_t count;
  // The channel. A frequency in Hz.
  double frequency;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the station was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `position` holds a value.
  bool has_position;
  // The position the station gave.
  sdrkit_position position;
} sdrkit_vdl4_station_row;

// A row of the channels heard now. The row key is the frequency in hertz.
typedef struct sdrkit_vdl4_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel. A frequency in Hz.
  double frequency;
  // The channel is inside the band of the input.
  bool in_band;
  // Bursts found.
  int64_t found;
  // Good bursts.
  int64_t good;
  // Bad bursts.
  int64_t bad;
  // The level of the channel. Unit: dB.
  double level_db;
  // The signal to noise ratio of the channel. Unit: dB.
  double snr_db;
} sdrkit_vdl4_channel_row;

// A row of the calls. The row key is the place in the log.
typedef struct sdrkit_selcal_call_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time. Unit: s.
  double signal_s;
  // The code, "AM-CP".
  sdrkit_string code;
  // The code as its row key in the codes.
  sdrkit_string code_row;
  // The four tones, "a b / c d". Unit: Hz.
  sdrkit_string tones;
  // True if `dial` holds a value.
  bool has_dial;
  // The dial frequency. No value when it is not known. A frequency in Hz.
  double dial;
  // The level of the tones, dB relative to a full-scale sine. Unit: dB.
  double level_db;
  // True if `aircraft` holds a value.
  bool has_aircraft;
  // The aircraft the code belongs to.
  sdrkit_string aircraft;
} sdrkit_selcal_call_row;

// A row of the codes heard. The row key is the code.
typedef struct sdrkit_selcal_code_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The code.
  sdrkit_string code;
  // True if `aircraft` holds a value.
  bool has_aircraft;
  // The aircraft the code belongs to.
  sdrkit_string aircraft;
  // True if `heard` holds a value.
  bool has_heard;
  // How often the code was heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the code was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `dial` holds a value.
  bool has_dial;
  // The last dial frequency. No value when it is not known. A frequency in Hz.
  double dial;
} sdrkit_selcal_code_row;

// The result fields of Airband.
typedef struct sdrkit_airband_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The rate of the channel receivers. A frequency in Hz.
  double channel_rate;
  // True if `tuning_error` holds a value.
  bool has_tuning_error;
  // How far the radio is off. No value while it is not known. A frequency in Hz.
  double tuning_error;
  // The squelch over the noise. Unit: dB.
  double squelch_db;
  // How long a channel stays open after its carrier goes. A length of time in seconds.
  double hang;
  // The scan is on.
  bool scan;
  // True if `scan_channel` holds a value.
  bool has_scan_channel;
  // The channel the scan holds. No value while it holds none. A frequency in Hz.
  double scan_channel;
  // Transmissions heard.
  int64_t transmissions;
} sdrkit_airband_fields;

// The result fields of the Airband transmitter. The measurements have no value while the stack does not transmit.
typedef struct sdrkit_airband_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the stack transmits now.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the tuned frequency. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_time_s` holds a value.
  bool has_tx_time_s;
  // The time on air of the transmission. A length of time in seconds.
  double tx_time_s;
  // True if `tx_mic_dbfs` holds a value.
  bool has_tx_mic_dbfs;
  // The peak of the microphone audio over the last report. Unit: dBFS.
  double tx_mic_dbfs;
  // True if `tx_depth` holds a value.
  bool has_tx_depth;
  // The peak modulation depth over the last report. Unit: %.
  double tx_depth;
} sdrkit_airband_tx_fields;

// A row of the channels. The row key is the name of the channel.
typedef struct sdrkit_airband_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the channel.
  sdrkit_string name;
  // True if `label` holds a value.
  bool has_label;
  // The label of the channel.
  sdrkit_string label;
  // The frequency of the channel. A frequency in Hz.
  double frequency;
  // The channel is inside the band of the input.
  bool in_band;
  // The carrier over the noise. Unit: dB.
  double cn_db;
  // The level of the channel. Unit: dB.
  double level_db;
  // True if `squelch_db` holds a value.
  bool has_squelch_db;
  // The squelch of the channel, when it has its own. Unit: dB.
  double squelch_db;
  // The squelch is open.
  bool open;
  // The channel plays.
  bool playing;
  // Two stations on the channel at once.
  bool heterodyne;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // Transmissions on the channel.
  int64_t count;
} sdrkit_airband_channel_row;

// A row of the band watch: a carrier on the air that is not in the list. The row key is its name.
typedef struct sdrkit_airband_seen_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the channel.
  sdrkit_string name;
  // The frequency. A frequency in Hz.
  double frequency;
  // The channel is on the 8.33 kHz grid.
  bool is833;
  // The level over the floor. Unit: dB.
  double excess_db;
  // How often the carrier was heard.
  int64_t reports;
  // The carrier is on now.
  bool active;
  // The frequency is approximate.
  bool approximate;
  // The signal time it was last seen. Unit: s.
  double last_s;
} sdrkit_airband_seen_row;

// A row of the transmissions. The row key is the place in the log.
typedef struct sdrkit_airband_activity_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time of the start. Unit: s.
  double start_s;
  // The name of the channel.
  sdrkit_string name;
  // True if `label` holds a value.
  bool has_label;
  // The label of the channel.
  sdrkit_string label;
  // The frequency of the channel. A frequency in Hz.
  double frequency;
  // The length of the transmission. A length of time in seconds.
  double length;
  // The carrier over the noise. Unit: dB.
  double cn_db;
  // The level. Unit: dB.
  double level_db;
  // Two stations at once.
  bool heterodyne;
} sdrkit_airband_activity_row;

// The result fields of HFDL.
typedef struct sdrkit_hfdl_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good PDUs: squitters and MPDUs, one a burst.
  int64_t pdus;
  // Bursts that failed.
  int64_t bursts_bad;
  // Squitters.
  int64_t squitters;
  // LPDUs.
  int64_t lpdus;
  // ACARS messages.
  int64_t acars;
  // Messages with a position.
  int64_t positions;
  // Good bursts at 300 bit/s.
  int64_t bursts_300;
  // Good bursts at 600 bit/s.
  int64_t bursts_600;
  // Good bursts at 1200 bit/s.
  int64_t bursts_1200;
  // Good bursts at 1800 bit/s.
  int64_t bursts_1800;
  // Bursts lost: the preamble was refused.
  int64_t lost_preamble;
  // Bursts lost: cut short.
  int64_t lost_short;
  // Squitters lost to a bad FCS.
  int64_t lost_squitter_fcs;
  // Bursts lost to a bad header FCS.
  int64_t lost_header_fcs;
  // LPDUs lost to a bad FCS.
  int64_t lost_lpdu_fcs;
  // LPDUs of an unknown type.
  int64_t lost_lpdu_type;
  // What the band hopper does.
  sdrkit_string hopper;
  // Aircraft in the table.
  int64_t aircraft;
} sdrkit_hfdl_fields;

// A row of the messages. The row key is the place in the log.
typedef struct sdrkit_hfdl_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time. Unit: s.
  double signal_s;
  // The dial frequency of the channel. A frequency in Hz.
  double dial;
  // The level of the burst. Unit: dB.
  double level_db;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // The offset of the burst. A frequency in Hz.
  double offset_hz;
  // The bit rate of the burst. Unit: bit/s.
  int64_t bps;
  // The ground station id.
  int64_t gs_id;
  // The ground station as its row key.
  sdrkit_string ground_station;
  // True if `ac_id` holds a value.
  bool has_ac_id;
  // The aircraft id the ground station gave.
  int64_t ac_id;
  // True if `kind` holds a value.
  bool has_kind;
  // The kind of the PDU.
  sdrkit_string kind;
  // True if `hfnpdu` holds a value.
  bool has_hfnpdu;
  // The kind of the HFNPDU.
  sdrkit_string hfnpdu;
  // Ground to aircraft.
  bool uplink;
  // True if `summary` holds a value.
  bool has_summary;
  // One line.
  sdrkit_string summary;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration of an ACARS message.
  sdrkit_string registration;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label of an ACARS message.
  sdrkit_string label;
  // True if `block_id` holds a value.
  bool has_block_id;
  // The block id of an ACARS message.
  sdrkit_string block_id;
  // True if `msn` holds a value.
  bool has_msn;
  // The message sequence number of an ACARS message.
  sdrkit_string msn;
  // True if `text` holds a value.
  bool has_text;
  // The text of an ACARS message.
  sdrkit_string text;
  // The ACARS check sequence passed.
  bool acars_ok;
  // True if `reading_kind` holds a value.
  bool has_reading_kind;
  // The kind of the reading.
  sdrkit_string reading_kind;
  // True if `reading_category` holds a value.
  bool has_reading_category;
  // The category of the reading.
  sdrkit_string reading_category;
  // True if `cpdlc_imi` holds a value.
  bool has_cpdlc_imi;
  // The CPDLC envelope: AT1, CR1, CC1 or DR1.
  sdrkit_string cpdlc_imi;
  // True if `cpdlc_ground` holds a value.
  bool has_cpdlc_ground;
  // The CPDLC ground facility.
  sdrkit_string cpdlc_ground;
  // True if `cpdlc_aircraft` holds a value.
  bool has_cpdlc_aircraft;
  // The aircraft named in the CPDLC envelope.
  sdrkit_string cpdlc_aircraft;
  // True if `cpdlc_crc_ok` holds a value.
  bool has_cpdlc_crc_ok;
  // The CPDLC envelope check passed.
  bool cpdlc_crc_ok;
  // True if `cpdlc_parsed` holds a value.
  bool has_cpdlc_parsed;
  // The CPDLC message body decoded completely.
  bool cpdlc_parsed;
  // True if `cpdlc_text` holds a value.
  bool has_cpdlc_text;
  // The decoded CPDLC message.
  sdrkit_string cpdlc_text;
  // True if `position` holds a value.
  bool has_position;
  // The position the message gives.
  sdrkit_position position;
  // True if `detail` holds a value.
  bool has_detail;
  // Every decoded layer, joined by " | ".
  sdrkit_string detail;
} sdrkit_hfdl_message_row;

// A row of the aircraft. The row key is the address, or the ground station and the aircraft id while the address is not known.
typedef struct sdrkit_hfdl_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key.
  sdrkit_string key_;
  // True if `address` holds a value.
  bool has_address;
  // The 24-bit address, six hex digits.
  sdrkit_string address;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration.
  sdrkit_string registration;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id.
  sdrkit_string flight;
  // The ground station id.
  int64_t gs_id;
  // The ground station as its row key.
  sdrkit_string ground_station;
  // Messages of the aircraft.
  int64_t count;
  // The dial frequency of the last message. A frequency in Hz.
  double dial;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the aircraft was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // The aircraft is logged on.
  bool logged_on;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude of the last position. Unit: ft.
  double alt_ft;
} sdrkit_hfdl_aircraft_row;

// A row of the ground stations. The row key is the station id.
typedef struct sdrkit_hfdl_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station id.
  int64_t gs_id;
  // The name of the station.
  sdrkit_string name;
  // Heard directly: a squitter or an uplink.
  bool heard_direct;
  // The station is synchronised to UTC.
  bool utc_sync;
  // Squitters.
  int64_t squitters;
  // Messages that name the station.
  int64_t count;
  // The dial frequency it was heard on. A frequency in Hz.
  double dial;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the station was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `frame_index` holds a value.
  bool has_frame_index;
  // The frame index of the last squitter.
  int64_t frame_index;
  // True if `active_khz` holds a value.
  bool has_active_khz;
  // The frequencies in use by the last squitter, kHz, separated by spaces.
  sdrkit_string active_khz;
} sdrkit_hfdl_station_row;

// A row of the channels. The row key is the dial frequency in hertz.
typedef struct sdrkit_hfdl_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The dial frequency. A frequency in Hz.
  double dial;
  // True if `name` holds a value.
  bool has_name;
  // The name of the ground station of the channel.
  sdrkit_string name;
  // True if `ground_station` holds a value.
  bool has_ground_station;
  // The ground station as its row key.
  sdrkit_string ground_station;
  // The channel is inside the band of the input.
  bool in_band;
  // Preambles found.
  int64_t found;
  // Good bursts.
  int64_t good;
  // Bad bursts.
  int64_t bad;
  // The signal to noise ratio of the channel. Unit: dB.
  double snr_db;
  // The offset of the last burst. A frequency in Hz.
  double offset_hz;
  // The bit rate of the last burst. Unit: bit/s.
  int64_t bps;
} sdrkit_hfdl_channel_row;

// The result fields of VDL Mode 3.
typedef struct sdrkit_vdl3_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The state: 0 listening, 1 bursts seen, 2 decoding.
  int64_t rx_state;
  // Training sequences found.
  int64_t trainings;
  // Good bursts.
  int64_t frames_ok;
  // Bursts that failed.
  int64_t frames_bad;
  // M uplink bursts.
  int64_t m_uplink;
  // M uplink bursts of the 3T configuration.
  int64_t m_uplink_3t;
  // Handoff check bursts.
  int64_t handoff;
  // Poll response bursts.
  int64_t poll_response;
  // M downlink bursts.
  int64_t m_downlink;
  // Net entry request bursts.
  int64_t net_entry;
  // V/D data bursts.
  int64_t vd_data;
  // V/D voice bursts.
  int64_t vd_voice;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the last burst. No value before a burst. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last burst. No value before a burst. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last burst. No value before a burst. A frequency in Hz.
  double offset_hz;
  // True if `text` holds a value.
  bool has_text;
  // The summary of the newest burst.
  sdrkit_string text;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_vdl3_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_avdata_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_avdata_decoder_fields;

// The options of `kind` of `sdrkit_vdl3_burst_row`.
typedef enum sdrkit_vdl3_burst_row_kind {
  // M uplink
  SDRKIT_VDL3_BURST_ROW_KIND_M_UPLINK = 0,
  // M uplink (3T)
  SDRKIT_VDL3_BURST_ROW_KIND_M_UPLINK_3T = 1,
  // H handoff check
  SDRKIT_VDL3_BURST_ROW_KIND_HANDOFF = 2,
  // Poll response
  SDRKIT_VDL3_BURST_ROW_KIND_POLL_RESPONSE = 3,
  // M downlink
  SDRKIT_VDL3_BURST_ROW_KIND_M_DOWNLINK = 4,
  // Net entry request
  SDRKIT_VDL3_BURST_ROW_KIND_NET_ENTRY = 5,
  // V/D data
  SDRKIT_VDL3_BURST_ROW_KIND_VD_DATA = 6,
  // V/D voice
  SDRKIT_VDL3_BURST_ROW_KIND_VD_VOICE = 7,
} sdrkit_vdl3_burst_row_kind;

// The options of `training` of `sdrkit_vdl3_burst_row`.
typedef enum sdrkit_vdl3_burst_row_training {
  // S1
  SDRKIT_VDL3_BURST_ROW_TRAINING_S1 = 0,
  // S1*
  SDRKIT_VDL3_BURST_ROW_TRAINING_S1_STAR = 1,
  // S2
  SDRKIT_VDL3_BURST_ROW_TRAINING_S2 = 2,
  // S2*
  SDRKIT_VDL3_BURST_ROW_TRAINING_S2_STAR = 3,
} sdrkit_vdl3_burst_row_training;

// A row of the bursts. The row key is the place in the log.
typedef struct sdrkit_vdl3_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the end of the training. Unit: s.
  double time_s;
  // The burst kind. A value of `sdrkit_vdl3_burst_row_kind`.
  int32_t kind;
  // The training sequence. A value of `sdrkit_vdl3_burst_row_training`.
  int32_t training;
  // Symbols after the training.
  int64_t symbols;
  // Every Golay word decoded.
  bool golay_ok;
  // Bits corrected by Golay. Unit: bits.
  int64_t golay_corrected;
  // The information words.
  sdrkit_string info;
  // The Reed-Solomon word was good.
  bool rs_ok;
  // Octets corrected by Reed-Solomon. Unit: octets.
  int64_t rs_corrected;
  // The data octets, hex.
  sdrkit_string data_hex;
  // The printable text of the data.
  sdrkit_string data_text;
  // The level of the burst. Unit: dBFS.
  double level_dbfs;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // The carrier offset of the burst. Unit: Hz.
  double cfo_hz;
  // The error vector magnitude of the burst. Unit: %.
  double evm_pct;
  // The summary of the burst.
  sdrkit_string summary;
} sdrkit_vdl3_burst_row;

// The result fields of GBAS VDB.
typedef struct sdrkit_gbas_vdb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Slots of 8 that held a burst in the last frame.
  int64_t slots_busy;
  // The line of the symbol rate was seen.
  bool symbol_rate_seen;
  // Bursts counted.
  int64_t bursts;
  // True if `length_ms` holds a value.
  bool has_length_ms;
  // The length of the last burst. No value before a burst. Unit: ms.
  double length_ms;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last burst. No value before a burst. Unit: dB.
  double snr_db;
  // The summary line.
  sdrkit_string text;
  // What the stack decodes.
  sdrkit_string content;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_gbas_vdb_fields;

// A row of the burst log: every 16th burst. The row key is the place in the log.
typedef struct sdrkit_gbas_vdb_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the burst. Unit: s.
  double time_s;
  // The length of the burst. Unit: ms.
  double length_ms;
  // Slots busy then.
  int64_t slots_busy;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
} sdrkit_gbas_vdb_burst_row;

// The result fields of an OFDM structure stack.
typedef struct sdrkit_avdata_ofdm_shape_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The OFDM of the numerology is there.
  bool detected;
  // True if `symbol_us` holds a value.
  bool has_symbol_us;
  // The symbol duration with its prefix. No value while the numerology of the settings is not valid. Unit: us.
  double symbol_us;
  // True if `prefix_us` holds a value.
  bool has_prefix_us;
  // The prefix duration. No value while the numerology of the settings is not valid. Unit: us.
  double prefix_us;
  // True if `timing` holds a value.
  bool has_timing;
  // The sample of the symbol period where the prefix correlation peaks, while detected. Unit: samples.
  int64_t timing;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The prefix correlation, while detected.
  double correlation;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset, while detected. A frequency in Hz.
  double offset_hz;
  // True if `frame` holds a value.
  bool has_frame;
  // The frame structure in words. No value while it is not known.
  sdrkit_string frame;
  // What the stack decodes.
  sdrkit_string content;
  // The summary line.
  sdrkit_string text;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_avdata_ofdm_shape_fields;

// A row of the detection starts. The row key is the place in the log.
typedef struct sdrkit_avdata_ofdm_shape_detection_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the start. Unit: s.
  double time_s;
  // The symbol duration with its prefix. Unit: us.
  double symbol_us;
  // The prefix duration. Unit: us.
  double prefix_us;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The prefix correlation.
  double correlation;
  // True if `frame` holds a value.
  bool has_frame;
  // The frame structure in words. No value while it was not known.
  sdrkit_string frame;
} sdrkit_avdata_ofdm_shape_detection_row;

// The options of `family` of `sdrkit_acars_message_event`.
typedef enum sdrkit_acars_message_event_family {
  // Aircraft
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ACARS_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_acars_message_event_family;

// An ACARS message. The source is the registration of the aircraft, as the link names it. The link does not name the ground station, so the target has no value.
typedef struct sdrkit_acars_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The registration.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver. No value: the link does not name it.
  sdrkit_string target;
  // The text.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. No value: the stack does not measure it per message. Unit: dB.
  double snr_db;
  // What the event is about: an aircraft. A value of `sdrkit_acars_message_event_family`.
  int32_t family;
  // The flight id of a downlink.
  sdrkit_string flight;
  // The label.
  sdrkit_string label;
  // Aircraft to ground.
  bool downlink;
  // The message sequence number.
  sdrkit_string msn;
  // True if `position` holds a value.
  bool has_position;
  // The position the message gives.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude the message gives. Unit: ft.
  double alt_ft;
} sdrkit_acars_message_event;

// The options of `family` of `sdrkit_vdl2_message_event`.
typedef enum sdrkit_vdl2_message_event_family {
  // Aircraft
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_VDL2_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_vdl2_message_event_family;

// A VDL2 frame. The source and the target are the addresses of the sender and the receiver, six hex digits.
typedef struct sdrkit_vdl2_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address of the sender.
  sdrkit_string source;
  // The address of the receiver.
  sdrkit_string target;
  // The summary of the frame.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // What the event is about: an aircraft. A value of `sdrkit_vdl2_message_event_family`.
  int32_t family;
  // An aircraft sent the frame.
  bool from_air;
  // The deepest layer read.
  sdrkit_string protocol;
  // The name of the frame.
  sdrkit_string frame;
} sdrkit_vdl2_message_event;

// The options of `family` of `sdrkit_vdl4_message_event`.
typedef enum sdrkit_vdl4_message_event_family {
  // Aircraft
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_VDL4_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_vdl4_message_event_family;

// A VDL4 message. The source is the address of the sender, six hex digits.
typedef struct sdrkit_vdl4_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address of the sender.
  sdrkit_string source;
  // The summary of the message.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // What the event is about: an aircraft. A value of `sdrkit_vdl4_message_event_family`.
  int32_t family;
  // The name of the message.
  sdrkit_string message;
  // True if `position` holds a value.
  bool has_position;
  // The position the message gives.
  sdrkit_position position;
} sdrkit_vdl4_message_event;

// The options of `family` of `sdrkit_airband_activity_event`.
typedef enum sdrkit_airband_activity_event_family {
  // Aircraft
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AIRBAND_ACTIVITY_EVENT_FAMILY_OTHER = 12,
} sdrkit_airband_activity_event_family;

// A transmission, when it ends. Analogue voice carries no identity: the source and the target have no value. The channel is the name of the channel, the text its label.
typedef struct sdrkit_airband_activity_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender. No value: voice carries no identity.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver. No value: voice carries no identity.
  sdrkit_string target;
  // The name of the channel.
  sdrkit_string channel;
  // True if `text` holds a value.
  bool has_text;
  // The label of the channel.
  sdrkit_string text;
  // The carrier over the noise. Unit: dB.
  double snr_db;
  // What the event is about: voice. A value of `sdrkit_airband_activity_event_family`.
  int32_t family;
  // The length of the transmission. A length of time in seconds.
  double length;
  // The carrier over the noise. Unit: dB.
  double cn_db;
} sdrkit_airband_activity_event;

// The options of `family` of `sdrkit_hfdl_message_event`.
typedef enum sdrkit_hfdl_message_event_family {
  // Aircraft
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFDL_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfdl_message_event_family;

// An HFDL message. The source is the sender (the address or registration of the aircraft, or the name of the ground station), the target the receiver.
typedef struct sdrkit_hfdl_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver. No value for a squitter.
  sdrkit_string target;
  // The summary of the message.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the burst. Unit: dB.
  double snr_db;
  // What the event is about: an aircraft. A value of `sdrkit_hfdl_message_event_family`.
  int32_t family;
  // True if `position` holds a value.
  bool has_position;
  // The position the message gives.
  sdrkit_position position;
  // The kind of the PDU.
  sdrkit_string kind;
  // Ground to aircraft.
  bool uplink;
  // The ground station id.
  int64_t gs_id;
} sdrkit_hfdl_message_event;

// The arguments of `remove` of VDL2.
typedef struct sdrkit_vdl2_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the ground station.
  sdrkit_string row;
} sdrkit_vdl2_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_VDL2_STATION_ARGS_INIT {sizeof(sdrkit_vdl2_station_args), {0}}

// The arguments of `remove` of VDL4.
typedef struct sdrkit_vdl4_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the ground station.
  sdrkit_string row;
} sdrkit_vdl4_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_VDL4_STATION_ARGS_INIT {sizeof(sdrkit_vdl4_station_args), {0}}

// The arguments of `remove` of SELCAL.
typedef struct sdrkit_selcal_code_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the code.
  sdrkit_string row;
} sdrkit_selcal_code_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SELCAL_CODE_ARGS_INIT {sizeof(sdrkit_selcal_code_args), {0}}

// The arguments of `ptt`.
typedef struct sdrkit_airband_ptt_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Push-to-talk pressed (on) or released (off).
  bool on;
} sdrkit_airband_ptt_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_AIRBAND_PTT_ARGS_INIT {sizeof(sdrkit_airband_ptt_args), 0}

// The arguments of `remove` of HFDL.
typedef struct sdrkit_hfdl_aircraft_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the aircraft.
  sdrkit_string row;
} sdrkit_hfdl_aircraft_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_HFDL_AIRCRAFT_ARGS_INIT {sizeof(sdrkit_hfdl_aircraft_args), {0}}

// Fills `rows` with the changed rows of the table "messages" of the stack "acars" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`.
static inline int32_t sdrkit_acars_messages_rows(SDRKIT_HANDLE handle,
                                                sdrkit_acars_message_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.AcarsMessageRow", "messages", rows, sizeof(sdrkit_acars_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "acars" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.
static inline int32_t sdrkit_acars_aircraft_rows(SDRKIT_HANDLE handle,
                                                sdrkit_acars_aircraft_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.AcarsAircraftRow", "aircraft", rows, sizeof(sdrkit_acars_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "acars" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_acars_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_acars_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.AcarsChannelRow", "channels", rows, sizeof(sdrkit_acars_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "acars.message"
// (ACARS message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_acars_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_acars_message_event *event) {
  return sdrkit_typed_event(handle, n, "acars.message", "avdata.AcarsMessageEvent", event, sizeof(sdrkit_acars_message_event));
}

// Runs the command "clear" (Clear) of the stack "acars" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_acars_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "acars" are the sdrkit_acars_gen_*() functions
// after the settings of its receiver.

// The setting "channels" (Channels) of the stack "acars". Frequencies in MHz separated by spaces. Empty watches every carrier on the 25 kHz grid. An 8.33 kHz channel such as 131.5167 needs the list. The host fills it from a channel file (data/avdata/datalink_channels.csv).
static inline int32_t sdrkit_acars_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_acars_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "threshold_db" (Threshold) of the stack "acars". Unit: dB. Carrier over the noise needed to start decoding a channel.
static inline int32_t sdrkit_acars_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_acars_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "join_blocks" (Join blocks) of the stack "acars". Joins the blocks of a long message into one message.
static inline int32_t sdrkit_acars_get_join_blocks(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "join_blocks", value);
}
static inline int32_t sdrkit_acars_set_join_blocks(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "join_blocks", value);
}

// The setting "gen_aircraft" (Test aircraft) of the test signal generator of the stack "acars". Aircraft in the test signal.
static inline int32_t sdrkit_acars_gen_get_aircraft(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_aircraft", value);
}
static inline int32_t sdrkit_acars_gen_set_aircraft(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value);
}

// The setting "gen_rate_factor" (Test traffic) of the test signal generator of the stack "acars". Scales how often the test aircraft send.
static inline int32_t sdrkit_acars_gen_get_rate_factor(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rate_factor", value);
}
static inline int32_t sdrkit_acars_gen_set_rate_factor(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rate_factor", value);
}

// The setting "gen_positions" (Test positions) of the test signal generator of the stack "acars". The first six test aircraft also send position reports.
static inline int32_t sdrkit_acars_gen_get_positions(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_positions", value);
}
static inline int32_t sdrkit_acars_gen_set_positions(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_positions", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "vdl2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`.
static inline int32_t sdrkit_vdl2_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdl2_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl2MessageRow", "messages", rows, sizeof(sdrkit_vdl2_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "vdl2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.
static inline int32_t sdrkit_vdl2_aircraft_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdl2_aircraft_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl2AircraftRow", "aircraft", rows, sizeof(sdrkit_vdl2_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ground_stations" of the stack "vdl2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `address` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_vdl2_ground_stations_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_vdl2_station_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl2StationRow", "ground_stations", rows, sizeof(sdrkit_vdl2_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "vdl2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_vdl2_channels_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdl2_channel_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl2ChannelRow", "channels", rows, sizeof(sdrkit_vdl2_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "vdl2.message"
// (VDL2 frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_vdl2_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_vdl2_message_event *event) {
  return sdrkit_typed_event(handle, n, "vdl2.message", "avdata.Vdl2MessageEvent", event, sizeof(sdrkit_vdl2_message_event));
}

// Runs the command "clear" (Clear) of the stack "vdl2" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_vdl2_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "vdl2" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_vdl2_remove(SDRKIT_HANDLE handle, const sdrkit_vdl2_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "avdata.Vdl2StationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "vdl2" are the sdrkit_vdl2_gen_*() functions
// after the settings of its receiver.

// The setting "channels" (Channels) of the stack "vdl2". Frequencies in MHz separated by spaces. Empty watches the common signalling channel 136.975 MHz and the tuned frequency. The host fills it from a channel file (data/avdata/datalink_channels.csv). Tune between the channels.
static inline int32_t sdrkit_vdl2_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_vdl2_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "track_tuning" (Track tuning) of the stack "vdl2". Measures the radio's tuning error from the bursts and takes it out.
static inline int32_t sdrkit_vdl2_get_track_tuning(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "track_tuning", value);
}
static inline int32_t sdrkit_vdl2_set_track_tuning(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "track_tuning", value);
}

// The setting "tune_error_hz" (Tuning error guess) of the stack "vdl2". A frequency in Hz. The radio's tuning error to start from.
static inline int32_t sdrkit_vdl2_get_tune_error_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tune_error_hz", value);
}
static inline int32_t sdrkit_vdl2_set_tune_error_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tune_error_hz", value);
}

// The setting "gen_aircraft" (Test aircraft) of the test signal generator of the stack "vdl2". Aircraft in the test signal.
static inline int32_t sdrkit_vdl2_gen_get_aircraft(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_aircraft", value);
}
static inline int32_t sdrkit_vdl2_gen_set_aircraft(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "vdl4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `address` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`.
static inline int32_t sdrkit_vdl4_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdl4_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl4MessageRow", "messages", rows, sizeof(sdrkit_vdl4_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "vdl4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.
static inline int32_t sdrkit_vdl4_aircraft_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdl4_aircraft_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl4AircraftRow", "aircraft", rows, sizeof(sdrkit_vdl4_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ground_stations" of the stack "vdl4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `address` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_vdl4_ground_stations_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_vdl4_station_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl4StationRow", "ground_stations", rows, sizeof(sdrkit_vdl4_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "vdl4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_vdl4_channels_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdl4_channel_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl4ChannelRow", "channels", rows, sizeof(sdrkit_vdl4_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "vdl4.message"
// (VDL4 message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_vdl4_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_vdl4_message_event *event) {
  return sdrkit_typed_event(handle, n, "vdl4.message", "avdata.Vdl4MessageEvent", event, sizeof(sdrkit_vdl4_message_event));
}

// Runs the command "clear" (Clear) of the stack "vdl4" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_vdl4_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "vdl4" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_vdl4_remove(SDRKIT_HANDLE handle, const sdrkit_vdl4_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "avdata.Vdl4StationArgs", args, NULL, NULL, 0);
}

// The setting "channels" (Channels) of the stack "vdl4". Frequencies in MHz separated by spaces. Empty watches the tuned frequency. The host fills it from a channel file (data/avdata/datalink_channels.csv).
static inline int32_t sdrkit_vdl4_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_vdl4_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "selcal" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_selcal_calls_rows(SDRKIT_HANDLE handle,
                                              sdrkit_selcal_call_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.SelcalCallRow", "calls", rows, sizeof(sdrkit_selcal_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "codes" of the stack "selcal" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_selcal_codes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_selcal_code_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.SelcalCodeRow", "codes", rows, sizeof(sdrkit_selcal_code_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "selcal" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_selcal_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "selcal" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_selcal_remove(SDRKIT_HANDLE handle, const sdrkit_selcal_code_args *args) {
  return sdrkit_typed_command(handle, "remove", "avdata.SelcalCodeArgs", args, NULL, NULL, 0);
}

// The setting "demod" (Demodulator) of the stack "selcal". A value of `sdrkit_selcal_demod`. Auto: USB below 30 MHz, AM above.
static inline int32_t sdrkit_selcal_get_demod(SDRKIT_HANDLE handle, sdrkit_selcal_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_selcal_demod)index;
  return code;
}
static inline int32_t sdrkit_selcal_set_demod(SDRKIT_HANDLE handle, sdrkit_selcal_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "selcal". Plays the channel audio. Decoding is the same either way.
static inline int32_t sdrkit_selcal_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_selcal_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "audio_input" (Audio input) of the stack "selcal". The samples are sound card audio (8 kHz or more in the real part).
static inline int32_t sdrkit_selcal_get_audio_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_input", value);
}
static inline int32_t sdrkit_selcal_set_audio_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_input", value);
}

// The setting "aircraft_table" (Aircraft table) of the stack "selcal". Lines of a SELCAL code and the aircraft's registration, separated by a tab, a comma or a semicolon.
static inline int32_t sdrkit_selcal_get_aircraft_table(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aircraft_table", value);
}
static inline int32_t sdrkit_selcal_set_aircraft_table(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aircraft_table", value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "airband" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_airband_channels_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_airband_channel_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.AirbandChannelRow", "channels", rows, sizeof(sdrkit_airband_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "seen" of the stack "airband" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_airband_seen_rows(SDRKIT_HANDLE handle,
                                              sdrkit_airband_seen_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.AirbandSeenRow", "seen", rows, sizeof(sdrkit_airband_seen_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "activity" of the stack "airband" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_airband_activity_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_airband_activity_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.AirbandActivityRow", "activity", rows, sizeof(sdrkit_airband_activity_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "airband.activity"
// (Transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_airband_activity_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_airband_activity_event *event) {
  return sdrkit_typed_event(handle, n, "airband.activity", "avdata.AirbandActivityEvent", event, sizeof(sdrkit_airband_activity_event));
}

// Runs the command "clear" (Clear) of the stack "airband" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_airband_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "ptt" (Push to talk) of the stack "airband" on the session of `handle`. With on true, sends the speech of the microphone input as AM with full carrier on the tuned frequency, in the audio band of the channel spacing, until ptt with on false, halt or the time-out. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
static inline int32_t sdrkit_airband_ptt(SDRKIT_HANDLE handle, const sdrkit_airband_ptt_args *args) {
  return sdrkit_typed_command(handle, "ptt", "avdata.PttArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "airband" on the session of `handle`. Ends the transmission at once, with the fall of the carrier.
static inline int32_t sdrkit_airband_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The setting "channels" (Channels) of the stack "airband". The channel list (v3: records). Empty uses the test layout around the dial.
static inline int32_t sdrkit_airband_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_airband_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "squelch_db" (Squelch) of the stack "airband". Unit: dB. Carrier-to-noise to open a channel.
static inline int32_t sdrkit_airband_get_squelch_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_db", value);
}
static inline int32_t sdrkit_airband_set_squelch_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_db", value);
}

// The setting "hang_s" (Hang) of the stack "airband". A length of time in seconds. How long a channel stays open after its carrier goes.
static inline int32_t sdrkit_airband_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_airband_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "scan" (Scan) of the stack "airband". Plays one open channel at a time, in turn.
static inline int32_t sdrkit_airband_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_airband_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "scan_hold" (Hold) of the stack "airband". Stays on the channel the scan stopped on.
static inline int32_t sdrkit_airband_get_scan_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan_hold", value);
}
static inline int32_t sdrkit_airband_set_scan_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan_hold", value);
}

// The setting "noise_remover" (Noise remover) of the stack "airband". Takes the steady noise out of the audio.
static inline int32_t sdrkit_airband_get_noise_remover(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "noise_remover", value);
}
static inline int32_t sdrkit_airband_set_noise_remover(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "noise_remover", value);
}

// The setting "noise_remover_strength" (Strength) of the stack "airband". Unit: %. How much of the noise remover's output is heard.
static inline int32_t sdrkit_airband_get_noise_remover_strength(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "noise_remover_strength", value);
}
static inline int32_t sdrkit_airband_set_noise_remover_strength(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "noise_remover_strength", value);
}

// The setting "only_active" (Only active) of the stack "airband". Shows only the open channels in the channel table.
static inline int32_t sdrkit_airband_get_only_active(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "only_active", value);
}
static inline int32_t sdrkit_airband_set_only_active(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "only_active", value);
}

// The setting "listen" (Listen) of the stack "airband". Plays the audio.
static inline int32_t sdrkit_airband_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_airband_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_spacing" (Channel spacing) of the stack "airband". A value of `sdrkit_airband_tx_spacing`. The spacing of the channel sent on. It sets the audio band: 350 to 2500 Hz for 8.33 kHz, 300 to 3000 Hz for 25 kHz. A signal for 8.33 kHz also fits a 25 kHz channel.
static inline int32_t sdrkit_airband_get_tx_spacing(SDRKIT_HANDLE handle, sdrkit_airband_tx_spacing *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_spacing", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_airband_tx_spacing)index;
  return code;
}
static inline int32_t sdrkit_airband_set_tx_spacing(SDRKIT_HANDLE handle, sdrkit_airband_tx_spacing value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_spacing", (int32_t)value);
}

// The setting "tx_am_depth" (Modulation depth) of the stack "airband". Unit: %. The peak modulation depth. The level control holds the audio peaks at this depth; between the samples a peak can rise by up to 4 % of it. 100 % gives 96 %, so that the carrier never pinches off.
static inline int32_t sdrkit_airband_get_tx_am_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_am_depth", value);
}
static inline int32_t sdrkit_airband_set_tx_am_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_am_depth", value);
}

// The setting "tx_mic_gain" (Microphone gain) of the stack "airband". Unit: dB. The gain of the microphone audio before the level control. The level control adds up to 12 dB to quiet speech and takes loud speech down to the depth.
static inline int32_t sdrkit_airband_get_tx_mic_gain(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_mic_gain", value);
}
static inline int32_t sdrkit_airband_set_tx_mic_gain(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_mic_gain", value);
}

// The setting "tx_timeout" (Time-out) of the stack "airband". A length of time in seconds. Push-to-talk ends after this time on air.
static inline int32_t sdrkit_airband_get_tx_timeout(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_timeout", value);
}
static inline int32_t sdrkit_airband_set_tx_timeout(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_timeout", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "hfdl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_hfdl_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfdl_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.HfdlMessageRow", "messages", rows, sizeof(sdrkit_hfdl_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "hfdl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `flight` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`.
static inline int32_t sdrkit_hfdl_aircraft_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfdl_aircraft_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.HfdlAircraftRow", "aircraft", rows, sizeof(sdrkit_hfdl_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ground_stations" of the stack "hfdl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_hfdl_ground_stations_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_hfdl_station_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.HfdlStationRow", "ground_stations", rows, sizeof(sdrkit_hfdl_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "hfdl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_hfdl_channels_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfdl_channel_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.HfdlChannelRow", "channels", rows, sizeof(sdrkit_hfdl_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfdl.message"
// (HFDL message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdl_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdl_message_event *event) {
  return sdrkit_typed_event(handle, n, "hfdl.message", "avdata.HfdlMessageEvent", event, sizeof(sdrkit_hfdl_message_event));
}

// Runs the command "clear" (Clear) of the stack "hfdl" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_hfdl_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "hfdl" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_hfdl_remove(SDRKIT_HANDLE handle, const sdrkit_hfdl_aircraft_args *args) {
  return sdrkit_typed_command(handle, "remove", "avdata.HfdlAircraftArgs", args, NULL, NULL, 0);
}

// The setting "channels" (Channels) of the stack "hfdl". Dial frequencies in kHz separated by spaces. Empty watches every frequency of the ground station table inside the capture.
static inline int32_t sdrkit_hfdl_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_hfdl_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "whole_span" (Whole span) of the stack "hfdl". Every table frequency inside the capture, and the list.
static inline int32_t sdrkit_hfdl_get_whole_span(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "whole_span", value);
}
static inline int32_t sdrkit_hfdl_set_whole_span(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "whole_span", value);
}

// The setting "shared_channeliser" (Shared channeliser) of the stack "hfdl". One polyphase bank feeds every channel (else one channeliser each).
static inline int32_t sdrkit_hfdl_get_shared_channeliser(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "shared_channeliser", value);
}
static inline int32_t sdrkit_hfdl_set_shared_channeliser(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "shared_channeliser", value);
}

// The setting "channel_filter_hz" (Channel filter) of the stack "hfdl". A frequency in Hz. The width of each channel's filter.
static inline int32_t sdrkit_hfdl_get_channel_filter_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_filter_hz", value);
}
static inline int32_t sdrkit_hfdl_set_channel_filter_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_filter_hz", value);
}

// The setting "follow_best_band" (Follow the best band) of the stack "hfdl". Asks the host to retune to the HFDL band with the most traffic.
static inline int32_t sdrkit_hfdl_get_follow_best_band(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_best_band", value);
}
static inline int32_t sdrkit_hfdl_set_follow_best_band(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_best_band", value);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "vdl3" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_vdl3_bursts_rows(SDRKIT_HANDLE handle,
                                             sdrkit_vdl3_burst_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.Vdl3BurstRow", "bursts", rows, sizeof(sdrkit_vdl3_burst_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "vdl3" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_vdl3_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "vdl3" are the sdrkit_vdl3_gen_*() functions
// after the settings of its receiver.

// The setting "sync_threshold" (Training threshold) of the stack "vdl3". The normalised differential correlation that starts a burst.
static inline int32_t sdrkit_vdl3_get_sync_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "sync_threshold", value);
}
static inline int32_t sdrkit_vdl3_set_sync_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "sync_threshold", value);
}

// The setting "evm_max" (Largest phase error) of the stack "vdl3". Unit: % of pi/4. A burst whose RMS phase error is larger is taken for noise and dropped.
static inline int32_t sdrkit_vdl3_get_evm_max(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "evm_max", value);
}
static inline int32_t sdrkit_vdl3_set_evm_max(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "evm_max", value);
}

// The setting "rs_bit_order" (Reed-Solomon octet bit order) of the stack "vdl3". A value of `sdrkit_vdl3_rs_bit_order`. The order of the bits of each octet of the data word on air.
static inline int32_t sdrkit_vdl3_get_rs_bit_order(SDRKIT_HANDLE handle, sdrkit_vdl3_rs_bit_order *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rs_bit_order", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_vdl3_rs_bit_order)index;
  return code;
}
static inline int32_t sdrkit_vdl3_set_rs_bit_order(SDRKIT_HANDLE handle, sdrkit_vdl3_rs_bit_order value) {
  return sdrkit_typed_setting_set_enum(handle, "rs_bit_order", (int32_t)value);
}

// The setting "gen_adjacent" (Test signal adjacent station) of the test signal generator of the stack "vdl3". A second station 25 kHz above the channel at the same level.
static inline int32_t sdrkit_vdl3_gen_get_adjacent(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_adjacent", value);
}
static inline int32_t sdrkit_vdl3_gen_set_adjacent(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_adjacent", value);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "gbas_vdb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gbas_vdb_bursts_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_gbas_vdb_burst_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.GbasVdbBurstRow", "bursts", rows, sizeof(sdrkit_gbas_vdb_burst_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "gbas_vdb" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_gbas_vdb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gbas_vdb" are the sdrkit_gbas_vdb_gen_*() functions
// after the settings of its receiver.

// The setting "gen_slots" (Test signal slots) of the test signal generator of the stack "gbas_vdb". The slots (0 to 7) of the 500 ms frame that hold a burst, separated by spaces.
static inline int32_t sdrkit_gbas_vdb_gen_get_slots(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_slots", value);
}
static inline int32_t sdrkit_gbas_vdb_gen_set_slots(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_slots", value);
}

// The setting "gen_frames" (Test signal frames) of the test signal generator of the stack "gbas_vdb". Frames with bursts, then silence; 0 sends without end.
static inline int32_t sdrkit_gbas_vdb_gen_get_frames(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_frames", value);
}
static inline int32_t sdrkit_gbas_vdb_gen_set_frames(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_frames", value);
}

// Fills `rows` with the changed rows of the table "detections" of the stack "ldacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ldacs_detections_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_avdata_ofdm_shape_detection_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.OfdmShapeDetectionRow", "detections", rows, sizeof(sdrkit_avdata_ofdm_shape_detection_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ldacs" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ldacs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ldacs" are the sdrkit_ldacs_gen_*() functions
// after the settings of its receiver.

// The setting "fft_len" (FFT length) of the stack "ldacs". Unit: samples. The FFT length of the numerology at 625 ksps.
static inline int32_t sdrkit_ldacs_get_fft_len(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "fft_len", value);
}
static inline int32_t sdrkit_ldacs_set_fft_len(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "fft_len", value);
}

// The setting "prefix" (Cyclic prefix) of the stack "ldacs". Unit: samples. The cyclic prefix of the numerology at 625 ksps.
static inline int32_t sdrkit_ldacs_get_prefix(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "prefix", value);
}
static inline int32_t sdrkit_ldacs_set_prefix(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "prefix", value);
}

// The setting "frame_ms" (Frame period) of the stack "ldacs". Unit: ms. The period over which the power is folded to tell bursts from a continuous signal.
static inline int32_t sdrkit_ldacs_get_frame_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "frame_ms", value);
}
static inline int32_t sdrkit_ldacs_set_frame_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "frame_ms", value);
}

// The setting "gen_duty" (Test signal duty) of the test signal generator of the stack "ldacs". Unit: %.
static inline int32_t sdrkit_ldacs_gen_get_duty(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_duty", value);
}
static inline int32_t sdrkit_ldacs_gen_set_duty(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_duty", value);
}

// Fills `rows` with the changed rows of the table "detections" of the stack "aeromacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aeromacs_detections_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_avdata_ofdm_shape_detection_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avdata.OfdmShapeDetectionRow", "detections", rows, sizeof(sdrkit_avdata_ofdm_shape_detection_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "aeromacs" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_aeromacs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "aeromacs" are the sdrkit_aeromacs_gen_*() functions
// after the settings of its receiver.

// The setting "fft_len" (FFT length) of the stack "aeromacs". Unit: samples. The FFT length of the numerology at 5.6 Msps.
static inline int32_t sdrkit_aeromacs_get_fft_len(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "fft_len", value);
}
static inline int32_t sdrkit_aeromacs_set_fft_len(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "fft_len", value);
}

// The setting "prefix" (Cyclic prefix) of the stack "aeromacs". Unit: samples. The cyclic prefix of the numerology at 5.6 Msps.
static inline int32_t sdrkit_aeromacs_get_prefix(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "prefix", value);
}
static inline int32_t sdrkit_aeromacs_set_prefix(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "prefix", value);
}

// The setting "frame_ms" (Frame period) of the stack "aeromacs". Unit: ms. The period over which the power is folded to tell bursts from a continuous signal.
static inline int32_t sdrkit_aeromacs_get_frame_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "frame_ms", value);
}
static inline int32_t sdrkit_aeromacs_set_frame_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "frame_ms", value);
}

// The setting "gen_duty" (Test signal duty) of the test signal generator of the stack "aeromacs". Unit: %.
static inline int32_t sdrkit_aeromacs_gen_get_duty(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_duty", value);
}
static inline int32_t sdrkit_aeromacs_gen_set_duty(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_duty", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_acars_fields_read(SDRKIT_HANDLE handle, sdrkit_acars_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.AcarsFields", fields, sizeof(sdrkit_acars_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_vdl2_fields_read(SDRKIT_HANDLE handle, sdrkit_vdl2_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.Vdl2Fields", fields, sizeof(sdrkit_vdl2_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_vdl4_fields_read(SDRKIT_HANDLE handle, sdrkit_vdl4_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.Vdl4Fields", fields, sizeof(sdrkit_vdl4_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_airband_fields_read(SDRKIT_HANDLE handle, sdrkit_airband_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.AirbandFields", fields, sizeof(sdrkit_airband_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_airband_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_airband_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.AirbandTxFields", fields, sizeof(sdrkit_airband_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfdl_fields_read(SDRKIT_HANDLE handle, sdrkit_hfdl_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.HfdlFields", fields, sizeof(sdrkit_hfdl_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_vdl3_fields_read(SDRKIT_HANDLE handle, sdrkit_vdl3_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.Vdl3Fields", fields, sizeof(sdrkit_vdl3_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_avdata_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_avdata_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.DecoderFields", fields, sizeof(sdrkit_avdata_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gbas_vdb_fields_read(SDRKIT_HANDLE handle, sdrkit_gbas_vdb_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.GbasVdbFields", fields, sizeof(sdrkit_gbas_vdb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_avdata_ofdm_shape_fields_read(SDRKIT_HANDLE handle, sdrkit_avdata_ofdm_shape_fields *fields) {
  return sdrkit_typed_fields(handle, "avdata.OfdmShapeFields", fields, sizeof(sdrkit_avdata_ofdm_shape_fields));
}

#endif
