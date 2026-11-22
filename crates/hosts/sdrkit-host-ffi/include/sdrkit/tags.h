/*
 * sdrkit: the typed interface of the stack family "tags", version 1.1. MIT licence.
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

#ifndef SDRKIT_TAGS_H
#define SDRKIT_TAGS_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_TAGS_VERSION_MAJOR 1
#define SDRKIT_TAGS_VERSION_MINOR 1

// The stack "rfid_lf": its id, and the keys of its event types.
#define SDRKIT_STACK_RFID_LF "rfid_lf"
#define SDRKIT_EVENT_RFID_TAG "rfid.tag"

// The stack "qi": its id, and the keys of its event types.
#define SDRKIT_STACK_QI "qi"
#define SDRKIT_EVENT_QI_PACKET "qi.packet"

// The stack "rfid_uhf": its id, and the keys of its event types.
#define SDRKIT_STACK_RFID_UHF "rfid_uhf"
#define SDRKIT_EVENT_RFID_COMMAND "rfid.command"

// The options of the setting "bit_rates" of the stack "rfid_lf".
typedef enum sdrkit_rfid_lf_bit_rates {
  // Both
  SDRKIT_RFID_LF_BIT_RATES_BOTH = 0,
  // RF/64
  SDRKIT_RFID_LF_BIT_RATES_RF64 = 1,
  // RF/32
  SDRKIT_RFID_LF_BIT_RATES_RF32 = 2,
} sdrkit_rfid_lf_bit_rates;

// The options of the setting "codes" of the stack "rfid_lf".
typedef enum sdrkit_rfid_lf_codes {
  // Both
  SDRKIT_RFID_LF_CODES_BOTH = 0,
  // Manchester
  SDRKIT_RFID_LF_CODES_MANCHESTER = 1,
  // Biphase
  SDRKIT_RFID_LF_CODES_BIPHASE = 2,
} sdrkit_rfid_lf_codes;

// The options of the setting "gen_rf_div" of the stack "rfid_lf".
typedef enum sdrkit_rfid_lf_gen_rf_div {
  // RF/64
  SDRKIT_RFID_LF_GEN_RF_DIV_RF64 = 0,
  // RF/32
  SDRKIT_RFID_LF_GEN_RF_DIV_RF32 = 1,
} sdrkit_rfid_lf_gen_rf_div;

// The options of the setting "gen_coding" of the stack "rfid_lf".
typedef enum sdrkit_rfid_lf_gen_coding {
  // Manchester
  SDRKIT_RFID_LF_GEN_CODING_MANCHESTER = 0,
  // Biphase
  SDRKIT_RFID_LF_GEN_CODING_BIPHASE = 1,
} sdrkit_rfid_lf_gen_coding;

// The options of `state` of `sdrkit_tags_status_fields`.
typedef enum sdrkit_tags_status_fields_state {
  // No signal
  SDRKIT_TAGS_STATUS_FIELDS_STATE_NO_SIGNAL = 0,
  // Energy
  SDRKIT_TAGS_STATUS_FIELDS_STATE_ENERGY = 1,
  // Decoding
  SDRKIT_TAGS_STATUS_FIELDS_STATE_DECODING = 2,
} sdrkit_tags_status_fields_state;

// The status of a stack of the family: the headline, the state, the counts of results and the input level.
typedef struct sdrkit_tags_status_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The headline.
  sdrkit_string text;
  // The state. A value of `sdrkit_tags_status_fields_state`.
  int32_t state;
  // The good results: what passed every check of its format.
  int64_t good;
  // The refused results: what failed a check.
  int64_t bad;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level of the last report, relative to full scale. Unit: dBFS.
  double level_dbfs;
  // What the stack reads and what no public document confirms.
  sdrkit_string note;
} sdrkit_tags_status_fields;

// The options of `bit_rate` of `sdrkit_rfid_lf_fields`.
typedef enum sdrkit_rfid_lf_fields_bit_rate {
  // RF/64
  SDRKIT_RFID_LF_FIELDS_BIT_RATE_RF64 = 0,
  // RF/32
  SDRKIT_RFID_LF_FIELDS_BIT_RATE_RF32 = 1,
} sdrkit_rfid_lf_fields_bit_rate;

// The options of `coding` of `sdrkit_rfid_lf_fields`.
typedef enum sdrkit_rfid_lf_fields_coding {
  // Manchester
  SDRKIT_RFID_LF_FIELDS_CODING_MANCHESTER = 0,
  // Biphase
  SDRKIT_RFID_LF_FIELDS_CODING_BIPHASE = 1,
} sdrkit_rfid_lf_fields_coding;

// The result fields of `rfid_lf`.
typedef struct sdrkit_rfid_lf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `carrier_db` holds a value.
  bool has_carrier_db;
  // The level of the reader carrier, relative; no value before the first input. Unit: dB.
  double carrier_db;
  // True if `depth_pct` holds a value.
  bool has_depth_pct;
  // The modulation depth of the load (swing over carrier); no value without load modulation in the last 2 s. Unit: %.
  double depth_pct;
  // The different tags seen.
  int64_t tags;
  // The reads: frames that passed every check.
  int64_t reads;
  // Frames at the place of the frames of the last read (the same code, the same position in the stream) whose header and stop bit held and whose row or column parity failed.
  int64_t refused;
  // True if `bit_rate` holds a value.
  bool has_bit_rate;
  // The bit rate of the last read. A value of `sdrkit_rfid_lf_fields_bit_rate`.
  int32_t bit_rate;
  // True if `coding` holds a value.
  bool has_coding;
  // The code of the last read. A value of `sdrkit_rfid_lf_fields_coding`.
  int32_t coding;
  // True if `last_tag` holds a value.
  bool has_last_tag;
  // The last tag: version and number, 10 hex digits.
  sdrkit_string last_tag;
} sdrkit_rfid_lf_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_tags_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_tags_decoder_fields;

// The options of `bit_rate` of `sdrkit_rfid_lf_tag_row`.
typedef enum sdrkit_rfid_lf_tag_row_bit_rate {
  // RF/64
  SDRKIT_RFID_LF_TAG_ROW_BIT_RATE_RF64 = 0,
  // RF/32
  SDRKIT_RFID_LF_TAG_ROW_BIT_RATE_RF32 = 1,
} sdrkit_rfid_lf_tag_row_bit_rate;

// The options of `coding` of `sdrkit_rfid_lf_tag_row`.
typedef enum sdrkit_rfid_lf_tag_row_coding {
  // Manchester
  SDRKIT_RFID_LF_TAG_ROW_CODING_MANCHESTER = 0,
  // Biphase
  SDRKIT_RFID_LF_TAG_ROW_CODING_BIPHASE = 1,
} sdrkit_rfid_lf_tag_row_coding;

// A row of the tag table: one tag, read until another tag came or it was not read for the hold time.
typedef struct sdrkit_rfid_lf_tag_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the last read. A length of time in seconds.
  double time_s;
  // Version (2 hex digits) and number (8 hex digits).
  sdrkit_string tag;
  // The version or customer code, 0 to 255.
  int64_t version;
  // The number, 0 to 4294967295.
  int64_t number;
  // The upper 16 bits of the number.
  int64_t number_hi;
  // The lower 16 bits of the number.
  int64_t number_lo;
  // The bit rate. A value of `sdrkit_rfid_lf_tag_row_bit_rate`.
  int32_t bit_rate;
  // The code. A value of `sdrkit_rfid_lf_tag_row_coding`.
  int32_t coding;
  // The reads of the row.
  int64_t reads;
  // The carrier level, relative. Unit: dB.
  double level_db;
} sdrkit_rfid_lf_tag_row;

// The options of `last_message` of `sdrkit_qi_fields`.
typedef enum sdrkit_qi_fields_last_message {
  // Signal strength
  SDRKIT_QI_FIELDS_LAST_MESSAGE_SIGNAL_STRENGTH = 0,
  // End power transfer
  SDRKIT_QI_FIELDS_LAST_MESSAGE_END_POWER_TRANSFER = 1,
  // Control error
  SDRKIT_QI_FIELDS_LAST_MESSAGE_CONTROL_ERROR = 2,
  // Received power
  SDRKIT_QI_FIELDS_LAST_MESSAGE_RECEIVED_POWER = 3,
  // Charge status
  SDRKIT_QI_FIELDS_LAST_MESSAGE_CHARGE_STATUS = 4,
  // Power control hold-off
  SDRKIT_QI_FIELDS_LAST_MESSAGE_HOLD_OFF = 5,
  // General request
  SDRKIT_QI_FIELDS_LAST_MESSAGE_GENERAL_REQUEST = 6,
  // Renegotiate
  SDRKIT_QI_FIELDS_LAST_MESSAGE_RENEGOTIATE = 7,
  // Specific request
  SDRKIT_QI_FIELDS_LAST_MESSAGE_SPECIFIC_REQUEST = 8,
  // FOD status
  SDRKIT_QI_FIELDS_LAST_MESSAGE_FOD_STATUS = 9,
  // Received power (24 bits)
  SDRKIT_QI_FIELDS_LAST_MESSAGE_RECEIVED_POWER_24 = 10,
  // Configuration
  SDRKIT_QI_FIELDS_LAST_MESSAGE_CONFIGURATION = 11,
  // Wireless power ID
  SDRKIT_QI_FIELDS_LAST_MESSAGE_WPID = 12,
  // Identification
  SDRKIT_QI_FIELDS_LAST_MESSAGE_IDENTIFICATION = 13,
  // Extended identification
  SDRKIT_QI_FIELDS_LAST_MESSAGE_EXTENDED_IDENTIFICATION = 14,
  // Proprietary
  SDRKIT_QI_FIELDS_LAST_MESSAGE_PROPRIETARY = 15,
  // Other
  SDRKIT_QI_FIELDS_LAST_MESSAGE_OTHER = 16,
} sdrkit_qi_fields_last_message;

// The result fields of `qi`.
typedef struct sdrkit_qi_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `power_signal_db` holds a value.
  bool has_power_signal_db;
  // The level of the power signal, relative; no value before the first input. Unit: dB.
  double power_signal_db;
  // True if `load_modulation_pct` holds a value.
  bool has_load_modulation_pct;
  // The load modulation (swing over power signal); no value without load modulation in the last 2 s. Unit: %.
  double load_modulation_pct;
  // The packets that passed every check.
  int64_t packets_ok;
  // The packets that failed a parity, a start or stop bit, the checksum, or lost their bit clock after the header.
  int64_t packets_failed;
  // True if `bit_rate` holds a value.
  bool has_bit_rate;
  // The bit rate that the bit clock follows; no value without load modulation in the last 2 s. Unit: bit/s.
  double bit_rate;
  // True if `last_message` holds a value.
  bool has_last_message;
  // The type of the last good packet. A value of `sdrkit_qi_fields_last_message`.
  int32_t last_message;
} sdrkit_qi_fields;

// The options of `message` of `sdrkit_qi_packet_row`.
typedef enum sdrkit_qi_packet_row_message {
  // Signal strength
  SDRKIT_QI_PACKET_ROW_MESSAGE_SIGNAL_STRENGTH = 0,
  // End power transfer
  SDRKIT_QI_PACKET_ROW_MESSAGE_END_POWER_TRANSFER = 1,
  // Control error
  SDRKIT_QI_PACKET_ROW_MESSAGE_CONTROL_ERROR = 2,
  // Received power
  SDRKIT_QI_PACKET_ROW_MESSAGE_RECEIVED_POWER = 3,
  // Charge status
  SDRKIT_QI_PACKET_ROW_MESSAGE_CHARGE_STATUS = 4,
  // Power control hold-off
  SDRKIT_QI_PACKET_ROW_MESSAGE_HOLD_OFF = 5,
  // General request
  SDRKIT_QI_PACKET_ROW_MESSAGE_GENERAL_REQUEST = 6,
  // Renegotiate
  SDRKIT_QI_PACKET_ROW_MESSAGE_RENEGOTIATE = 7,
  // Specific request
  SDRKIT_QI_PACKET_ROW_MESSAGE_SPECIFIC_REQUEST = 8,
  // FOD status
  SDRKIT_QI_PACKET_ROW_MESSAGE_FOD_STATUS = 9,
  // Received power (24 bits)
  SDRKIT_QI_PACKET_ROW_MESSAGE_RECEIVED_POWER_24 = 10,
  // Configuration
  SDRKIT_QI_PACKET_ROW_MESSAGE_CONFIGURATION = 11,
  // Wireless power ID
  SDRKIT_QI_PACKET_ROW_MESSAGE_WPID = 12,
  // Identification
  SDRKIT_QI_PACKET_ROW_MESSAGE_IDENTIFICATION = 13,
  // Extended identification
  SDRKIT_QI_PACKET_ROW_MESSAGE_EXTENDED_IDENTIFICATION = 14,
  // Proprietary
  SDRKIT_QI_PACKET_ROW_MESSAGE_PROPRIETARY = 15,
  // Other
  SDRKIT_QI_PACKET_ROW_MESSAGE_OTHER = 16,
} sdrkit_qi_packet_row_message;

// A row of the packet table.
typedef struct sdrkit_qi_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the end of the packet. A length of time in seconds.
  double time_s;
  // The header, 2 hex digits.
  sdrkit_string header;
  // The type. A value of `sdrkit_qi_packet_row_message`.
  int32_t message;
  // The message bytes in hex.
  sdrkit_string bytes;
  // True if `value` holds a value.
  bool has_value;
  // Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
  int64_t value;
  // True if `end_reason` holds a value.
  bool has_end_reason;
  // The reason of an End Power Transfer packet.
  sdrkit_string end_reason;
  // True if `charge` holds a value.
  bool has_charge;
  // The meaning of a Charge Status value.
  sdrkit_string charge;
  // True if `power_class` holds a value.
  bool has_power_class;
  // The power class of a Configuration packet.
  int64_t power_class;
  // True if `max_power` holds a value.
  bool has_max_power;
  // The maximum power value of a Configuration packet.
  int64_t max_power;
  // True if `version` holds a value.
  bool has_version;
  // The version of an Identification packet, major.minor.
  sdrkit_string version;
  // True if `manufacturer` holds a value.
  bool has_manufacturer;
  // The manufacturer code of an Identification packet, 4 hex digits.
  sdrkit_string manufacturer;
  // True if `device_id` holds a value.
  bool has_device_id;
  // The basic device identifier of an Identification packet, 8 hex digits (31 bits).
  sdrkit_string device_id;
  // True if `extended` holds a value.
  bool has_extended;
  // An Extended Identification packet follows an Identification packet.
  bool extended;
  // The power signal level, relative. Unit: dB.
  double level_db;
} sdrkit_qi_packet_row;

// The result fields of `rfid_uhf`. The timings are those of the last good command; they have no value without a reader carrier in the last 8 s.
typedef struct sdrkit_rfid_uhf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `tari_us` holds a value.
  bool has_tari_us;
  // Tari: the length of a data-0. Unit: us.
  double tari_us;
  // True if `rtcal_us` holds a value.
  bool has_rtcal_us;
  // RTcal: a data-0 and a data-1. Unit: us.
  double rtcal_us;
  // True if `rtcal_tari` holds a value.
  bool has_rtcal_tari;
  // RTcal in Tari.
  double rtcal_tari;
  // True if `trcal_us` holds a value.
  bool has_trcal_us;
  // TRcal of the last Query. Unit: us.
  double trcal_us;
  // True if `blf_dr8_khz` holds a value.
  bool has_blf_dr8_khz;
  // The backscatter link frequency that the last Query asks with the divide ratio 8: 8 / TRcal. Unit: kHz.
  double blf_dr8_khz;
  // True if `blf_dr643_khz` holds a value.
  bool has_blf_dr643_khz;
  // The backscatter link frequency with the divide ratio 64/3. Unit: kHz.
  double blf_dr643_khz;
  // The commands that passed every check.
  int64_t commands_ok;
  // The frames that failed a timing rule, their length or their CRC.
  int64_t commands_failed;
  // The Query commands decoded.
  int64_t count_query;
  // The QueryRep commands decoded.
  int64_t count_query_rep;
  // The QueryAdjust commands decoded.
  int64_t count_query_adjust;
  // The ACK commands decoded.
  int64_t count_ack;
  // The NAK commands decoded.
  int64_t count_nak;
  // The Select commands decoded.
  int64_t count_select;
  // The Req_RN commands decoded.
  int64_t count_req_rn;
  // The Read commands decoded.
  int64_t count_read;
  // The Write commands decoded.
  int64_t count_write;
  // The Kill commands decoded.
  int64_t count_kill;
  // The Lock commands decoded.
  int64_t count_lock;
  // The Access commands decoded.
  int64_t count_access;
  // A modulated reader carrier was seen in the last 2 s.
  bool reader_carrier;
} sdrkit_rfid_uhf_fields;

// The options of `command` of `sdrkit_rfid_uhf_command_row`.
typedef enum sdrkit_rfid_uhf_command_row_command {
  // Query
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_QUERY = 0,
  // QueryRep
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_QUERY_REP = 1,
  // QueryAdjust
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_QUERY_ADJUST = 2,
  // ACK
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_ACK = 3,
  // NAK
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_NAK = 4,
  // Select
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_SELECT = 5,
  // Req_RN
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_REQ_RN = 6,
  // Read
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_READ = 7,
  // Write
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_WRITE = 8,
  // Kill
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_KILL = 9,
  // Lock
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_LOCK = 10,
  // Access
  SDRKIT_RFID_UHF_COMMAND_ROW_COMMAND_ACCESS = 11,
} sdrkit_rfid_uhf_command_row_command;

// A row of the command table.
typedef struct sdrkit_rfid_uhf_command_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the end of the command. A length of time in seconds.
  double time_s;
  // The command. A value of `sdrkit_rfid_uhf_command_row_command`.
  int32_t command;
  // True if `session` holds a value.
  bool has_session;
  // The session, 0 to 3.
  int64_t session;
  // True if `q` holds a value.
  bool has_q;
  // Q of a Query.
  int64_t q;
  // True if `slots` holds a value.
  bool has_slots;
  // The slots of a Query, 2 to the power Q.
  int64_t slots;
  // True if `target` holds a value.
  bool has_target;
  // The target: A or B of a Query, S0 to S3, SL or RFU of a Select.
  sdrkit_string target;
  // True if `sel` holds a value.
  bool has_sel;
  // The tags a Query addresses: all, ~SL or SL.
  sdrkit_string sel;
  // True if `coding` holds a value.
  bool has_coding;
  // The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8.
  sdrkit_string coding;
  // True if `pilot` holds a value.
  bool has_pilot;
  // A Query asks for a pilot tone (TRext).
  bool pilot;
  // True if `dr` holds a value.
  bool has_dr;
  // The divide ratio of a Query: 8 or 64/3.
  sdrkit_string dr;
  // True if `updn` holds a value.
  bool has_updn;
  // The change of Q of a QueryAdjust: +1, -1, unchanged or reserved.
  sdrkit_string updn;
  // True if `rn16` holds a value.
  bool has_rn16;
  // The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits.
  sdrkit_string rn16;
  // True if `bank` holds a value.
  bool has_bank;
  // The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User).
  sdrkit_string bank;
  // True if `pointer` holds a value.
  bool has_pointer;
  // The bit pointer of a Select, or the word pointer of a Read or a Write.
  int64_t pointer;
  // True if `word_count` holds a value.
  bool has_word_count;
  // The words that a Read asks.
  int64_t word_count;
  // True if `mask` holds a value.
  bool has_mask;
  // The mask of a Select, as bits (the first 96).
  sdrkit_string mask;
  // True if `truncate` holds a value.
  bool has_truncate;
  // The Truncate bit of a Select.
  bool truncate;
  // True if `action` holds a value.
  bool has_action;
  // The action of a Select, 0 to 7.
  int64_t action;
  // True if `lock_payload` holds a value.
  bool has_lock_payload;
  // The mask and action bits of a Lock, 5 hex digits.
  sdrkit_string lock_payload;
  // The level of the reader carrier, relative. Unit: dB.
  double level_db;
} sdrkit_rfid_uhf_command_row;

// The options of `bit_rate` of `sdrkit_rfid_lf_rfid_tag_event`.
typedef enum sdrkit_rfid_lf_rfid_tag_event_bit_rate {
  // RF/64
  SDRKIT_RFID_LF_RFID_TAG_EVENT_BIT_RATE_RF64 = 0,
  // RF/32
  SDRKIT_RFID_LF_RFID_TAG_EVENT_BIT_RATE_RF32 = 1,
} sdrkit_rfid_lf_rfid_tag_event_bit_rate;

// The options of `coding` of `sdrkit_rfid_lf_rfid_tag_event`.
typedef enum sdrkit_rfid_lf_rfid_tag_event_coding {
  // Manchester
  SDRKIT_RFID_LF_RFID_TAG_EVENT_CODING_MANCHESTER = 0,
  // Biphase
  SDRKIT_RFID_LF_RFID_TAG_EVENT_CODING_BIPHASE = 1,
} sdrkit_rfid_lf_rfid_tag_event_coding;

// A tag was read for the first time, or again after another tag or after the hold time.
typedef struct sdrkit_rfid_lf_rfid_tag_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the read. A length of time in seconds.
  double time_s;
  // Version (2 hex digits) and number (8 hex digits).
  sdrkit_string tag;
  // The version or customer code, 0 to 255.
  int64_t version;
  // The number, 0 to 4294967295.
  int64_t number;
  // The upper 16 bits of the number.
  int64_t number_hi;
  // The lower 16 bits of the number.
  int64_t number_lo;
  // The bit rate. A value of `sdrkit_rfid_lf_rfid_tag_event_bit_rate`.
  int32_t bit_rate;
  // The code. A value of `sdrkit_rfid_lf_rfid_tag_event_coding`.
  int32_t coding;
  // The reads of the row.
  int64_t reads;
  // The carrier level, relative. Unit: dB.
  double level_db;
} sdrkit_rfid_lf_rfid_tag_event;

// The options of `message` of `sdrkit_qi_packet_event`.
typedef enum sdrkit_qi_packet_event_message {
  // Signal strength
  SDRKIT_QI_PACKET_EVENT_MESSAGE_SIGNAL_STRENGTH = 0,
  // End power transfer
  SDRKIT_QI_PACKET_EVENT_MESSAGE_END_POWER_TRANSFER = 1,
  // Control error
  SDRKIT_QI_PACKET_EVENT_MESSAGE_CONTROL_ERROR = 2,
  // Received power
  SDRKIT_QI_PACKET_EVENT_MESSAGE_RECEIVED_POWER = 3,
  // Charge status
  SDRKIT_QI_PACKET_EVENT_MESSAGE_CHARGE_STATUS = 4,
  // Power control hold-off
  SDRKIT_QI_PACKET_EVENT_MESSAGE_HOLD_OFF = 5,
  // General request
  SDRKIT_QI_PACKET_EVENT_MESSAGE_GENERAL_REQUEST = 6,
  // Renegotiate
  SDRKIT_QI_PACKET_EVENT_MESSAGE_RENEGOTIATE = 7,
  // Specific request
  SDRKIT_QI_PACKET_EVENT_MESSAGE_SPECIFIC_REQUEST = 8,
  // FOD status
  SDRKIT_QI_PACKET_EVENT_MESSAGE_FOD_STATUS = 9,
  // Received power (24 bits)
  SDRKIT_QI_PACKET_EVENT_MESSAGE_RECEIVED_POWER_24 = 10,
  // Configuration
  SDRKIT_QI_PACKET_EVENT_MESSAGE_CONFIGURATION = 11,
  // Wireless power ID
  SDRKIT_QI_PACKET_EVENT_MESSAGE_WPID = 12,
  // Identification
  SDRKIT_QI_PACKET_EVENT_MESSAGE_IDENTIFICATION = 13,
  // Extended identification
  SDRKIT_QI_PACKET_EVENT_MESSAGE_EXTENDED_IDENTIFICATION = 14,
  // Proprietary
  SDRKIT_QI_PACKET_EVENT_MESSAGE_PROPRIETARY = 15,
  // Other
  SDRKIT_QI_PACKET_EVENT_MESSAGE_OTHER = 16,
} sdrkit_qi_packet_event_message;

// A packet passed every check.
typedef struct sdrkit_qi_packet_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the end of the packet. A length of time in seconds.
  double time_s;
  // The header, 2 hex digits.
  sdrkit_string header;
  // The type. A value of `sdrkit_qi_packet_event_message`.
  int32_t message;
  // The message bytes in hex.
  sdrkit_string bytes;
  // True if `value` holds a value.
  bool has_value;
  // Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
  int64_t value;
  // True if `end_reason` holds a value.
  bool has_end_reason;
  // The reason of an End Power Transfer packet.
  sdrkit_string end_reason;
  // True if `charge` holds a value.
  bool has_charge;
  // The meaning of a Charge Status value.
  sdrkit_string charge;
  // True if `power_class` holds a value.
  bool has_power_class;
  // The power class of a Configuration packet.
  int64_t power_class;
  // True if `max_power` holds a value.
  bool has_max_power;
  // The maximum power value of a Configuration packet.
  int64_t max_power;
  // True if `version` holds a value.
  bool has_version;
  // The version of an Identification packet, major.minor.
  sdrkit_string version;
  // True if `manufacturer` holds a value.
  bool has_manufacturer;
  // The manufacturer code of an Identification packet, 4 hex digits.
  sdrkit_string manufacturer;
  // True if `device_id` holds a value.
  bool has_device_id;
  // The basic device identifier of an Identification packet, 8 hex digits (31 bits).
  sdrkit_string device_id;
  // True if `extended` holds a value.
  bool has_extended;
  // An Extended Identification packet follows an Identification packet.
  bool extended;
  // The power signal level, relative. Unit: dB.
  double level_db;
} sdrkit_qi_packet_event;

// The options of `command` of `sdrkit_rfid_uhf_rfid_command_event`.
typedef enum sdrkit_rfid_uhf_rfid_command_event_command {
  // Query
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_QUERY = 0,
  // QueryRep
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_QUERY_REP = 1,
  // QueryAdjust
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_QUERY_ADJUST = 2,
  // ACK
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_ACK = 3,
  // NAK
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_NAK = 4,
  // Select
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_SELECT = 5,
  // Req_RN
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_REQ_RN = 6,
  // Read
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_READ = 7,
  // Write
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_WRITE = 8,
  // Kill
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_KILL = 9,
  // Lock
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_LOCK = 10,
  // Access
  SDRKIT_RFID_UHF_RFID_COMMAND_EVENT_COMMAND_ACCESS = 11,
} sdrkit_rfid_uhf_rfid_command_event_command;

// A reader command passed every check.
typedef struct sdrkit_rfid_uhf_rfid_command_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the end of the command. A length of time in seconds.
  double time_s;
  // The command. A value of `sdrkit_rfid_uhf_rfid_command_event_command`.
  int32_t command;
  // True if `session` holds a value.
  bool has_session;
  // The session, 0 to 3.
  int64_t session;
  // True if `q` holds a value.
  bool has_q;
  // Q of a Query.
  int64_t q;
  // True if `slots` holds a value.
  bool has_slots;
  // The slots of a Query, 2 to the power Q.
  int64_t slots;
  // True if `target` holds a value.
  bool has_target;
  // The target: A or B of a Query, S0 to S3, SL or RFU of a Select.
  sdrkit_string target;
  // True if `sel` holds a value.
  bool has_sel;
  // The tags a Query addresses: all, ~SL or SL.
  sdrkit_string sel;
  // True if `coding` holds a value.
  bool has_coding;
  // The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8.
  sdrkit_string coding;
  // True if `pilot` holds a value.
  bool has_pilot;
  // A Query asks for a pilot tone (TRext).
  bool pilot;
  // True if `dr` holds a value.
  bool has_dr;
  // The divide ratio of a Query: 8 or 64/3.
  sdrkit_string dr;
  // True if `updn` holds a value.
  bool has_updn;
  // The change of Q of a QueryAdjust: +1, -1, unchanged or reserved.
  sdrkit_string updn;
  // True if `rn16` holds a value.
  bool has_rn16;
  // The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits.
  sdrkit_string rn16;
  // True if `bank` holds a value.
  bool has_bank;
  // The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User).
  sdrkit_string bank;
  // True if `pointer` holds a value.
  bool has_pointer;
  // The bit pointer of a Select, or the word pointer of a Read or a Write.
  int64_t pointer;
  // True if `word_count` holds a value.
  bool has_word_count;
  // The words that a Read asks.
  int64_t word_count;
  // True if `mask` holds a value.
  bool has_mask;
  // The mask of a Select, as bits (the first 96).
  sdrkit_string mask;
  // True if `truncate` holds a value.
  bool has_truncate;
  // The Truncate bit of a Select.
  bool truncate;
  // True if `action` holds a value.
  bool has_action;
  // The action of a Select, 0 to 7.
  int64_t action;
  // True if `lock_payload` holds a value.
  bool has_lock_payload;
  // The mask and action bits of a Lock, 5 hex digits.
  sdrkit_string lock_payload;
  // The level of the reader carrier, relative. Unit: dB.
  double level_db;
} sdrkit_rfid_uhf_rfid_command_event;

// Fills `rows` with the changed rows of the table "tags" of the stack "rfid_lf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rfid_lf_tags_rows(SDRKIT_HANDLE handle,
                                              sdrkit_rfid_lf_tag_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tags.RfidLfTagRow", "tags", rows, sizeof(sdrkit_rfid_lf_tag_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "rfid.tag"
// (Tag read). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_rfid_lf_rfid_tag_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_rfid_lf_rfid_tag_event *event) {
  return sdrkit_typed_event(handle, n, "rfid.tag", "tags.RfidTagEvent", event, sizeof(sdrkit_rfid_lf_rfid_tag_event));
}

// Runs the command "clear" (Clear) of the stack "rfid_lf" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_rfid_lf_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "rfid_lf" on the session of `handle`. Drops the trackers, the bit clocks and the half-bits so far; the table stays.
static inline int32_t sdrkit_rfid_lf_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "rfid_lf" are the sdrkit_rfid_lf_gen_*() functions
// after the settings of its receiver.

// The setting "bit_rates" (Bit rates read) of the stack "rfid_lf". A value of `sdrkit_rfid_lf_bit_rates`.
static inline int32_t sdrkit_rfid_lf_get_bit_rates(SDRKIT_HANDLE handle, sdrkit_rfid_lf_bit_rates *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bit_rates", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rfid_lf_bit_rates)index;
  return code;
}
static inline int32_t sdrkit_rfid_lf_set_bit_rates(SDRKIT_HANDLE handle, sdrkit_rfid_lf_bit_rates value) {
  return sdrkit_typed_setting_set_enum(handle, "bit_rates", (int32_t)value);
}

// The setting "codes" (Codes read) of the stack "rfid_lf". A value of `sdrkit_rfid_lf_codes`.
static inline int32_t sdrkit_rfid_lf_get_codes(SDRKIT_HANDLE handle, sdrkit_rfid_lf_codes *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "codes", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rfid_lf_codes)index;
  return code;
}
static inline int32_t sdrkit_rfid_lf_set_codes(SDRKIT_HANDLE handle, sdrkit_rfid_lf_codes value) {
  return sdrkit_typed_setting_set_enum(handle, "codes", (int32_t)value);
}

// The setting "same_tag_s" (Same tag held for) of the stack "rfid_lf". Unit: s. A read of the same tag within this time of its last read adds to its row.
static inline int32_t sdrkit_rfid_lf_get_same_tag_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "same_tag_s", value);
}
static inline int32_t sdrkit_rfid_lf_set_same_tag_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "same_tag_s", value);
}

// The setting "gate_sigmas" (Swing gate) of the stack "rfid_lf". Unit: sigma. Level changes are decided only while the swing of the envelope is this many deviations of its noise.
static inline int32_t sdrkit_rfid_lf_get_gate_sigmas(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gate_sigmas", value);
}
static inline int32_t sdrkit_rfid_lf_set_gate_sigmas(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gate_sigmas", value);
}

// The setting "gen_id" (Tags) of the test signal generator of the stack "rfid_lf". Tags of 10 hex digits (version and number), separated by commas; each is sent in turn for the time of gen_id_s.
static inline int32_t sdrkit_rfid_lf_gen_get_id(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_id", value);
}
static inline int32_t sdrkit_rfid_lf_gen_set_id(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_id", value);
}

// The setting "gen_id_s" (Time of each tag) of the test signal generator of the stack "rfid_lf". Unit: s.
static inline int32_t sdrkit_rfid_lf_gen_get_id_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_id_s", value);
}
static inline int32_t sdrkit_rfid_lf_gen_set_id_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_id_s", value);
}

// The setting "gen_depth" (Modulation depth) of the test signal generator of the stack "rfid_lf".
static inline int32_t sdrkit_rfid_lf_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_rfid_lf_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// The setting "gen_rf_div" (Bit rate) of the test signal generator of the stack "rfid_lf". A value of `sdrkit_rfid_lf_gen_rf_div`.
static inline int32_t sdrkit_rfid_lf_gen_get_rf_div(SDRKIT_HANDLE handle, sdrkit_rfid_lf_gen_rf_div *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rf_div", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rfid_lf_gen_rf_div)index;
  return code;
}
static inline int32_t sdrkit_rfid_lf_gen_set_rf_div(SDRKIT_HANDLE handle, sdrkit_rfid_lf_gen_rf_div value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rf_div", (int32_t)value);
}

// The setting "gen_coding" (Code) of the test signal generator of the stack "rfid_lf". A value of `sdrkit_rfid_lf_gen_coding`.
static inline int32_t sdrkit_rfid_lf_gen_get_coding(SDRKIT_HANDLE handle, sdrkit_rfid_lf_gen_coding *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_coding", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rfid_lf_gen_coding)index;
  return code;
}
static inline int32_t sdrkit_rfid_lf_gen_set_coding(SDRKIT_HANDLE handle, sdrkit_rfid_lf_gen_coding value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_coding", (int32_t)value);
}

// The setting "gen_bit_clock_ppm" (Tag clock error) of the test signal generator of the stack "rfid_lf". Unit: ppm. The bits run this much fast (positive) or slow.
static inline int32_t sdrkit_rfid_lf_gen_get_bit_clock_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bit_clock_ppm", value);
}
static inline int32_t sdrkit_rfid_lf_gen_set_bit_clock_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bit_clock_ppm", value);
}

// The setting "gen_start_s" (Tag starts at) of the test signal generator of the stack "rfid_lf". Unit: s.
static inline int32_t sdrkit_rfid_lf_gen_get_start_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_start_s", value);
}
static inline int32_t sdrkit_rfid_lf_gen_set_start_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_start_s", value);
}

// The setting "gen_invert" (Inverted load) of the test signal generator of the stack "rfid_lf".
static inline int32_t sdrkit_rfid_lf_gen_get_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert", value);
}
static inline int32_t sdrkit_rfid_lf_gen_set_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "qi" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `device_id` names a row.
static inline int32_t sdrkit_qi_packets_rows(SDRKIT_HANDLE handle,
                                            sdrkit_qi_packet_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tags.QiPacketRow", "packets", rows, sizeof(sdrkit_qi_packet_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "qi.packet"
// (Packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_qi_packet_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_qi_packet_event *event) {
  return sdrkit_typed_event(handle, n, "qi.packet", "tags.QiPacketEvent", event, sizeof(sdrkit_qi_packet_event));
}

// Runs the command "clear" (Clear) of the stack "qi" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_qi_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "qi" on the session of `handle`. Drops the trackers, the bit clock and the packet in progress; the table stays.
static inline int32_t sdrkit_qi_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "qi" are the sdrkit_qi_gen_*() functions
// after the settings of its receiver.

// The setting "gate_sigmas" (Swing gate) of the stack "qi". Unit: sigma. Level changes are decided only while the swing of the envelope is this many deviations of its noise.
static inline int32_t sdrkit_qi_get_gate_sigmas(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gate_sigmas", value);
}
static inline int32_t sdrkit_qi_set_gate_sigmas(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gate_sigmas", value);
}

// The setting "clock_tolerance_pct" (Bit clock followed within) of the stack "qi". Unit: %.
static inline int32_t sdrkit_qi_get_clock_tolerance_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock_tolerance_pct", value);
}
static inline int32_t sdrkit_qi_set_clock_tolerance_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock_tolerance_pct", value);
}

// The setting "gen_depth" (Load modulation depth) of the test signal generator of the stack "qi".
static inline int32_t sdrkit_qi_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_qi_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// The setting "gen_bit_clock_ppm" (Bit clock error) of the test signal generator of the stack "qi". Unit: ppm. The bits run this much fast (positive) or slow.
static inline int32_t sdrkit_qi_gen_get_bit_clock_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bit_clock_ppm", value);
}
static inline int32_t sdrkit_qi_gen_set_bit_clock_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bit_clock_ppm", value);
}

// The setting "gen_wander_hz" (Wander frequency) of the test signal generator of the stack "qi". Unit: Hz.
static inline int32_t sdrkit_qi_gen_get_wander_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wander_hz", value);
}
static inline int32_t sdrkit_qi_gen_set_wander_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wander_hz", value);
}

// The setting "gen_wander_depth" (Wander depth) of the test signal generator of the stack "qi".
static inline int32_t sdrkit_qi_gen_get_wander_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wander_depth", value);
}
static inline int32_t sdrkit_qi_gen_set_wander_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wander_depth", value);
}

// The setting "gen_gap_s" (Time between packets) of the test signal generator of the stack "qi". Unit: s.
static inline int32_t sdrkit_qi_gen_get_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_s", value);
}
static inline int32_t sdrkit_qi_gen_set_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value);
}

// The setting "gen_preamble" (Preamble) of the test signal generator of the stack "qi". Unit: bits.
static inline int32_t sdrkit_qi_gen_get_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_preamble", value);
}
static inline int32_t sdrkit_qi_gen_set_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_preamble", value);
}

// The setting "gen_packets" (Packets) of the test signal generator of the stack "qi". Packets separated by ';': the header and the message in hex, the checksum added. In an entry, 'pN' sets a preamble of N bits, 'x' sends the bytes as given (the last is the checksum), 'fN' sends bit N of the packet inverted.
static inline int32_t sdrkit_qi_gen_get_packets(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_packets", value);
}
static inline int32_t sdrkit_qi_gen_set_packets(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_packets", value);
}

// The setting "gen_end_edge_bits" (Change after each packet) of the test signal generator of the stack "qi". Unit: bit. One more change this long after the middle of the last stop bit; 0 for none.
static inline int32_t sdrkit_qi_gen_get_end_edge_bits(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_end_edge_bits", value);
}
static inline int32_t sdrkit_qi_gen_set_end_edge_bits(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_end_edge_bits", value);
}

// Fills `rows` with the changed rows of the table "commands" of the stack "rfid_uhf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rfid_uhf_commands_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_rfid_uhf_command_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tags.RfidUhfCommandRow", "commands", rows, sizeof(sdrkit_rfid_uhf_command_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "rfid.command"
// (Reader command). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_rfid_uhf_rfid_command_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_rfid_uhf_rfid_command_event *event) {
  return sdrkit_typed_event(handle, n, "rfid.command", "tags.RfidCommandEvent", event, sizeof(sdrkit_rfid_uhf_rfid_command_event));
}

// Runs the command "clear" (Clear) of the stack "rfid_uhf" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_rfid_uhf_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "rfid_uhf" on the session of `handle`. Drops the trackers and the frame in progress; the table stays.
static inline int32_t sdrkit_rfid_uhf_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "rfid_uhf" are the sdrkit_rfid_uhf_gen_*() functions
// after the settings of its receiver.

// The setting "list_unchecked" (List commands that have no checksum) of the stack "rfid_uhf". QueryRep, QueryAdjust, ACK and NAK are counted either way; off, they make no row and no event.
static inline int32_t sdrkit_rfid_uhf_get_list_unchecked(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "list_unchecked", value);
}
static inline int32_t sdrkit_rfid_uhf_set_list_unchecked(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "list_unchecked", value);
}

// The setting "gen_tari_us" (Tari) of the test signal generator of the stack "rfid_uhf". Unit: us.
static inline int32_t sdrkit_rfid_uhf_gen_get_tari_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tari_us", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_tari_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tari_us", value);
}

// The setting "gen_data1_tari" (Data-1) of the test signal generator of the stack "rfid_uhf". Unit: Tari.
static inline int32_t sdrkit_rfid_uhf_gen_get_data1_tari(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_data1_tari", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_data1_tari(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_data1_tari", value);
}

// The setting "gen_pw_tari" (Low pulse (PW)) of the test signal generator of the stack "rfid_uhf". Unit: Tari.
static inline int32_t sdrkit_rfid_uhf_gen_get_pw_tari(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pw_tari", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_pw_tari(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pw_tari", value);
}

// The setting "gen_depth" (Modulation depth) of the test signal generator of the stack "rfid_uhf".
static inline int32_t sdrkit_rfid_uhf_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// The setting "gen_trcal_rtcal" (TRcal) of the test signal generator of the stack "rfid_uhf". Unit: RTcal.
static inline int32_t sdrkit_rfid_uhf_gen_get_trcal_rtcal(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_trcal_rtcal", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_trcal_rtcal(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_trcal_rtcal", value);
}

// The setting "gen_reader_clock_ppm" (Reader clock error) of the test signal generator of the stack "rfid_uhf". Unit: ppm. Every time of the reader runs this much fast (positive) or slow.
static inline int32_t sdrkit_rfid_uhf_gen_get_reader_clock_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_reader_clock_ppm", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_reader_clock_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_reader_clock_ppm", value);
}

// The setting "gen_gap_s" (Gap between commands) of the test signal generator of the stack "rfid_uhf". Unit: s.
static inline int32_t sdrkit_rfid_uhf_gen_get_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_s", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value);
}

// The setting "gen_lead_s" (Lead) of the test signal generator of the stack "rfid_uhf". Unit: s.
static inline int32_t sdrkit_rfid_uhf_gen_get_lead_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lead_s", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_lead_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lead_s", value);
}

// The setting "gen_commands" (Commands) of the test signal generator of the stack "rfid_uhf". Commands separated by ';': a name (query, queryrep, queryadjust, ack, nak, select, reqrn, read, write, kill, lock, access) and key=value fields; rn, data, half and payload are hex, updn and mask are bits, flip=N sends bit N inverted.
static inline int32_t sdrkit_rfid_uhf_gen_get_commands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_commands", value);
}
static inline int32_t sdrkit_rfid_uhf_gen_set_commands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_commands", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tags_status_fields_read(SDRKIT_HANDLE handle, sdrkit_tags_status_fields *fields) {
  return sdrkit_typed_fields(handle, "tags.TagsStatusFields", fields, sizeof(sdrkit_tags_status_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_rfid_lf_fields_read(SDRKIT_HANDLE handle, sdrkit_rfid_lf_fields *fields) {
  return sdrkit_typed_fields(handle, "tags.RfidLfFields", fields, sizeof(sdrkit_rfid_lf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tags_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_tags_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "tags.DecoderFields", fields, sizeof(sdrkit_tags_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_qi_fields_read(SDRKIT_HANDLE handle, sdrkit_qi_fields *fields) {
  return sdrkit_typed_fields(handle, "tags.QiFields", fields, sizeof(sdrkit_qi_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_rfid_uhf_fields_read(SDRKIT_HANDLE handle, sdrkit_rfid_uhf_fields *fields) {
  return sdrkit_typed_fields(handle, "tags.RfidUhfFields", fields, sizeof(sdrkit_rfid_uhf_fields));
}

#endif
