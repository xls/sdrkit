/*
 * sdrkit: the typed interface of the stack family "wlan", version 2.0. MIT licence.
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

#ifndef SDRKIT_WLAN_H
#define SDRKIT_WLAN_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_WLAN_VERSION_MAJOR 2
#define SDRKIT_WLAN_VERSION_MINOR 0

// The stack "ble": its id, and the keys of its event types.
#define SDRKIT_STACK_BLE "ble"
#define SDRKIT_EVENT_BLE_DEVICE "ble.device"
#define SDRKIT_EVENT_BLE_TX "ble.tx"

// The stack "bletracker": its id, and the keys of its event types.
#define SDRKIT_STACK_BLETRACKER "bletracker"
#define SDRKIT_EVENT_BLE_TRACKER "ble.tracker"

// The stack "wifi": its id, and the keys of its event types.
#define SDRKIT_STACK_WIFI "wifi"

// The stack "halow": its id, and the keys of its event types.
#define SDRKIT_STACK_HALOW "halow"

// The stack "btclassic": its id, and the keys of its event types.
#define SDRKIT_STACK_BTCLASSIC "btclassic"
#define SDRKIT_EVENT_BTCLASSIC_DEVICE_NEW "btclassic.device_new"
#define SDRKIT_EVENT_BTCLASSIC_INQUIRY_RESPONSE "btclassic.inquiry_response"

// The stack "wfb": its id, and the keys of its event types.
#define SDRKIT_STACK_WFB "wfb"

// The stack "espnow": its id, and the keys of its event types.
#define SDRKIT_STACK_ESPNOW "espnow"
#define SDRKIT_EVENT_ESPNOW_FRAME "espnow.frame"
#define SDRKIT_EVENT_ESPNOW_DEVICE "espnow.device"

// The options of the setting "channel" of the stack "ble".
typedef enum sdrkit_ble_channel {
  // 37  2402 MHz
  SDRKIT_BLE_CHANNEL_37_2402_MHZ = 0,
  // 38  2426 MHz
  SDRKIT_BLE_CHANNEL_38_2426_MHZ = 1,
  // 39  2480 MHz
  SDRKIT_BLE_CHANNEL_39_2480_MHZ = 2,
} sdrkit_ble_channel;

// The options of the setting "gen_channel" of the stack "ble".
typedef enum sdrkit_ble_gen_channel {
  // Round the three channels
  SDRKIT_BLE_GEN_CHANNEL_ROUND = 0,
  // All on 37
  SDRKIT_BLE_GEN_CHANNEL_ALL_ON_37 = 1,
  // All on 38
  SDRKIT_BLE_GEN_CHANNEL_ALL_ON_38 = 2,
  // All on 39
  SDRKIT_BLE_GEN_CHANNEL_ALL_ON_39 = 3,
} sdrkit_ble_gen_channel;

// The options of the setting "channel" of the stack "bletracker".
typedef enum sdrkit_bletracker_channel {
  // 37  2402 MHz
  SDRKIT_BLETRACKER_CHANNEL_37_2402_MHZ = 0,
  // 38  2426 MHz
  SDRKIT_BLETRACKER_CHANNEL_38_2426_MHZ = 1,
  // 39  2480 MHz
  SDRKIT_BLETRACKER_CHANNEL_39_2480_MHZ = 2,
} sdrkit_bletracker_channel;

// The options of the setting "gen_schedule" of the stack "bletracker".
typedef enum sdrkit_bletracker_gen_schedule {
  // Every tracker and a phone
  SDRKIT_BLETRACKER_GEN_SCHEDULE_ALL = 0,
  // Only the Find My frame
  SDRKIT_BLETRACKER_GEN_SCHEDULE_FIND_MY = 1,
} sdrkit_bletracker_gen_schedule;

// The options of the setting "band" of the stack "wifi".
typedef enum sdrkit_wifi_band {
  // 2.4 GHz
  SDRKIT_WIFI_BAND_2_4 = 0,
  // 5 GHz
  SDRKIT_WIFI_BAND_5_GHZ = 1,
  // 6 GHz
  SDRKIT_WIFI_BAND_6_GHZ = 2,
} sdrkit_wifi_band;

// The options of the setting "band_filter" of the stack "wifi".
typedef enum sdrkit_wifi_band_filter {
  // All
  SDRKIT_WIFI_BAND_FILTER_ALL = 0,
  // 2.4 GHz
  SDRKIT_WIFI_BAND_FILTER_2_4 = 1,
  // 5 GHz
  SDRKIT_WIFI_BAND_FILTER_5_GHZ = 2,
  // 6 GHz
  SDRKIT_WIFI_BAND_FILTER_6_GHZ = 3,
} sdrkit_wifi_band_filter;

// The options of the setting "gen_scene" of the stack "wifi".
typedef enum sdrkit_wifi_gen_scene {
  // 2.4 GHz
  SDRKIT_WIFI_GEN_SCENE_2_4 = 0,
  // 5 GHz
  SDRKIT_WIFI_GEN_SCENE_5_GHZ = 1,
  // 6 GHz
  SDRKIT_WIFI_GEN_SCENE_6_GHZ = 2,
} sdrkit_wifi_gen_scene;

// The options of the setting "list_width" of the stack "halow".
typedef enum sdrkit_halow_list_width {
  // 1 MHz
  SDRKIT_HALOW_LIST_WIDTH_1_MHZ = 0,
  // 2 MHz
  SDRKIT_HALOW_LIST_WIDTH_2_MHZ = 1,
  // 4 MHz
  SDRKIT_HALOW_LIST_WIDTH_4_MHZ = 2,
  // 8 MHz
  SDRKIT_HALOW_LIST_WIDTH_8_MHZ = 3,
  // 16 MHz
  SDRKIT_HALOW_LIST_WIDTH_16_MHZ = 4,
} sdrkit_halow_list_width;

// The options of the setting "receive" of the stack "halow".
typedef enum sdrkit_halow_receive {
  // Auto 1 - 4 MHz
  SDRKIT_HALOW_RECEIVE_AUTO14 = 0,
  // Auto all widths
  SDRKIT_HALOW_RECEIVE_AUTO_ALL = 1,
  // 1 MHz
  SDRKIT_HALOW_RECEIVE_1_MHZ = 2,
  // 2 MHz
  SDRKIT_HALOW_RECEIVE_2_MHZ = 3,
  // 4 MHz
  SDRKIT_HALOW_RECEIVE_4_MHZ = 4,
  // 8 MHz
  SDRKIT_HALOW_RECEIVE_8_MHZ = 5,
  // 16 MHz
  SDRKIT_HALOW_RECEIVE_16_MHZ = 6,
} sdrkit_halow_receive;

// The options of the setting "links" of the stack "wfb".
typedef enum sdrkit_wfb_links {
  // wfb-ng
  SDRKIT_WFB_LINKS_WFB_NG = 0,
  // OpenHD
  SDRKIT_WFB_LINKS_OPENHD = 1,
  // Both
  SDRKIT_WFB_LINKS_BOTH = 2,
} sdrkit_wfb_links;

// The options of the setting "gen_scene" of the stack "espnow".
typedef enum sdrkit_espnow_gen_scene {
  // Mixed scene
  SDRKIT_ESPNOW_GEN_SCENE_MIXED = 0,
  // One frame
  SDRKIT_ESPNOW_GEN_SCENE_SINGLE = 1,
} sdrkit_espnow_gen_scene;

// The options of the setting "gen_phy" of the stack "espnow".
typedef enum sdrkit_espnow_gen_phy {
  // DSSS 1 Mbit/s
  SDRKIT_ESPNOW_GEN_PHY_DSSS1 = 0,
  // DSSS 2 Mbit/s, long preamble
  SDRKIT_ESPNOW_GEN_PHY_DSSS2_LONG = 1,
  // DSSS 2 Mbit/s, short preamble
  SDRKIT_ESPNOW_GEN_PHY_DSSS2_SHORT = 2,
  // OFDM 6 Mbit/s
  SDRKIT_ESPNOW_GEN_PHY_OFDM6 = 3,
  // OFDM 12 Mbit/s
  SDRKIT_ESPNOW_GEN_PHY_OFDM12 = 4,
} sdrkit_espnow_gen_phy;

// The result fields of `ble`.
typedef struct sdrkit_ble_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Devices in the table.
  int64_t devices;
  // Good packets.
  int64_t frames_ok;
  // Exact access-address matches whose CRC failed on every channel.
  int64_t frames_bad;
  // Access-address candidates.
  int64_t aa_hits;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The running mean of the eye SNR. No value before the first good packet. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last good packet. No value before it. A frequency in Hz.
  double offset_hz;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The deviation of the last good packet. No value before it. A frequency in Hz.
  double deviation_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the channel. No value before the first input. Unit: dBFS.
  double level_dbfs;
  // True if `channel` holds a value.
  bool has_channel;
  // The tuned advertising channel. No value when it is not known, as for a recording.
  int64_t channel;
  // True if `last_channel` holds a value.
  bool has_last_channel;
  // The channel of the last good packet. No value before it.
  int64_t last_channel;
  // Good packets on channel 37.
  int64_t packets_37;
  // Good packets on channel 38.
  int64_t packets_38;
  // Good packets on channel 39.
  int64_t packets_39;
  // The host steps through the advertising channels.
  bool hopping;
  // The privacy note.
  sdrkit_string privacy;
} sdrkit_ble_fields;

// The result fields of the advertising sender of `ble` and `bletracker`.
typedef struct sdrkit_ble_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a send is on air.
  bool tx_sending;
  // The packets of the send on air that went out.
  int64_t tx_sent;
  // The packets still to go: the rest of the send on air and the sends that wait.
  int64_t tx_left;
} sdrkit_ble_tx_fields;

// The options of `address_type` of `sdrkit_ble_device_row`.
typedef enum sdrkit_ble_device_row_address_type {
  // Public
  SDRKIT_BLE_DEVICE_ROW_ADDRESS_TYPE_PUBLIC = 0,
  // Random static
  SDRKIT_BLE_DEVICE_ROW_ADDRESS_TYPE_RANDOM_STATIC = 1,
  // Resolvable private
  SDRKIT_BLE_DEVICE_ROW_ADDRESS_TYPE_RESOLVABLE_PRIVATE = 2,
  // Non-resolvable private
  SDRKIT_BLE_DEVICE_ROW_ADDRESS_TYPE_NON_RESOLVABLE_PRIVATE = 3,
  // Random
  SDRKIT_BLE_DEVICE_ROW_ADDRESS_TYPE_RANDOM = 4,
} sdrkit_ble_device_row_address_type;

// A row of the devices of `ble`. The row key is the address and "r" (random) or "p" (public).
typedef struct sdrkit_ble_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address.
  sdrkit_string address;
  // The kind of the address. A value of `sdrkit_ble_device_row_address_type`.
  int32_t address_type;
  // True if `name` holds a value.
  bool has_name;
  // The name, if a packet gave one.
  sdrkit_string name;
  // The level of the last packet. Unit: dBFS.
  double level_dbfs;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Packets heard.
  int64_t heard;
  // True if `vendor` holds a value.
  bool has_vendor;
  // The vendor, if known.
  sdrkit_string vendor;
  // True if `beacon` holds a value.
  bool has_beacon;
  // The beacon, the services and the TX power, if a packet gave them.
  sdrkit_string beacon;
  // The channels the device was heard on, as "37 38 39".
  sdrkit_string channels;
  // The device takes connections.
  bool connectable;
  // Another device sent a scan request to it.
  bool scanned;
  // True if `manufacturer_data` holds a value.
  bool has_manufacturer_data;
  // The first 32 bytes of manufacturer data after the company identifier, hex, if a packet gave them.
  sdrkit_string manufacturer_data;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first packet. Seconds since 1970 (UTC).
  double first_heard;
} sdrkit_ble_device_row;

// A row of the packet log of `ble`.
typedef struct sdrkit_ble_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the packet. Unit: s.
  double signal_s;
  // The advertising channel of the packet.
  int64_t channel;
  // The PDU type name.
  sdrkit_string pdu;
  // True if `address` holds a value.
  bool has_address;
  // The address of the sender, if the PDU has one.
  sdrkit_string address;
  // The level of the packet. Unit: dBFS.
  double level_dbfs;
  // The payload length. Unit: bytes.
  int64_t length;
  // The one-line summary.
  sdrkit_string details;
} sdrkit_ble_packet_row;

// A row of the good packets of each PDU type of `ble`. The row key is the PDU type name.
typedef struct sdrkit_ble_pdu_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PDU type name.
  sdrkit_string pdu;
  // Good packets of the type.
  int64_t packets;
} sdrkit_ble_pdu_type_row;

// A row of the level history of the devices of `ble`. The row key is `<device row>/<slot>`.
typedef struct sdrkit_ble_device_level_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row of the device.
  sdrkit_string device;
  // The number of the level in the history of its device, counted from the first one heard.
  int64_t sample;
  // The level. Unit: dBFS.
  double level_dbfs;
} sdrkit_ble_device_level_row;

// The result fields of `bletracker`.
typedef struct sdrkit_bletracker_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Trackers that follow the radio now.
  int64_t following_now;
  // Tracker advertisements taken since the last clear.
  int64_t tracker_packets;
  // Trackers heard in the last minute.
  int64_t trackers_now;
  // Trackers in the table.
  int64_t trackers;
  // How long a tracker must stay before the alert. A length of time in seconds.
  double alert_after;
  // The averaged level from which a tracker counts as near. Unit: dBFS.
  double near_dbfs;
  // The signal time of the book. Unit: s.
  double now_s;
  // Good packets.
  int64_t frames_ok;
  // Exact access-address matches whose CRC failed on every channel.
  int64_t frames_bad;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The running mean of the eye SNR. No value before the first good packet. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last good packet. No value before it. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the channel. No value before the first input. Unit: dBFS.
  double level_dbfs;
  // True if `channel` holds a value.
  bool has_channel;
  // The tuned advertising channel. No value when it is not known, as for a recording.
  int64_t channel;
  // What the detector cannot tell.
  sdrkit_string limits;
  // The privacy note.
  sdrkit_string privacy;
} sdrkit_bletracker_fields;

// The options of `kind` of `sdrkit_bletracker_tracker_row`.
typedef enum sdrkit_bletracker_tracker_row_kind {
  // Apple Find My (AirTag or compatible)
  SDRKIT_BLETRACKER_TRACKER_ROW_KIND_FIND_MY = 0,
  // Tile
  SDRKIT_BLETRACKER_TRACKER_ROW_KIND_TILE = 1,
  // Chipolo
  SDRKIT_BLETRACKER_TRACKER_ROW_KIND_CHIPOLO = 2,
  // Samsung SmartTag
  SDRKIT_BLETRACKER_TRACKER_ROW_KIND_SMART_TAG = 3,
  // Google Find My Device tag
  SDRKIT_BLETRACKER_TRACKER_ROW_KIND_GOOGLE = 4,
} sdrkit_bletracker_tracker_row_kind;

// The options of `state` of `sdrkit_bletracker_tracker_row`.
typedef enum sdrkit_bletracker_tracker_row_state {
  // your own
  SDRKIT_BLETRACKER_TRACKER_ROW_STATE_MINE = 0,
  // following you
  SDRKIT_BLETRACKER_TRACKER_ROW_STATE_FOLLOWING = 1,
  // separated from its owner
  SDRKIT_BLETRACKER_TRACKER_ROW_STATE_SEPARATED = 2,
  // nearby
  SDRKIT_BLETRACKER_TRACKER_ROW_STATE_NEARBY = 3,
} sdrkit_bletracker_tracker_row_state;

// The options of `trend` of `sdrkit_bletracker_tracker_row`.
typedef enum sdrkit_bletracker_tracker_row_trend {
  // closer
  SDRKIT_BLETRACKER_TRACKER_ROW_TREND_CLOSER = 0,
  // farther
  SDRKIT_BLETRACKER_TRACKER_ROW_TREND_FARTHER = 1,
  // steady
  SDRKIT_BLETRACKER_TRACKER_ROW_TREND_STEADY = 2,
} sdrkit_bletracker_tracker_row_trend;

// The options of `mode` of `sdrkit_bletracker_tracker_row`.
typedef enum sdrkit_bletracker_tracker_row_mode {
  // activated
  SDRKIT_BLETRACKER_TRACKER_ROW_MODE_ACTIVATED = 0,
  // shipping mode
  SDRKIT_BLETRACKER_TRACKER_ROW_MODE_SHIPPING = 1,
  // registered
  SDRKIT_BLETRACKER_TRACKER_ROW_MODE_REGISTERED = 2,
  // unregistered
  SDRKIT_BLETRACKER_TRACKER_ROW_MODE_UNREGISTERED = 3,
  // disconnected from its owner
  SDRKIT_BLETRACKER_TRACKER_ROW_MODE_DISCONNECTED = 4,
} sdrkit_bletracker_tracker_row_mode;

// The options of `tag_state` of `sdrkit_bletracker_tracker_row`.
typedef enum sdrkit_bletracker_tracker_row_tag_state {
  // premature lost
  SDRKIT_BLETRACKER_TRACKER_ROW_TAG_STATE_PREMATURE_LOST = 0,
  // lost
  SDRKIT_BLETRACKER_TRACKER_ROW_TAG_STATE_LOST = 1,
  // overmature lost
  SDRKIT_BLETRACKER_TRACKER_ROW_TAG_STATE_OVERMATURE_LOST = 2,
  // paired with one device
  SDRKIT_BLETRACKER_TRACKER_ROW_TAG_STATE_PAIRED = 3,
  // connected to one device
  SDRKIT_BLETRACKER_TRACKER_ROW_TAG_STATE_CONNECTED_ONE = 4,
  // connected to two devices
  SDRKIT_BLETRACKER_TRACKER_ROW_TAG_STATE_CONNECTED_TWO = 5,
} sdrkit_bletracker_tracker_row_tag_state;

// A row of the trackers of `bletracker`. The row key is the tracker id.
typedef struct sdrkit_bletracker_tracker_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The label of the tracker: its kind and the last two address bytes.
  sdrkit_string tracker;
  // True if `kind` holds a value.
  bool has_kind;
  // The kind, if recognised. A value of `sdrkit_bletracker_tracker_row_kind`.
  int32_t kind;
  // What the tracker does now. A value of `sdrkit_bletracker_tracker_row_state`.
  int32_t state;
  // How long it stayed with the radio. A length of time in seconds.
  double stays;
  // The share of the time it was near. Unit: %.
  double near_pct;
  // The averaged level. Unit: dBFS.
  double level_dbfs;
  // The highest level. Unit: dBFS.
  double peak_dbfs;
  // Whether the level rises or falls. A value of `sdrkit_bletracker_tracker_row_trend`.
  int32_t trend;
  // The addresses it used.
  int64_t addresses;
  // The signal time it was last heard. Unit: s.
  double last_s;
  // Packets heard.
  int64_t packets;
  // It says it is away from its owner.
  bool separated;
  // True if `status` holds a value.
  bool has_status;
  // The status byte, if the advertisement has one.
  int64_t status;
  // It follows the radio.
  bool following;
  // It is marked as the user's own.
  bool mine;
  // The tracker id.
  int64_t id;
  // True if `mode` holds a value.
  bool has_mode;
  // The mode that the advertised UUID tells: Tile activated or shipping, SmartTag registered or unregistered, Chipolo disconnected from its owner. No value for other families. A value of `sdrkit_bletracker_tracker_row_mode`.
  int32_t mode;
  // True if `tag_state` holds a value.
  bool has_tag_state;
  // The SmartTag tag state. No value for the codes 0 and 7, which have no name, and for other families. A value of `sdrkit_bletracker_tracker_row_tag_state`.
  int32_t tag_state;
  // True if `tag_state_code` holds a value.
  bool has_tag_state_code;
  // The SmartTag tag state code, 0 to 7.
  int64_t tag_state_code;
  // True if `aging_counter` holds a value.
  bool has_aging_counter;
  // The SmartTag aging counter: 15-minute steps of the tag's clock from 2020-07-02 00:00 UTC.
  int64_t aging_counter;
  // True if `region` holds a value.
  bool has_region;
  // The SmartTag region ID, 0 to 15.
  int64_t region;
  // True if `e2e` holds a value.
  bool has_e2e;
  // The SmartTag E2E flag: finders must encrypt their location reports.
  bool e2e;
  // True if `uwb` holds a value.
  bool has_uwb;
  // The SmartTag UWB flag.
  bool uwb;
  // True if `battery` holds a value.
  bool has_battery;
  // The SmartTag battery code, 0 to 3.
  int64_t battery;
  // True if `identifier` holds a value.
  bool has_identifier;
  // The rolling identifier in hex, opaque: the SmartTag privacy ID.
  sdrkit_string identifier;
  // True if `service_data` holds a value.
  bool has_service_data;
  // The service data value under the family's UUID in hex, as sent (Tile, Chipolo, SmartTag).
  sdrkit_string service_data;
} sdrkit_bletracker_tracker_row;

// A row of the alerts of `bletracker`.
typedef struct sdrkit_bletracker_alert_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the alert, for a live source. Seconds since 1970 (UTC).
  double time;
  // The signal time of the alert. Unit: s.
  double signal_s;
  // The level of the tracker. Unit: dBFS.
  double level_dbfs;
  // The text of the alert.
  sdrkit_string alert;
} sdrkit_bletracker_alert_row;

// The options of `kind` of `sdrkit_bletracker_kind_row`.
typedef enum sdrkit_bletracker_kind_row_kind {
  // Apple Find My (AirTag or compatible)
  SDRKIT_BLETRACKER_KIND_ROW_KIND_FIND_MY = 0,
  // Tile
  SDRKIT_BLETRACKER_KIND_ROW_KIND_TILE = 1,
  // Chipolo
  SDRKIT_BLETRACKER_KIND_ROW_KIND_CHIPOLO = 2,
  // Samsung SmartTag
  SDRKIT_BLETRACKER_KIND_ROW_KIND_SMART_TAG = 3,
  // Google Find My Device tag
  SDRKIT_BLETRACKER_KIND_ROW_KIND_GOOGLE = 4,
} sdrkit_bletracker_kind_row_kind;

// A row of the trackers heard of each kind of `bletracker`. The row key is the key of the kind.
typedef struct sdrkit_bletracker_kind_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The kind. A value of `sdrkit_bletracker_kind_row_kind`.
  int32_t kind;
  // Trackers of the kind heard in the last minute.
  int64_t last_minute;
  // Trackers of the kind heard in the last 30 minutes.
  int64_t last_30_min;
} sdrkit_bletracker_kind_row;

// The options of `band` of `sdrkit_wifi_fields`.
typedef enum sdrkit_wifi_fields_band {
  // 2.4 GHz
  SDRKIT_WIFI_FIELDS_BAND_2_4 = 0,
  // 5 GHz
  SDRKIT_WIFI_FIELDS_BAND_5_GHZ = 1,
  // 6 GHz
  SDRKIT_WIFI_FIELDS_BAND_6_GHZ = 2,
} sdrkit_wifi_fields_band;

// The options of `phy` of `sdrkit_wifi_fields`.
typedef enum sdrkit_wifi_fields_phy {
  // DSSS
  SDRKIT_WIFI_FIELDS_PHY_DSSS = 0,
  // OFDM
  SDRKIT_WIFI_FIELDS_PHY_OFDM = 1,
} sdrkit_wifi_fields_phy;

// The result fields of `wifi`.
typedef struct sdrkit_wifi_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Networks in the table.
  int64_t networks;
  // Frames with a good FCS.
  int64_t frames_ok;
  // Frames with a bad FCS.
  int64_t frames_bad;
  // DSSS SFDs found.
  int64_t dsss_sfds;
  // DSSS PLCP headers with a good CRC.
  int64_t dsss_headers_ok;
  // DSSS PLCP headers with a bad CRC.
  int64_t dsss_headers_bad;
  // DSSS headers of a CCK rate, not decoded.
  int64_t dsss_cck_headers;
  // OFDM short preambles found.
  int64_t ofdm_preambles;
  // OFDM SIGNAL fields with a good parity.
  int64_t ofdm_signal_ok;
  // OFDM SIGNAL fields with a bad parity or rate.
  int64_t ofdm_signal_bad;
  // Management frames.
  int64_t mgmt_frames;
  // Control frames.
  int64_t control_frames;
  // Data frames.
  int64_t data_frames;
  // Beacons.
  int64_t beacons;
  // Probe responses.
  int64_t probe_responses;
  // FILS discovery frames.
  int64_t fils_frames;
  // Frames heard again on a second channel at once.
  int64_t duplicate_frames;
  // True if `band` holds a value.
  bool has_band;
  // The band of the tuned frequency, if it is in one. A value of `sdrkit_wifi_fields_band`.
  int32_t band;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel of the tuned frequency, if it is one.
  int64_t channel;
  // The 20 MHz channels that the stack reads at once.
  int64_t sub_channels;
  // True if `phy` holds a value.
  bool has_phy;
  // The PHY of the last good frame. No value before it. A value of `sdrkit_wifi_fields_phy`.
  int32_t phy;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The running mean of the SNR. No value before the first good frame. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last good frame. No value before it. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. No value before the first input. Unit: dBFS.
  double level_dbfs;
  // The host steps through the hop list.
  bool hopping;
  // The passive note.
  sdrkit_string passive;
} sdrkit_wifi_fields;

// The options of `band` of `sdrkit_wifi_network_row`.
typedef enum sdrkit_wifi_network_row_band {
  // 2.4 GHz
  SDRKIT_WIFI_NETWORK_ROW_BAND_2_4 = 0,
  // 5 GHz
  SDRKIT_WIFI_NETWORK_ROW_BAND_5_GHZ = 1,
  // 6 GHz
  SDRKIT_WIFI_NETWORK_ROW_BAND_6_GHZ = 2,
} sdrkit_wifi_network_row_band;

// A row of the networks of `wifi`. The row key is the BSSID.
typedef struct sdrkit_wifi_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The SSID, or "(hidden)".
  sdrkit_string ssid;
  // The BSSID.
  sdrkit_string bssid;
  // The vendor of the BSSID, or "(local)" for a locally administered one.
  sdrkit_string vendor;
  // True if `band` holds a value.
  bool has_band;
  // The band. A value of `sdrkit_wifi_network_row_band`.
  int32_t band;
  // The primary channel.
  int64_t channel;
  // The channel the frames were heard on.
  int64_t heard_on;
  // The channel width. A frequency in Hz.
  double width_hz;
  // The newest standard ("ax").
  sdrkit_string standard;
  // The security.
  sdrkit_string security;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // Beacons heard.
  int64_t beacons;
  // Probe responses heard.
  int64_t probe_responses;
  // The beacon interval. A length of time in seconds.
  double interval;
  // The pairwise ciphers.
  sdrkit_string ciphers;
  // The key management.
  sdrkit_string akm;
  // Protected management frames: "required", "capable" or "".
  sdrkit_string pmf;
  // The capability bits as text.
  sdrkit_string capability;
  // The other elements as text.
  sdrkit_string details;
} sdrkit_wifi_network_row;

// A row of the frame log of `wifi`.
typedef struct sdrkit_wifi_beacon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double signal_s;
  // The channel the frame was heard on.
  int64_t channel;
  // The BSSID.
  sdrkit_string bssid;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The length, FCS included.
  int64_t bytes;
  // The summary.
  sdrkit_string details;
} sdrkit_wifi_beacon_row;

// A row of the good OFDM frames at each rate of `wifi`. The row key is the rate in Mbit/s.
typedef struct sdrkit_wifi_rate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The rate. Unit: Mbit/s.
  double rate_mbps;
  // Good frames at the rate.
  int64_t frames;
} sdrkit_wifi_rate_row;

// The options of `band` of `sdrkit_wifi_band_row`.
typedef enum sdrkit_wifi_band_row_band {
  // 2.4 GHz
  SDRKIT_WIFI_BAND_ROW_BAND_2_4 = 0,
  // 5 GHz
  SDRKIT_WIFI_BAND_ROW_BAND_5_GHZ = 1,
  // 6 GHz
  SDRKIT_WIFI_BAND_ROW_BAND_6_GHZ = 2,
} sdrkit_wifi_band_row_band;

// A row of the summary of each band of `wifi`. The row key is the key of the band.
typedef struct sdrkit_wifi_band_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The band. A value of `sdrkit_wifi_band_row_band`.
  int32_t band;
  // Networks in the band.
  int64_t networks;
  // Beacons heard in the band.
  int64_t beacons;
  // Networks of 802.11ax or 802.11be.
  int64_t ax_be;
  // Networks with WPA3 or OWE.
  int64_t wpa3_owe;
  // Open networks.
  int64_t open;
  // The level of the strongest network. Unit: dBFS.
  double strongest_dbfs;
} sdrkit_wifi_band_row;

// The result fields of `halow`.
typedef struct sdrkit_halow_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Networks in the table.
  int64_t networks;
  // Frames with a good FCS.
  int64_t frames_ok;
  // Frames with a bad FCS.
  int64_t frames_bad;
  // PPDUs read from their SIG field only.
  int64_t undecoded;
  // PPDUs with several spatial streams.
  int64_t mimo_frames;
  // PPDUs with STBC.
  int64_t stbc_frames;
  // PPDUs with LDPC coding.
  int64_t ldpc_frames;
  // A-MPDUs.
  int64_t ampdu_frames;
  // Null data packets.
  int64_t ndp_frames;
  // Preambles detected.
  int64_t preambles;
  // SIG fields read.
  int64_t sig_ok;
  // SIG fields not readable.
  int64_t sig_bad;
  // Data fields cut short.
  int64_t data_bad;
  // Network frames (beacons and probe responses).
  int64_t beacons_seen;
  // Frames in the last whole second.
  int64_t frames_per_second;
  // True if `tuned_hz` holds a value.
  bool has_tuned_hz;
  // The tuned frequency. No value when it is not known, as for a recording. A frequency in Hz.
  double tuned_hz;
  // True if `channel` holds a value.
  bool has_channel;
  // The nearest 1 MHz channel of the plan, if one is within 1 MHz of the tuned frequency.
  int64_t channel;
  // The widest channel that the rate holds. A frequency in Hz.
  double max_width_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The running mean of the SNR. No value before the first good frame. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last good frame. No value before it. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. No value before the first input. Unit: dBFS.
  double level_dbfs;
  // The host steps through the channels.
  bool scanning;
  // The receive and privacy note.
  sdrkit_string receive_note;
  // The note on what the data decoder reads.
  sdrkit_string data_note;
} sdrkit_halow_fields;

// A row of the networks of `halow`. The row key is the BSSID.
typedef struct sdrkit_halow_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The SSID, "(hidden)", the compressed SSID or "-" when not known.
  sdrkit_string ssid;
  // The BSSID.
  sdrkit_string bssid;
  // The vendor.
  sdrkit_string vendor;
  // The channel number.
  int64_t channel;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency of the channel, if known. A frequency in Hz.
  double freq_hz;
  // The width of the last PPDU. A frequency in Hz.
  double width_hz;
  // True if `op_width_hz` holds a value.
  bool has_op_width_hz;
  // The operating width, if a beacon gave it. A frequency in Hz.
  double op_width_hz;
  // The MCS of the last PPDU.
  int64_t mcs;
  // The spatial streams of the last PPDU.
  int64_t nss;
  // The SNR of the last PPDU. Unit: dB.
  double snr_db;
  // The security.
  sdrkit_string security;
  // The beacon interval. A length of time in seconds.
  double interval;
  // The level of the last PPDU. Unit: dBFS.
  double level_dbfs;
  // Beacons heard.
  int64_t beacons;
  // The country.
  sdrkit_string country;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_halow_network_row;

// A row of the frame log of `halow`.
typedef struct sdrkit_halow_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double signal_s;
  // The width of the PPDU. A frequency in Hz.
  double width_hz;
  // The MCS.
  int64_t mcs;
  // The spatial streams.
  int64_t nss;
  // True if `preamble` holds a value.
  bool has_preamble;
  // The name of the preamble, if the SIG field gave it.
  sdrkit_string preamble;
  // The SNR of the PPDU. Unit: dB.
  double snr_db;
  // The kind of the frame.
  sdrkit_string type;
  // The BSSID.
  sdrkit_string bssid;
  // The length.
  int64_t length;
  // The details of the frame.
  sdrkit_string details;
} sdrkit_halow_frame_row;

// The options of `state` of `sdrkit_halow_width_row`.
typedef enum sdrkit_halow_width_row_state {
  // listening
  SDRKIT_HALOW_WIDTH_ROW_STATE_LISTENING = 0,
  // not selected
  SDRKIT_HALOW_WIDTH_ROW_STATE_NOT_SELECTED = 1,
  // sample rate too low
  SDRKIT_HALOW_WIDTH_ROW_STATE_RATE_TOO_LOW = 2,
} sdrkit_halow_width_row_state;

// A row of the width view of `halow`. The row key is the width in MHz.
typedef struct sdrkit_halow_width_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The width. A frequency in Hz.
  double width_hz;
  // SIG fields read at the width.
  int64_t sig_ok;
  // The lowest radio rate for the width. A frequency in Hz.
  double min_rate_hz;
  // The radio rate recommended for the width. A frequency in Hz.
  double rate_hz;
  // Whether the stack reads the width. A value of `sdrkit_halow_width_row_state`.
  int32_t state;
} sdrkit_halow_width_row;

// A row of the PPDUs of each MCS of `halow`. The row key is the MCS.
typedef struct sdrkit_halow_mcs_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The MCS.
  int64_t mcs;
  // PPDUs counted.
  int64_t ppdus;
} sdrkit_halow_mcs_row;

// The options of `preamble` of `sdrkit_halow_preamble_row`.
typedef enum sdrkit_halow_preamble_row_preamble {
  // S1G_SHORT
  SDRKIT_HALOW_PREAMBLE_ROW_PREAMBLE_S1G_SHORT = 0,
  // S1G_LONG
  SDRKIT_HALOW_PREAMBLE_ROW_PREAMBLE_S1G_LONG = 1,
  // S1G_1M
  SDRKIT_HALOW_PREAMBLE_ROW_PREAMBLE_S1G_1M = 2,
} sdrkit_halow_preamble_row_preamble;

// A row of the PPDUs of each preamble of `halow`. The row key is the key of the preamble.
typedef struct sdrkit_halow_preamble_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The preamble. A value of `sdrkit_halow_preamble_row_preamble`.
  int32_t preamble;
  // PPDUs counted.
  int64_t ppdus;
} sdrkit_halow_preamble_row;

// The result fields of `btclassic`.
typedef struct sdrkit_btclassic_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good access codes.
  int64_t access_codes;
  // Bursts analysed without an access code.
  int64_t bursts_without_code;
  // GIAC and LIAC ID packets.
  int64_t inquiry_ids;
  // FHS packets read.
  int64_t inquiry_responses;
  // Bursts analysed.
  int64_t bursts_analysed;
  // Wideband bursts skipped.
  int64_t bursts_wideband;
  // The channels inside the capture.
  int64_t channels_covered;
  // The channel numbers count from a capture centre taken as channel 39 (the radio centre is not known).
  bool channels_relative;
  // True if `cfo_hz` holds a value.
  bool has_cfo_hz;
  // The carrier offset of the last good packet. No value before it. Unit: Hz.
  double cfo_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last good packet. No value before it. Unit: dB.
  double snr_db;
  // True if `input_level_dbfs` holds a value.
  bool has_input_level_dbfs;
  // The input level. No value before the first input. Unit: dBFS.
  double input_level_dbfs;
  // 1 when an access code was found in the last 5 s of signal.
  int64_t state;
  // Devices in the table.
  int64_t devices;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_btclassic_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_wlan_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_wlan_decoder_fields;

// A row of the devices of `btclassic`. The row key is the LAP.
typedef struct sdrkit_btclassic_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The LAP as hex text.
  sdrkit_string lap;
  // What the device is.
  sdrkit_string kind;
  // True if `address` holds a value.
  bool has_address;
  // The address from an FHS packet.
  sdrkit_string address;
  // True if `class_` holds a value.
  bool has_class_;
  // The class of device as text.
  sdrkit_string class_;
  // True if `major_class` holds a value.
  bool has_major_class;
  // The major class name.
  sdrkit_string major_class;
  // True if `class_raw` holds a value.
  bool has_class_raw;
  // The class of device value.
  int64_t class_raw;
  // True if `clock` holds a value.
  bool has_clock;
  // CLK27..CLK2 from the FHS packet. Unit: 1.25 ms.
  int64_t clock;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the last packet. Unit: dBFS.
  double level_dbfs;
  // The last levels. Unit: dBFS.
  sdrkit_string level_history;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first packet. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet. Seconds since 1970 (UTC).
  double last_heard;
  // Packets heard.
  int64_t packets;
  // The channel of the last packet. Unit: MHz.
  double last_channel_mhz;
  // The channels heard, as hex: bit k is channel k.
  sdrkit_string channels_heard;
} sdrkit_btclassic_device_row;

// A row of the packet log of `btclassic`.
typedef struct sdrkit_btclassic_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the packet, for a live source. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the packet. Unit: s.
  double time_s;
  // The channel of the packet. Unit: MHz.
  double channel_mhz;
  // The LAP as hex text.
  sdrkit_string lap;
  // The level of the packet. Unit: dBFS.
  double level_dbfs;
  // Sync-word bits that differed.
  int64_t sync_errors;
  // What the packet is.
  sdrkit_string text;
} sdrkit_btclassic_packet_row;

// The result fields of `wfb`.
typedef struct sdrkit_wfb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames of wfb-ng or OpenHD links.
  int64_t frames_ok;
  // HT frames with a good FCS.
  int64_t ht_frames;
  // HT-mixed PPDUs that the decoder does not take.
  int64_t ht_not_decodable;
  // HT frames with a bad FCS.
  int64_t bad_fcs;
  // Links in the table.
  int64_t links_heard;
  // Frames lost over every link.
  int64_t frames_lost;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last frame. No value before the first frame. Unit: dB.
  double snr_db;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_wfb_fields;

// A row of the links of `wfb`. The row key is the system and the link id.
typedef struct sdrkit_wfb_link_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // "wfb-ng" or "OpenHD".
  sdrkit_string link;
  // The link id.
  sdrkit_string id;
  // Frames heard.
  int64_t frames;
  // Frames lost.
  int64_t frames_lost;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The SNR of the last frame. Unit: dB.
  double snr_db;
  // The type of the last frame.
  sdrkit_string last_type;
  // The detail of the last frame.
  sdrkit_string last_detail;
} sdrkit_wfb_link_row;

// A row of the frame log of `wfb`.
typedef struct sdrkit_wfb_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the frame, for a live source. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the frame. Unit: s.
  double time_s;
  // "wfb-ng" or "OpenHD".
  sdrkit_string link;
  // The link id.
  sdrkit_string id;
  // The type of the frame.
  sdrkit_string type;
  // The detail of the frame.
  sdrkit_string detail;
  // The first payload bytes, hex.
  sdrkit_string hex;
  // The SNR of the frame. Unit: dB.
  double snr_db;
} sdrkit_wfb_frame_row;

// The result fields of `espnow`.
typedef struct sdrkit_espnow_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Devices in the table.
  int64_t devices;
  // Wi-Fi frames with a good FCS.
  int64_t frames_ok;
  // Wi-Fi frames with a bad FCS.
  int64_t frames_bad;
  // Management action frames.
  int64_t action_frames;
  // ESP-NOW frames.
  int64_t espnow_frames;
  // ESP-NOW frames that were protected.
  int64_t encrypted;
  // Protected frames that a key opened.
  int64_t decrypted;
  // ESP-NOW frames to the broadcast address.
  int64_t broadcast;
  // An ESP-NOW frame came in the last 5 s.
  bool heard;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel nearest the radio centre. No value when the centre is not known.
  int64_t channel;
  // True if `channel_hz` holds a value.
  bool has_channel_hz;
  // The centre frequency of the channel. No value when the centre is not known. A frequency in Hz.
  double channel_hz;
  // True if `channel_offset_hz` holds a value.
  bool has_channel_offset_hz;
  // The offset of the channel's centre from the radio centre. No value when the centre is not known. A frequency in Hz.
  double channel_offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the last good frame. No value before it. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last good frame. No value before it. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last good frame. No value before it. A frequency in Hz.
  double offset_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // DSSS SFDs found.
  int64_t dsss_sfds;
  // DSSS PLCP headers with a good CRC.
  int64_t dsss_headers_ok;
  // DSSS PLCP headers with a bad CRC.
  int64_t dsss_headers_bad;
  // DSSS headers of a CCK rate, not decoded.
  int64_t dsss_cck_headers;
  // OFDM short preambles found.
  int64_t ofdm_preambles;
  // OFDM SIGNAL fields with a good parity.
  int64_t ofdm_signal_ok;
  // OFDM SIGNAL fields with a bad parity or rate.
  int64_t ofdm_signal_bad;
  // The host steps through the hop list.
  bool hopping;
  // What is decoded, what is only detected, and what no public document confirms.
  sdrkit_string note;
} sdrkit_espnow_fields;

// A row of the devices of `espnow`. The row key is the sender address.
typedef struct sdrkit_espnow_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sender address.
  sdrkit_string address;
  // The maker of the address, or "(local)" for a locally administered one.
  sdrkit_string vendor;
  // The address is one of Espressif.
  bool espressif;
  // ESP-NOW frames heard.
  int64_t packets;
  // Protected frames heard.
  int64_t encrypted;
  // Frames to the broadcast address.
  int64_t broadcasts;
  // Packets per second over the present window of 1 to 10 s; 0 after 10 s unheard. Unit: 1/s.
  double rate;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // The channel of the last frame (0: not known).
  int64_t channel;
  // The PHY rate of the last frame. Unit: kbit/s.
  int64_t phy_rate_kbps;
  // "broadcast" or the destination of the last frame.
  sdrkit_string last_to;
  // The payload of the last frame as hex; empty when it was encrypted.
  sdrkit_string last_payload;
  // True if `version` holds a value.
  bool has_version;
  // The ESP-NOW version of the last readable frame. No value before one.
  int64_t version;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The signal time of the first frame. Unit: s.
  double first_s;
  // The signal time of the last frame. Unit: s.
  double last_s;
} sdrkit_espnow_device_row;

// A row of the frame log of `espnow`.
typedef struct sdrkit_espnow_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the frame. No value for a recording. Seconds since 1970 (UTC).
  double time;
  // The signal time of the frame. Unit: s.
  double signal_s;
  // The channel (0: not known).
  int64_t channel;
  // The PHY rate. Unit: kbit/s.
  int64_t phy_rate_kbps;
  // A DSSS frame with the short preamble.
  bool short_preamble;
  // The sender address.
  sdrkit_string from;
  // "broadcast" or the destination address.
  sdrkit_string to;
  // The sequence number.
  int64_t seq;
  // True if `version` holds a value.
  bool has_version;
  // The ESP-NOW version. No value for a frame that stayed encrypted.
  int64_t version;
  // The frame was protected.
  bool encrypted;
  // A key opened the frame.
  bool decrypted;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the frame. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the frame. A frequency in Hz.
  double offset_hz;
  // The payload as hex; empty when it stayed encrypted.
  sdrkit_string payload;
  // The summary.
  sdrkit_string details;
} sdrkit_espnow_frame_row;

// A row of the ESP-NOW frames at each PHY rate of `espnow`. The row key is the key of the rate.
typedef struct sdrkit_espnow_rate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PHY rate group.
  sdrkit_string rate;
  // ESP-NOW frames at the rate.
  int64_t frames;
} sdrkit_espnow_rate_row;

// A row of the ESP-NOW frames on each channel of `espnow`. The row key is the channel number.
typedef struct sdrkit_espnow_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel.
  int64_t channel;
  // ESP-NOW frames on the channel.
  int64_t frames;
} sdrkit_espnow_channel_row;

// The items that each decoder component of the stack "ble" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ble_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `adv` holds a value.
  bool has_adv;
  // The items that the component "adv" dropped.
  int64_t adv;
} sdrkit_ble_dropped;

// The items that each decoder component of the stack "bletracker" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_bletracker_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `adv` holds a value.
  bool has_adv;
  // The items that the component "adv" dropped.
  int64_t adv;
} sdrkit_bletracker_dropped;

// The items that each decoder component of the stack "wifi" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_wifi_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_wifi_dropped;

// The items that each decoder component of the stack "halow" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_halow_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_halow_dropped;

// The items that each decoder component of the stack "espnow" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_espnow_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_espnow_dropped;

// The options of `family` of `sdrkit_ble_device_event`.
typedef enum sdrkit_ble_device_event_family {
  // Aircraft
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_BLE_DEVICE_EVENT_FAMILY_OTHER = 12,
} sdrkit_ble_device_event_family;

// The fields of a good packet with an address.
typedef struct sdrkit_ble_device_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address.
  sdrkit_string source;
  // True if `text` holds a value.
  bool has_text;
  // The name of the device, if known.
  sdrkit_string text;
  // The level of the packet. Unit: dBFS.
  double level_dbfs;
  // The family of the source. A value of `sdrkit_ble_device_event_family`.
  int32_t family;
} sdrkit_ble_device_event;

// The options of `family` of `sdrkit_ble_tx_event`.
typedef enum sdrkit_ble_tx_event_family {
  // Aircraft
  SDRKIT_BLE_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_BLE_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_BLE_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_BLE_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_BLE_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_BLE_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_BLE_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_BLE_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_BLE_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_BLE_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_BLE_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_BLE_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_BLE_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_ble_tx_event_family;

// The fields of a change of an advertising send.
typedef struct sdrkit_ble_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, the frequency changed, or why a send was refused.
  sdrkit_string reason;
  // The advertiser address.
  sdrkit_string target;
  // The PDU type, the channel, the packets and the advertising data in hex; empty for finished.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_ble_tx_event_family`.
  int32_t family;
} sdrkit_ble_tx_event;

// The options of `kind` of `sdrkit_ble_tracker_event`.
typedef enum sdrkit_ble_tracker_event_kind {
  // Apple Find My (AirTag or compatible)
  SDRKIT_BLE_TRACKER_EVENT_KIND_FIND_MY = 0,
  // Tile
  SDRKIT_BLE_TRACKER_EVENT_KIND_TILE = 1,
  // Chipolo
  SDRKIT_BLE_TRACKER_EVENT_KIND_CHIPOLO = 2,
  // Samsung SmartTag
  SDRKIT_BLE_TRACKER_EVENT_KIND_SMART_TAG = 3,
  // Google Find My Device tag
  SDRKIT_BLE_TRACKER_EVENT_KIND_GOOGLE = 4,
} sdrkit_ble_tracker_event_kind;

// The options of `urgency` of `sdrkit_ble_tracker_event`.
typedef enum sdrkit_ble_tracker_event_urgency {
  // None
  SDRKIT_BLE_TRACKER_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_BLE_TRACKER_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_BLE_TRACKER_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_BLE_TRACKER_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_ble_tracker_event_urgency;

// The options of `family` of `sdrkit_ble_tracker_event`.
typedef enum sdrkit_ble_tracker_event_family {
  // Aircraft
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_BLE_TRACKER_EVENT_FAMILY_OTHER = 12,
} sdrkit_ble_tracker_event_family;

// The fields of an unwanted tracking alert.
typedef struct sdrkit_ble_tracker_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The label of the tracker.
  sdrkit_string source;
  // True if `kind` holds a value.
  bool has_kind;
  // The kind, if recognised. A value of `sdrkit_ble_tracker_event_kind`.
  int32_t kind;
  // The text of the alert.
  sdrkit_string text;
  // How long it stayed with the radio. A length of time in seconds.
  double stays;
  // The level of the tracker. Unit: dBFS.
  double level_dbfs;
  // Urgent for a tracker away from its owner, else a notice. A value of `sdrkit_ble_tracker_event_urgency`.
  int32_t urgency;
  // The family of the source: an alert. A value of `sdrkit_ble_tracker_event_family`.
  int32_t family;
} sdrkit_ble_tracker_event;

// The options of `family` of `sdrkit_btclassic_device_new_event`.
typedef enum sdrkit_btclassic_device_new_event_family {
  // Aircraft
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_BTCLASSIC_DEVICE_NEW_EVENT_FAMILY_OTHER = 12,
} sdrkit_btclassic_device_new_event_family;

// The fields of a LAP heard for the first time.
typedef struct sdrkit_btclassic_device_new_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The LAP as hex text.
  sdrkit_string lap;
  // What the device is.
  sdrkit_string kind;
  // The family of the source. A value of `sdrkit_btclassic_device_new_event_family`.
  int32_t family;
} sdrkit_btclassic_device_new_event;

// The options of `family` of `sdrkit_btclassic_inquiry_response_event`.
typedef enum sdrkit_btclassic_inquiry_response_event_family {
  // Aircraft
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_BTCLASSIC_INQUIRY_RESPONSE_EVENT_FAMILY_OTHER = 12,
} sdrkit_btclassic_inquiry_response_event_family;

// The fields of an inquiry response.
typedef struct sdrkit_btclassic_inquiry_response_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address.
  sdrkit_string address;
  // The class of device as text.
  sdrkit_string class_;
  // The family of the source. A value of `sdrkit_btclassic_inquiry_response_event_family`.
  int32_t family;
} sdrkit_btclassic_inquiry_response_event;

// The options of `family` of `sdrkit_espnow_frame_event`.
typedef enum sdrkit_espnow_frame_event_family {
  // Aircraft
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ESPNOW_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_espnow_frame_event_family;

// The fields of an ESP-NOW frame.
typedef struct sdrkit_espnow_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender address.
  sdrkit_string source;
  // "broadcast" or the destination address.
  sdrkit_string to;
  // The summary.
  sdrkit_string text;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The family of the source. A value of `sdrkit_espnow_frame_event_family`.
  int32_t family;
} sdrkit_espnow_frame_event;

// The options of `family` of `sdrkit_espnow_device_event`.
typedef enum sdrkit_espnow_device_event_family {
  // Aircraft
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ESPNOW_DEVICE_EVENT_FAMILY_OTHER = 12,
} sdrkit_espnow_device_event_family;

// The fields of a device heard for the first time.
typedef struct sdrkit_espnow_device_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender address.
  sdrkit_string source;
  // The maker of the address.
  sdrkit_string vendor;
  // The family of the source. A value of `sdrkit_espnow_device_event_family`.
  int32_t family;
} sdrkit_espnow_device_event;

// The arguments of `remove` of `ble`.
typedef struct sdrkit_ble_device_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the device.
  sdrkit_string row;
} sdrkit_ble_device_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_BLE_DEVICE_ARGS_INIT {sizeof(sdrkit_ble_device_args), {0}}

// The arguments of `send` of `ble`.
typedef struct sdrkit_ble_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The advertiser address.
  sdrkit_string address;
  // True if `address_type` holds a value.
  bool has_address_type;
  // Public or random. The index of an option.
  int32_t address_type;
  // True if `pdu_type` holds a value.
  bool has_pdu_type;
  // The PDU type. The index of an option.
  int32_t pdu_type;
  // True if `data` holds a value.
  bool has_data;
  // The advertising data.
  sdrkit_string data;
  // True if `name` holds a value.
  bool has_name;
  // The complete local name.
  sdrkit_string name;
  // True if `uuid16` holds a value.
  bool has_uuid16;
  // A 16-bit service UUID.
  sdrkit_string uuid16;
  // True if `company` holds a value.
  bool has_company;
  // The company identifier.
  sdrkit_string company;
  // True if `manufacturer_data` holds a value.
  bool has_manufacturer_data;
  // The manufacturer specific data.
  sdrkit_string manufacturer_data;
  // True if `count` holds a value.
  bool has_count;
  // The packets.
  int64_t count;
  // True if `interval_ms` holds a value.
  bool has_interval_ms;
  // The interval. Unit: ms.
  int64_t interval_ms;
} sdrkit_ble_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_BLE_SEND_ARGS_INIT {sizeof(sdrkit_ble_send_args), {0}, 0, 0, 0, 0, 0, {0}, 0, {0}, 0, {0}, 0, {0}, 0, {0}, 0, 0, 0, 0}

// The arguments of `mine` of `bletracker`.
typedef struct sdrkit_bletracker_mine_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the tracker.
  sdrkit_string row;
  // True if `mine` holds a value.
  bool has_mine;
  // Mark or unmark.
  bool mine;
} sdrkit_bletracker_mine_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_BLETRACKER_MINE_ARGS_INIT {sizeof(sdrkit_bletracker_mine_args), {0}, 0, 0}

// The arguments of `send` of `bletracker`.
typedef struct sdrkit_bletracker_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `tracker` holds a value.
  bool has_tracker;
  // The tracker family. The index of an option.
  int32_t tracker;
  // The advertiser address.
  sdrkit_string address;
  // True if `address_type` holds a value.
  bool has_address_type;
  // Public or random. The index of an option.
  int32_t address_type;
  // True if `pdu_type` holds a value.
  bool has_pdu_type;
  // The PDU type. The index of an option.
  int32_t pdu_type;
  // True if `status` holds a value.
  bool has_status;
  // The Find My status byte.
  int64_t status;
  // True if `protection` holds a value.
  bool has_protection;
  // The Google frame type.
  bool protection;
  // True if `payload` holds a value.
  bool has_payload;
  // The rest of the frame.
  sdrkit_string payload;
  // True if `count` holds a value.
  bool has_count;
  // The packets.
  int64_t count;
  // True if `interval_ms` holds a value.
  bool has_interval_ms;
  // The interval. Unit: ms.
  int64_t interval_ms;
  // True if `shipping` holds a value.
  bool has_shipping;
  // The Tile shipping mode.
  bool shipping;
  // True if `service_data` holds a value.
  bool has_service_data;
  // The service data value of Tile and Chipolo.
  sdrkit_string service_data;
  // True if `tag_state` holds a value.
  bool has_tag_state;
  // The SmartTag tag state. The index of an option.
  int32_t tag_state;
  // True if `aging_counter` holds a value.
  bool has_aging_counter;
  // The SmartTag aging counter.
  int64_t aging_counter;
  // True if `privacy_id` holds a value.
  bool has_privacy_id;
  // The SmartTag privacy ID.
  sdrkit_string privacy_id;
  // True if `region` holds a value.
  bool has_region;
  // The SmartTag region ID.
  int64_t region;
  // True if `e2e` holds a value.
  bool has_e2e;
  // The SmartTag E2E flag.
  bool e2e;
  // True if `uwb` holds a value.
  bool has_uwb;
  // The SmartTag UWB flag.
  bool uwb;
  // True if `battery` holds a value.
  bool has_battery;
  // The SmartTag battery code.
  int64_t battery;
  // True if `signature` holds a value.
  bool has_signature;
  // The SmartTag signature.
  sdrkit_string signature;
} sdrkit_bletracker_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_BLETRACKER_SEND_ARGS_INIT {sizeof(sdrkit_bletracker_send_args), 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, {0}, 0, 0, 0, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0, 0, {0}}

// The arguments of `remove` of `wifi` and `halow`.
typedef struct sdrkit_wlan_network_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the network.
  sdrkit_string row;
} sdrkit_wlan_network_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WLAN_NETWORK_ARGS_INIT {sizeof(sdrkit_wlan_network_args), {0}}

// Fills `rows` with the changed rows of the table "devices" of the stack "ble" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ble_devices_rows(SDRKIT_HANDLE handle,
                                             sdrkit_ble_device_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BleDeviceRow", "devices", rows, sizeof(sdrkit_ble_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "ble" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ble_packets_rows(SDRKIT_HANDLE handle,
                                             sdrkit_ble_packet_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BlePacketRow", "packets", rows, sizeof(sdrkit_ble_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "pdu_types" of the stack "ble" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ble_pdu_types_rows(SDRKIT_HANDLE handle,
                                               sdrkit_ble_pdu_type_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BlePduTypeRow", "pdu_types", rows, sizeof(sdrkit_ble_pdu_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "device_levels" of the stack "ble" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ble_device_levels_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_ble_device_level_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BleDeviceLevelRow", "device_levels", rows, sizeof(sdrkit_ble_device_level_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ble.device"
// (Bluetooth LE device). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ble_device_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ble_device_event *event) {
  return sdrkit_typed_event(handle, n, "ble.device", "wlan.BleDeviceEvent", event, sizeof(sdrkit_ble_device_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ble.tx"
// (Bluetooth LE transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ble_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ble_tx_event *event) {
  return sdrkit_typed_event(handle, n, "ble.tx", "wlan.BleTxEvent", event, sizeof(sdrkit_ble_tx_event));
}

// Runs the command "clear" (Clear) of the stack "ble" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ble_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ble" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ble_remove(SDRKIT_HANDLE handle, const sdrkit_ble_device_args *args) {
  return sdrkit_typed_command(handle, "remove", "wlan.BleDeviceArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send an advertisement) of the stack "ble" on the session of `handle`. Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: 2402, 2426 or 2480 MHz. A send during another waits for it. Transmit must be on. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_ble_send(SDRKIT_HANDLE handle, const sdrkit_ble_send_args *args) {
  return sdrkit_typed_command(handle, "send", "wlan.BleSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "ble" on the session of `handle`. Drops the packets of the send on air that are still to go, and the sends that wait.
static inline int32_t sdrkit_ble_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ble" are the sdrkit_ble_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Channel) of the stack "ble". A value of `sdrkit_ble_channel`. Asks the host to tune to the advertising channel.
static inline int32_t sdrkit_ble_get_channel(SDRKIT_HANDLE handle, sdrkit_ble_channel *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ble_channel)index;
  return code;
}
static inline int32_t sdrkit_ble_set_channel(SDRKIT_HANDLE handle, sdrkit_ble_channel value) {
  return sdrkit_typed_setting_set_enum(handle, "channel", (int32_t)value);
}

// The setting "hop" (Hop) of the stack "ble". Asks the host to step 37, 38, 39: some devices skip a channel, and interference differs between them.
static inline int32_t sdrkit_ble_get_hop(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop", value);
}
static inline int32_t sdrkit_ble_set_hop(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop", value);
}

// The setting "hop_seconds" (Dwell) of the stack "ble". A length of time in seconds. Time on each channel while hopping.
static inline int32_t sdrkit_ble_get_hop_seconds(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hop_seconds", value);
}
static inline int32_t sdrkit_ble_set_hop_seconds(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hop_seconds", value);
}

// The setting "filter_text" (Filter) of the stack "ble". Shows only the devices whose name, address, vendor or beacon holds this text.
static inline int32_t sdrkit_ble_get_filter_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "filter_text", value);
}
static inline int32_t sdrkit_ble_set_filter_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "filter_text", value);
}

// The setting "min_level_dbfs" (Level above) of the stack "ble". Unit: dBFS.
static inline int32_t sdrkit_ble_get_min_level_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_level_dbfs", value);
}
static inline int32_t sdrkit_ble_set_min_level_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_level_dbfs", value);
}

// The setting "named_only" (Named only) of the stack "ble".
static inline int32_t sdrkit_ble_get_named_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "named_only", value);
}
static inline int32_t sdrkit_ble_set_named_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "named_only", value);
}

// The setting "tx_level" (Level) of the stack "ble". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_ble_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_ble_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_channel" (Whitening) of the test signal generator of the stack "ble". A value of `sdrkit_ble_gen_channel`.
static inline int32_t sdrkit_ble_gen_get_channel(SDRKIT_HANDLE handle, sdrkit_ble_gen_channel *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_channel", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ble_gen_channel)index;
  return code;
}
static inline int32_t sdrkit_ble_gen_set_channel(SDRKIT_HANDLE handle, sdrkit_ble_gen_channel value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_channel", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "trackers" of the stack "bletracker" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_bletracker_trackers_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_bletracker_tracker_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BletrackerTrackerRow", "trackers", rows, sizeof(sdrkit_bletracker_tracker_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "alerts" of the stack "bletracker" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_bletracker_alerts_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_bletracker_alert_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BletrackerAlertRow", "alerts", rows, sizeof(sdrkit_bletracker_alert_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracker_kinds" of the stack "bletracker" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_bletracker_tracker_kinds_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_bletracker_kind_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BletrackerKindRow", "tracker_kinds", rows, sizeof(sdrkit_bletracker_kind_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ble.tracker"
// (Unwanted tracking alert). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ble_tracker_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ble_tracker_event *event) {
  return sdrkit_typed_event(handle, n, "ble.tracker", "wlan.BletrackerAlertEvent", event, sizeof(sdrkit_ble_tracker_event));
}

// Runs the command "clear" (Clear) of the stack "bletracker" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_bletracker_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "mine" (Mark as mine) of the stack "bletracker" on the session of `handle`. A tracker marked as the user's own raises no alert.
static inline int32_t sdrkit_bletracker_mine(SDRKIT_HANDLE handle, const sdrkit_bletracker_mine_args *args) {
  return sdrkit_typed_command(handle, "mine", "wlan.BletrackerMineArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a tracker advertisement) of the stack "bletracker" on the session of `handle`. Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: Find My manufacturer data of company 0x004C (type 0x12, length 25, the status, the payload); Google service data of UUID 0xFEAA (frame type 0x40 or 0x41, the payload); Tile, a UUID list of 0xFEED and the service data under 0xFEED, or in shipping mode a UUID list of 0xFEEC alone; Chipolo, a UUID list of 0xFE33 and the service data under 0xFE33; SmartTag, the 20-byte service data of UUID 0xFD5A from the tag state, the aging counter, the privacy ID, the region, the E2E and UWB flags, the battery code and the signature. No flags structure goes out. A send during another waits for it. Transmit must be on. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_bletracker_send(SDRKIT_HANDLE handle, const sdrkit_bletracker_send_args *args) {
  return sdrkit_typed_command(handle, "send", "wlan.BletrackerSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "bletracker" on the session of `handle`. Drops the packets of the send on air that are still to go, and the sends that wait.
static inline int32_t sdrkit_bletracker_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "bletracker" are the sdrkit_bletracker_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Channel) of the stack "bletracker". A value of `sdrkit_bletracker_channel`. Trackers advertise on all three channels in turn, so any one of them hears every tracker.
static inline int32_t sdrkit_bletracker_get_channel(SDRKIT_HANDLE handle, sdrkit_bletracker_channel *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bletracker_channel)index;
  return code;
}
static inline int32_t sdrkit_bletracker_set_channel(SDRKIT_HANDLE handle, sdrkit_bletracker_channel value) {
  return sdrkit_typed_setting_set_enum(handle, "channel", (int32_t)value);
}

// The setting "alert_after_min" (Alert after) of the stack "bletracker". Unit: min. How long a tracker must stay with the radio before the alert.
static inline int32_t sdrkit_bletracker_get_alert_after_min(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "alert_after_min", value);
}
static inline int32_t sdrkit_bletracker_set_alert_after_min(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "alert_after_min", value);
}

// The setting "near_dbfs" (Near above) of the stack "bletracker". Unit: dBFS. The averaged level from which a tracker counts as near. The radio's level, not a calibrated distance.
static inline int32_t sdrkit_bletracker_get_near_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "near_dbfs", value);
}
static inline int32_t sdrkit_bletracker_set_near_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "near_dbfs", value);
}

// The setting "alert_after_s" (Alert after (seconds, for tests)) of the stack "bletracker". A length of time in seconds. When above 0, replaces the alert time in minutes.
static inline int32_t sdrkit_bletracker_get_alert_after_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "alert_after_s", value);
}
static inline int32_t sdrkit_bletracker_set_alert_after_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "alert_after_s", value);
}

// The setting "tx_level" (Level) of the stack "bletracker". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_bletracker_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_bletracker_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_schedule" (Schedule) of the test signal generator of the stack "bletracker". A value of `sdrkit_bletracker_gen_schedule`.
static inline int32_t sdrkit_bletracker_gen_get_schedule(SDRKIT_HANDLE handle, sdrkit_bletracker_gen_schedule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_schedule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bletracker_gen_schedule)index;
  return code;
}
static inline int32_t sdrkit_bletracker_gen_set_schedule(SDRKIT_HANDLE handle, sdrkit_bletracker_gen_schedule value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_schedule", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "wifi" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wifi_networks_rows(SDRKIT_HANDLE handle,
                                               sdrkit_wifi_network_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.WifiNetworkRow", "networks", rows, sizeof(sdrkit_wifi_network_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "beacons" of the stack "wifi" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wifi_beacons_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wifi_beacon_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.WifiBeaconRow", "beacons", rows, sizeof(sdrkit_wifi_beacon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ofdm_rates" of the stack "wifi" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wifi_ofdm_rates_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wifi_rate_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.WifiRateRow", "ofdm_rates", rows, sizeof(sdrkit_wifi_rate_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bands" of the stack "wifi" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wifi_bands_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wifi_band_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.WifiBandRow", "bands", rows, sizeof(sdrkit_wifi_band_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "wifi" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_wifi_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "wifi" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_wifi_remove(SDRKIT_HANDLE handle, const sdrkit_wlan_network_args *args) {
  return sdrkit_typed_command(handle, "remove", "wlan.NetworkArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wifi" are the sdrkit_wifi_gen_*() functions
// after the settings of its receiver.

// The setting "band" (Band) of the stack "wifi". A value of `sdrkit_wifi_band`. Choosing a band asks the host to tune to the first channel of its hop list.
static inline int32_t sdrkit_wifi_get_band(SDRKIT_HANDLE handle, sdrkit_wifi_band *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "band", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wifi_band)index;
  return code;
}
static inline int32_t sdrkit_wifi_set_band(SDRKIT_HANDLE handle, sdrkit_wifi_band value) {
  return sdrkit_typed_setting_set_enum(handle, "band", (int32_t)value);
}

// The setting "channel" (Channel) of the stack "wifi". A channel of the band: the host tunes to it.
static inline int32_t sdrkit_wifi_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channel", value);
}
static inline int32_t sdrkit_wifi_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channel", value);
}

// The setting "hop" (Hop) of the stack "wifi". Asks the host to step through the hop list; dwell longer than 102.4 ms (the beacon interval).
static inline int32_t sdrkit_wifi_get_hop(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop", value);
}
static inline int32_t sdrkit_wifi_set_hop(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop", value);
}

// The setting "dwell_ms" (Dwell) of the stack "wifi". Unit: ms.
static inline int32_t sdrkit_wifi_get_dwell_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell_ms", value);
}
static inline int32_t sdrkit_wifi_set_dwell_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell_ms", value);
}

// The setting "psc_only" (6 GHz: preferred scanning channels only) of the stack "wifi".
static inline int32_t sdrkit_wifi_get_psc_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "psc_only", value);
}
static inline int32_t sdrkit_wifi_set_psc_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "psc_only", value);
}

// The setting "channels_24" (2.4 GHz channels) of the stack "wifi". Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band.
static inline int32_t sdrkit_wifi_get_channels_24(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_24", value);
}
static inline int32_t sdrkit_wifi_set_channels_24(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_24", value);
}

// The setting "channels_5" (5 GHz channels) of the stack "wifi". Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band.
static inline int32_t sdrkit_wifi_get_channels_5(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_5", value);
}
static inline int32_t sdrkit_wifi_set_channels_5(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_5", value);
}

// The setting "channels_6" (6 GHz channels) of the stack "wifi". Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band.
static inline int32_t sdrkit_wifi_get_channels_6(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_6", value);
}
static inline int32_t sdrkit_wifi_set_channels_6(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_6", value);
}

// The setting "filter_text" (Filter) of the stack "wifi". Shows only the networks whose SSID, BSSID, vendor or security holds this text.
static inline int32_t sdrkit_wifi_get_filter_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "filter_text", value);
}
static inline int32_t sdrkit_wifi_set_filter_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "filter_text", value);
}

// The setting "min_level_dbfs" (Level above) of the stack "wifi". Unit: dBFS.
static inline int32_t sdrkit_wifi_get_min_level_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_level_dbfs", value);
}
static inline int32_t sdrkit_wifi_set_min_level_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_level_dbfs", value);
}

// The setting "open_only" (Open only) of the stack "wifi".
static inline int32_t sdrkit_wifi_get_open_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "open_only", value);
}
static inline int32_t sdrkit_wifi_set_open_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "open_only", value);
}

// The setting "band_filter" (Band shown) of the stack "wifi". A value of `sdrkit_wifi_band_filter`.
static inline int32_t sdrkit_wifi_get_band_filter(SDRKIT_HANDLE handle, sdrkit_wifi_band_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "band_filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wifi_band_filter)index;
  return code;
}
static inline int32_t sdrkit_wifi_set_band_filter(SDRKIT_HANDLE handle, sdrkit_wifi_band_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "band_filter", (int32_t)value);
}

// The setting "gen_scene" (Scene) of the test signal generator of the stack "wifi". A value of `sdrkit_wifi_gen_scene`.
static inline int32_t sdrkit_wifi_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_wifi_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wifi_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_wifi_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_wifi_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_channel" (Primary channel) of the test signal generator of the stack "wifi". 0: the scene's own channel.
static inline int32_t sdrkit_wifi_gen_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_channel", value);
}
static inline int32_t sdrkit_wifi_gen_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_channel", value);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "halow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_halow_networks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_halow_network_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.HalowNetworkRow", "networks", rows, sizeof(sdrkit_halow_network_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "halow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_halow_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_halow_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.HalowFrameRow", "frames", rows, sizeof(sdrkit_halow_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "widths" of the stack "halow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_halow_widths_rows(SDRKIT_HANDLE handle,
                                              sdrkit_halow_width_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.HalowWidthRow", "widths", rows, sizeof(sdrkit_halow_width_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_mcs" of the stack "halow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_halow_by_mcs_rows(SDRKIT_HANDLE handle,
                                              sdrkit_halow_mcs_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.HalowMcsRow", "by_mcs", rows, sizeof(sdrkit_halow_mcs_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_preamble" of the stack "halow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_halow_by_preamble_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_halow_preamble_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.HalowPreambleRow", "by_preamble", rows, sizeof(sdrkit_halow_preamble_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "halow" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_halow_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "halow" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_halow_remove(SDRKIT_HANDLE handle, const sdrkit_wlan_network_args *args) {
  return sdrkit_typed_command(handle, "remove", "wlan.NetworkArgs", args, NULL, NULL, 0);
}

// The setting "start_hz" (Channel starting frequency) of the stack "halow". A frequency in Hz. Channel n is centred at this frequency plus n x 500 kHz.
static inline int32_t sdrkit_halow_get_start_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_hz", value);
}
static inline int32_t sdrkit_halow_set_start_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_hz", value);
}

// The setting "channels_1" (1 MHz channels) of the stack "halow". Channel numbers apart by spaces or commas ("1 3 5").
static inline int32_t sdrkit_halow_get_channels_1(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_1", value);
}
static inline int32_t sdrkit_halow_set_channels_1(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_1", value);
}

// The setting "channels_2" (2 MHz channels) of the stack "halow". Channel numbers apart by spaces or commas ("1 3 5").
static inline int32_t sdrkit_halow_get_channels_2(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_2", value);
}
static inline int32_t sdrkit_halow_set_channels_2(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_2", value);
}

// The setting "channels_4" (4 MHz channels) of the stack "halow". Channel numbers apart by spaces or commas ("1 3 5").
static inline int32_t sdrkit_halow_get_channels_4(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_4", value);
}
static inline int32_t sdrkit_halow_set_channels_4(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_4", value);
}

// The setting "channels_8" (8 MHz channels) of the stack "halow". Channel numbers apart by spaces or commas ("1 3 5").
static inline int32_t sdrkit_halow_get_channels_8(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_8", value);
}
static inline int32_t sdrkit_halow_set_channels_8(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_8", value);
}

// The setting "channels_16" (16 MHz channels) of the stack "halow". Channel numbers apart by spaces or commas ("1 3 5").
static inline int32_t sdrkit_halow_get_channels_16(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_16", value);
}
static inline int32_t sdrkit_halow_set_channels_16(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_16", value);
}

// The setting "list_width" (Channel list width) of the stack "halow". A value of `sdrkit_halow_list_width`.
static inline int32_t sdrkit_halow_get_list_width(SDRKIT_HANDLE handle, sdrkit_halow_list_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "list_width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_halow_list_width)index;
  return code;
}
static inline int32_t sdrkit_halow_set_list_width(SDRKIT_HANDLE handle, sdrkit_halow_list_width value) {
  return sdrkit_typed_setting_set_enum(handle, "list_width", (int32_t)value);
}

// The setting "channel" (Channel) of the stack "halow". A channel of the plan at the list width: the host tunes to it.
static inline int32_t sdrkit_halow_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channel", value);
}
static inline int32_t sdrkit_halow_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channel", value);
}

// The setting "scan" (Scan) of the stack "halow". Asks the host to step through the plan's channels at the list width.
static inline int32_t sdrkit_halow_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_halow_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "dwell_s" (Dwell) of the stack "halow". A length of time in seconds.
static inline int32_t sdrkit_halow_get_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell_s", value);
}
static inline int32_t sdrkit_halow_set_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell_s", value);
}

// The setting "filter_text" (Filter) of the stack "halow". Shows only the networks whose SSID, BSSID, vendor or security holds this text.
static inline int32_t sdrkit_halow_get_filter_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "filter_text", value);
}
static inline int32_t sdrkit_halow_set_filter_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "filter_text", value);
}

// The setting "min_level_dbfs" (Level above) of the stack "halow". Unit: dBFS.
static inline int32_t sdrkit_halow_get_min_level_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_level_dbfs", value);
}
static inline int32_t sdrkit_halow_set_min_level_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_level_dbfs", value);
}

// The setting "receive" (Receive) of the stack "halow". A value of `sdrkit_halow_receive`. The widths read; the rate asked follows, and the radio is reopened when it changes.
static inline int32_t sdrkit_halow_get_receive(SDRKIT_HANDLE handle, sdrkit_halow_receive *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "receive", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_halow_receive)index;
  return code;
}
static inline int32_t sdrkit_halow_set_receive(SDRKIT_HANDLE handle, sdrkit_halow_receive value) {
  return sdrkit_typed_setting_set_enum(handle, "receive", (int32_t)value);
}

// The setting "sub_channel_scan" (Sub-channel scan) of the stack "halow". Look for 1 and 2 MHz primary channels away from the centre of a wider capture.
static inline int32_t sdrkit_halow_get_sub_channel_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sub_channel_scan", value);
}
static inline int32_t sdrkit_halow_set_sub_channel_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sub_channel_scan", value);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "btclassic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_btclassic_devices_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_btclassic_device_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BtclassicDeviceRow", "devices", rows, sizeof(sdrkit_btclassic_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "btclassic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_btclassic_packets_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_btclassic_packet_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.BtclassicPacketRow", "packets", rows, sizeof(sdrkit_btclassic_packet_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "btclassic.device_new"
// (New Bluetooth device). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_btclassic_device_new_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_btclassic_device_new_event *event) {
  return sdrkit_typed_event(handle, n, "btclassic.device_new", "wlan.BtclassicDeviceEvent", event, sizeof(sdrkit_btclassic_device_new_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "btclassic.inquiry_response"
// (Bluetooth inquiry response). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_btclassic_inquiry_response_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_btclassic_inquiry_response_event *event) {
  return sdrkit_typed_event(handle, n, "btclassic.inquiry_response", "wlan.BtclassicInquiryEvent", event, sizeof(sdrkit_btclassic_inquiry_response_event));
}

// Runs the command "clear_devices" (Clear) of the stack "btclassic" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_btclassic_clear_devices(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear_devices", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "btclassic" are the sdrkit_btclassic_gen_*() functions
// after the settings of its receiver.

// The setting "sync_errors" (Sync word errors allowed) of the stack "btclassic". Unit: bits. How many of the 64 sync-word bits may differ from the sync word of the LAP they carry.
static inline int32_t sdrkit_btclassic_get_sync_errors(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sync_errors", value);
}
static inline int32_t sdrkit_btclassic_set_sync_errors(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sync_errors", value);
}

// The setting "threshold_db" (Burst threshold) of the stack "btclassic". Unit: dB. How far above the noise floor of a 1 MHz channel a burst must rise.
static inline int32_t sdrkit_btclassic_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_btclassic_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "wide_channels" (Wideband from) of the stack "btclassic". Unit: channels. Adjacent active channels of similar level, this many or more, are a wideband signal (Wi-Fi) and are skipped.
static inline int32_t sdrkit_btclassic_get_wide_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "wide_channels", value);
}
static inline int32_t sdrkit_btclassic_set_wide_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "wide_channels", value);
}

// The setting "gen_wideband" (Test signal wideband bursts) of the test signal generator of the stack "btclassic".
static inline int32_t sdrkit_btclassic_gen_get_wideband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_wideband", value);
}
static inline int32_t sdrkit_btclassic_gen_set_wideband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_wideband", value);
}

// Fills `rows` with the changed rows of the table "links" of the stack "wfb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wfb_links_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wfb_link_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.WfbLinkRow", "links", rows, sizeof(sdrkit_wfb_link_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "wfb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wfb_frames_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wfb_frame_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.WfbFrameRow", "frames", rows, sizeof(sdrkit_wfb_frame_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "wfb" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_wfb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "links" (Links shown) of the stack "wfb". A value of `sdrkit_wfb_links`. Which systems the tables show.
static inline int32_t sdrkit_wfb_get_links(SDRKIT_HANDLE handle, sdrkit_wfb_links *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "links", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wfb_links)index;
  return code;
}
static inline int32_t sdrkit_wfb_set_links(SDRKIT_HANDLE handle, sdrkit_wfb_links value) {
  return sdrkit_typed_setting_set_enum(handle, "links", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "espnow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_espnow_devices_rows(SDRKIT_HANDLE handle,
                                                sdrkit_espnow_device_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.EspnowDeviceRow", "devices", rows, sizeof(sdrkit_espnow_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "espnow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_espnow_frames_rows(SDRKIT_HANDLE handle,
                                               sdrkit_espnow_frame_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.EspnowFrameRow", "frames", rows, sizeof(sdrkit_espnow_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_rate" of the stack "espnow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_espnow_by_rate_rows(SDRKIT_HANDLE handle,
                                                sdrkit_espnow_rate_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.EspnowRateRow", "by_rate", rows, sizeof(sdrkit_espnow_rate_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_channel" of the stack "espnow" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_espnow_by_channel_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_espnow_channel_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wlan.EspnowChannelRow", "by_channel", rows, sizeof(sdrkit_espnow_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "espnow.frame"
// (ESP-NOW frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_espnow_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_espnow_frame_event *event) {
  return sdrkit_typed_event(handle, n, "espnow.frame", "wlan.EspnowFrameEvent", event, sizeof(sdrkit_espnow_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "espnow.device"
// (New ESP-NOW device). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_espnow_device_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_espnow_device_event *event) {
  return sdrkit_typed_event(handle, n, "espnow.device", "wlan.EspnowDeviceEvent", event, sizeof(sdrkit_espnow_device_event));
}

// Runs the command "clear" (Clear) of the stack "espnow" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_espnow_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "espnow" are the sdrkit_espnow_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Channel) of the stack "espnow". A 2.4 GHz Wi-Fi channel: the host tunes to it.
static inline int32_t sdrkit_espnow_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channel", value);
}
static inline int32_t sdrkit_espnow_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channel", value);
}

// The setting "hop" (Hop) of the stack "espnow". Asks the host to step through the channels of the hop list.
static inline int32_t sdrkit_espnow_get_hop(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop", value);
}
static inline int32_t sdrkit_espnow_set_hop(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop", value);
}

// The setting "dwell_ms" (Dwell) of the stack "espnow". Unit: ms.
static inline int32_t sdrkit_espnow_get_dwell_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell_ms", value);
}
static inline int32_t sdrkit_espnow_set_dwell_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell_ms", value);
}

// The setting "channels_24" (2.4 GHz channels) of the stack "espnow". Channel numbers apart by spaces or commas ("1 6 11"). Empty: every channel of the band.
static inline int32_t sdrkit_espnow_get_channels_24(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels_24", value);
}
static inline int32_t sdrkit_espnow_set_channels_24(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels_24", value);
}

// The setting "keys" (Keys) of the stack "espnow". 16-byte keys, 32 hex digits each, apart by spaces or commas. Each is tried as the CCMP key of a protected frame.
static inline int32_t sdrkit_espnow_get_keys(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "keys", value);
}
static inline int32_t sdrkit_espnow_set_keys(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "keys", value);
}

// The setting "gen_scene" (Scene) of the test signal generator of the stack "espnow". A value of `sdrkit_espnow_gen_scene`.
static inline int32_t sdrkit_espnow_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_espnow_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_espnow_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_espnow_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_espnow_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_phy" (PHY) of the test signal generator of the stack "espnow". A value of `sdrkit_espnow_gen_phy`.
static inline int32_t sdrkit_espnow_gen_get_phy(SDRKIT_HANDLE handle, sdrkit_espnow_gen_phy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_phy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_espnow_gen_phy)index;
  return code;
}
static inline int32_t sdrkit_espnow_gen_set_phy(SDRKIT_HANDLE handle, sdrkit_espnow_gen_phy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_phy", (int32_t)value);
}

// The setting "gen_sender" (Sender) of the test signal generator of the stack "espnow". A MAC address, AA:BB:CC:DD:EE:FF.
static inline int32_t sdrkit_espnow_gen_get_sender(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_sender", value);
}
static inline int32_t sdrkit_espnow_gen_set_sender(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_sender", value);
}

// The setting "gen_peer" (Peer) of the test signal generator of the stack "espnow". A MAC address, AA:BB:CC:DD:EE:FF, or broadcast.
static inline int32_t sdrkit_espnow_gen_get_peer(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_peer", value);
}
static inline int32_t sdrkit_espnow_gen_set_peer(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_peer", value);
}

// The setting "gen_payload" (Payload) of the test signal generator of the stack "espnow". Up to 250 bytes in version 1, 1470 in version 2.
static inline int32_t sdrkit_espnow_gen_get_payload(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_payload", value);
}
static inline int32_t sdrkit_espnow_gen_set_payload(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_payload", value);
}

// The setting "gen_version" (Version) of the test signal generator of the stack "espnow".
static inline int32_t sdrkit_espnow_gen_get_version(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_version", value);
}
static inline int32_t sdrkit_espnow_gen_set_version(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_version", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ble_fields_read(SDRKIT_HANDLE handle, sdrkit_ble_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.BleFields", fields, sizeof(sdrkit_ble_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ble_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_ble_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.BleTxFields", fields, sizeof(sdrkit_ble_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_bletracker_fields_read(SDRKIT_HANDLE handle, sdrkit_bletracker_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.BletrackerFields", fields, sizeof(sdrkit_bletracker_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wifi_fields_read(SDRKIT_HANDLE handle, sdrkit_wifi_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.WifiFields", fields, sizeof(sdrkit_wifi_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_halow_fields_read(SDRKIT_HANDLE handle, sdrkit_halow_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.HalowFields", fields, sizeof(sdrkit_halow_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_btclassic_fields_read(SDRKIT_HANDLE handle, sdrkit_btclassic_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.BtclassicFields", fields, sizeof(sdrkit_btclassic_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wlan_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wlan_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.DecoderFields", fields, sizeof(sdrkit_wlan_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wfb_fields_read(SDRKIT_HANDLE handle, sdrkit_wfb_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.WfbFields", fields, sizeof(sdrkit_wfb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_espnow_fields_read(SDRKIT_HANDLE handle, sdrkit_espnow_fields *fields) {
  return sdrkit_typed_fields(handle, "wlan.EspnowFields", fields, sizeof(sdrkit_espnow_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ble" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ble_dropped_read(SDRKIT_HANDLE handle, sdrkit_ble_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wlan.ble.dropped", dropped, sizeof(sdrkit_ble_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "bletracker" in the
// snapshot of `handle`.
static inline int32_t sdrkit_bletracker_dropped_read(SDRKIT_HANDLE handle, sdrkit_bletracker_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wlan.bletracker.dropped", dropped, sizeof(sdrkit_bletracker_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "wifi" in the
// snapshot of `handle`.
static inline int32_t sdrkit_wifi_dropped_read(SDRKIT_HANDLE handle, sdrkit_wifi_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wlan.wifi.dropped", dropped, sizeof(sdrkit_wifi_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "halow" in the
// snapshot of `handle`.
static inline int32_t sdrkit_halow_dropped_read(SDRKIT_HANDLE handle, sdrkit_halow_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wlan.halow.dropped", dropped, sizeof(sdrkit_halow_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "espnow" in the
// snapshot of `handle`.
static inline int32_t sdrkit_espnow_dropped_read(SDRKIT_HANDLE handle, sdrkit_espnow_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wlan.espnow.dropped", dropped, sizeof(sdrkit_espnow_dropped));
}

#endif
