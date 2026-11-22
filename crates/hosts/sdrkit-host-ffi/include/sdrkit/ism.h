/*
 * sdrkit: the typed interface of the stack family "ism", version 3.1. MIT licence.
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

#ifndef SDRKIT_ISM_H
#define SDRKIT_ISM_H

#include "../sdrkit.h"
#include "ookdev.h"

// The version of the family that this header describes.
#define SDRKIT_ISM_VERSION_MAJOR 3
#define SDRKIT_ISM_VERSION_MINOR 1

// The stack "ism": its id, and the keys of its event types.
#define SDRKIT_STACK_ISM "ism"
#define SDRKIT_EVENT_ISM_DEVICE "ism.device"
#define SDRKIT_EVENT_SIGFOX_MESSAGE "sigfox.message"
#define SDRKIT_EVENT_ISM_PULSE_FRAME "ism.pulse_frame"
#define SDRKIT_EVENT_ISM_TX "ism.tx"

// The stack "wmbus": its id, and the keys of its event types.
#define SDRKIT_STACK_WMBUS "wmbus"
#define SDRKIT_EVENT_WMBUS_FRAME "wmbus.frame"
#define SDRKIT_EVENT_WMBUS_TX "wmbus.tx"

// The stack "zwave": its id, and the keys of its event types.
#define SDRKIT_STACK_ZWAVE "zwave"
#define SDRKIT_EVENT_ZWAVE_FRAME "zwave.frame"
#define SDRKIT_EVENT_ZWAVE_STATE "zwave.state"
#define SDRKIT_EVENT_ZWAVE_TX "zwave.tx"

// The stack "zigbee": its id, and the keys of its event types.
#define SDRKIT_STACK_ZIGBEE "zigbee"
#define SDRKIT_EVENT_ZIGBEE_PAN "zigbee.pan"
#define SDRKIT_EVENT_ZIGBEE_TX "zigbee.tx"

// The stack "sigfox": its id, and the keys of its event types.
#define SDRKIT_STACK_SIGFOX "sigfox"

// The stack "ant": its id, and the keys of its event types.
#define SDRKIT_STACK_ANT "ant"
#define SDRKIT_EVENT_ANT_SENSOR "ant.sensor"
#define SDRKIT_EVENT_ANT_TX "ant.tx"

// The stack "nrf24": its id, and the keys of its event types.
#define SDRKIT_STACK_NRF24 "nrf24"
#define SDRKIT_EVENT_NRF24_ADDRESS "nrf24.address"

// The stack "subghzchat": its id, and the keys of its event types.
#define SDRKIT_STACK_SUBGHZCHAT "subghzchat"
#define SDRKIT_EVENT_SUBGHZCHAT_LINE "subghzchat.line"
#define SDRKIT_EVENT_SUBGHZCHAT_STATE "subghzchat.state"

// The stack "dect": its id, and the keys of its event types.
#define SDRKIT_STACK_DECT "dect"
#define SDRKIT_EVENT_DECT_BASE "dect.base"

// The stack "nfc": its id, and the keys of its event types.
#define SDRKIT_STACK_NFC "nfc"
#define SDRKIT_EVENT_NFC_TAG "nfc.tag"

// The stack "wisun": its id, and the keys of its event types.
#define SDRKIT_STACK_WISUN "wisun"
#define SDRKIT_EVENT_WISUN_PAN "wisun.pan"
#define SDRKIT_EVENT_WISUN_TX "wisun.tx"

// The stack "thread": its id, and the keys of its event types.
#define SDRKIT_STACK_THREAD "thread"
#define SDRKIT_EVENT_THREAD_NETWORK "thread.network"
#define SDRKIT_EVENT_THREAD_DEVICE "thread.device"

// The stack "mioty": its id, and the keys of its event types.
#define SDRKIT_STACK_MIOTY "mioty"
#define SDRKIT_EVENT_MIOTY_FRAME "mioty.frame"

// The stack "dect_nrplus": its id, and the keys of its event types.
#define SDRKIT_STACK_DECT_NRPLUS "dect_nrplus"
#define SDRKIT_EVENT_DECT_NRPLUS_PACKET "dect_nrplus.packet"

// The stack "npr70": its id, and the keys of its event types.
#define SDRKIT_STACK_NPR70 "npr70"
#define SDRKIT_EVENT_NPR70_FRAME "npr70.frame"

// The stack "dsrc_toll": its id, and the keys of its event types.
#define SDRKIT_STACK_DSRC_TOLL "dsrc_toll"
#define SDRKIT_EVENT_DSRC_TOLL_FRAME "dsrc_toll.frame"

// The options of the setting "gen_scene" of the stack "ism".
typedef enum sdrkit_ism_gen_scene {
  // 433.92 MHz sensors
  SDRKIT_ISM_GEN_SCENE_433_92_MHZ_SENSORS = 0,
  // 868.3 MHz sensors
  SDRKIT_ISM_GEN_SCENE_868_3_MHZ_SENSORS = 1,
  // Z-Wave
  SDRKIT_ISM_GEN_SCENE_Z_WAVE = 2,
} sdrkit_ism_gen_scene;

// The options of the setting "submode" of the stack "ism".
typedef enum sdrkit_ism_submode {
  // Device decoders
  SDRKIT_ISM_SUBMODE_DEVICES = 0,
  // OOK / ASK pulse analyser
  SDRKIT_ISM_SUBMODE_PULSE_ANALYSER = 1,
} sdrkit_ism_submode;

// The options of the setting "gen_pattern" of the stack "ism".
typedef enum sdrkit_ism_gen_pattern {
  // PWM (PT2262 style)
  SDRKIT_ISM_GEN_PATTERN_PWM = 0,
  // PPM
  SDRKIT_ISM_GEN_PATTERN_PPM = 1,
  // Manchester
  SDRKIT_ISM_GEN_PATTERN_MANCHESTER = 2,
} sdrkit_ism_gen_pattern;

// The options of the setting "gen_plan" of the stack "wmbus".
typedef enum sdrkit_wmbus_gen_plan {
  // 868 MHz
  SDRKIT_WMBUS_GEN_PLAN_868_MHZ = 0,
  // 169 MHz
  SDRKIT_WMBUS_GEN_PLAN_169_MHZ = 1,
} sdrkit_wmbus_gen_plan;

// The options of the setting "gen_phy" of the stack "wmbus".
typedef enum sdrkit_wmbus_gen_phy {
  // All
  SDRKIT_WMBUS_GEN_PHY_ALL = 0,
  // S1
  SDRKIT_WMBUS_GEN_PHY_S1 = 1,
  // T1
  SDRKIT_WMBUS_GEN_PHY_T1 = 2,
  // C1 format A
  SDRKIT_WMBUS_GEN_PHY_C1_A = 3,
  // C1 format B
  SDRKIT_WMBUS_GEN_PHY_C1_B = 4,
  // R2
  SDRKIT_WMBUS_GEN_PHY_R2 = 5,
  // N1
  SDRKIT_WMBUS_GEN_PHY_N1 = 6,
} sdrkit_wmbus_gen_phy;

// The options of the setting "gen_bit_rate" of the stack "sigfox".
typedef enum sdrkit_sigfox_gen_bit_rate {
  // 100 bit/s
  SDRKIT_SIGFOX_GEN_BIT_RATE_100_BIT_S = 0,
  // 600 bit/s
  SDRKIT_SIGFOX_GEN_BIT_RATE_600_BIT_S = 1,
} sdrkit_sigfox_gen_bit_rate;

// The options of the setting "gen_reps" of the stack "sigfox".
typedef enum sdrkit_sigfox_gen_reps {
  // 3 frames
  SDRKIT_SIGFOX_GEN_REPS_3_FRAMES = 0,
  // 1 frame
  SDRKIT_SIGFOX_GEN_REPS_1_FRAME = 1,
} sdrkit_sigfox_gen_reps;

// The options of the setting "gen_sensors" of the stack "ant".
typedef enum sdrkit_ant_gen_sensors {
  // All three
  SDRKIT_ANT_GEN_SENSORS_ALL = 0,
  // Heart rate strap alone
  SDRKIT_ANT_GEN_SENSORS_STRAP = 1,
  // No sensor
  SDRKIT_ANT_GEN_SENSORS_NONE = 2,
} sdrkit_ant_gen_sensors;

// The options of the setting "gen_rate" of the stack "nrf24".
typedef enum sdrkit_nrf24_gen_rate {
  // 1 Mbit/s
  SDRKIT_NRF24_GEN_RATE_1_MBIT_S = 0,
  // 2 Mbit/s
  SDRKIT_NRF24_GEN_RATE_2_MBIT_S = 1,
  // 250 kbit/s
  SDRKIT_NRF24_GEN_RATE_250_KBIT_S = 2,
  // All three in turn
  SDRKIT_NRF24_GEN_RATE_ALL = 3,
} sdrkit_nrf24_gen_rate;

// The options of the setting "gen_crc" of the stack "nrf24".
typedef enum sdrkit_nrf24_gen_crc {
  // CRC-16
  SDRKIT_NRF24_GEN_CRC_CRC_16 = 0,
  // CRC-8
  SDRKIT_NRF24_GEN_CRC_CRC_8 = 1,
  // Both
  SDRKIT_NRF24_GEN_CRC_BOTH = 2,
} sdrkit_nrf24_gen_crc;

// The options of the setting "encryption" of the stack "subghzchat".
typedef enum sdrkit_subghzchat_encryption {
  // Off
  SDRKIT_SUBGHZCHAT_ENCRYPTION_OFF = 0,
  // Password
  SDRKIT_SUBGHZCHAT_ENCRYPTION_PASSWORD = 1,
  // Hex key
  SDRKIT_SUBGHZCHAT_ENCRYPTION_HEX = 2,
} sdrkit_subghzchat_encryption;

// The options of the setting "frequency" of the stack "subghzchat".
typedef enum sdrkit_subghzchat_frequency {
  // 433.92 MHz
  SDRKIT_SUBGHZCHAT_FREQUENCY_433_92 = 0,
  // 315.00 MHz
  SDRKIT_SUBGHZCHAT_FREQUENCY_315_00 = 1,
  // 390.00 MHz
  SDRKIT_SUBGHZCHAT_FREQUENCY_390_00 = 2,
  // 868.35 MHz
  SDRKIT_SUBGHZCHAT_FREQUENCY_868_35 = 3,
  // 915.00 MHz
  SDRKIT_SUBGHZCHAT_FREQUENCY_915_00 = 4,
} sdrkit_subghzchat_frequency;

// The options of the setting "band" of the stack "dect".
typedef enum sdrkit_dect_band {
  // Europe
  SDRKIT_DECT_BAND_EU = 0,
  // US / Canada (DECT 6.0)
  SDRKIT_DECT_BAND_US = 1,
  // Brazil
  SDRKIT_DECT_BAND_BR = 2,
  // Latin America
  SDRKIT_DECT_BAND_LA = 3,
  // US ISM 900
  SDRKIT_DECT_BAND_ISM900 = 4,
} sdrkit_dect_band;

// The options of the setting "gen_option" of the stack "dect".
typedef enum sdrkit_dect_gen_option {
  // Two bases with handsets
  SDRKIT_DECT_GEN_OPTION_TWO_BASES_WITH_HANDSETS = 0,
  // One base
  SDRKIT_DECT_GEN_OPTION_ONE_BASE = 1,
  // No base
  SDRKIT_DECT_GEN_OPTION_NO_BASE = 2,
} sdrkit_dect_gen_option;

// The result fields of the burst finder of the ISM sensor stack.
typedef struct sdrkit_ism_detect_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts found in the band.
  int64_t bursts_detected;
  // True if `noise_floor_dbfs` holds a value.
  bool has_noise_floor_dbfs;
  // The mean noise floor of a frequency bin, once the finder has frames. Unit: dBFS.
  double noise_floor_dbfs;
  // True if `input_level_dbfs` holds a value.
  bool has_input_level_dbfs;
  // The mean input level since the last report, if samples came. Unit: dBFS.
  double input_level_dbfs;
} sdrkit_ism_detect_fields;

// The count of the Sigfox carrier finder.
typedef struct sdrkit_sigfox_finder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Narrow carriers passed to the demodulator.
  int64_t carriers;
} sdrkit_sigfox_finder_fields;

// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.
typedef struct sdrkit_ism_guard_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Input samples replaced because they were not finite or far beyond full scale.
  int64_t bad_samples;
} sdrkit_ism_guard_fields;

// The result fields of the device table of the ISM sensor stack.
typedef struct sdrkit_ism_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Devices in the table.
  int64_t devices;
  // Distinct transmissions decoded.
  int64_t packets_decoded;
  // Trains no protocol decoded.
  int64_t unknown_bursts;
  // Sigfox messages in the list.
  int64_t sigfox_messages;
} sdrkit_ism_fields;

// The options of `line_code` of `sdrkit_ism_pulse_fields`.
typedef enum sdrkit_ism_pulse_fields_line_code {
  // PWM
  SDRKIT_ISM_PULSE_FIELDS_LINE_CODE_PWM = 0,
  // PPM
  SDRKIT_ISM_PULSE_FIELDS_LINE_CODE_PPM = 1,
  // Manchester
  SDRKIT_ISM_PULSE_FIELDS_LINE_CODE_MANCHESTER = 2,
  // PCM (NRZ-like)
  SDRKIT_ISM_PULSE_FIELDS_LINE_CODE_PCM = 3,
} sdrkit_ism_pulse_fields_line_code;

// The result fields of the pulse analyser (submode `pulse_analyser`): the facts of the last frame with a line code, and the counts. A fact has no value before the first such frame and after a change of frequency, and a timing has a value only for the line codes that have it.
typedef struct sdrkit_ism_pulse_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `line_code` holds a value.
  bool has_line_code;
  // The line code guessed from the pulse timings. A value of `sdrkit_ism_pulse_fields_line_code`.
  int32_t line_code;
  // True if `bits` holds a value.
  bool has_bits;
  // Bits read.
  int64_t bits;
  // True if `bits_bin` holds a value.
  bool has_bits_bin;
  // The first 96 bits as 0 and 1, with "..." when there are more.
  sdrkit_string bits_bin;
  // True if `bits_hex` holds a value.
  bool has_bits_hex;
  // The first 96 bits as hex; the last digit is padded with zero bits.
  sdrkit_string bits_hex;
  // True if `short_us` holds a value.
  bool has_short_us;
  // PWM: the short width. PPM: the short gap. Unit: µs.
  double short_us;
  // True if `long_us` holds a value.
  bool has_long_us;
  // PWM: the long width. PPM: the long gap. Unit: µs.
  double long_us;
  // True if `pulse_us` holds a value.
  bool has_pulse_us;
  // PPM: the pulse width. Unit: µs.
  double pulse_us;
  // True if `period_us` holds a value.
  bool has_period_us;
  // PWM: the bit period. Unit: µs.
  double period_us;
  // True if `half_bit_us` holds a value.
  bool has_half_bit_us;
  // Manchester: the half-bit. Unit: µs.
  double half_bit_us;
  // True if `unit_us` holds a value.
  bool has_unit_us;
  // PCM: the unit. Unit: µs.
  double unit_us;
  // True if `bit_rate` holds a value.
  bool has_bit_rate;
  // Manchester and PCM: the bit rate. Unit: bit/s.
  double bit_rate;
  // True if `pulses` holds a value.
  bool has_pulses;
  // The pulses of the frame.
  int64_t pulses;
  // True if `frame_ms` holds a value.
  bool has_frame_ms;
  // From the start of the first pulse to the end of the last. Unit: ms.
  double frame_ms;
  // True if `repeats` holds a value.
  bool has_repeats;
  // Frames in a row with the same bits, each within 0.5 s of the one before.
  int64_t repeats;
  // Frames analysed, with a line code or not.
  int64_t frames_seen;
  // Frames with a line code.
  int64_t frames_good;
} sdrkit_ism_pulse_fields;

// The result fields of the sender of a frame stack.
typedef struct sdrkit_ism_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a frame is on air.
  bool tx_sending;
  // The frames sent since the stack started.
  int64_t tx_sent;
  // The frames waiting for the transmitter.
  int64_t tx_waiting;
} sdrkit_ism_tx_fields;

// The options of `check` of `sdrkit_ism_device_row`.
typedef enum sdrkit_ism_device_row_check {
  // CRC
  SDRKIT_ISM_DEVICE_ROW_CHECK_CRC = 0,
  // Checksum
  SDRKIT_ISM_DEVICE_ROW_CHECK_CHECKSUM = 1,
  // Parity
  SDRKIT_ISM_DEVICE_ROW_CHECK_PARITY = 2,
  // Repeat
  SDRKIT_ISM_DEVICE_ROW_CHECK_REPEAT = 3,
  // Manchester
  SDRKIT_ISM_DEVICE_ROW_CHECK_MANCHESTER = 4,
} sdrkit_ism_device_row_check;

// A row of the devices. The row key is the model, the id and the channel.
typedef struct sdrkit_ism_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The device model.
  sdrkit_string model;
  // The device kind.
  sdrkit_string kind;
  // The device id.
  sdrkit_string id;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel, if the device sends one.
  sdrkit_string channel;
  // True if `battery_ok` holds a value.
  bool has_battery_ok;
  // The battery state: yes for good, no for low.
  bool battery_ok;
  // The fields of the last reading as one line.
  sdrkit_string readings;
  // True if `history` holds a value.
  bool has_history;
  // The history of the first number with a unit, once the device sent one.
  sdrkit_string history;
  // The level of the last transmission. Unit: dBFS.
  double level_dbfs;
  // The signal to noise ratio of the last transmission. Unit: dB.
  double snr_db;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last transmission. Seconds since 1970 (UTC).
  double last_heard;
  // Distinct transmissions.
  int64_t count;
  // The frequency of the last transmission. A frequency in Hz.
  double frequency;
  // The frame test of the last reading. A value of `sdrkit_ism_device_row_check`.
  int32_t check;
} sdrkit_ism_device_row;

// A row of the Z-Wave view of the devices. The row key is the key of the device.
typedef struct sdrkit_ism_zwave_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `home_id` holds a value.
  bool has_home_id;
  // The home id, if the frame gave one.
  sdrkit_string home_id;
  // True if `node` holds a value.
  bool has_node;
  // The node, if the frame gave one.
  int64_t node;
  // The rate.
  sdrkit_string rate;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // Distinct frames.
  int64_t count;
  // True if `command` holds a value.
  bool has_command;
  // The command, if the frame gave one.
  sdrkit_string command;
  // True if `value` holds a value.
  bool has_value;
  // The value of the command, if the frame gave one.
  sdrkit_string value;
} sdrkit_ism_zwave_row;

// A row of the reading log.
typedef struct sdrkit_ism_log_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The device model.
  sdrkit_string model;
  // The device id.
  sdrkit_string id;
  // The frequency. A frequency in Hz.
  double frequency;
  // The level. Unit: dBFS.
  double level_dbfs;
  // The fields of the reading as one line.
  sdrkit_string text;
} sdrkit_ism_log_row;

// A row of the unknown signals: a train no protocol decoded, with its line-code guess.
typedef struct sdrkit_ism_unknown_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The frequency. A frequency in Hz.
  double frequency;
  // OOK or FSK.
  sdrkit_string modulation;
  // The guessed line code.
  sdrkit_string line_code;
  // Pulses of the train.
  int64_t pulses;
  // The short width of the guess. Unit: µs.
  double short_us;
  // The long width of the guess. Unit: µs.
  double long_us;
  // The level. Unit: dBFS.
  double level_dbfs;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The pulses as text to copy ("OOK PWM 464 us/1404 us; 464/1404 ...").
  sdrkit_string copy;
} sdrkit_ism_unknown_row;

// A row of the Sigfox messages heard inside the ISM sensor stack. The row key is the key of the message.
typedef struct sdrkit_ism_sigfox_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the first frame. Unit: s.
  double time_s;
  // The device id.
  sdrkit_string device_id;
  // The message counter.
  int64_t seq;
  // The payload as hex, or the one-bit value.
  sdrkit_string payload;
  // The frequency of the first repetition. A frequency in Hz.
  double frequency;
  // The level of the strongest repetition. Unit: dBFS.
  double level_dbfs;
  // Repetitions heard.
  int64_t reps;
  // The authentication code, never checked, and the CRC.
  sdrkit_string mac_crc;
} sdrkit_ism_sigfox_row;

// The options of `line_code` of `sdrkit_ism_pulse_frame_row`.
typedef enum sdrkit_ism_pulse_frame_row_line_code {
  // PWM
  SDRKIT_ISM_PULSE_FRAME_ROW_LINE_CODE_PWM = 0,
  // PPM
  SDRKIT_ISM_PULSE_FRAME_ROW_LINE_CODE_PPM = 1,
  // Manchester
  SDRKIT_ISM_PULSE_FRAME_ROW_LINE_CODE_MANCHESTER = 2,
  // PCM (NRZ-like)
  SDRKIT_ISM_PULSE_FRAME_ROW_LINE_CODE_PCM = 3,
} sdrkit_ism_pulse_frame_row_line_code;

// A row of the analysed frames with a line code. The row key is the number of the frame in the log.
typedef struct sdrkit_ism_pulse_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the first pulse. Unit: s.
  double time_s;
  // The frequency. A frequency in Hz.
  double frequency;
  // The line code guessed from the pulse timings. A value of `sdrkit_ism_pulse_frame_row_line_code`.
  int32_t line_code;
  // Bits read.
  int64_t bits;
  // The first 96 bits as 0 and 1, with "..." when there are more.
  sdrkit_string bits_bin;
  // The first 96 bits as hex.
  sdrkit_string bits_hex;
  // True if `short_us` holds a value.
  bool has_short_us;
  // PWM: the short width. PPM: the short gap. Unit: µs.
  double short_us;
  // True if `long_us` holds a value.
  bool has_long_us;
  // PWM: the long width. PPM: the long gap. Unit: µs.
  double long_us;
  // True if `pulse_us` holds a value.
  bool has_pulse_us;
  // PPM: the pulse width. Unit: µs.
  double pulse_us;
  // True if `period_us` holds a value.
  bool has_period_us;
  // PWM: the bit period. Unit: µs.
  double period_us;
  // True if `half_bit_us` holds a value.
  bool has_half_bit_us;
  // Manchester: the half-bit. Unit: µs.
  double half_bit_us;
  // True if `unit_us` holds a value.
  bool has_unit_us;
  // PCM: the unit. Unit: µs.
  double unit_us;
  // True if `bit_rate` holds a value.
  bool has_bit_rate;
  // Manchester and PCM: the bit rate. Unit: bit/s.
  double bit_rate;
  // The pulses of the frame.
  int64_t pulses;
  // From the start of the first pulse to the end of the last. Unit: ms.
  double frame_ms;
  // Frames in a row with the same bits, this one included.
  int64_t repeats;
  // The level of the marks. Unit: dBFS.
  double level_dbfs;
} sdrkit_ism_pulse_frame_row;

// The result fields of the Wireless M-Bus stack.
typedef struct sdrkit_wmbus_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames whose CRCs passed.
  int64_t frames_ok;
  // Frames with a failed CRC.
  int64_t frames_bad;
  // The meters in the table.
  int64_t meters;
  // Syncs found.
  int64_t syncs;
  // Syncs that did not give a frame.
  int64_t false_syncs;
  // Frames with an encrypted payload.
  int64_t encrypted_frames;
} sdrkit_wmbus_fields;

// A row of the meters. The row key is the maker, the id and the type.
typedef struct sdrkit_wmbus_meter_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The manufacturer letters.
  sdrkit_string maker;
  // The meter id (8 digits).
  sdrkit_string meter_id;
  // The device type name.
  sdrkit_string device_type;
  // The physical mode of the last frame ("S1", "T1", "C1 (A)" ...).
  sdrkit_string phy;
  // The channel of the last frame. A frequency in Hz.
  double frequency;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // The payload is encrypted.
  bool encrypted;
  // The summary of the last frame.
  sdrkit_string last_reading;
  // The application status.
  sdrkit_string status;
} sdrkit_wmbus_meter_row;

// A row of the data records of the last frame of each meter. The row key is the row key of the meter, "/" and the index.
typedef struct sdrkit_wmbus_record_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the meter.
  sdrkit_string meter;
  // The place of the record in its frame, from 0.
  int64_t index;
  // The quantity ("Volume").
  sdrkit_string quantity;
  // The value with its unit ("123.456 m3").
  sdrkit_string value;
  // True if `notes` holds a value.
  bool has_notes;
  // The notes ("storage 1"), if any.
  sdrkit_string notes;
} sdrkit_wmbus_record_row;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_wmbus_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the frame, when the samples are live. Seconds since 1970 (UTC).
  double time;
  // The physical mode.
  sdrkit_string phy;
  // The channel. A frequency in Hz.
  double frequency;
  // True if `meter` holds a value.
  bool has_meter;
  // The meter ("KAM 12345678"), if the frame names one.
  sdrkit_string meter;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // Every CRC of the frame passed.
  bool crc_ok;
  // The payload is encrypted.
  bool encrypted;
  // The summary of the frame.
  sdrkit_string result;
  // The frame bytes in hex.
  sdrkit_string frame;
} sdrkit_wmbus_frame_row;

// A row of the channels. The row key is the frequency in MHz and the modes.
typedef struct sdrkit_wmbus_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel frequency. A frequency in Hz.
  double frequency;
  // The physical modes on the channel.
  sdrkit_string phy;
  // The slow power of the channel. Unit: dBFS.
  double level_dbfs;
  // The noise floor of the channel. Unit: dBFS.
  double noise_dbfs;
  // Good frames on the channel.
  int64_t frames;
  // Frames with a failed CRC on the channel.
  int64_t crc_failures;
} sdrkit_wmbus_channel_row;

// The result fields of the Z-Wave stack.
typedef struct sdrkit_zwave_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames decoded at every rate.
  int64_t frames_ok;
  // Bursts whose check failed.
  int64_t frames_bad;
  // The networks in the table.
  int64_t networks;
  // The nodes in the table.
  int64_t nodes;
  // Frames at R1.
  int64_t frames_r1;
  // Frames at R2.
  int64_t frames_r2;
  // Frames at R3.
  int64_t frames_r3;
  // Security S0 frames.
  int64_t s0_frames;
  // Security S2 frames.
  int64_t s2_frames;
  // Acknowledgements.
  int64_t acks;
} sdrkit_zwave_fields;

// A row of the Z-Wave frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_zwave_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the frame, when the samples are live. Seconds since 1970 (UTC).
  double time;
  // The channel frequency. A frequency in Hz.
  double frequency;
  // The rate ("R1", "R2", "R3").
  sdrkit_string rate;
  // The home id (8 hex digits).
  sdrkit_string home_id;
  // Source and destination ("2 > 1").
  sdrkit_string route;
  // The header type.
  sdrkit_string frame_type;
  // The sequence number.
  int64_t seq;
  // An acknowledgement is requested.
  bool ack_requested;
  // The encryption ("none", "S0", "S2").
  sdrkit_string encryption;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The SNR of the frame. Unit: dB.
  double snr_db;
  // The one-line description.
  sdrkit_string command;
  // The MPDU in hex.
  sdrkit_string mpdu;
} sdrkit_zwave_frame_row;

// A row of the Z-Wave nodes. The row key is the home id and the node id.
typedef struct sdrkit_zwave_node_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the network of the node.
  sdrkit_string home_id;
  // The node id.
  int64_t node;
  // "controller" or "node".
  sdrkit_string role;
  // Frames the node sent.
  int64_t sent;
  // Frames addressed to the node.
  int64_t addressed;
  // Encrypted frames the node sent.
  int64_t encrypted;
  // The rate of the last frame.
  sdrkit_string rate;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The last command class the node sent.
  sdrkit_string last_class;
  // The node uses beaming.
  bool beaming;
  // The node sent at low power.
  bool low_power;
} sdrkit_zwave_node_row;

// A row of the Z-Wave networks. The row key is the home id.
typedef struct sdrkit_zwave_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The home id.
  sdrkit_string home_id;
  // The nodes heard.
  int64_t nodes;
  // Node 1 of the network has sent.
  bool controller;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // Encrypted frames.
  int64_t encrypted;
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
} sdrkit_zwave_network_row;

// A row of the Z-Wave channels in use. The row key is the frequency and the rate.
typedef struct sdrkit_zwave_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel frequency. A frequency in Hz.
  double frequency;
  // The rate.
  sdrkit_string rate;
  // The channel is in the band taken in.
  bool covered;
  // Frames decoded on the channel.
  int64_t frames;
  // Bursts whose check failed on the channel.
  int64_t failed_checks;
} sdrkit_zwave_channel_row;

// A row of the command class counts. The row key is the name of the class.
typedef struct sdrkit_zwave_class_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The command class.
  sdrkit_string class_;
  // How often it was sent.
  int64_t count;
} sdrkit_zwave_class_row;

// The result fields of the Zigbee stack.
typedef struct sdrkit_zigbee_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good frames.
  int64_t frames_ok;
  // Frames whose SFD was found and whose FCS failed.
  int64_t frames_bad;
  // Preamble candidates.
  int64_t candidates;
  // Frames with an encrypted payload.
  int64_t encrypted_frames;
  // The networks in the table.
  int64_t networks;
  // The devices in the table.
  int64_t devices;
  // The channel listened to (0 when not known).
  int64_t channel;
  // Good frames in the last second.
  int64_t frames_per_s;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last frame since the last change of frequency. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame since the last change of frequency. A frequency in Hz.
  double offset_hz;
} sdrkit_zigbee_fields;

// A row of the Zigbee networks. The row key is the PAN and the channel as one number.
typedef struct sdrkit_zigbee_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PAN id.
  sdrkit_string pan;
  // True if `ext_pan` holds a value.
  bool has_ext_pan;
  // The extended PAN id, if a beacon gave it.
  sdrkit_string ext_pan;
  // The channel.
  int64_t ch;
  // True if `coordinator` holds a value.
  bool has_coordinator;
  // The coordinator, if known.
  sdrkit_string coordinator;
  // The devices in the PAN.
  int64_t devices;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_zigbee_network_row;

// A row of the devices of an IEEE 802.15.4 stack. The row key is the number of the device.
typedef struct sdrkit_ism_pan_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `short_` holds a value.
  bool has_short_;
  // The short address, if heard.
  sdrkit_string short_;
  // True if `extended` holds a value.
  bool has_extended;
  // The extended address, if heard.
  sdrkit_string extended;
  // True if `vendor` holds a value.
  bool has_vendor;
  // The vendor of the extended address.
  sdrkit_string vendor;
  // True if `pan` holds a value.
  bool has_pan;
  // The PAN id, if known.
  sdrkit_string pan;
  // The channel.
  int64_t ch;
  // "coordinator", "router" or "device".
  sdrkit_string role;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The description of the last frame.
  sdrkit_string last_frame;
} sdrkit_ism_pan_device_row;

// A row of the Zigbee frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_zigbee_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The channel.
  int64_t ch;
  // The MAC frame type.
  sdrkit_string type;
  // "source > destination".
  sdrkit_string route;
  // True if `pan` holds a value.
  bool has_pan;
  // The PAN id, if known.
  sdrkit_string pan;
  // The PSDU length with the FCS.
  int64_t len;
  // Some payload is encrypted.
  bool encrypted;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The description.
  sdrkit_string details;
} sdrkit_zigbee_frame_row;

// The result fields of the message list of the Sigfox stack.
typedef struct sdrkit_sigfox_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames decoded, every repetition.
  int64_t frames_ok;
  // The messages in the list.
  int64_t messages;
  // True if `zone` holds a value.
  bool has_zone;
  // The zone of the dial, if the dial is in one.
  sdrkit_string zone;
} sdrkit_sigfox_fields;

// The count of the Sigfox demodulator.
typedef struct sdrkit_sigfox_demod_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Carriers that did not decode.
  int64_t carriers_bad;
} sdrkit_sigfox_demod_fields;

// A row of the messages. The row key is the device id, the counter and the signal time of the first frame.
typedef struct sdrkit_sigfox_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the first frame. Unit: s.
  double time_s;
  // The device id.
  sdrkit_string device_id;
  // The message counter.
  int64_t seq;
  // The payload as hex, or the one-bit value.
  sdrkit_string payload;
  // The payload length.
  int64_t payload_bytes;
  // The frequency of the first repetition. A frequency in Hz.
  double frequency;
  // The level of the strongest repetition. Unit: dBFS.
  double level_dbfs;
  // Repetitions heard.
  int64_t reps;
  // The ranks heard as a mask (bit 0 is rank 1).
  int64_t ranks;
  // The bit rate. A frequency in Hz.
  double bit_rate;
  // The authentication code, as received and never checked.
  sdrkit_string auth;
  // The CRC, checked.
  sdrkit_string crc;
  // The frame type of the first repetition.
  sdrkit_string frame_type;
  // The length indicator.
  int64_t li;
  // The container size. Unit: bytes.
  int64_t container_bytes;
  // The device requests a downlink.
  bool downlink;
  // A control message.
  bool control;
} sdrkit_sigfox_message_row;

// The result fields of the sensor table of the ANT+ stack.
typedef struct sdrkit_ant_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a good CRC.
  int64_t frames_ok;
  // The sensors in the table.
  int64_t sensors;
  // True if `heart_rate_bpm` holds a value.
  bool has_heart_rate_bpm;
  // The newest heart rate of any strap, if a strap sent one. Unit: bpm.
  double heart_rate_bpm;
  // True if `deviation` holds a value.
  bool has_deviation;
  // The deviation of the last frame. It has no value before the first frame. A frequency in Hz.
  double deviation;
  // The channel the receiver is tuned to. A frequency in Hz.
  double channel;
} sdrkit_ant_fields;

// The counts of the ANT+ demodulator.
typedef struct sdrkit_ant_demod_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames whose CRC failed.
  int64_t frames_bad;
  // Sync words found.
  int64_t syncs;
} sdrkit_ant_demod_fields;

// A row of the sensors. The row key is the device number, type and transmission type as one number.
typedef struct sdrkit_ant_sensor_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The device number.
  int64_t device;
  // The device type name.
  sdrkit_string type;
  // The network marker as hex.
  sdrkit_string marker;
  // True if `manufacturer` holds a value.
  bool has_manufacturer;
  // The manufacturer, once a page names it.
  sdrkit_string manufacturer;
  // True if `serial` holds a value.
  bool has_serial;
  // The serial number, once a page gives it.
  int64_t serial;
  // The main value of the sensor in words.
  sdrkit_string value;
  // True if `battery` holds a value.
  bool has_battery;
  // The battery status, once a page gives it.
  sdrkit_string battery;
  // True if `period` holds a value.
  bool has_period;
  // The measured message period. A length of time in seconds.
  double period;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // The carrier frequency. A frequency in Hz.
  double frequency;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_ant_sensor_row;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_ant_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The device number.
  int64_t device;
  // The device type name.
  sdrkit_string type;
  // The transmission type.
  int64_t tx_type;
  // The network marker as hex.
  sdrkit_string marker;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The payload as hex.
  sdrkit_string payload;
  // The page and its decoded text.
  sdrkit_string text;
} sdrkit_ant_frame_row;

// The result fields of the tables of the nRF24L01 stack.
typedef struct sdrkit_nrf24_nrf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames decoded.
  int64_t frames;
  // Bursts without a frame.
  int64_t bad_bursts;
  // Bursts.
  int64_t bursts;
  // Frames with a CRC-16.
  int64_t crc16_frames;
  // Frames with a CRC-8.
  int64_t crc8_frames;
  // The addresses in the table.
  int64_t addresses;
  // The channel number N (2400 + N MHz).
  int64_t channel;
} sdrkit_nrf24_nrf_fields;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_nrf24_nrf_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The bit rate. A frequency in Hz.
  double bit_rate;
  // The address as hex.
  sdrkit_string address;
  // The address length in bytes.
  int64_t address_len;
  // The payload length in bytes.
  int64_t length;
  // The PID.
  int64_t pid;
  // The NO_ACK bit.
  bool no_ack;
  // The CRC bits.
  int64_t crc_bits;
  // The payload as hex.
  sdrkit_string payload;
  // The printable text of the payload.
  sdrkit_string text;
  // The SNR. Unit: dB.
  double snr_db;
  // The same frame again in a row within 2 s.
  int64_t repeats;
} sdrkit_nrf24_nrf_frame_row;

// A row of the addresses. The row key is the address as hex.
typedef struct sdrkit_nrf24_nrf_address_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address as hex.
  sdrkit_string address;
  // The address length in bytes.
  int64_t address_len;
  // The bit rate. A frequency in Hz.
  double bit_rate;
  // The CRC bits.
  int64_t crc_bits;
  // The last payload length in bytes.
  int64_t length;
  // The carrier frequency. A frequency in Hz.
  double frequency;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_nrf24_nrf_address_row;

// A row of the bursts without a frame. The row key is the number of the burst in the log.
typedef struct sdrkit_nrf24_nrf_bad_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the burst. Unit: s.
  double time_s;
  // The duration of the burst. Unit: µs.
  double duration_us;
  // The SNR. Unit: dB.
  double snr_db;
} sdrkit_nrf24_nrf_bad_burst_row;

// The result fields of the Sub-GHz chat stack.
typedef struct sdrkit_subghzchat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Lines heard.
  int64_t messages;
  // Packets with a good CRC.
  int64_t frames_ok;
  // Packets whose CRC failed.
  int64_t frames_bad;
  // Bursts seen.
  int64_t bursts;
  // Lines decrypted.
  int64_t decrypted;
  // Encrypted lines that the key does not open.
  int64_t unreadable;
  // Encrypted repeats dropped.
  int64_t repeats;
  // Lines the user sent.
  int64_t sent;
  // A key is set.
  bool key_set;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the packets. It has no value before the first good packet. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the last good packet. It has no value before the first good packet. Unit: dBFS.
  double level_dbfs;
} sdrkit_subghzchat_fields;

// The options of `kind` of `sdrkit_subghzchat_line_row`.
typedef enum sdrkit_subghzchat_line_row_kind {
  // Plain
  SDRKIT_SUBGHZCHAT_LINE_ROW_KIND_PLAIN = 0,
  // Decrypted
  SDRKIT_SUBGHZCHAT_LINE_ROW_KIND_DECRYPTED = 1,
  // Encrypted
  SDRKIT_SUBGHZCHAT_LINE_ROW_KIND_ENCRYPTED = 2,
  // Repeat
  SDRKIT_SUBGHZCHAT_LINE_ROW_KIND_REPEAT = 3,
  // Octets
  SDRKIT_SUBGHZCHAT_LINE_ROW_KIND_OCTETS = 4,
} sdrkit_subghzchat_line_row_kind;

// A row of the chat. The row key is the number of the line in the log.
typedef struct sdrkit_subghzchat_line_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the line. Unit: s.
  double time_s;
  // "heard" or "sent".
  sdrkit_string direction;
  // How the line was read. A value of `sdrkit_subghzchat_line_row_kind`.
  int32_t kind;
  // The text.
  sdrkit_string text;
  // The packets joined into the line.
  int64_t packets;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the first packet of a line heard. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the first packet of a line heard. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the first packet of a line heard. A frequency in Hz.
  double offset_hz;
  // True if `run` holds a value.
  bool has_run;
  // The run id and counter of an encrypted line.
  sdrkit_string run;
} sdrkit_subghzchat_line_row;

// A row of the packet log. The row key is the number of the packet in the log.
typedef struct sdrkit_subghzchat_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the packet. Unit: s.
  double time_s;
  // The payload length. Unit: octets.
  int64_t length;
  // The CRC is right.
  bool crc_ok;
  // The packet was found inverted.
  bool inverted;
  // The level of the packet. Unit: dBFS.
  double level_dbfs;
  // The SNR of the packet. Unit: dB.
  double snr_db;
  // The carrier offset of the packet. A frequency in Hz.
  double offset_hz;
  // The payload in hex.
  sdrkit_string hex;
} sdrkit_subghzchat_packet_row;

// The result fields of the base station table of the DECT stack.
typedef struct sdrkit_dect_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The base stations in the table.
  int64_t bases;
  // Bursts from base stations.
  int64_t base_bursts;
  // Bursts from handsets.
  int64_t handset_bursts;
  // The carrier the receiver is tuned to. A frequency in Hz.
  double carrier;
  // True if `band` holds a value.
  bool has_band;
  // The band of the carrier, if the carrier is one of a band.
  sdrkit_string band;
  // True if `carrier_number` holds a value.
  bool has_carrier_number;
  // The number of the carrier in its band, if the carrier is one of a band.
  int64_t carrier_number;
} sdrkit_dect_fields;

// The counts of the DECT demodulator.
typedef struct sdrkit_dect_demod_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A-fields with a good R-CRC.
  int64_t frames_ok;
  // Bursts whose S-field was found and whose A-field failed.
  int64_t frames_bad;
} sdrkit_dect_demod_fields;

// A row of the base stations. The row key is the RFPI.
typedef struct sdrkit_dect_base_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The RFPI as 10 hex digits.
  sdrkit_string rfpi;
  // The access rights class.
  sdrkit_string arc;
  // True if `emc` holds a value.
  bool has_emc;
  // The equipment manufacturer code (class A).
  int64_t emc;
  // True if `fpn` holds a value.
  bool has_fpn;
  // The fixed part number (class A).
  int64_t fpn;
  // True if `rpn` holds a value.
  bool has_rpn;
  // The radio fixed part number (class A).
  int64_t rpn;
  // The slots in use, as 24 digits.
  sdrkit_string slots;
  // The level of the last beacon. Unit: dBFS.
  double level_dbfs;
  // The carrier frequency. A frequency in Hz.
  double frequency;
  // True if `last_qt` holds a value.
  bool has_last_qt;
  // The last Qt tail as hex.
  sdrkit_string last_qt;
  // True if `system_info` holds a value.
  bool has_system_info;
  // The decoded system information of the last Qt tail.
  sdrkit_string system_info;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last beacon. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_dect_base_row;

// A row of the burst log. The row key is the number of the burst in the log.
typedef struct sdrkit_dect_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the burst. Unit: s.
  double time_s;
  // "base" or "handset".
  sdrkit_string from;
  // The name of the tail type.
  sdrkit_string tail_type;
  // The tail as hex.
  sdrkit_string tail;
  // The level of the burst. Unit: dBFS.
  double level_dbfs;
} sdrkit_dect_burst_row;

// The result fields of the tag table of the NFC stack.
typedef struct sdrkit_nfc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames heard.
  int64_t frames;
  // Tags identified.
  int64_t tags;
  // CRC or parity failures outside encrypted sessions.
  int64_t failures;
  // Frames from tags.
  int64_t tag_frames;
} sdrkit_nfc_fields;

// The levels of the NFC detector. They have no value before the first input.
typedef struct sdrkit_nfc_detect_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `field_dbfs` holds a value.
  bool has_field_dbfs;
  // The field level of the input. Unit: dBFS.
  double field_dbfs;
  // True if `subcarrier_dbfs` holds a value.
  bool has_subcarrier_dbfs;
  // The level of the stronger subcarrier detector. Unit: dBFS.
  double subcarrier_dbfs;
} sdrkit_nfc_detect_fields;

// A row of the tags. The row key is the UID as hex.
typedef struct sdrkit_nfc_tag_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UID as hex, most significant first.
  sdrkit_string uid;
  // The type of the tag.
  sdrkit_string type;
  // True if `hint` holds a value.
  bool has_hint;
  // The card-type hint.
  sdrkit_string hint;
  // True if `atqa` holds a value.
  bool has_atqa;
  // The ATQA.
  sdrkit_string atqa;
  // True if `sak` holds a value.
  bool has_sak;
  // The SAK.
  sdrkit_string sak;
  // True if `historical` holds a value.
  bool has_historical;
  // The ATS historical bytes.
  sdrkit_string historical;
  // True if `app_data` holds a value.
  bool has_app_data;
  // The PUPI application data.
  sdrkit_string app_data;
  // True if `system` holds a value.
  bool has_system;
  // The DSFID, AFI, block count and size and IC version byte of an ISO 15693 tag.
  sdrkit_string system;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_nfc_tag_row;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_nfc_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // "reader" or "tag".
  sdrkit_string direction;
  // The link.
  sdrkit_string link;
  // The bytes in hex, or nothing for content not shown.
  sdrkit_string hex;
  // The name of the frame.
  sdrkit_string name;
  // The CRC result.
  sdrkit_string crc;
  // The parity result.
  bool parity_ok;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
} sdrkit_nfc_frame_row;

// The result fields of the Wi-SUN FAN stack.
typedef struct sdrkit_wisun_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good frames.
  int64_t frames_ok;
  // Bad frames.
  int64_t frames_bad;
  // Frames corrected by the least-reliable-bit search.
  int64_t corrected_frames;
  // Frames with an FEC SFD, counted and not decoded.
  int64_t fec_frames;
  // Mode switches.
  int64_t mode_switches;
  // Secured frames.
  int64_t encrypted_frames;
  // The networks in the table.
  int64_t networks;
  // The nodes in the table.
  int64_t nodes;
  // The links in the table.
  int64_t links;
} sdrkit_wisun_fields;

// A row of the Wi-SUN networks. The row key is the PAN id.
typedef struct sdrkit_wisun_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `name` holds a value.
  bool has_name;
  // The network name, once heard.
  sdrkit_string name;
  // True if `border_router` holds a value.
  bool has_border_router;
  // The border router EUI-64, once known.
  sdrkit_string border_router;
  // True if `size_` holds a value.
  bool has_size_;
  // The PAN size, once heard.
  int64_t size_;
  // The nodes in the PAN.
  int64_t nodes;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // True if `schedule` holds a value.
  bool has_schedule;
  // The unicast schedule, once heard.
  sdrkit_string schedule;
} sdrkit_wisun_network_row;

// A row of the Wi-SUN nodes. The row key is the EUI-64.
typedef struct sdrkit_wisun_node_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `vendor` holds a value.
  bool has_vendor;
  // The vendor of the EUI-64.
  sdrkit_string vendor;
  // The role: node, LFN, joining, router or border router.
  sdrkit_string role;
  // True if `pan` holds a value.
  bool has_pan;
  // The row key of the network of the node, once known.
  sdrkit_string pan;
  // True if `cost` holds a value.
  bool has_cost;
  // The routing cost, once heard.
  int64_t cost;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // Frames sent.
  int64_t frames;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // The last channel centre. A frequency in Hz.
  double freq;
  // True if `authenticator` holds a value.
  bool has_authenticator;
  // The authenticator EUI-64, once heard.
  sdrkit_string authenticator;
  // True if `eap` holds a value.
  bool has_eap;
  // The EAP state, once heard.
  sdrkit_string eap;
  // The summary of the last frame.
  sdrkit_string last;
} sdrkit_wisun_node_row;

// A row of the Wi-SUN links. The row key is the two ends.
typedef struct sdrkit_wisun_link_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the sender.
  sdrkit_string from;
  // The row key of the receiver.
  sdrkit_string to;
  // Frames from the sender.
  int64_t frames;
  // Acks from the receiver.
  int64_t acks;
  // True if `rsl_dbm` holds a value.
  bool has_rsl_dbm;
  // The level the receiver reported for the frames of the sender, once reported. Unit: dBm.
  double rsl_dbm;
  // The level of the sender here. Unit: dBFS.
  double level_dbfs;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_wisun_link_row;

// A row of the Wi-SUN frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_wisun_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The channel centre. A frequency in Hz.
  double freq;
  // True if `ch` holds a value.
  bool has_ch;
  // The operating class and channel number in the plan of the bit rate, if the channel is in the plan.
  sdrkit_string ch;
  // The bit rate.
  int64_t kbps;
  // The UTT or 802.15.4 frame type name.
  sdrkit_string type;
  // "source > destination".
  sdrkit_string route;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The summary of the frame.
  sdrkit_string details;
  // The MPDU without the FCS in hex.
  sdrkit_string mpdu;
} sdrkit_wisun_frame_row;

// The result fields of the Thread stack.
typedef struct sdrkit_thread_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Good frames.
  int64_t frames_ok;
  // Candidates whose check failed.
  int64_t frames_bad;
  // Frames that the link layer encrypts.
  int64_t encrypted_frames;
  // Beacon frames.
  int64_t beacons;
  // Data frames.
  int64_t data_frames;
  // Acknowledgements.
  int64_t acks;
  // Command frames.
  int64_t commands;
  // Good frames on channel 11.
  int64_t ch11;
  // Good frames on channel 12.
  int64_t ch12;
  // Good frames on channel 13.
  int64_t ch13;
  // Good frames on channel 14.
  int64_t ch14;
  // Good frames on channel 15.
  int64_t ch15;
  // Good frames on channel 16.
  int64_t ch16;
  // Good frames on channel 17.
  int64_t ch17;
  // Good frames on channel 18.
  int64_t ch18;
  // Good frames on channel 19.
  int64_t ch19;
  // Good frames on channel 20.
  int64_t ch20;
  // Good frames on channel 21.
  int64_t ch21;
  // Good frames on channel 22.
  int64_t ch22;
  // Good frames on channel 23.
  int64_t ch23;
  // Good frames on channel 24.
  int64_t ch24;
  // Good frames on channel 25.
  int64_t ch25;
  // Good frames on channel 26.
  int64_t ch26;
  // Good frames in the last second.
  int64_t frames_per_s;
  // The channel listened to (0 when not known).
  int64_t channel;
  // The stack steps through the channels.
  bool scanning;
  // The networks in the table.
  int64_t networks;
  // The devices in the table.
  int64_t devices;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the last frame since the last change of frequency. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last frame since the last change of frequency. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame since the last change of frequency. A frequency in Hz.
  double offset_hz;
} sdrkit_thread_fields;

// A row of the Thread networks. The row key is the PAN and the channel as one number.
typedef struct sdrkit_thread_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PAN id.
  sdrkit_string pan;
  // The channel where it was heard (0 when not known).
  int64_t ch;
  // True if `network_name` holds a value.
  bool has_network_name;
  // The network name of a Thread beacon.
  sdrkit_string network_name;
  // True if `ext_pan` holds a value.
  bool has_ext_pan;
  // The extended PAN id.
  sdrkit_string ext_pan;
  // True if `version` holds a value.
  bool has_version;
  // The Thread version of the beacon.
  int64_t version;
  // The last beacon permitted joining.
  bool joining_open;
  // A Thread beacon or a 6LoWPAN header was seen in the PAN.
  bool thread_seen;
  // True if `coordinator` holds a value.
  bool has_coordinator;
  // The sender of the beacon.
  sdrkit_string coordinator;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_thread_network_row;

// A row of the Thread frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_thread_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the frame, when the samples are live. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The channel.
  int64_t ch;
  // The MAC frame type.
  sdrkit_string type;
  // The source address.
  sdrkit_string source;
  // The destination address.
  sdrkit_string target;
  // True if `pan` holds a value.
  bool has_pan;
  // The PAN id, if known.
  sdrkit_string pan;
  // The PSDU length with the FCS.
  int64_t len;
  // The link layer encrypts the payload.
  bool encrypted;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // True if `seq` holds a value.
  bool has_seq;
  // The sequence number, if the frame has one.
  int64_t seq;
  // The frame requests an acknowledgement.
  bool ack_request;
  // The description.
  sdrkit_string details;
} sdrkit_thread_frame_row;

// The result fields of the mioty stack.
typedef struct sdrkit_mioty_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames read.
  int64_t frames_ok;
  // Bursts detected.
  int64_t bursts_detected;
  // Patterns that did not decode.
  int64_t failed_hypotheses;
  // The devices in the table.
  int64_t devices;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last frame. It has no value before the first frame. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The power of the input. It has no value before the first input. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_mioty_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_ism_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_ism_decoder_fields;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_mioty_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the frame, when the samples are live. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The device.
  sdrkit_string id;
  // The pattern of the bursts.
  sdrkit_string pattern;
  // The bytes of the MPDU.
  int64_t psi;
  // The MAC mode.
  int64_t mmode;
  // The MAC.
  sdrkit_string mac;
  // The first bytes of the MPDU as hex.
  sdrkit_string mpdu;
  // The frequency. A frequency in Hz.
  double freq_hz;
  // The SNR. Unit: dB.
  double snr_db;
  // The bursts of the pattern that matched. Unit: of 24.
  int64_t bursts_matched;
} sdrkit_mioty_frame_row;

// A row of the devices. The row key is the device.
typedef struct sdrkit_mioty_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The device.
  sdrkit_string id;
  // The frames heard.
  int64_t frames;
  // The last packet counter.
  int64_t counter;
  // The SNR of the last frame. Unit: dB.
  double snr_db;
  // The frequency of the last frame. A frequency in Hz.
  double freq_hz;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_mioty_device_row;

// The result fields of the DECT NR+ stack.
typedef struct sdrkit_dect_nrplus_nr_plus_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Packets read with a good check.
  int64_t frames_ok;
  // Short training field detections.
  int64_t stf_detections;
  // The devices in the table.
  int64_t devices;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last packet. It has no value before the first packet. Unit: dB.
  double snr_db;
  // True if `cfo_hz` holds a value.
  bool has_cfo_hz;
  // The carrier offset of the last packet. It has no value before the first packet. Unit: Hz.
  double cfo_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The power of the input. It has no value before the first input. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_dect_nrplus_nr_plus_fields;

// A row of the packet log. The row key is the number of the packet in the log.
typedef struct sdrkit_dect_nrplus_nr_plus_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the packet, when the samples are live. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the packet. Unit: s.
  double time_s;
  // The device.
  sdrkit_string id;
  // The type of the packet.
  sdrkit_string type;
  // The fields of the packet.
  sdrkit_string text;
  // The carrier offset. Unit: Hz.
  double cfo_hz;
  // The SNR. Unit: dB.
  double snr_db;
} sdrkit_dect_nrplus_nr_plus_packet_row;

// A row of the devices. The row key is the device number.
typedef struct sdrkit_dect_nrplus_nr_plus_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The transmitter.
  sdrkit_string transmitter;
  // The device.
  sdrkit_string id;
  // The packets heard.
  int64_t packets;
  // The fields of the last packet.
  sdrkit_string text;
  // The SNR of the last packet. Unit: dB.
  double snr_db;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first packet. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_dect_nrplus_nr_plus_device_row;

// The result fields of the New Packet Radio stack.
typedef struct sdrkit_npr70_npr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Sync words found.
  int64_t sync_words;
  // The symbol rates searched, in ksym/s. Unit: ksym/s.
  sdrkit_string rates_searched;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The power of the input. It has no value before the first input. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_npr70_npr_fields;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_npr70_npr_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the frame, when the samples are live. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The network.
  sdrkit_string network;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // The sync word.
  sdrkit_string sync;
  // The tuning offset of the frame. Unit: Hz.
  double tuning_hz;
  // What the payload is.
  sdrkit_string payload;
} sdrkit_npr70_npr_frame_row;

// A row of the networks heard. The row key is the network number.
typedef struct sdrkit_npr70_npr_network_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The network.
  sdrkit_string network;
  // The frames heard.
  int64_t frames;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_npr70_npr_network_row;

// The result fields of the DSRC road toll stack.
typedef struct sdrkit_dsrc_toll_dsrc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a good check.
  int64_t frames_ok;
  // Broadcast frames.
  int64_t broadcast_frames;
  // Private link frames.
  int64_t private_frames;
  // Frames that failed their check.
  int64_t frames_bad;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The power of the input. It has no value before the first input. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_dsrc_toll_dsrc_fields;

// A row of the frame log. The row key is the number of the frame in the log.
typedef struct sdrkit_dsrc_toll_dsrc_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The class of the frame.
  sdrkit_string kind;
  // The bytes of the frame, without the check.
  int64_t bytes;
  // True if `control` holds a value.
  bool has_control;
  // The control byte as hex, if the frame has one.
  sdrkit_string control;
  // What is not shown.
  sdrkit_string identifier;
} sdrkit_dsrc_toll_dsrc_frame_row;

// The items that each decoder component of the stack "ant" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ant_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `demod` holds a value.
  bool has_demod;
  // The items that the component "demod" dropped.
  int64_t demod;
} sdrkit_ant_dropped;

// The items that each decoder component of the stack "nrf24" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_nrf24_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `demod` holds a value.
  bool has_demod;
  // The items that the component "demod" dropped.
  int64_t demod;
} sdrkit_nrf24_dropped;

// The items that each decoder component of the stack "dect" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_dect_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `demod` holds a value.
  bool has_demod;
  // The items that the component "demod" dropped.
  int64_t demod;
} sdrkit_dect_dropped;

// The options of `family` of `sdrkit_ism_device_event`.
typedef enum sdrkit_ism_device_event_family {
  // Aircraft
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ISM_DEVICE_EVENT_FAMILY_OTHER = 12,
} sdrkit_ism_device_event_family;

// The fields of a device heard for the first time. `source` is the model and the id.
typedef struct sdrkit_ism_device_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The model and the id.
  sdrkit_string source;
  // The line of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_ism_device_event_family`.
  int32_t family;
  // The device model.
  sdrkit_string model;
  // The device id.
  sdrkit_string id;
} sdrkit_ism_device_event;

// The options of `family` of `sdrkit_sigfox_message_event`.
typedef enum sdrkit_sigfox_message_event_family {
  // Aircraft
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SIGFOX_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_sigfox_message_event_family;

// The fields of a new message event.
typedef struct sdrkit_sigfox_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The device id.
  sdrkit_string source;
  // The log line of the message.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_sigfox_message_event_family`.
  int32_t family;
  // The device id.
  sdrkit_string device_id;
  // The message counter.
  int64_t seq;
  // The payload as hex.
  sdrkit_string payload;
} sdrkit_sigfox_message_event;

// The options of `family` of `sdrkit_ism_pulse_frame_event`.
typedef enum sdrkit_ism_pulse_frame_event_family {
  // Aircraft
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ISM_PULSE_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_ism_pulse_frame_event_family;

// The options of `line_code` of `sdrkit_ism_pulse_frame_event`.
typedef enum sdrkit_ism_pulse_frame_event_line_code {
  // PWM
  SDRKIT_ISM_PULSE_FRAME_EVENT_LINE_CODE_PWM = 0,
  // PPM
  SDRKIT_ISM_PULSE_FRAME_EVENT_LINE_CODE_PPM = 1,
  // Manchester
  SDRKIT_ISM_PULSE_FRAME_EVENT_LINE_CODE_MANCHESTER = 2,
  // PCM (NRZ-like)
  SDRKIT_ISM_PULSE_FRAME_EVENT_LINE_CODE_PCM = 3,
} sdrkit_ism_pulse_frame_event_line_code;

// The fields of an analysed frame with a line code.
typedef struct sdrkit_ism_pulse_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The line of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_ism_pulse_frame_event_family`.
  int32_t family;
  // The line code guessed from the pulse timings. A value of `sdrkit_ism_pulse_frame_event_line_code`.
  int32_t line_code;
  // Bits read.
  int64_t bits;
  // The first 96 bits as hex.
  sdrkit_string bits_hex;
  // Frames in a row with the same bits, this one included.
  int64_t repeats;
  // The level of the marks. Unit: dBFS.
  double level_dbfs;
} sdrkit_ism_pulse_frame_event;

// The options of `family` of `sdrkit_ism_tx_event`.
typedef enum sdrkit_ism_tx_event_family {
  // Aircraft
  SDRKIT_ISM_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ISM_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ISM_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ISM_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ISM_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ISM_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ISM_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ISM_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ISM_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ISM_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ISM_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ISM_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ISM_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_ism_tx_event_family;

// The fields of a change of a transmission of a frame stack.
typedef struct sdrkit_ism_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The addressee or the identity of the frame.
  sdrkit_string target;
  // What the frame carries, for started and queued; empty otherwise.
  sdrkit_string text;
  // The family of the source: a sensor. A value of `sdrkit_ism_tx_event_family`.
  int32_t family;
} sdrkit_ism_tx_event;

// The options of `family` of `sdrkit_wmbus_frame_event`.
typedef enum sdrkit_wmbus_frame_event_family {
  // Aircraft
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WMBUS_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_wmbus_frame_event_family;

// The fields of a frame event.
typedef struct sdrkit_wmbus_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The meter: maker and id ("KAM 12345678").
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_wmbus_frame_event_family`.
  int32_t family;
  // The manufacturer letters.
  sdrkit_string maker;
  // The meter id.
  sdrkit_string meter_id;
  // The device type name.
  sdrkit_string device_type;
  // The physical mode.
  sdrkit_string phy;
  // The payload is encrypted.
  bool encrypted;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
} sdrkit_wmbus_frame_event;

// The options of `family` of `sdrkit_wmbus_tx_event`.
typedef enum sdrkit_wmbus_tx_event_family {
  // Aircraft
  SDRKIT_WMBUS_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WMBUS_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WMBUS_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WMBUS_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WMBUS_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WMBUS_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WMBUS_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WMBUS_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WMBUS_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WMBUS_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WMBUS_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WMBUS_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WMBUS_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_wmbus_tx_event_family;

// The fields of a change of a transmission of a frame stack.
typedef struct sdrkit_wmbus_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The addressee or the identity of the frame.
  sdrkit_string target;
  // What the frame carries, for started and queued; empty otherwise.
  sdrkit_string text;
  // The family of the source: a sensor. A value of `sdrkit_wmbus_tx_event_family`.
  int32_t family;
} sdrkit_wmbus_tx_event;

// The options of `family` of `sdrkit_zwave_frame_event`.
typedef enum sdrkit_zwave_frame_event_family {
  // Aircraft
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ZWAVE_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_zwave_frame_event_family;

// The fields of a Z-Wave frame event.
typedef struct sdrkit_zwave_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender: home id and node ("C0FFEE01:2").
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The addressee, for a frame to one node.
  sdrkit_string target;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_zwave_frame_event_family`.
  int32_t family;
  // The home id.
  sdrkit_string home_id;
  // Source and destination.
  sdrkit_string route;
  // The rate.
  sdrkit_string rate;
  // The header type.
  sdrkit_string frame_type;
  // The encryption.
  sdrkit_string encryption;
  // The one-line description.
  sdrkit_string command;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The SNR of the frame. Unit: dB.
  double snr_db;
} sdrkit_zwave_frame_event;

// The options of `family` of `sdrkit_zwave_state_event`.
typedef enum sdrkit_zwave_state_event_family {
  // Aircraft
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ZWAVE_STATE_EVENT_FAMILY_OTHER = 12,
} sdrkit_zwave_state_event_family;

// The fields of the event that Z-Wave frames start or stop coming in.
typedef struct sdrkit_zwave_state_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_zwave_state_event_family`.
  int32_t family;
} sdrkit_zwave_state_event;

// The options of `family` of `sdrkit_zwave_tx_event`.
typedef enum sdrkit_zwave_tx_event_family {
  // Aircraft
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ZWAVE_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_zwave_tx_event_family;

// The fields of a change of a transmission of a frame stack.
typedef struct sdrkit_zwave_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The addressee or the identity of the frame.
  sdrkit_string target;
  // What the frame carries, for started and queued; empty otherwise.
  sdrkit_string text;
  // The family of the source: a sensor. A value of `sdrkit_zwave_tx_event_family`.
  int32_t family;
} sdrkit_zwave_tx_event;

// The options of `family` of `sdrkit_zigbee_pan_event`.
typedef enum sdrkit_zigbee_pan_event_family {
  // Aircraft
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ZIGBEE_PAN_EVENT_FAMILY_OTHER = 12,
} sdrkit_zigbee_pan_event_family;

// The fields of a new PAN event.
typedef struct sdrkit_zigbee_pan_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The PAN id.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_zigbee_pan_event_family`.
  int32_t family;
  // The PAN id.
  sdrkit_string pan;
  // The channel.
  int64_t ch;
} sdrkit_zigbee_pan_event;

// The options of `family` of `sdrkit_zigbee_tx_event`.
typedef enum sdrkit_zigbee_tx_event_family {
  // Aircraft
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ZIGBEE_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_zigbee_tx_event_family;

// The fields of a change of a transmission of a frame stack.
typedef struct sdrkit_zigbee_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The addressee or the identity of the frame.
  sdrkit_string target;
  // What the frame carries, for started and queued; empty otherwise.
  sdrkit_string text;
  // The family of the source: a sensor. A value of `sdrkit_zigbee_tx_event_family`.
  int32_t family;
} sdrkit_zigbee_tx_event;

// The options of `family` of `sdrkit_ant_sensor_event`.
typedef enum sdrkit_ant_sensor_event_family {
  // Aircraft
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ANT_SENSOR_EVENT_FAMILY_OTHER = 12,
} sdrkit_ant_sensor_event_family;

// The fields of a new sensor event.
typedef struct sdrkit_ant_sensor_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The device number.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_ant_sensor_event_family`.
  int32_t family;
  // The device number.
  int64_t device;
  // The device type name.
  sdrkit_string type;
  // The network marker as hex.
  sdrkit_string marker;
} sdrkit_ant_sensor_event;

// The options of `family` of `sdrkit_ant_tx_event`.
typedef enum sdrkit_ant_tx_event_family {
  // Aircraft
  SDRKIT_ANT_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ANT_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ANT_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ANT_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ANT_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ANT_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ANT_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ANT_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ANT_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ANT_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ANT_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ANT_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ANT_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_ant_tx_event_family;

// The fields of a change of a transmission of a frame stack.
typedef struct sdrkit_ant_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The addressee or the identity of the frame.
  sdrkit_string target;
  // What the frame carries, for started and queued; empty otherwise.
  sdrkit_string text;
  // The family of the source: a sensor. A value of `sdrkit_ant_tx_event_family`.
  int32_t family;
} sdrkit_ant_tx_event;

// The options of `family` of `sdrkit_nrf24_address_event`.
typedef enum sdrkit_nrf24_address_event_family {
  // Aircraft
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_NRF24_ADDRESS_EVENT_FAMILY_OTHER = 12,
} sdrkit_nrf24_address_event_family;

// The fields of a new address event.
typedef struct sdrkit_nrf24_address_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address as hex.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_nrf24_address_event_family`.
  int32_t family;
  // The bit rate. A frequency in Hz.
  double bit_rate;
  // The CRC bits.
  int64_t crc_bits;
} sdrkit_nrf24_address_event;

// The options of `family` of `sdrkit_subghzchat_line_event`.
typedef enum sdrkit_subghzchat_line_event_family {
  // Aircraft
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SUBGHZCHAT_LINE_EVENT_FAMILY_OTHER = 12,
} sdrkit_subghzchat_line_event_family;

// The options of `kind` of `sdrkit_subghzchat_line_event`.
typedef enum sdrkit_subghzchat_line_event_kind {
  // Plain
  SDRKIT_SUBGHZCHAT_LINE_EVENT_KIND_PLAIN = 0,
  // Decrypted
  SDRKIT_SUBGHZCHAT_LINE_EVENT_KIND_DECRYPTED = 1,
  // Encrypted
  SDRKIT_SUBGHZCHAT_LINE_EVENT_KIND_ENCRYPTED = 2,
  // Repeat
  SDRKIT_SUBGHZCHAT_LINE_EVENT_KIND_REPEAT = 3,
  // Octets
  SDRKIT_SUBGHZCHAT_LINE_EVENT_KIND_OCTETS = 4,
} sdrkit_subghzchat_line_event_kind;

// The fields of a chat line event.
typedef struct sdrkit_subghzchat_line_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_subghzchat_line_event_family`.
  int32_t family;
  // How the line was read. A value of `sdrkit_subghzchat_line_event_kind`.
  int32_t kind;
  // The SNR of the first packet. Unit: dB.
  double snr_db;
} sdrkit_subghzchat_line_event;

// The options of `family` of `sdrkit_subghzchat_state_event`.
typedef enum sdrkit_subghzchat_state_event_family {
  // Aircraft
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SUBGHZCHAT_STATE_EVENT_FAMILY_OTHER = 12,
} sdrkit_subghzchat_state_event_family;

// The fields of the event that packets come in after a quiet time.
typedef struct sdrkit_subghzchat_state_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_subghzchat_state_event_family`.
  int32_t family;
} sdrkit_subghzchat_state_event;

// The options of `family` of `sdrkit_dect_base_event`.
typedef enum sdrkit_dect_base_event_family {
  // Aircraft
  SDRKIT_DECT_BASE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DECT_BASE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DECT_BASE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DECT_BASE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DECT_BASE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DECT_BASE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DECT_BASE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DECT_BASE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DECT_BASE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DECT_BASE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DECT_BASE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DECT_BASE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DECT_BASE_EVENT_FAMILY_OTHER = 12,
} sdrkit_dect_base_event_family;

// The fields of a new base station event.
typedef struct sdrkit_dect_base_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The RFPI as 10 hex digits.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_dect_base_event_family`.
  int32_t family;
  // The RFPI as 10 hex digits.
  sdrkit_string rfpi;
  // True if `emc` holds a value.
  bool has_emc;
  // The equipment manufacturer code (class A).
  int64_t emc;
} sdrkit_dect_base_event;

// The options of `family` of `sdrkit_nfc_tag_event`.
typedef enum sdrkit_nfc_tag_event_family {
  // Aircraft
  SDRKIT_NFC_TAG_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_NFC_TAG_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_NFC_TAG_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_NFC_TAG_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_NFC_TAG_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_NFC_TAG_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_NFC_TAG_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_NFC_TAG_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_NFC_TAG_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_NFC_TAG_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_NFC_TAG_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_NFC_TAG_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_NFC_TAG_EVENT_FAMILY_OTHER = 12,
} sdrkit_nfc_tag_event_family;

// The fields of a tag event.
typedef struct sdrkit_nfc_tag_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The UID as hex.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_nfc_tag_event_family`.
  int32_t family;
  // The UID as hex.
  sdrkit_string uid;
} sdrkit_nfc_tag_event;

// The options of `family` of `sdrkit_wisun_pan_event`.
typedef enum sdrkit_wisun_pan_event_family {
  // Aircraft
  SDRKIT_WISUN_PAN_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WISUN_PAN_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WISUN_PAN_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WISUN_PAN_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WISUN_PAN_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WISUN_PAN_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WISUN_PAN_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WISUN_PAN_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WISUN_PAN_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WISUN_PAN_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WISUN_PAN_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WISUN_PAN_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WISUN_PAN_EVENT_FAMILY_OTHER = 12,
} sdrkit_wisun_pan_event_family;

// The fields of a new Wi-SUN PAN event.
typedef struct sdrkit_wisun_pan_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The PAN id.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_wisun_pan_event_family`.
  int32_t family;
  // The PAN id.
  sdrkit_string pan;
} sdrkit_wisun_pan_event;

// The options of `family` of `sdrkit_wisun_tx_event`.
typedef enum sdrkit_wisun_tx_event_family {
  // Aircraft
  SDRKIT_WISUN_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WISUN_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WISUN_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WISUN_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WISUN_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WISUN_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WISUN_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WISUN_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WISUN_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WISUN_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WISUN_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WISUN_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WISUN_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_wisun_tx_event_family;

// The fields of a change of a transmission of a frame stack.
typedef struct sdrkit_wisun_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The addressee or the identity of the frame.
  sdrkit_string target;
  // What the frame carries, for started and queued; empty otherwise.
  sdrkit_string text;
  // The family of the source: a sensor. A value of `sdrkit_wisun_tx_event_family`.
  int32_t family;
} sdrkit_wisun_tx_event;

// The options of `family` of `sdrkit_thread_network_event`.
typedef enum sdrkit_thread_network_event_family {
  // Aircraft
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_THREAD_NETWORK_EVENT_FAMILY_OTHER = 12,
} sdrkit_thread_network_event_family;

// The fields of a new Thread network event.
typedef struct sdrkit_thread_network_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The PAN id.
  sdrkit_string source;
  // The text of the event.
  sdrkit_string text;
  // The network name.
  sdrkit_string network_name;
  // The extended PAN id.
  sdrkit_string ext_pan;
  // The channel.
  int64_t ch;
  // The family of the event. A value of `sdrkit_thread_network_event_family`.
  int32_t family;
} sdrkit_thread_network_event;

// The options of `family` of `sdrkit_thread_device_event`.
typedef enum sdrkit_thread_device_event_family {
  // Aircraft
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_THREAD_DEVICE_EVENT_FAMILY_OTHER = 12,
} sdrkit_thread_device_event_family;

// The fields of a new Thread device event.
typedef struct sdrkit_thread_device_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address of the device.
  sdrkit_string source;
  // True if `pan` holds a value.
  bool has_pan;
  // The PAN id, if known.
  sdrkit_string pan;
  // The channel.
  int64_t ch;
  // The family of the event. A value of `sdrkit_thread_device_event_family`.
  int32_t family;
} sdrkit_thread_device_event;

// The options of `family` of `sdrkit_mioty_frame_event`.
typedef enum sdrkit_mioty_frame_event_family {
  // Aircraft
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MIOTY_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_mioty_frame_event_family;

// The fields of a frame event.
typedef struct sdrkit_mioty_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The device.
  sdrkit_string id;
  // The pattern of the bursts.
  sdrkit_string pattern;
  // The bytes of the MPDU.
  int64_t psi;
  // The MAC.
  sdrkit_string mac;
  // The family of the event. A value of `sdrkit_mioty_frame_event_family`.
  int32_t family;
} sdrkit_mioty_frame_event;

// The options of `family` of `sdrkit_dect_nrplus_packet_event`.
typedef enum sdrkit_dect_nrplus_packet_event_family {
  // Aircraft
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DECT_NRPLUS_PACKET_EVENT_FAMILY_OTHER = 12,
} sdrkit_dect_nrplus_packet_event_family;

// The fields of a packet event.
typedef struct sdrkit_dect_nrplus_packet_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The device.
  sdrkit_string id;
  // The type of the packet.
  sdrkit_string type;
  // The fields of the packet.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_dect_nrplus_packet_event_family`.
  int32_t family;
} sdrkit_dect_nrplus_packet_event;

// The options of `family` of `sdrkit_npr70_frame_event`.
typedef enum sdrkit_npr70_frame_event_family {
  // Aircraft
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_NPR70_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_npr70_frame_event_family;

// The fields of a frame event.
typedef struct sdrkit_npr70_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The network.
  sdrkit_string network;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // The family of the event. A value of `sdrkit_npr70_frame_event_family`.
  int32_t family;
} sdrkit_npr70_frame_event;

// The options of `family` of `sdrkit_dsrc_toll_frame_event`.
typedef enum sdrkit_dsrc_toll_frame_event_family {
  // Aircraft
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DSRC_TOLL_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_dsrc_toll_frame_event_family;

// The fields of a frame event.
typedef struct sdrkit_dsrc_toll_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The class of the frame.
  sdrkit_string kind;
  // The bytes of the frame, without the check.
  int64_t bytes;
  // The family of the event. A value of `sdrkit_dsrc_toll_frame_event_family`.
  int32_t family;
} sdrkit_dsrc_toll_frame_event;

// The arguments of `remove` of an IEEE 802.15.4 stack.
typedef struct sdrkit_ism_pan_device_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the device.
  sdrkit_string row;
} sdrkit_ism_pan_device_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ISM_PAN_DEVICE_ARGS_INIT {sizeof(sdrkit_ism_pan_device_args), {0}}

// The arguments of `send` of the ISM sensor stack. Each family reads only the arguments its help names; the values are those that the stack reports for the family.
typedef struct sdrkit_ism_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `family` holds a value.
  bool has_family;
  // The device family. The index of an option.
  int32_t family;
  // The id of the device.
  int64_t id;
  // True if `command` holds a value.
  bool has_command;
  // The command of a remote.
  int64_t command;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel of a sensor.
  int64_t channel;
  // True if `battery_ok` holds a value.
  bool has_battery_ok;
  // Whether the battery of a sensor is good.
  bool battery_ok;
  // True if `temperature` holds a value.
  bool has_temperature;
  // The temperature of a sensor. Unit: °C.
  double temperature;
  // True if `humidity` holds a value.
  bool has_humidity;
  // The humidity of a sensor. Unit: %.
  int64_t humidity;
  // True if `wind_speed` holds a value.
  bool has_wind_speed;
  // The mean wind speed. Unit: m/s.
  double wind_speed;
  // True if `wind_gust` holds a value.
  bool has_wind_gust;
  // The wind gust. Unit: m/s.
  double wind_gust;
  // True if `wind_dir` holds a value.
  bool has_wind_dir;
  // The wind direction. Unit: °.
  double wind_dir;
  // True if `rain` holds a value.
  bool has_rain;
  // The rain count. Unit: mm.
  double rain;
  // True if `uv` holds a value.
  bool has_uv;
  // The UV index.
  double uv;
  // True if `light` holds a value.
  bool has_light;
  // The light level. Unit: lx.
  double light;
  // True if `pressure` holds a value.
  bool has_pressure;
  // The tyre pressure. Unit: kPa.
  double pressure;
  // True if `flags` holds a value.
  bool has_flags;
  // The flags of a tyre sensor.
  int64_t flags;
  // True if `current` holds a value.
  bool has_current;
  // The current of a clamp. Unit: A.
  double current;
  // True if `interval` holds a value.
  bool has_interval;
  // The report interval. Unit: s.
  int64_t interval;
  // True if `power` holds a value.
  bool has_power;
  // The power of a meter. Unit: W.
  int64_t power;
  // True if `payload` holds a value.
  bool has_payload;
  // The frame octets of a family with a raw payload.
  sdrkit_string payload;
  // True if `temperature_2` holds a value.
  bool has_temperature_2;
  // The second temperature of a sensor with two probes. Unit: °C.
  double temperature_2;
  // True if `meter_type` holds a value.
  bool has_meter_type;
  // The type of a meter.
  int64_t meter_type;
  // True if `consumption` holds a value.
  bool has_consumption;
  // The consumption count of a meter.
  int64_t consumption;
  // True if `moisture` holds a value.
  bool has_moisture;
  // The soil moisture of a sensor. Unit: %.
  int64_t moisture;
} sdrkit_ism_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ISM_SEND_ARGS_INIT {sizeof(sdrkit_ism_send_args), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0}

// The arguments of `remove` of the Wireless M-Bus stack.
typedef struct sdrkit_wmbus_meter_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the meter.
  sdrkit_string row;
} sdrkit_wmbus_meter_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WMBUS_METER_ARGS_INIT {sizeof(sdrkit_wmbus_meter_args), {0}}

// The arguments of `send` of the Wireless M-Bus stack.
typedef struct sdrkit_wmbus_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `mode` holds a value.
  bool has_mode;
  // The physical mode and frame format. The index of an option.
  int32_t mode;
  // The manufacturer.
  sdrkit_string manufacturer;
  // The address (identification number).
  sdrkit_string address;
  // The version.
  int64_t version;
  // The device type.
  int64_t device_type;
  // The application data.
  sdrkit_string payload;
} sdrkit_wmbus_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WMBUS_SEND_ARGS_INIT {sizeof(sdrkit_wmbus_send_args), 0, 0, {0}, {0}, 0, 0, {0}}

// The arguments of `remove` of the Z-Wave stack.
typedef struct sdrkit_zwave_node_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the node.
  sdrkit_string row;
} sdrkit_zwave_node_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ZWAVE_NODE_ARGS_INIT {sizeof(sdrkit_zwave_node_args), {0}}

// The arguments of `send` of the Z-Wave stack.
typedef struct sdrkit_zwave_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The home id.
  sdrkit_string home_id;
  // The source node.
  int64_t source;
  // The destination node.
  int64_t destination;
  // The application command.
  sdrkit_string payload;
  // True if `rate` holds a value.
  bool has_rate;
  // The rate. The index of an option.
  int32_t rate;
  // True if `ack` holds a value.
  bool has_ack;
  // Whether the frame asks for an acknowledgement.
  bool ack;
  // True if `sequence` holds a value.
  bool has_sequence;
  // The sequence number.
  int64_t sequence;
} sdrkit_zwave_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ZWAVE_SEND_ARGS_INIT {sizeof(sdrkit_zwave_send_args), {0}, 0, 0, {0}, 0, 0, 0, 0, 0, 0}

// The arguments of `send` of the Zigbee stack.
typedef struct sdrkit_zigbee_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The PAN id.
  sdrkit_string pan_id;
  // The destination address.
  sdrkit_string destination;
  // The source address.
  sdrkit_string source;
  // The MAC payload.
  sdrkit_string payload;
  // True if `frame_type` holds a value.
  bool has_frame_type;
  // The frame type. The index of an option.
  int32_t frame_type;
  // True if `sequence` holds a value.
  bool has_sequence;
  // The sequence number.
  int64_t sequence;
  // True if `ack` holds a value.
  bool has_ack;
  // Whether the frame asks for an acknowledgement.
  bool ack;
  // True if `security_level` holds a value.
  bool has_security_level;
  // The security level.
  int64_t security_level;
  // True if `key` holds a value.
  bool has_key;
  // The key of a secured frame.
  sdrkit_string key;
  // True if `frame_counter` holds a value.
  bool has_frame_counter;
  // The frame counter of a secured frame.
  int64_t frame_counter;
  // True if `key_id_mode` holds a value.
  bool has_key_id_mode;
  // The key identifier mode of a secured frame.
  int64_t key_id_mode;
  // True if `key_index` holds a value.
  bool has_key_index;
  // The key index of a secured frame.
  int64_t key_index;
  // True if `key_source` holds a value.
  bool has_key_source;
  // The key source of a secured frame.
  sdrkit_string key_source;
} sdrkit_zigbee_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ZIGBEE_SEND_ARGS_INIT {sizeof(sdrkit_zigbee_send_args), {0}, {0}, {0}, {0}, 0, 0, 0, 0, 0, 0, 0, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, {0}}

// The arguments of `remove` of the ANT+ stack.
typedef struct sdrkit_ant_sensor_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the sensor.
  sdrkit_string row;
} sdrkit_ant_sensor_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ANT_SENSOR_ARGS_INIT {sizeof(sdrkit_ant_sensor_args), {0}}

// The arguments of `send` of the ANT+ stack.
typedef struct sdrkit_ant_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The device number.
  int64_t device;
  // The device type.
  int64_t device_type;
  // The transmission type.
  int64_t tx_type;
  // The data page.
  sdrkit_string payload;
  // True if `pairing` holds a value.
  bool has_pairing;
  // The pairing bit.
  bool pairing;
} sdrkit_ant_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ANT_SEND_ARGS_INIT {sizeof(sdrkit_ant_send_args), 0, 0, 0, {0}, 0, 0}

// The arguments of `remove` of the nRF24L01 stack.
typedef struct sdrkit_nrf24_nrf_address_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the address.
  sdrkit_string row;
} sdrkit_nrf24_nrf_address_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_NRF24_NRF_ADDRESS_ARGS_INIT {sizeof(sdrkit_nrf24_nrf_address_args), {0}}

// The arguments of `remove` of the DECT stack.
typedef struct sdrkit_dect_base_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the base station.
  sdrkit_string row;
} sdrkit_dect_base_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DECT_BASE_ARGS_INIT {sizeof(sdrkit_dect_base_args), {0}}

// The arguments of `remove` of the NFC stack.
typedef struct sdrkit_nfc_tag_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the tag.
  sdrkit_string row;
} sdrkit_nfc_tag_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_NFC_TAG_ARGS_INIT {sizeof(sdrkit_nfc_tag_args), {0}}

// The arguments of `remove` of the Wi-SUN FAN stack.
typedef struct sdrkit_wisun_node_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the node.
  sdrkit_string row;
} sdrkit_wisun_node_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WISUN_NODE_ARGS_INIT {sizeof(sdrkit_wisun_node_args), {0}}

// The arguments of `send` of the Wi-SUN stack.
typedef struct sdrkit_wisun_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `frame_type` holds a value.
  bool has_frame_type;
  // The frame type of the UTT IE. The index of an option.
  int32_t frame_type;
  // The PAN id.
  sdrkit_string pan_id;
  // The source.
  sdrkit_string source;
  // The destination.
  sdrkit_string destination;
  // The payload IEs.
  sdrkit_string payload;
  // True if `header_ies` holds a value.
  bool has_header_ies;
  // More header IEs.
  sdrkit_string header_ies;
  // True if `sequence` holds a value.
  bool has_sequence;
  // The sequence number.
  int64_t sequence;
  // True if `kbps` holds a value.
  bool has_kbps;
  // The bit rate. Unit: kbit/s.
  int64_t kbps;
  // True if `ufsi` holds a value.
  bool has_ufsi;
  // The UFSI of the UTT-IE.
  int64_t ufsi;
} sdrkit_wisun_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WISUN_SEND_ARGS_INIT {sizeof(sdrkit_wisun_send_args), 0, 0, {0}, {0}, {0}, {0}, 0, {0}, 0, 0, 0, 0, 0, 0}

// Fills `rows` with the changed rows of the table "devices" of the stack "ism" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_ism_devices_rows(SDRKIT_HANDLE handle,
                                             sdrkit_ism_device_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.IsmDeviceRow", "devices", rows, sizeof(sdrkit_ism_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "zwave" of the stack "ism" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ism_zwave_rows(SDRKIT_HANDLE handle,
                                           sdrkit_ism_zwave_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.IsmZwaveRow", "zwave", rows, sizeof(sdrkit_ism_zwave_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "log" of the stack "ism" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_ism_log_rows(SDRKIT_HANDLE handle,
                                         sdrkit_ism_log_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.IsmLogRow", "log", rows, sizeof(sdrkit_ism_log_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "unknown" of the stack "ism" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ism_unknown_rows(SDRKIT_HANDLE handle,
                                             sdrkit_ism_unknown_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.IsmUnknownRow", "unknown", rows, sizeof(sdrkit_ism_unknown_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sigfox" of the stack "ism" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `device_id` names a row.
static inline int32_t sdrkit_ism_sigfox_rows(SDRKIT_HANDLE handle,
                                            sdrkit_ism_sigfox_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.IsmSigfoxRow", "sigfox", rows, sizeof(sdrkit_ism_sigfox_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "pulse_frames" of the stack "ism" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ism_pulse_frames_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_ism_pulse_frame_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.IsmPulseFrameRow", "pulse_frames", rows, sizeof(sdrkit_ism_pulse_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ism.device"
// (ISM device). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ism_device_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ism_device_event *event) {
  return sdrkit_typed_event(handle, n, "ism.device", "ism.IsmDeviceEvent", event, sizeof(sdrkit_ism_device_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sigfox.message"
// (Sigfox message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sigfox_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sigfox_message_event *event) {
  return sdrkit_typed_event(handle, n, "sigfox.message", "ism.SigfoxMessageEvent", event, sizeof(sdrkit_sigfox_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ism.pulse_frame"
// (Pulse frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ism_pulse_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ism_pulse_frame_event *event) {
  return sdrkit_typed_event(handle, n, "ism.pulse_frame", "ism.IsmPulseFrameEvent", event, sizeof(sdrkit_ism_pulse_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ism.tx"
// (ISM transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ism_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ism_tx_event *event) {
  return sdrkit_typed_event(handle, n, "ism.tx", "ism.IsmTxEvent", event, sizeof(sdrkit_ism_tx_event));
}

// Runs the command "clear" (Clear) of the stack "ism" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ism_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ism" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ism_remove(SDRKIT_HANDLE handle, const sdrkit_ism_pan_device_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.PanDeviceArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "ism" on the session of `handle`. Sends one transmission of the device family (its copies of the frame, as the device sends them) on the tuned frequency, as OOK or FSK by the family. Transmissions sent during one wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_ism_send(SDRKIT_HANDLE handle, const sdrkit_ism_send_args *args) {
  return sdrkit_typed_command(handle, "send", "ism.IsmSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "ism" on the session of `handle`. Drops the frames waiting and ends the frame on air with the fall of the carrier.
static inline int32_t sdrkit_ism_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ism" are the sdrkit_ism_gen_*() functions
// after the settings of its receiver.

// The setting "threshold" (Detection threshold) of the stack "ism". Unit: dB. How far above the noise floor of a frequency bin a burst must rise.
static inline int32_t sdrkit_ism_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_ism_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// The setting "submode" (Submode) of the stack "ism". A value of `sdrkit_ism_submode`. The pulse analyser measures the frames of an unknown on-off keyed signal and guesses their line code; it decodes no device.
static inline int32_t sdrkit_ism_get_submode(SDRKIT_HANDLE handle, sdrkit_ism_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ism_submode)index;
  return code;
}
static inline int32_t sdrkit_ism_set_submode(SDRKIT_HANDLE handle, sdrkit_ism_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "frame_gap_ms" (Longest gap inside a frame) of the stack "ism". Unit: ms. Pulse analyser: a longer gap ends a frame. After 4 pulses the limit is 12 times the median width (at least 1.2 ms), if that is shorter.
static inline int32_t sdrkit_ism_get_frame_gap_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "frame_gap_ms", value);
}
static inline int32_t sdrkit_ism_set_frame_gap_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "frame_gap_ms", value);
}

// The setting "min_pulses" (Fewest pulses in a frame) of the stack "ism". Pulse analyser: a frame with fewer pulses is not analysed.
static inline int32_t sdrkit_ism_get_min_pulses(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "min_pulses", value);
}
static inline int32_t sdrkit_ism_set_min_pulses(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "min_pulses", value);
}

// The setting "cluster_ratio" (Cluster split ratio) of the stack "ism". Pulse analyser: sorted widths or gaps start a new cluster where a value is this many times the one before.
static inline int32_t sdrkit_ism_get_cluster_ratio(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "cluster_ratio", value);
}
static inline int32_t sdrkit_ism_set_cluster_ratio(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "cluster_ratio", value);
}

// The setting "tx_level" (Level) of the stack "ism". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_ism_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_ism_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "ism". A value of `sdrkit_ism_gen_scene`.
static inline int32_t sdrkit_ism_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_ism_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ism_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_ism_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_ism_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_carrier" (Strong continuous carrier) of the test signal generator of the stack "ism". A carrier 20 dB above the devices, 300 kHz above the centre.
static inline int32_t sdrkit_ism_gen_get_carrier(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_carrier", value);
}
static inline int32_t sdrkit_ism_gen_set_carrier(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_carrier", value);
}

// The setting "gen_pattern" (Pulse analyser test pattern) of the test signal generator of the stack "ism". A value of `sdrkit_ism_gen_pattern`.
static inline int32_t sdrkit_ism_gen_get_pattern(SDRKIT_HANDLE handle, sdrkit_ism_gen_pattern *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_pattern", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ism_gen_pattern)index;
  return code;
}
static inline int32_t sdrkit_ism_gen_set_pattern(SDRKIT_HANDLE handle, sdrkit_ism_gen_pattern value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_pattern", (int32_t)value);
}

// The setting "gen_bits" (Pulse analyser test bits) of the test signal generator of the stack "ism". 0 and 1; other characters are left out. Empty: 101100101001110010100110 (PWM), 1100101001101011 (PPM), 10110010111000101101 (Manchester).
static inline int32_t sdrkit_ism_gen_get_bits(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_bits", value);
}
static inline int32_t sdrkit_ism_gen_set_bits(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_bits", value);
}

// The setting "gen_t_us" (Pulse analyser test unit) of the test signal generator of the stack "ism". Unit: µs.
static inline int32_t sdrkit_ism_gen_get_t_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_t_us", value);
}
static inline int32_t sdrkit_ism_gen_set_t_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_t_us", value);
}

// The setting "gen_pulse_us" (Pulse analyser test PPM pulse) of the test signal generator of the stack "ism". Unit: µs.
static inline int32_t sdrkit_ism_gen_get_pulse_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pulse_us", value);
}
static inline int32_t sdrkit_ism_gen_set_pulse_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pulse_us", value);
}

// The setting "gen_gap0_us" (Pulse analyser test PPM gap of a 0) of the test signal generator of the stack "ism". Unit: µs.
static inline int32_t sdrkit_ism_gen_get_gap0_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap0_us", value);
}
static inline int32_t sdrkit_ism_gen_set_gap0_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap0_us", value);
}

// The setting "gen_gap1_us" (Pulse analyser test PPM gap of a 1) of the test signal generator of the stack "ism". Unit: µs.
static inline int32_t sdrkit_ism_gen_get_gap1_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap1_us", value);
}
static inline int32_t sdrkit_ism_gen_set_gap1_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap1_us", value);
}

// The setting "gen_jitter" (Pulse analyser test jitter) of the test signal generator of the stack "ism". Each mark and space is stretched by a uniform share up to this, either way.
static inline int32_t sdrkit_ism_gen_get_jitter(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_jitter", value);
}
static inline int32_t sdrkit_ism_gen_set_jitter(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_jitter", value);
}

// The setting "gen_frames" (Pulse analyser test frames) of the test signal generator of the stack "ism".
static inline int32_t sdrkit_ism_gen_get_frames(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_frames", value);
}
static inline int32_t sdrkit_ism_gen_set_frames(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_frames", value);
}

// The setting "gen_frame_gap_s" (Pulse analyser test gap after a frame) of the test signal generator of the stack "ism". Unit: s.
static inline int32_t sdrkit_ism_gen_get_frame_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_frame_gap_s", value);
}
static inline int32_t sdrkit_ism_gen_set_frame_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_frame_gap_s", value);
}

// Fills `rows` with the changed rows of the table "meters" of the stack "wmbus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wmbus_meters_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wmbus_meter_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WmbusMeterRow", "meters", rows, sizeof(sdrkit_wmbus_meter_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "meter_records" of the stack "wmbus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wmbus_meter_records_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_wmbus_record_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WmbusRecordRow", "meter_records", rows, sizeof(sdrkit_wmbus_record_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "wmbus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wmbus_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wmbus_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WmbusFrameRow", "frames", rows, sizeof(sdrkit_wmbus_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "wmbus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wmbus_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wmbus_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WmbusChannelRow", "channels", rows, sizeof(sdrkit_wmbus_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wmbus.frame"
// (Wireless M-Bus frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wmbus_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wmbus_frame_event *event) {
  return sdrkit_typed_event(handle, n, "wmbus.frame", "ism.WmbusFrameEvent", event, sizeof(sdrkit_wmbus_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wmbus.tx"
// (Wireless M-Bus transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wmbus_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wmbus_tx_event *event) {
  return sdrkit_typed_event(handle, n, "wmbus.tx", "ism.IsmTxEvent", event, sizeof(sdrkit_wmbus_tx_event));
}

// Runs the command "clear" (Clear) of the stack "wmbus" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_wmbus_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "wmbus" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_wmbus_remove(SDRKIT_HANDLE handle, const sdrkit_wmbus_meter_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.WmbusMeterArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "wmbus" on the session of `handle`. Sends a link-layer frame with the C field 0x44 (SND-NR) on the channel of the mode nearest the tuned frequency, with the CRCs of its format. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_wmbus_send(SDRKIT_HANDLE handle, const sdrkit_wmbus_send_args *args) {
  return sdrkit_typed_command(handle, "send", "ism.WmbusSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "wmbus" on the session of `handle`. Drops the frames waiting and ends the frame on air with the fall of the carrier.
static inline int32_t sdrkit_wmbus_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wmbus" are the sdrkit_wmbus_gen_*() functions
// after the settings of its receiver.

// The setting "tx_level" (Level) of the stack "wmbus". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_wmbus_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_wmbus_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_plan" (Test signal plan) of the test signal generator of the stack "wmbus". A value of `sdrkit_wmbus_gen_plan`.
static inline int32_t sdrkit_wmbus_gen_get_plan(SDRKIT_HANDLE handle, sdrkit_wmbus_gen_plan *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_plan", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wmbus_gen_plan)index;
  return code;
}
static inline int32_t sdrkit_wmbus_gen_set_plan(SDRKIT_HANDLE handle, sdrkit_wmbus_gen_plan value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_plan", (int32_t)value);
}

// The setting "gen_phy" (Test signal mode) of the test signal generator of the stack "wmbus". A value of `sdrkit_wmbus_gen_phy`.
static inline int32_t sdrkit_wmbus_gen_get_phy(SDRKIT_HANDLE handle, sdrkit_wmbus_gen_phy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_phy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wmbus_gen_phy)index;
  return code;
}
static inline int32_t sdrkit_wmbus_gen_set_phy(SDRKIT_HANDLE handle, sdrkit_wmbus_gen_phy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_phy", (int32_t)value);
}

// The setting "gen_encrypted" (Encrypted meter) of the test signal generator of the stack "wmbus".
static inline int32_t sdrkit_wmbus_gen_get_encrypted(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_encrypted", value);
}
static inline int32_t sdrkit_wmbus_gen_set_encrypted(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_encrypted", value);
}

// The setting "gen_silence" (No meters (noise only)) of the test signal generator of the stack "wmbus".
static inline int32_t sdrkit_wmbus_gen_get_silence(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_silence", value);
}
static inline int32_t sdrkit_wmbus_gen_set_silence(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_silence", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "zwave" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_zwave_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_zwave_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZwaveFrameRow", "frames", rows, sizeof(sdrkit_zwave_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "zwave" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_zwave_nodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_zwave_node_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZwaveNodeRow", "nodes", rows, sizeof(sdrkit_zwave_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "zwave" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_dbfs`, highest first.
static inline int32_t sdrkit_zwave_networks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_zwave_network_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZwaveNetworkRow", "networks", rows, sizeof(sdrkit_zwave_network_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "zwave" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_zwave_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_zwave_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZwaveChannelRow", "channels", rows, sizeof(sdrkit_zwave_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "classes" of the stack "zwave" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_zwave_classes_rows(SDRKIT_HANDLE handle,
                                               sdrkit_zwave_class_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZwaveClassRow", "classes", rows, sizeof(sdrkit_zwave_class_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "zwave.frame"
// (Z-Wave frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_zwave_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_zwave_frame_event *event) {
  return sdrkit_typed_event(handle, n, "zwave.frame", "ism.ZwaveFrameEvent", event, sizeof(sdrkit_zwave_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "zwave.state"
// (Z-Wave activity). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_zwave_state_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_zwave_state_event *event) {
  return sdrkit_typed_event(handle, n, "zwave.state", "ism.ZwaveStateEvent", event, sizeof(sdrkit_zwave_state_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "zwave.tx"
// (Z-Wave transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_zwave_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_zwave_tx_event *event) {
  return sdrkit_typed_event(handle, n, "zwave.tx", "ism.IsmTxEvent", event, sizeof(sdrkit_zwave_tx_event));
}

// Runs the command "clear" (Clear) of the stack "zwave" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_zwave_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "zwave" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_zwave_remove(SDRKIT_HANDLE handle, const sdrkit_zwave_node_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.ZwaveNodeArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "zwave" on the session of `handle`. Sends a singlecast MPDU on the first channel of the rate in the setting channels (the tuned frequency when the setting is empty). Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_zwave_send(SDRKIT_HANDLE handle, const sdrkit_zwave_send_args *args) {
  return sdrkit_typed_command(handle, "send", "ism.ZwaveSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "zwave" on the session of `handle`. Drops the frames waiting and ends the frame on air with the fall of the carrier.
static inline int32_t sdrkit_zwave_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "zwave" are the sdrkit_zwave_gen_*() functions
// after the settings of its receiver.

// The setting "channels" (Channels) of the stack "zwave". Entries separated by ";", each the frequency in Hz and the rates on it (1, 2, 3) separated by "/", for example "868420000/1;868400000/2;869850000/3". At most 64 entries. Empty: one channel at the tuned frequency with the three rates.
static inline int32_t sdrkit_zwave_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_zwave_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "sof_f0_r3" (Accept 0xF0 as the R3 start of frame) of the stack "zwave". For recordings made with the ISM-mode start of frame at every rate.
static inline int32_t sdrkit_zwave_get_sof_f0_r3(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sof_f0_r3", value);
}
static inline int32_t sdrkit_zwave_set_sof_f0_r3(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sof_f0_r3", value);
}

// The setting "tx_level" (Level) of the stack "zwave". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_zwave_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_zwave_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_centre_hz" (Test signal radio centre) of the test signal generator of the stack "zwave". A frequency in Hz. The frequency that the radio is tuned to: the test signal puts the channels of the setting channels around it.
static inline int32_t sdrkit_zwave_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_zwave_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "zigbee" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_dbfs`, highest first.
static inline int32_t sdrkit_zigbee_networks_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_zigbee_network_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZigbeeNetworkRow", "networks", rows, sizeof(sdrkit_zigbee_network_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "zigbee" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_zigbee_devices_rows(SDRKIT_HANDLE handle,
                                                sdrkit_ism_pan_device_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.PanDeviceRow", "devices", rows, sizeof(sdrkit_ism_pan_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "zigbee" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_zigbee_frames_rows(SDRKIT_HANDLE handle,
                                               sdrkit_zigbee_frame_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ZigbeeFrameRow", "frames", rows, sizeof(sdrkit_zigbee_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "zigbee.pan"
// (New Zigbee PAN). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_zigbee_pan_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_zigbee_pan_event *event) {
  return sdrkit_typed_event(handle, n, "zigbee.pan", "ism.ZigbeePanEvent", event, sizeof(sdrkit_zigbee_pan_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "zigbee.tx"
// (Zigbee transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_zigbee_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_zigbee_tx_event *event) {
  return sdrkit_typed_event(handle, n, "zigbee.tx", "ism.IsmTxEvent", event, sizeof(sdrkit_zigbee_tx_event));
}

// Runs the command "clear" (Clear) of the stack "zigbee" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_zigbee_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "zigbee" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_zigbee_remove(SDRKIT_HANDLE handle, const sdrkit_ism_pan_device_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.PanDeviceArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "zigbee" on the session of `handle`. Sends an IEEE 802.15.4 MAC frame on the tuned frequency: frame version 2003 without security, frame version 2006 with the auxiliary security header and CCM* at the security level. With both addresses the source PAN id is left out. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_zigbee_send(SDRKIT_HANDLE handle, const sdrkit_zigbee_send_args *args) {
  return sdrkit_typed_command(handle, "send", "ism.ZigbeeSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "zigbee" on the session of `handle`. Drops the frames waiting and ends the frame on air with the fall of the carrier.
static inline int32_t sdrkit_zigbee_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "zigbee" are the sdrkit_zigbee_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Channel) of the stack "zigbee". Asks the host to tune 2405 + 5 (k - 11) MHz.
static inline int32_t sdrkit_zigbee_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channel", value);
}
static inline int32_t sdrkit_zigbee_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channel", value);
}

// The setting "scan" (Scan channels 11 to 26) of the stack "zigbee". Asks the host to visit each channel in turn.
static inline int32_t sdrkit_zigbee_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_zigbee_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "dwell" (Scan dwell) of the stack "zigbee". A length of time in seconds.
static inline int32_t sdrkit_zigbee_get_dwell(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell", value);
}
static inline int32_t sdrkit_zigbee_set_dwell(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell", value);
}

// The setting "keys" (Keys (kept in memory only)) of the stack "zigbee". AES-128 keys of 32 hex digits, separated by ';', tried in turn on each secured MAC frame (IEEE 802.15.4 CCM*). Empty: secured payloads are shown as encrypted.
static inline int32_t sdrkit_zigbee_get_keys(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "keys", value);
}
static inline int32_t sdrkit_zigbee_set_keys(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "keys", value);
}

// The setting "tx_level" (Level) of the stack "zigbee". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_zigbee_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_zigbee_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_bad_fcs" (Every frame with a bad FCS) of the test signal generator of the stack "zigbee".
static inline int32_t sdrkit_zigbee_gen_get_bad_fcs(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_bad_fcs", value);
}
static inline int32_t sdrkit_zigbee_gen_set_bad_fcs(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_bad_fcs", value);
}

// The setting "gen_interferer_db" (Adjacent transmitter level (0 off)) of the test signal generator of the stack "zigbee". Unit: dB.
static inline int32_t sdrkit_zigbee_gen_get_interferer_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_interferer_db", value);
}
static inline int32_t sdrkit_zigbee_gen_set_interferer_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_interferer_db", value);
}

// The setting "gen_interferer_hz" (Adjacent transmitter offset) of the test signal generator of the stack "zigbee". A frequency in Hz.
static inline int32_t sdrkit_zigbee_gen_get_interferer_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_interferer_hz", value);
}
static inline int32_t sdrkit_zigbee_gen_set_interferer_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_interferer_hz", value);
}

// Fills `rows` with the changed rows of the table "sigfox" of the stack "sigfox" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `device_id` names a row.
static inline int32_t sdrkit_sigfox_rows(SDRKIT_HANDLE handle,
                                        sdrkit_sigfox_message_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.SigfoxMessageRow", "sigfox", rows, sizeof(sdrkit_sigfox_message_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "sigfox" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_sigfox_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sigfox" are the sdrkit_sigfox_gen_*() functions
// after the settings of its receiver.

// The setting "gen_bit_rate" (Test signal bit rate) of the test signal generator of the stack "sigfox". A value of `sdrkit_sigfox_gen_bit_rate`.
static inline int32_t sdrkit_sigfox_gen_get_bit_rate(SDRKIT_HANDLE handle, sdrkit_sigfox_gen_bit_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bit_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sigfox_gen_bit_rate)index;
  return code;
}
static inline int32_t sdrkit_sigfox_gen_set_bit_rate(SDRKIT_HANDLE handle, sdrkit_sigfox_gen_bit_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bit_rate", (int32_t)value);
}

// The setting "gen_reps" (Repetitions) of the test signal generator of the stack "sigfox". A value of `sdrkit_sigfox_gen_reps`.
static inline int32_t sdrkit_sigfox_gen_get_reps(SDRKIT_HANDLE handle, sdrkit_sigfox_gen_reps *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_reps", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sigfox_gen_reps)index;
  return code;
}
static inline int32_t sdrkit_sigfox_gen_set_reps(SDRKIT_HANDLE handle, sdrkit_sigfox_gen_reps value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_reps", (int32_t)value);
}

// The setting "gen_gap" (Gap between messages) of the test signal generator of the stack "sigfox". A length of time in seconds.
static inline int32_t sdrkit_sigfox_gen_get_gap(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap", value);
}
static inline int32_t sdrkit_sigfox_gen_set_gap(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap", value);
}

// Fills `rows` with the changed rows of the table "sensors" of the stack "ant" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ant_sensors_rows(SDRKIT_HANDLE handle,
                                             sdrkit_ant_sensor_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.AntSensorRow", "sensors", rows, sizeof(sdrkit_ant_sensor_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "ant" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ant_frames_rows(SDRKIT_HANDLE handle,
                                            sdrkit_ant_frame_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.AntFrameRow", "frames", rows, sizeof(sdrkit_ant_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ant.sensor"
// (New ANT sensor). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ant_sensor_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ant_sensor_event *event) {
  return sdrkit_typed_event(handle, n, "ant.sensor", "ism.AntSensorEvent", event, sizeof(sdrkit_ant_sensor_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ant.tx"
// (ANT+ transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ant_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ant_tx_event *event) {
  return sdrkit_typed_event(handle, n, "ant.tx", "ism.IsmTxEvent", event, sizeof(sdrkit_ant_tx_event));
}

// Runs the command "clear" (Clear) of the stack "ant" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ant_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ant" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ant_remove(SDRKIT_HANDLE handle, const sdrkit_ant_sensor_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.AntSensorArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "ant" on the session of `handle`. Sends one ANT+ broadcast frame (the ANT+ network marker, the channel id, the data page and the CRC) on the tuned frequency. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_ant_send(SDRKIT_HANDLE handle, const sdrkit_ant_send_args *args) {
  return sdrkit_typed_command(handle, "send", "ism.AntSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "ant" on the session of `handle`. Drops the frames waiting and ends the frame on air with the fall of the carrier.
static inline int32_t sdrkit_ant_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ant" are the sdrkit_ant_gen_*() functions
// after the settings of its receiver.

// The setting "other_networks" (Other ANT networks) of the stack "ant". Also decodes frames of networks other than ANT+.
static inline int32_t sdrkit_ant_get_other_networks(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "other_networks", value);
}
static inline int32_t sdrkit_ant_set_other_networks(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "other_networks", value);
}

// The setting "wheel_mm" (Wheel circumference) of the stack "ant". Unit: mm.
static inline int32_t sdrkit_ant_get_wheel_mm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wheel_mm", value);
}
static inline int32_t sdrkit_ant_set_wheel_mm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wheel_mm", value);
}

// The setting "tx_level" (Level) of the stack "ant". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_ant_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_ant_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_sensors" (Test signal sensors) of the test signal generator of the stack "ant". A value of `sdrkit_ant_gen_sensors`.
static inline int32_t sdrkit_ant_gen_get_sensors(SDRKIT_HANDLE handle, sdrkit_ant_gen_sensors *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_sensors", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ant_gen_sensors)index;
  return code;
}
static inline int32_t sdrkit_ant_gen_set_sensors(SDRKIT_HANDLE handle, sdrkit_ant_gen_sensors value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_sensors", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "nrf24" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `address` names a row.
static inline int32_t sdrkit_nrf24_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_nrf24_nrf_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NrfFrameRow", "frames", rows, sizeof(sdrkit_nrf24_nrf_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "addresses" of the stack "nrf24" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `address` names a row.
static inline int32_t sdrkit_nrf24_addresses_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_nrf24_nrf_address_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NrfAddressRow", "addresses", rows, sizeof(sdrkit_nrf24_nrf_address_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bad_bursts" of the stack "nrf24" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nrf24_bad_bursts_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_nrf24_nrf_bad_burst_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NrfBadBurstRow", "bad_bursts", rows, sizeof(sdrkit_nrf24_nrf_bad_burst_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "nrf24.address"
// (New nRF24 address). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_nrf24_address_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_nrf24_address_event *event) {
  return sdrkit_typed_event(handle, n, "nrf24.address", "ism.NrfAddressEvent", event, sizeof(sdrkit_nrf24_address_event));
}

// Runs the command "clear" (Clear) of the stack "nrf24" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_nrf24_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "nrf24" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_nrf24_remove(SDRKIT_HANDLE handle, const sdrkit_nrf24_nrf_address_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.NrfAddressArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "nrf24" are the sdrkit_nrf24_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Channel) of the stack "nrf24". Asks the host to tune 2400 + N MHz.
static inline int32_t sdrkit_nrf24_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channel", value);
}
static inline int32_t sdrkit_nrf24_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channel", value);
}

// The setting "gen_rate" (Test signal rate) of the test signal generator of the stack "nrf24". A value of `sdrkit_nrf24_gen_rate`.
static inline int32_t sdrkit_nrf24_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_nrf24_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_nrf24_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_nrf24_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_nrf24_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// The setting "gen_crc" (Test frames) of the test signal generator of the stack "nrf24". A value of `sdrkit_nrf24_gen_crc`.
static inline int32_t sdrkit_nrf24_gen_get_crc(SDRKIT_HANDLE handle, sdrkit_nrf24_gen_crc *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_crc", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_nrf24_gen_crc)index;
  return code;
}
static inline int32_t sdrkit_nrf24_gen_set_crc(SDRKIT_HANDLE handle, sdrkit_nrf24_gen_crc value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_crc", (int32_t)value);
}

// The setting "gen_random" (Random-bit bursts (no frame)) of the test signal generator of the stack "nrf24".
static inline int32_t sdrkit_nrf24_gen_get_random(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_random", value);
}
static inline int32_t sdrkit_nrf24_gen_set_random(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_random", value);
}

// Fills `rows` with the changed rows of the table "chat" of the stack "subghzchat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// Text: `text`. Time: `time_s`.
static inline int32_t sdrkit_subghzchat_chat_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_subghzchat_line_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.SubghzchatLineRow", "chat", rows, sizeof(sdrkit_subghzchat_line_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "subghzchat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_subghzchat_packets_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_subghzchat_packet_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.SubghzchatPacketRow", "packets", rows, sizeof(sdrkit_subghzchat_packet_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "subghzchat.line"
// (Chat line). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_subghzchat_line_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_subghzchat_line_event *event) {
  return sdrkit_typed_event(handle, n, "subghzchat.line", "ism.SubghzchatLineEvent", event, sizeof(sdrkit_subghzchat_line_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "subghzchat.state"
// (Chat packets coming in). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_subghzchat_state_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_subghzchat_state_event *event) {
  return sdrkit_typed_event(handle, n, "subghzchat.state", "ism.SubghzchatStateEvent", event, sizeof(sdrkit_subghzchat_state_event));
}

// Runs the command "clear" (Clear) of the stack "subghzchat" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_subghzchat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "name" (Your chat name) of the stack "subghzchat".
static inline int32_t sdrkit_subghzchat_get_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "name", value);
}
static inline int32_t sdrkit_subghzchat_set_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "name", value);
}

// The setting "encryption" (Encryption) of the stack "subghzchat". A value of `sdrkit_subghzchat_encryption`.
static inline int32_t sdrkit_subghzchat_get_encryption(SDRKIT_HANDLE handle, sdrkit_subghzchat_encryption *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "encryption", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_subghzchat_encryption)index;
  return code;
}
static inline int32_t sdrkit_subghzchat_set_encryption(SDRKIT_HANDLE handle, sdrkit_subghzchat_encryption value) {
  return sdrkit_typed_setting_set_enum(handle, "encryption", (int32_t)value);
}

// The setting "password" (Password (kept in memory only)) of the stack "subghzchat".
static inline int32_t sdrkit_subghzchat_get_password(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "password", value);
}
static inline int32_t sdrkit_subghzchat_set_password(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "password", value);
}

// The setting "key" (Hex key (kept in memory only)) of the stack "subghzchat". 64 hex digits. Spaces, ':' and '-' are ignored.
static inline int32_t sdrkit_subghzchat_get_key(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "key", value);
}
static inline int32_t sdrkit_subghzchat_set_key(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "key", value);
}

// The setting "frequency" (Chat frequency) of the stack "subghzchat". A value of `sdrkit_subghzchat_frequency`.
static inline int32_t sdrkit_subghzchat_get_frequency(SDRKIT_HANDLE handle, sdrkit_subghzchat_frequency *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frequency", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_subghzchat_frequency)index;
  return code;
}
static inline int32_t sdrkit_subghzchat_set_frequency(SDRKIT_HANDLE handle, sdrkit_subghzchat_frequency value) {
  return sdrkit_typed_setting_set_enum(handle, "frequency", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "bases" of the stack "dect" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_dbfs`, highest first.
static inline int32_t sdrkit_dect_bases_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dect_base_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.DectBaseRow", "bases", rows, sizeof(sdrkit_dect_base_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "dect" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `from` names a row.
static inline int32_t sdrkit_dect_bursts_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dect_burst_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.DectBurstRow", "bursts", rows, sizeof(sdrkit_dect_burst_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dect.base"
// (New DECT base station). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dect_base_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dect_base_event *event) {
  return sdrkit_typed_event(handle, n, "dect.base", "ism.DectBaseEvent", event, sizeof(sdrkit_dect_base_event));
}

// Runs the command "clear" (Clear) of the stack "dect" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_dect_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dect" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dect_remove(SDRKIT_HANDLE handle, const sdrkit_dect_base_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.DectBaseArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dect" are the sdrkit_dect_gen_*() functions
// after the settings of its receiver.

// The setting "band" (Band) of the stack "dect". A value of `sdrkit_dect_band`.
static inline int32_t sdrkit_dect_get_band(SDRKIT_HANDLE handle, sdrkit_dect_band *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "band", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dect_band)index;
  return code;
}
static inline int32_t sdrkit_dect_set_band(SDRKIT_HANDLE handle, sdrkit_dect_band value) {
  return sdrkit_typed_setting_set_enum(handle, "band", (int32_t)value);
}

// The setting "scan" (Scan the band's carriers) of the stack "dect". Asks the host to visit each carrier of the band in turn.
static inline int32_t sdrkit_dect_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_dect_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "dwell" (Scan dwell) of the stack "dect". A length of time in seconds.
static inline int32_t sdrkit_dect_get_dwell(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell", value);
}
static inline int32_t sdrkit_dect_set_dwell(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell", value);
}

// The setting "gen_option" (Test signal bases) of the test signal generator of the stack "dect". A value of `sdrkit_dect_gen_option`.
static inline int32_t sdrkit_dect_gen_get_option(SDRKIT_HANDLE handle, sdrkit_dect_gen_option *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_option", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dect_gen_option)index;
  return code;
}
static inline int32_t sdrkit_dect_gen_set_option(SDRKIT_HANDLE handle, sdrkit_dect_gen_option value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_option", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "tags" of the stack "nfc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `uid` names a row.
static inline int32_t sdrkit_nfc_tags_rows(SDRKIT_HANDLE handle,
                                          sdrkit_nfc_tag_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NfcTagRow", "tags", rows, sizeof(sdrkit_nfc_tag_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "nfc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_nfc_frames_rows(SDRKIT_HANDLE handle,
                                            sdrkit_nfc_frame_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NfcFrameRow", "frames", rows, sizeof(sdrkit_nfc_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "nfc.tag"
// (NFC tag). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_nfc_tag_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_nfc_tag_event *event) {
  return sdrkit_typed_event(handle, n, "nfc.tag", "ism.NfcTagEvent", event, sizeof(sdrkit_nfc_tag_event));
}

// Runs the command "clear" (Clear) of the stack "nfc" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_nfc_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "nfc" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_nfc_remove(SDRKIT_HANDLE handle, const sdrkit_nfc_tag_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.NfcTagArgs", args, NULL, NULL, 0);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "wisun" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Default order: the column `level_dbfs`, highest first.
static inline int32_t sdrkit_wisun_networks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wisun_network_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WisunNetworkRow", "networks", rows, sizeof(sdrkit_wisun_network_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "wisun" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wisun_nodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wisun_node_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WisunNodeRow", "nodes", rows, sizeof(sdrkit_wisun_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "links" of the stack "wisun" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `from` names a row.
static inline int32_t sdrkit_wisun_links_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wisun_link_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WisunLinkRow", "links", rows, sizeof(sdrkit_wisun_link_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "wisun" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wisun_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wisun_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.WisunFrameRow", "frames", rows, sizeof(sdrkit_wisun_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wisun.pan"
// (New Wi-SUN PAN). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wisun_pan_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wisun_pan_event *event) {
  return sdrkit_typed_event(handle, n, "wisun.pan", "ism.WisunPanEvent", event, sizeof(sdrkit_wisun_pan_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wisun.tx"
// (Wi-SUN transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wisun_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wisun_tx_event *event) {
  return sdrkit_typed_event(handle, n, "wisun.tx", "ism.IsmTxEvent", event, sizeof(sdrkit_wisun_tx_event));
}

// Runs the command "clear" (Clear) of the stack "wisun" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_wisun_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "wisun" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_wisun_remove(SDRKIT_HANDLE handle, const sdrkit_wisun_node_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.WisunNodeArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "wisun" on the session of `handle`. Sends an IEEE 802.15.4 frame of version 2015 with the UTT-IE of the frame type, the header IEs, a header termination IE and the payload IEs, whitened with a 4-octet FCS, on the channel of the plan nearest the tuned frequency. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_wisun_send(SDRKIT_HANDLE handle, const sdrkit_wisun_send_args *args) {
  return sdrkit_typed_command(handle, "send", "ism.WisunSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "wisun" on the session of `handle`. Drops the frames waiting and ends the frame on air with the fall of the carrier.
static inline int32_t sdrkit_wisun_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wisun" are the sdrkit_wisun_gen_*() functions
// after the settings of its receiver.

// The setting "ranges" (Usable ranges) of the stack "wisun". Entries separated by ";", each the lower and the upper edge in Hz separated by "-", for example "902000000-907500000;915000000-928000000". At most 16 entries. Empty: every channel of the classes is usable.
static inline int32_t sdrkit_wisun_get_ranges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "ranges", value);
}
static inline int32_t sdrkit_wisun_set_ranges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "ranges", value);
}

// The setting "classes" (Operating classes) of the stack "wisun". Entries separated by ";", one for each operating class in the order of the class numbers: the first channel in Hz, the spacing in Hz, the number of channels and the FSK modes, separated by "/". The modes are separated by spaces, each the bit rate in kbit/s and the modulation index separated by ":", for example "915200000/200000/64/50:1;915400000/400000/32/100:0.5 150:0.5". At most 16 entries. Empty: class 1 every 200 kHz at 50 kbit/s and class 2 every 400 kHz at 100 and 150 kbit/s, on grids through the tuned frequency, without channel numbers.
static inline int32_t sdrkit_wisun_get_classes(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "classes", value);
}
static inline int32_t sdrkit_wisun_set_classes(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "classes", value);
}

// The setting "tx_level" (Level) of the stack "wisun". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_wisun_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_wisun_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_span" (Span of channels used) of the test signal generator of the stack "wisun". A frequency in Hz.
static inline int32_t sdrkit_wisun_gen_get_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_span", value);
}
static inline int32_t sdrkit_wisun_gen_set_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_span", value);
}

// The setting "gen_centre_hz" (Test signal radio centre) of the test signal generator of the stack "wisun". A frequency in Hz. The frequency that the radio is tuned to: the test signal puts the channels of the plan around it.
static inline int32_t sdrkit_wisun_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_wisun_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "thread" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `network_name` names a row.
// Default order: the column `level_dbfs`, highest first.
static inline int32_t sdrkit_thread_networks_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_thread_network_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ThreadNetworkRow", "networks", rows, sizeof(sdrkit_thread_network_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "thread" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_thread_devices_rows(SDRKIT_HANDLE handle,
                                                sdrkit_ism_pan_device_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.PanDeviceRow", "devices", rows, sizeof(sdrkit_ism_pan_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "thread" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `source` names a row.
static inline int32_t sdrkit_thread_frames_rows(SDRKIT_HANDLE handle,
                                               sdrkit_thread_frame_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.ThreadFrameRow", "frames", rows, sizeof(sdrkit_thread_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "thread.network"
// (Thread network). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_thread_network_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_thread_network_event *event) {
  return sdrkit_typed_event(handle, n, "thread.network", "ism.ThreadNetworkEvent", event, sizeof(sdrkit_thread_network_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "thread.device"
// (Thread device). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_thread_device_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_thread_device_event *event) {
  return sdrkit_typed_event(handle, n, "thread.device", "ism.ThreadDeviceEvent", event, sizeof(sdrkit_thread_device_event));
}

// Runs the command "clear" (Clear) of the stack "thread" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_thread_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "thread" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_thread_remove(SDRKIT_HANDLE handle, const sdrkit_ism_pan_device_args *args) {
  return sdrkit_typed_command(handle, "remove", "ism.PanDeviceArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "thread" are the sdrkit_thread_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Channel) of the stack "thread". Asks the host to tune 2405 + 5 (k - 11) MHz.
static inline int32_t sdrkit_thread_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channel", value);
}
static inline int32_t sdrkit_thread_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channel", value);
}

// The setting "scan" (Scan channels 11 to 26) of the stack "thread". Asks a live radio to visit each channel in turn. A network is found on the channel that its commissioner selected.
static inline int32_t sdrkit_thread_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_thread_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "dwell" (Scan dwell) of the stack "thread". A length of time in seconds.
static inline int32_t sdrkit_thread_get_dwell(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell", value);
}
static inline int32_t sdrkit_thread_set_dwell(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell", value);
}

// The setting "gen_network_name" (Test signal network name) of the test signal generator of the stack "thread".
static inline int32_t sdrkit_thread_gen_get_network_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_network_name", value);
}
static inline int32_t sdrkit_thread_gen_set_network_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_network_name", value);
}

// The setting "gen_bad_fcs" (Every frame with a bad FCS) of the test signal generator of the stack "thread".
static inline int32_t sdrkit_thread_gen_get_bad_fcs(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_bad_fcs", value);
}
static inline int32_t sdrkit_thread_gen_set_bad_fcs(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_bad_fcs", value);
}

// The setting "gen_interferer_db" (Adjacent transmitter level (0 off)) of the test signal generator of the stack "thread". Unit: dB.
static inline int32_t sdrkit_thread_gen_get_interferer_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_interferer_db", value);
}
static inline int32_t sdrkit_thread_gen_set_interferer_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_interferer_db", value);
}

// The setting "gen_interferer_hz" (Adjacent transmitter offset) of the test signal generator of the stack "thread". A frequency in Hz.
static inline int32_t sdrkit_thread_gen_get_interferer_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_interferer_hz", value);
}
static inline int32_t sdrkit_thread_gen_set_interferer_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_interferer_hz", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "mioty" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_mioty_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mioty_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.MiotyFrameRow", "frames", rows, sizeof(sdrkit_mioty_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "mioty" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_mioty_devices_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mioty_device_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.MiotyDeviceRow", "devices", rows, sizeof(sdrkit_mioty_device_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mioty.frame"
// (mioty frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mioty_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mioty_frame_event *event) {
  return sdrkit_typed_event(handle, n, "mioty.frame", "ism.MiotyFrameEvent", event, sizeof(sdrkit_mioty_frame_event));
}

// Runs the command "clear" (Clear) of the stack "mioty" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_mioty_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "min_bursts" (Bursts needed to try a pattern) of the stack "mioty". Unit: count. A TSMA pattern is tried when this many of its 24 bursts were detected where it puts them.
static inline int32_t sdrkit_mioty_get_min_bursts(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "min_bursts", value);
}
static inline int32_t sdrkit_mioty_set_min_bursts(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "min_bursts", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "dect_nrplus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_dect_nrplus_packets_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dect_nrplus_nr_plus_packet_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NrPlusPacketRow", "packets", rows, sizeof(sdrkit_dect_nrplus_nr_plus_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "dect_nrplus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_dect_nrplus_devices_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dect_nrplus_nr_plus_device_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NrPlusDeviceRow", "devices", rows, sizeof(sdrkit_dect_nrplus_nr_plus_device_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dect_nrplus.packet"
// (DECT NR+ packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dect_nrplus_packet_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dect_nrplus_packet_event *event) {
  return sdrkit_typed_event(handle, n, "dect_nrplus.packet", "ism.NrPlusPacketEvent", event, sizeof(sdrkit_dect_nrplus_packet_event));
}

// Runs the command "clear" (Clear) of the stack "dect_nrplus" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dect_nrplus_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "npr70" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_npr70_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_npr70_npr_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NprFrameRow", "frames", rows, sizeof(sdrkit_npr70_npr_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "networks" of the stack "npr70" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_npr70_networks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_npr70_npr_network_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.NprNetworkRow", "networks", rows, sizeof(sdrkit_npr70_npr_network_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "npr70.frame"
// (New Packet Radio frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_npr70_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_npr70_frame_event *event) {
  return sdrkit_typed_event(handle, n, "npr70.frame", "ism.NprFrameEvent", event, sizeof(sdrkit_npr70_frame_event));
}

// Runs the command "clear" (Clear) of the stack "npr70" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_npr70_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "rates" (Symbol rates searched) of the stack "npr70". Unit: ksym/s. "all" searches every rate that fits the sample rate; else a list such as "100,500" (50, 100, 180, 300, 500).
static inline int32_t sdrkit_npr70_get_rates(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "rates", value);
}
static inline int32_t sdrkit_npr70_set_rates(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "rates", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "dsrc_toll" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `identifier` names a row.
static inline int32_t sdrkit_dsrc_toll_frames_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dsrc_toll_dsrc_frame_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "ism.DsrcFrameRow", "frames", rows, sizeof(sdrkit_dsrc_toll_dsrc_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dsrc_toll.frame"
// (DSRC frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dsrc_toll_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dsrc_toll_frame_event *event) {
  return sdrkit_typed_event(handle, n, "dsrc_toll.frame", "ism.DsrcFrameEvent", event, sizeof(sdrkit_dsrc_toll_frame_event));
}

// Runs the command "clear" (Clear) of the stack "dsrc_toll" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dsrc_toll_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dsrc_toll" are the sdrkit_dsrc_toll_gen_*() functions
// after the settings of its receiver.

// The setting "bit_rate" (Bit rate) of the stack "dsrc_toll". A frequency in Hz. The downlink bit rate. 500 kbit/s is the usual value; no public document was read to confirm it.
static inline int32_t sdrkit_dsrc_toll_get_bit_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "bit_rate", value);
}
static inline int32_t sdrkit_dsrc_toll_set_bit_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "bit_rate", value);
}

// The setting "gen_depth" (Test signal keying depth) of the test signal generator of the stack "dsrc_toll".
static inline int32_t sdrkit_dsrc_toll_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_dsrc_toll_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ism_detect_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_detect_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.IsmDetectFields", fields, sizeof(sdrkit_ism_detect_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sigfox_finder_fields_read(SDRKIT_HANDLE handle, sdrkit_sigfox_finder_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SigfoxFinderFields", fields, sizeof(sdrkit_sigfox_finder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ism_guard_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_guard_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.GuardFields", fields, sizeof(sdrkit_ism_guard_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ism_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.IsmFields", fields, sizeof(sdrkit_ism_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ism_pulse_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_pulse_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.IsmPulseFields", fields, sizeof(sdrkit_ism_pulse_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ism_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.IsmTxFields", fields, sizeof(sdrkit_ism_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wmbus_fields_read(SDRKIT_HANDLE handle, sdrkit_wmbus_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.WmbusFields", fields, sizeof(sdrkit_wmbus_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_zwave_fields_read(SDRKIT_HANDLE handle, sdrkit_zwave_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.ZwaveFields", fields, sizeof(sdrkit_zwave_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_zigbee_fields_read(SDRKIT_HANDLE handle, sdrkit_zigbee_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.ZigbeeFields", fields, sizeof(sdrkit_zigbee_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sigfox_fields_read(SDRKIT_HANDLE handle, sdrkit_sigfox_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SigfoxFields", fields, sizeof(sdrkit_sigfox_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sigfox_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_sigfox_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SigfoxDemodFields", fields, sizeof(sdrkit_sigfox_demod_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ant_fields_read(SDRKIT_HANDLE handle, sdrkit_ant_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.AntFields", fields, sizeof(sdrkit_ant_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ant_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_ant_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.AntDemodFields", fields, sizeof(sdrkit_ant_demod_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nrf24_nrf_fields_read(SDRKIT_HANDLE handle, sdrkit_nrf24_nrf_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.NrfFields", fields, sizeof(sdrkit_nrf24_nrf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_subghzchat_fields_read(SDRKIT_HANDLE handle, sdrkit_subghzchat_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SubghzchatFields", fields, sizeof(sdrkit_subghzchat_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dect_fields_read(SDRKIT_HANDLE handle, sdrkit_dect_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.DectFields", fields, sizeof(sdrkit_dect_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dect_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_dect_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.DectDemodFields", fields, sizeof(sdrkit_dect_demod_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nfc_fields_read(SDRKIT_HANDLE handle, sdrkit_nfc_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.NfcFields", fields, sizeof(sdrkit_nfc_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nfc_detect_fields_read(SDRKIT_HANDLE handle, sdrkit_nfc_detect_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.NfcDetectFields", fields, sizeof(sdrkit_nfc_detect_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wisun_fields_read(SDRKIT_HANDLE handle, sdrkit_wisun_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.WisunFields", fields, sizeof(sdrkit_wisun_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_thread_fields_read(SDRKIT_HANDLE handle, sdrkit_thread_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.ThreadFields", fields, sizeof(sdrkit_thread_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mioty_fields_read(SDRKIT_HANDLE handle, sdrkit_mioty_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.MiotyFields", fields, sizeof(sdrkit_mioty_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ism_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.DecoderFields", fields, sizeof(sdrkit_ism_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dect_nrplus_nr_plus_fields_read(SDRKIT_HANDLE handle, sdrkit_dect_nrplus_nr_plus_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.NrPlusFields", fields, sizeof(sdrkit_dect_nrplus_nr_plus_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_npr70_npr_fields_read(SDRKIT_HANDLE handle, sdrkit_npr70_npr_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.NprFields", fields, sizeof(sdrkit_npr70_npr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dsrc_toll_dsrc_fields_read(SDRKIT_HANDLE handle, sdrkit_dsrc_toll_dsrc_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.DsrcFields", fields, sizeof(sdrkit_dsrc_toll_dsrc_fields));
}

// Fills `fields` with the result fields of the component "burstdetect" of the stack "ism" in the snapshot of
// `handle`. A generic consumer reads them under the keys "burstdetect.<key>".
static inline int32_t sdrkit_ism_burstdetect_ism_detect_fields_read(SDRKIT_HANDLE handle, sdrkit_ism_detect_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.IsmDetectFields@burstdetect", fields, sizeof(sdrkit_ism_detect_fields));
}

// Fills `fields` with the result fields of the component "decode" of the stack "ism" in the snapshot of
// `handle`. A generic consumer reads them under the keys "decode.<key>".
static inline int32_t sdrkit_ism_decode_fields_read(SDRKIT_HANDLE handle, sdrkit_ookdev_decode_fields *fields) {
  return sdrkit_typed_fields(handle, "ookdev.DecodeFields@decode", fields, sizeof(sdrkit_ookdev_decode_fields));
}

// Fills `fields` with the result fields of the component "sfx_finder" of the stack "ism" in the snapshot of
// `handle`. A generic consumer reads them under the keys "sfx_finder.<key>".
static inline int32_t sdrkit_ism_sfx_finder_sigfox_finder_fields_read(SDRKIT_HANDLE handle, sdrkit_sigfox_finder_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SigfoxFinderFields@sfx_finder", fields, sizeof(sdrkit_sigfox_finder_fields));
}

// Fills `fields` with the result fields of the component "finder" of the stack "sigfox" in the snapshot of
// `handle`. A generic consumer reads them under the keys "finder.<key>".
static inline int32_t sdrkit_sigfox_finder_sigfox_finder_fields_read(SDRKIT_HANDLE handle, sdrkit_sigfox_finder_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SigfoxFinderFields@finder", fields, sizeof(sdrkit_sigfox_finder_fields));
}

// Fills `fields` with the result fields of the component "demod" of the stack "sigfox" in the snapshot of
// `handle`. A generic consumer reads them under the keys "demod.<key>".
static inline int32_t sdrkit_sigfox_demod_sigfox_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_sigfox_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.SigfoxDemodFields@demod", fields, sizeof(sdrkit_sigfox_demod_fields));
}

// Fills `fields` with the result fields of the component "demod" of the stack "ant" in the snapshot of
// `handle`. A generic consumer reads them under the keys "demod.<key>".
static inline int32_t sdrkit_ant_demod_ant_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_ant_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.AntDemodFields@demod", fields, sizeof(sdrkit_ant_demod_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ant" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ant_dropped_read(SDRKIT_HANDLE handle, sdrkit_ant_dropped *dropped) {
  return sdrkit_typed_fields(handle, "ism.ant.dropped", dropped, sizeof(sdrkit_ant_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "nrf24" in the
// snapshot of `handle`.
static inline int32_t sdrkit_nrf24_dropped_read(SDRKIT_HANDLE handle, sdrkit_nrf24_dropped *dropped) {
  return sdrkit_typed_fields(handle, "ism.nrf24.dropped", dropped, sizeof(sdrkit_nrf24_dropped));
}

// Fills `fields` with the result fields of the component "demod" of the stack "dect" in the snapshot of
// `handle`. A generic consumer reads them under the keys "demod.<key>".
static inline int32_t sdrkit_dect_demod_dect_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_dect_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.DectDemodFields@demod", fields, sizeof(sdrkit_dect_demod_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "dect" in the
// snapshot of `handle`.
static inline int32_t sdrkit_dect_dropped_read(SDRKIT_HANDLE handle, sdrkit_dect_dropped *dropped) {
  return sdrkit_typed_fields(handle, "ism.dect.dropped", dropped, sizeof(sdrkit_dect_dropped));
}

// Fills `fields` with the result fields of the component "detect" of the stack "nfc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "detect.<key>".
static inline int32_t sdrkit_nfc_detect_nfc_detect_fields_read(SDRKIT_HANDLE handle, sdrkit_nfc_detect_fields *fields) {
  return sdrkit_typed_fields(handle, "ism.NfcDetectFields@detect", fields, sizeof(sdrkit_nfc_detect_fields));
}

#endif
