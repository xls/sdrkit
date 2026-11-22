/*
 * sdrkit: the typed interface of the stack family "dvoice", version 2.1. MIT licence.
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

#ifndef SDRKIT_DVOICE_H
#define SDRKIT_DVOICE_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_DVOICE_VERSION_MAJOR 2
#define SDRKIT_DVOICE_VERSION_MINOR 1

// The stack "dmr": its id, and the keys of its event types.
#define SDRKIT_STACK_DMR "dmr"
#define SDRKIT_EVENT_DVOICE_CALL "dvoice.call"
#define SDRKIT_EVENT_DVOICE_MESSAGE "dvoice.message"
#define SDRKIT_EVENT_DVOICE_POSITION "dvoice.position"

// The stack "p25": its id, and the keys of its event types.
#define SDRKIT_STACK_P25 "p25"

// The stack "m17": its id, and the keys of its event types.
#define SDRKIT_STACK_M17 "m17"

// The stack "dstar": its id, and the keys of its event types.
#define SDRKIT_STACK_DSTAR "dstar"

// The stack "nxdn": its id, and the keys of its event types.
#define SDRKIT_STACK_NXDN "nxdn"

// The stack "dpmr": its id, and the keys of its event types.
#define SDRKIT_STACK_DPMR "dpmr"

// The stack "ysf": its id, and the keys of its event types.
#define SDRKIT_STACK_YSF "ysf"

// The stack "p25p2": its id, and the keys of its event types.
#define SDRKIT_STACK_P25P2 "p25p2"

// The stack "tetrapol": its id, and the keys of its event types.
#define SDRKIT_STACK_TETRAPOL "tetrapol"

// The stack "tetra": its id, and the keys of its event types.
#define SDRKIT_STACK_TETRA "tetra"

// The stack "tetra_dmo": its id, and the keys of its event types.
#define SDRKIT_STACK_TETRA_DMO "tetra_dmo"

// The stack "freedv": its id, and the keys of its event types.
#define SDRKIT_STACK_FREEDV "freedv"

// The stack "dvoice-auto": its id, and the keys of its event types.
#define SDRKIT_STACK_DVOICE_AUTO "dvoice-auto"

// The stack "dcr": its id, and the keys of its event types.
#define SDRKIT_STACK_DCR "dcr"

// The options of the setting "slot" of the stack "dmr".
typedef enum sdrkit_dmr_slot {
  // Slot 1
  SDRKIT_DMR_SLOT_SLOT_1 = 0,
  // Slot 2
  SDRKIT_DMR_SLOT_SLOT_2 = 1,
} sdrkit_dmr_slot;

// The options of the setting "voice_mode" of the stack "m17".
typedef enum sdrkit_m17_voice_mode {
  // Voice, 3200 bit/s
  SDRKIT_M17_VOICE_MODE_VOICE_3200_BIT_S = 0,
} sdrkit_m17_voice_mode;

// The options of the setting "gen_voice" of the stack "ysf".
typedef enum sdrkit_ysf_gen_voice {
  // V/D mode 1
  SDRKIT_YSF_GEN_VOICE_VD1 = 0,
  // V/D mode 2
  SDRKIT_YSF_GEN_VOICE_VD2 = 1,
  // Voice full rate
  SDRKIT_YSF_GEN_VOICE_VOICE_FR = 2,
} sdrkit_ysf_gen_voice;

// The options of the setting "gen_content" of the stack "dcr".
typedef enum sdrkit_dcr_gen_content {
  // User code test 1
  SDRKIT_DCR_GEN_CONTENT_USER_CODE_1 = 0,
  // User code test 2
  SDRKIT_DCR_GEN_CONTENT_USER_CODE_2 = 1,
  // User code test 3
  SDRKIT_DCR_GEN_CONTENT_USER_CODE_3 = 2,
  // Private communication test
  SDRKIT_DCR_GEN_CONTENT_PRIVACY = 3,
  // Eight calls
  SDRKIT_DCR_GEN_CONTENT_SWEEP = 4,
  // Messages and data
  SDRKIT_DCR_GEN_CONTENT_MESSAGES = 5,
} sdrkit_dcr_gen_content;

// The options of `verdict` of `sdrkit_dvoice_shared_fields`.
typedef enum sdrkit_dvoice_shared_fields_verdict {
  // No signal
  SDRKIT_DVOICE_SHARED_FIELDS_VERDICT_NO_INPUT = 0,
  // No frame sync
  SDRKIT_DVOICE_SHARED_FIELDS_VERDICT_NO_SYNC = 1,
  // Identity fails
  SDRKIT_DVOICE_SHARED_FIELDS_VERDICT_IDENTITY = 2,
  // Error check fails
  SDRKIT_DVOICE_SHARED_FIELDS_VERDICT_FEC = 3,
  // Marginal
  SDRKIT_DVOICE_SHARED_FIELDS_VERDICT_MARGINAL = 4,
  // Decoding
  SDRKIT_DVOICE_SHARED_FIELDS_VERDICT_DECODING = 5,
} sdrkit_dvoice_shared_fields_verdict;

// The measurements that every digital voice stack publishes.
typedef struct sdrkit_dvoice_shared_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The channel level. Unit: dBFS.
  double level_dbfs;
  // The noise floor. Unit: dBFS.
  double noise_dbfs;
  // True if `cnr_db` holds a value.
  bool has_cnr_db;
  // The carrier to noise ratio of the channel. Unit: dB.
  double cnr_db;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio on the symbols. Unit: dB.
  double snr_db;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The measured deviation. A frequency in Hz.
  double deviation_hz;
  // The symbol clock error. Unit: ppm.
  double clock_ppm;
  // The bit error rate.
  double ber;
  // The signal is inverted.
  bool inverted;
  // Blocks that passed their error check.
  int64_t blocks_ok;
  // Blocks that failed their error check.
  int64_t blocks_bad;
  // Frames whose identity field was read.
  int64_t frames;
  // Frame syncs found.
  int64_t syncs;
  // Reports written.
  int64_t reports;
  // Why the stack decodes or does not. A value of `sdrkit_dvoice_shared_fields_verdict`.
  int32_t verdict;
} sdrkit_dvoice_shared_fields;

// The options of `link` of `sdrkit_dmr_fields`.
typedef enum sdrkit_dmr_fields_link {
  // base station
  SDRKIT_DMR_FIELDS_LINK_BASE_STATION = 0,
  // mobile
  SDRKIT_DMR_FIELDS_LINK_MOBILE = 1,
  // direct mode
  SDRKIT_DMR_FIELDS_LINK_DIRECT_MODE = 2,
} sdrkit_dmr_fields_link;

// The options of `cach_activity_1` of `sdrkit_dmr_fields`.
typedef enum sdrkit_dmr_fields_cach_activity_1 {
  // none
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_NONE = 0,
  // group CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_GROUP_CSBK = 1,
  // private CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_PRIVATE_CSBK = 2,
  // group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_GROUP_VOICE = 3,
  // private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_PRIVATE_VOICE = 4,
  // private data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_PRIVATE_DATA = 5,
  // group data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_GROUP_DATA = 6,
  // emergency group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_EMERGENCY_GROUP_VOICE = 7,
  // emergency private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_EMERGENCY_PRIVATE_VOICE = 8,
  // other
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_1_OTHER = 9,
} sdrkit_dmr_fields_cach_activity_1;

// The options of `cach_activity_2` of `sdrkit_dmr_fields`.
typedef enum sdrkit_dmr_fields_cach_activity_2 {
  // none
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_NONE = 0,
  // group CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_GROUP_CSBK = 1,
  // private CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_PRIVATE_CSBK = 2,
  // group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_GROUP_VOICE = 3,
  // private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_PRIVATE_VOICE = 4,
  // private data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_PRIVATE_DATA = 5,
  // group data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_GROUP_DATA = 6,
  // emergency group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_EMERGENCY_GROUP_VOICE = 7,
  // emergency private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_EMERGENCY_PRIVATE_VOICE = 8,
  // other
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_2_OTHER = 9,
} sdrkit_dmr_fields_cach_activity_2;

// The options of `cach_activity_last_1` of `sdrkit_dmr_fields`.
typedef enum sdrkit_dmr_fields_cach_activity_last_1 {
  // none
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_NONE = 0,
  // group CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_GROUP_CSBK = 1,
  // private CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_PRIVATE_CSBK = 2,
  // group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_GROUP_VOICE = 3,
  // private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_PRIVATE_VOICE = 4,
  // private data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_PRIVATE_DATA = 5,
  // group data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_GROUP_DATA = 6,
  // emergency group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_EMERGENCY_GROUP_VOICE = 7,
  // emergency private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_EMERGENCY_PRIVATE_VOICE = 8,
  // other
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_1_OTHER = 9,
} sdrkit_dmr_fields_cach_activity_last_1;

// The options of `cach_activity_last_2` of `sdrkit_dmr_fields`.
typedef enum sdrkit_dmr_fields_cach_activity_last_2 {
  // none
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_NONE = 0,
  // group CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_GROUP_CSBK = 1,
  // private CSBK
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_PRIVATE_CSBK = 2,
  // group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_GROUP_VOICE = 3,
  // private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_PRIVATE_VOICE = 4,
  // private data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_PRIVATE_DATA = 5,
  // group data
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_GROUP_DATA = 6,
  // emergency group voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_EMERGENCY_GROUP_VOICE = 7,
  // emergency private voice
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_EMERGENCY_PRIVATE_VOICE = 8,
  // other
  SDRKIT_DMR_FIELDS_CACH_ACTIVITY_LAST_2_OTHER = 9,
} sdrkit_dmr_fields_cach_activity_last_2;

// The result fields of DMR.
typedef struct sdrkit_dmr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `colour_code` holds a value.
  bool has_colour_code;
  // Colour code.
  int64_t colour_code;
  // True if `link` holds a value.
  bool has_link;
  // Link. A value of `sdrkit_dmr_fields_link`.
  int32_t link;
  // Slots locked.
  int64_t slots_locked;
  // True if `cach_activity_1` holds a value.
  bool has_cach_activity_1;
  // Activity, slot 1. A value of `sdrkit_dmr_fields_cach_activity_1`.
  int32_t cach_activity_1;
  // True if `cach_activity_2` holds a value.
  bool has_cach_activity_2;
  // Activity, slot 2. A value of `sdrkit_dmr_fields_cach_activity_2`.
  int32_t cach_activity_2;
  // True if `cach_activity_last_1` holds a value.
  bool has_cach_activity_last_1;
  // Last activity, slot 1. A value of `sdrkit_dmr_fields_cach_activity_last_1`.
  int32_t cach_activity_last_1;
  // True if `cach_activity_last_2` holds a value.
  bool has_cach_activity_last_2;
  // Last activity, slot 2. A value of `sdrkit_dmr_fields_cach_activity_last_2`.
  int32_t cach_activity_last_2;
  // Talker alias, slot 1.
  sdrkit_string talker_alias_1;
  // Talker alias, slot 2.
  sdrkit_string talker_alias_2;
} sdrkit_dmr_fields;

// The result fields of the voice monitor.
typedef struct sdrkit_dvoice_monitor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `playing` holds a value.
  bool has_playing;
  // The call that plays now: a row key of the `calls` table.
  sdrkit_string playing;
  // True if `held` holds a value.
  bool has_held;
  // The target held.
  sdrkit_string held;
  // True if `codec` holds a value.
  bool has_codec;
  // What the codec plug-ins say: a codec that is not installed or was turned off.
  sdrkit_string codec;
} sdrkit_dvoice_monitor_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_dvoice_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_dvoice_decoder_fields;

// The options of `tx_kind` of `sdrkit_dmr_tx_fields`.
typedef enum sdrkit_dmr_tx_fields_tx_kind {
  // Text
  SDRKIT_DMR_TX_FIELDS_TX_KIND_TEXT = 0,
  // Voice
  SDRKIT_DMR_TX_FIELDS_TX_KIND_VOICE = 1,
} sdrkit_dmr_tx_fields_tx_kind;

// The result fields of the DMR sender.
typedef struct sdrkit_dmr_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_kind` holds a value.
  bool has_tx_kind;
  // What the transmission sends, or the last one sent. A value of `sdrkit_dmr_tx_fields_tx_kind`.
  int32_t tx_kind;
  // The bursts or frames of the transmission, or of the last one, that are written.
  int64_t tx_frames;
  // What the sender sends.
  sdrkit_string tx_note;
} sdrkit_dmr_tx_fields;

// The options of `kind` of `sdrkit_dvoice_call_row`.
typedef enum sdrkit_dvoice_call_row_kind {
  // Voice
  SDRKIT_DVOICE_CALL_ROW_KIND_VOICE = 0,
  // Group voice
  SDRKIT_DVOICE_CALL_ROW_KIND_GROUP_VOICE = 1,
  // Private voice
  SDRKIT_DVOICE_CALL_ROW_KIND_PRIVATE_VOICE = 2,
  // All call
  SDRKIT_DVOICE_CALL_ROW_KIND_ALL_CALL = 3,
  // Data
  SDRKIT_DVOICE_CALL_ROW_KIND_DATA = 4,
  // Packet
  SDRKIT_DVOICE_CALL_ROW_KIND_PACKET = 5,
  // Status
  SDRKIT_DVOICE_CALL_ROW_KIND_STATUS = 6,
  // Short message
  SDRKIT_DVOICE_CALL_ROW_KIND_SHORT_MESSAGE = 7,
  // Short data
  SDRKIT_DVOICE_CALL_ROW_KIND_SHORT_DATA = 8,
  // Control
  SDRKIT_DVOICE_CALL_ROW_KIND_CONTROL = 9,
  // Grant
  SDRKIT_DVOICE_CALL_ROW_KIND_GRANT = 10,
} sdrkit_dvoice_call_row_kind;

// The options of `access_kind` of `sdrkit_dvoice_call_row`.
typedef enum sdrkit_dvoice_call_row_access_kind {
  // Colour code
  SDRKIT_DVOICE_CALL_ROW_ACCESS_KIND_COLOUR_CODE = 0,
  // NAC
  SDRKIT_DVOICE_CALL_ROW_ACCESS_KIND_NAC = 1,
  // RAN
  SDRKIT_DVOICE_CALL_ROW_ACCESS_KIND_RAN = 2,
  // CAN
  SDRKIT_DVOICE_CALL_ROW_ACCESS_KIND_CAN = 3,
  // Squelch code
  SDRKIT_DVOICE_CALL_ROW_ACCESS_KIND_SQUELCH_CODE = 4,
} sdrkit_dvoice_call_row_access_kind;

// The options of `encryption` of `sdrkit_dvoice_call_row`.
typedef enum sdrkit_dvoice_call_row_encryption {
  // Unknown
  SDRKIT_DVOICE_CALL_ROW_ENCRYPTION_UNKNOWN = 0,
  // Clear
  SDRKIT_DVOICE_CALL_ROW_ENCRYPTION_CLEAR = 1,
  // Encrypted
  SDRKIT_DVOICE_CALL_ROW_ENCRYPTION_ENCRYPTED = 2,
  // Decrypted
  SDRKIT_DVOICE_CALL_ROW_ENCRYPTION_DECRYPTED = 3,
} sdrkit_dvoice_call_row_encryption;

// The options of `end` of `sdrkit_dvoice_call_row`.
typedef enum sdrkit_dvoice_call_row_end {
  // On air
  SDRKIT_DVOICE_CALL_ROW_END_ACTIVE = 0,
  // Terminator
  SDRKIT_DVOICE_CALL_ROW_END_TERMINATOR = 1,
  // Hang time
  SDRKIT_DVOICE_CALL_ROW_END_HANG_TIME = 2,
  // Replaced
  SDRKIT_DVOICE_CALL_ROW_END_REPLACED = 3,
  // Closed
  SDRKIT_DVOICE_CALL_ROW_END_CLOSED = 4,
  // Single
  SDRKIT_DVOICE_CALL_ROW_END_SINGLE = 5,
} sdrkit_dvoice_call_row_end;

// A row of the call log. The row key is the call id.
typedef struct sdrkit_dvoice_call_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The id of a call.
  int64_t id;
  // Signal time of the start of a call, seconds. Unit: s.
  double start_s;
  // How long a call lasted. A length of time in seconds.
  double duration;
  // The time slot, 0 when the stack has none.
  int64_t slot;
  // What a transmission carries. A value of `sdrkit_dvoice_call_row_kind`.
  int32_t kind;
  // Who sends.
  sdrkit_string src;
  // Who is called.
  sdrkit_string dst;
  // The target is a group.
  bool group;
  // An emergency call.
  bool emergency;
  // True if `access_kind` holds a value.
  bool has_access_kind;
  // Which access code the call carried. A value of `sdrkit_dvoice_call_row_access_kind`.
  int32_t access_kind;
  // True if `access_code` holds a value.
  bool has_access_code;
  // The value of the access code.
  int64_t access_code;
  // Encrypted or not. A value of `sdrkit_dvoice_call_row_encryption`.
  int32_t encryption;
  // The name of the algorithm.
  sdrkit_string algorithm;
  // True if `alg_id` holds a value.
  bool has_alg_id;
  // The algorithm id as sent.
  int64_t alg_id;
  // True if `enc_key_id` holds a value.
  bool has_enc_key_id;
  // The key id.
  int64_t enc_key_id;
  // The talker alias.
  sdrkit_string alias;
  // Text carried by a transmission.
  sdrkit_string text;
  // Voice frames received.
  int64_t voice_frames;
  // Voice frames passed to the media output.
  int64_t media_frames;
  // The bit error rate of a call.
  double ber;
  // The call is on air.
  bool active;
  // The call started from an identity inside it.
  bool late_entry;
  // The call ended by its terminator.
  bool end_seen;
  // The call ended after the hang time.
  bool signal_lost;
  // The carrier frequency. A frequency in Hz.
  double carrier_hz;
  // Where a call is in its life. A value of `sdrkit_dvoice_call_row_end`.
  int32_t end;
} sdrkit_dvoice_call_row;

// The options of `detail` of `sdrkit_dvoice_call_detail_row`.
typedef enum sdrkit_dvoice_call_detail_row_detail {
  // Symbol rate
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_BAUD = 0,
  // Call type
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_CALL_TYPE = 1,
  // Communication mode
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_COMM_MODE = 2,
  // Status
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_STATUS = 3,
  // Channel
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_CHANNEL = 4,
  // Frequency
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_FREQ_HZ = 5,
  // SAP
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_SAP = 6,
  // Blocks
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_BLOCKS = 7,
  // MY
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_MY = 8,
  // Suffix
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_SUFFIX = 9,
  // UR
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_UR = 10,
  // RPT1
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_RPT1 = 11,
  // RPT2
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_RPT2 = 12,
  // Flag 1
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_FLAG1 = 13,
  // Flags
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_FLAGS = 14,
  // Header good
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_HEADER_OK = 15,
  // Header from slow data
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_HEADER_FROM_SLOW_DATA = 16,
  // Squelch code
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_SQUELCH = 17,
  // Position
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_POSITION = 18,
  // Stream type
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_STREAM_TYPE = 19,
  // Encryption type
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_ENCRYPTION_TYPE = 20,
  // Link setup from LICH
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_LSF_FROM_LICH = 21,
  // Signed
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_SIGNED = 22,
  // Data
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_DATA = 23,
  // Frames
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_FRAMES = 24,
  // Protocol
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_PROTOCOL = 25,
  // Feature set
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_FID = 26,
  // Service options
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_SERVICE_OPTIONS = 27,
  // Note
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_NOTE = 28,
  // Opcode
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_OPCODE = 29,
  // Format
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_FORMAT = 30,
  // CRC good
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_CRC_OK = 31,
  // Packet format
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_DPF = 32,
  // Downlink
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_DOWNLINK = 33,
  // Uplink
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_UPLINK = 34,
  // Data type
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_DATA_TYPE = 35,
  // Call identifier
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_CALL_ID = 36,
  // Address
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_ADDRESS = 37,
  // MCC
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_MCC = 38,
  // MNC
  SDRKIT_DVOICE_CALL_DETAIL_ROW_DETAIL_MNC = 39,
} sdrkit_dvoice_call_detail_row_detail;

// A fact of a call that only some stacks send. The row key is the call id, a slash and the detail key.
typedef struct sdrkit_dvoice_call_detail_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The call a row belongs to: a row key of the `calls` table.
  sdrkit_string call;
  // Which fact a detail row holds. A value of `sdrkit_dvoice_call_detail_row_detail`.
  int32_t detail;
  // The value of a detail.
  sdrkit_variant value;
} sdrkit_dvoice_call_detail_row;

// The options of `format` of `sdrkit_dvoice_message_row`.
typedef enum sdrkit_dvoice_message_row_format {
  // Short data
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_SHORT_DATA = 0,
  // Packet data
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_PACKET_DATA = 1,
  // Slow data
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_SLOW_DATA = 2,
  // SMS
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_SMS = 3,
  // META text
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_META_TEXT = 4,
  // FreeDV text
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_FREEDV_TEXT = 5,
  // System Fusion text
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_YSF_TEXT = 6,
  // Short message
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_SHORT_MESSAGE = 7,
  // Packet
  SDRKIT_DVOICE_MESSAGE_ROW_FORMAT_PACKET = 8,
} sdrkit_dvoice_message_row_format;

// A text message. The row key counts the messages.
typedef struct sdrkit_dvoice_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time, seconds. Unit: s.
  double time_s;
  // Who sends.
  sdrkit_string src;
  // Who is called.
  sdrkit_string dst;
  // The target is a group.
  bool group;
  // How a message was sent. A value of `sdrkit_dvoice_message_row_format`.
  int32_t format;
  // True if `data_format` holds a value.
  bool has_data_format;
  // The DMR short data format code.
  int64_t data_format;
  // Text carried by a transmission.
  sdrkit_string text;
  // The error check was good.
  bool crc_ok;
  // The time slot, 0 when the stack has none.
  int64_t slot;
  // An SMS.
  bool sms;
} sdrkit_dvoice_message_row;

// The options of `format` of `sdrkit_dvoice_position_row`.
typedef enum sdrkit_dvoice_position_row_format {
  // LRRP
  SDRKIT_DVOICE_POSITION_ROW_FORMAT_LRRP = 0,
  // NMEA
  SDRKIT_DVOICE_POSITION_ROW_FORMAT_NMEA = 1,
  // D-PRS
  SDRKIT_DVOICE_POSITION_ROW_FORMAT_DPRS = 2,
  // M17 GNSS
  SDRKIT_DVOICE_POSITION_ROW_FORMAT_M17_GNSS = 3,
  // System Fusion GPS
  SDRKIT_DVOICE_POSITION_ROW_FORMAT_YSF_GPS = 4,
  // DMR GPS
  SDRKIT_DVOICE_POSITION_ROW_FORMAT_DMR_GPS = 5,
} sdrkit_dvoice_position_row_format;

// The options of `station_type` of `sdrkit_dvoice_position_row`.
typedef enum sdrkit_dvoice_position_row_station_type {
  // Fixed
  SDRKIT_DVOICE_POSITION_ROW_STATION_TYPE_FIXED = 0,
  // Mobile
  SDRKIT_DVOICE_POSITION_ROW_STATION_TYPE_MOBILE = 1,
  // Handheld
  SDRKIT_DVOICE_POSITION_ROW_STATION_TYPE_HANDHELD = 2,
  // Other
  SDRKIT_DVOICE_POSITION_ROW_STATION_TYPE_OTHER = 3,
} sdrkit_dvoice_position_row_station_type;

// A position. The row key counts the positions.
typedef struct sdrkit_dvoice_position_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time, seconds. Unit: s.
  double time_s;
  // Who sends.
  sdrkit_string src;
  // The position, latitude first.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height. Unit: m.
  double height_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course. Unit: °.
  double course_deg;
  // True if `accuracy_m` holds a value.
  bool has_accuracy_m;
  // The accuracy of a position. Unit: m.
  double accuracy_m;
  // How a position was sent. A value of `sdrkit_dvoice_position_row_format`.
  int32_t format;
  // The comment sent with a position.
  sdrkit_string comment;
  // True if `station_type` holds a value.
  bool has_station_type;
  // The kind of station that sent a position. A value of `sdrkit_dvoice_position_row_station_type`.
  int32_t station_type;
  // True if `data_source` holds a value.
  bool has_data_source;
  // The data source code of a position.
  int64_t data_source;
} sdrkit_dvoice_position_row;

// The site and system identity of the stack. The row key is the stack id.
typedef struct sdrkit_dvoice_site_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time, seconds. Unit: s.
  double time_s;
  // The identity is complete.
  bool valid;
  // True if `system` holds a value.
  bool has_system;
  // The system id.
  int64_t system;
  // True if `wacn` holds a value.
  bool has_wacn;
  // The wide area communications network.
  int64_t wacn;
  // True if `rfss` holds a value.
  bool has_rfss;
  // The RF subsystem.
  int64_t rfss;
  // True if `site` holds a value.
  bool has_site;
  // The site.
  int64_t site;
  // True if `nac` holds a value.
  bool has_nac;
  // The network access code.
  int64_t nac;
  // True if `control_channel` holds a value.
  bool has_control_channel;
  // The control channel.
  sdrkit_string control_channel;
  // True if `location` holds a value.
  bool has_location;
  // The location id.
  int64_t location;
  // True if `ran` holds a value.
  bool has_ran;
  // The radio access number.
  int64_t ran;
  // True if `country` holds a value.
  bool has_country;
  // The country code.
  int64_t country;
  // True if `network` holds a value.
  bool has_network;
  // The network code.
  int64_t network;
  // True if `base_station` holds a value.
  bool has_base_station;
  // The base station.
  int64_t base_station;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel of a site.
  int64_t channel;
  // True if `colour_code` holds a value.
  bool has_colour_code;
  // The colour code of a cell.
  int64_t colour_code;
  // True if `location_area` holds a value.
  bool has_location_area;
  // The location area of a cell.
  int64_t location_area;
  // True if `main_carrier` holds a value.
  bool has_main_carrier;
  // The main carrier number of a cell.
  int64_t main_carrier;
  // True if `band` holds a value.
  bool has_band;
  // The frequency band of a cell.
  int64_t band;
  // True if `main_carrier_hz` holds a value.
  bool has_main_carrier_hz;
  // The main carrier frequency of a cell. A frequency in Hz.
  double main_carrier_hz;
} sdrkit_dvoice_site_row;

// A channel grant. The row key counts the grants.
typedef struct sdrkit_dvoice_grant_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time, seconds. Unit: s.
  double time_s;
  // The talkgroup or radio granted.
  int64_t target;
  // The target is a group.
  bool group;
  // The source of a grant request.
  int64_t source_id;
  // The channel number of a grant as sent.
  int64_t channel_number;
  // The channel of a grant for people.
  sdrkit_string channel;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency of a grant. A frequency in Hz.
  double freq_hz;
  // True if `slot` holds a value.
  bool has_slot;
  // The time slot, 0 when the stack has none.
  int64_t slot;
  // An emergency call.
  bool emergency;
  // The call is encrypted.
  bool encrypted;
  // The priority of a grant.
  int64_t priority;
  // An update of a running call.
  bool update;
} sdrkit_dvoice_grant_row;

// A talkgroup heard, with its counters. The row key is the talkgroup.
typedef struct sdrkit_dvoice_talkgroup_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A talkgroup.
  int64_t talkgroup;
  // Grants to a talkgroup.
  int64_t grants;
  // Voice calls to a talkgroup.
  int64_t calls;
  // The call is encrypted.
  bool encrypted;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_dvoice_talkgroup_row;

// The options of `verdict` of `sdrkit_dvoice_history_row`.
typedef enum sdrkit_dvoice_history_row_verdict {
  // Play
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_PLAY = 0,
  // another call is playing
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_BUSY = 1,
  // Lower priority
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_LOWER_PRIORITY = 2,
  // Muted
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_MUTED = 3,
  // Filtered
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_FILTERED = 4,
  // Not held
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_NOT_HELD = 5,
  // Squelched
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_SQUELCHED = 6,
  // never decoded
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_ENCRYPTED = 7,
  // No codec
  SDRKIT_DVOICE_HISTORY_ROW_VERDICT_NO_CODEC = 8,
} sdrkit_dvoice_history_row_verdict;

// A call the monitor heard. The row key is the call id.
typedef struct sdrkit_dvoice_history_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The call a row belongs to: a row key of the `calls` table.
  sdrkit_string call;
  // Signal time of the start of a call, seconds. Unit: s.
  double start_s;
  // Who sends.
  sdrkit_string src;
  // Who is called.
  sdrkit_string dst;
  // Who is called and its name.
  sdrkit_string label;
  // Why a call is or is not heard. A value of `sdrkit_dvoice_history_row_verdict`.
  int32_t verdict;
  // Frames played.
  int64_t played_frames;
  // The call is encrypted.
  bool encrypted;
  // The call is on air.
  bool active;
} sdrkit_dvoice_history_row;

// A source radio or callsign. The row key is the source.
typedef struct sdrkit_dvoice_heard_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Who sends.
  sdrkit_string src;
  // Calls heard.
  int64_t calls;
  // Time on air. A length of time in seconds.
  double airtime;
  // Signal time of the end of the last call, seconds. Unit: s.
  double last_s;
  // The mean bit error rate.
  double mean_ber;
  // An emergency call.
  bool emergency;
} sdrkit_dvoice_heard_row;

// The result fields of P25 phase 1.
typedef struct sdrkit_p25_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `nac` holds a value.
  bool has_nac;
  // The network access code heard.
  int64_t nac;
  // Frames.
  int64_t p25_frames;
} sdrkit_p25_fields;

// A channel identifier. The row key is the identifier.
typedef struct sdrkit_dvoice_identifier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Identifier.
  int64_t id;
  // VHF or UHF.
  bool vhf_uhf;
  // Base. A frequency in Hz.
  double base_hz;
  // Spacing. A frequency in Hz.
  double spacing_hz;
  // Transmit offset. A frequency in Hz.
  double tx_offset_hz;
  // Bandwidth. A frequency in Hz.
  double bandwidth_hz;
} sdrkit_dvoice_identifier_row;

// An adjacent site. The row key is the system, the RFSS and the site.
typedef struct sdrkit_dvoice_adjacent_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The system id.
  int64_t system;
  // The RF subsystem.
  int64_t rfss;
  // The site.
  int64_t site;
  // The channel of a grant for people.
  sdrkit_string channel;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency of a grant. A frequency in Hz.
  double freq_hz;
} sdrkit_dvoice_adjacent_row;

// The result fields of M17.
typedef struct sdrkit_m17_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Link setup frames.
  int64_t lsf_ok;
  // Stream frames.
  int64_t stream_frames;
  // Packets.
  int64_t packets_ok;
} sdrkit_m17_fields;

// The options of `tx_kind` of `sdrkit_m17_tx_fields`.
typedef enum sdrkit_m17_tx_fields_tx_kind {
  // Voice
  SDRKIT_M17_TX_FIELDS_TX_KIND_VOICE = 0,
  // Packet
  SDRKIT_M17_TX_FIELDS_TX_KIND_PACKET = 1,
} sdrkit_m17_tx_fields_tx_kind;

// The result fields of the M17 sender.
typedef struct sdrkit_m17_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_kind` holds a value.
  bool has_tx_kind;
  // What the transmission sends, or the last one sent. A value of `sdrkit_m17_tx_fields_tx_kind`.
  int32_t tx_kind;
  // The stream or packet frames of the transmission, or of the last one, that are written.
  int64_t tx_frames;
} sdrkit_m17_tx_fields;

// The result fields of D-STAR.
typedef struct sdrkit_dstar_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Headers.
  int64_t headers_ok;
  // Voice frames.
  int64_t dstar_voice_frames;
  // Inverted sync.
  bool sync_inverted;
} sdrkit_dstar_fields;

// The options of `tx_kind` of `sdrkit_dstar_tx_fields`.
typedef enum sdrkit_dstar_tx_fields_tx_kind {
  // Text
  SDRKIT_DSTAR_TX_FIELDS_TX_KIND_TEXT = 0,
} sdrkit_dstar_tx_fields_tx_kind;

// The result fields of the D-STAR sender.
typedef struct sdrkit_dstar_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_kind` holds a value.
  bool has_tx_kind;
  // What the transmission sends, or the last one sent. A value of `sdrkit_dstar_tx_fields_tx_kind`.
  int32_t tx_kind;
  // The voice frames of the transmission, or of the last one, that are written.
  int64_t tx_frames;
  // What the sender puts in the voice frames.
  sdrkit_string tx_note;
} sdrkit_dstar_tx_fields;

// The result fields of NXDN.
typedef struct sdrkit_nxdn_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `ran` holds a value.
  bool has_ran;
  // RAN.
  int64_t ran;
  // Frames.
  int64_t nxdn_frames;
  // CRC good.
  int64_t crc_ok;
  // CRC bad.
  int64_t crc_bad;
} sdrkit_nxdn_fields;

// The result fields of dPMR.
typedef struct sdrkit_dpmr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `colour_code` holds a value.
  bool has_colour_code;
  // Colour code.
  int64_t colour_code;
} sdrkit_dpmr_fields;

// The result fields of System Fusion.
typedef struct sdrkit_ysf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Data type.
  sdrkit_string data_type;
} sdrkit_ysf_fields;

// The result fields of P25 phase 2.
typedef struct sdrkit_p25p2_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Superframe location.
  int64_t location;
} sdrkit_p25p2_fields;

// The result fields of TETRAPOL.
typedef struct sdrkit_tetrapol_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Voice frames.
  int64_t tetrapol_voice_frames;
  // Signalling frames whose CRC checks.
  int64_t tetrapol_signalling_frames;
} sdrkit_tetrapol_fields;

// The result fields of TETRA.
typedef struct sdrkit_tetra_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts.
  int64_t bursts;
} sdrkit_tetra_fields;

// The result fields of TETRA DMO.
typedef struct sdrkit_tetra_dmo_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Synchronisation bursts.
  int64_t dsb;
  // Normal bursts.
  int64_t dnb;
  // SCH/F and STCH good.
  int64_t data_blocks;
  // SCH/S and SCH/H bad.
  int64_t sync_blocks_bad;
  // Traffic bursts.
  int64_t traffic_bursts;
  // Traffic refused.
  int64_t traffic_refused;
  // True if `comm_type` holds a value.
  bool has_comm_type;
  // Communication type.
  int64_t comm_type;
  // True if `frame` holds a value.
  bool has_frame;
  // Frame.
  int64_t frame;
  // True if `slot` holds a value.
  bool has_slot;
  // Slot.
  int64_t slot;
  // True if `aie_state` holds a value.
  bool has_aie_state;
  // Encryption state.
  int64_t aie_state;
  // True if `last_source` holds a value.
  bool has_last_source;
  // Last source.
  sdrkit_string last_source;
  // True if `last_destination` holds a value.
  bool has_last_destination;
  // Last destination.
  sdrkit_string last_destination;
  // True if `mni` holds a value.
  bool has_mni;
  // MNI.
  sdrkit_string mni;
  // Note.
  sdrkit_string note;
} sdrkit_tetra_dmo_fields;

// The result fields of FreeDV.
typedef struct sdrkit_freedv_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The active waveform: "700D", "700E", "1600", or empty when none is.
  sdrkit_string submode;
  // The sync state of the 700D waveform: "search", "trial" or "synced"; empty when it does not run.
  sdrkit_string sync_700d;
  // The sync state of the 700E waveform: "search", "trial" or "synced"; empty when it does not run.
  sdrkit_string sync_700e;
  // The sync state of the 1600 waveform: "search" or "synced"; empty when it does not run.
  sdrkit_string sync_1600;
  // The active waveform held sync at 3 dB or more for 1.5 s.
  bool confirmed;
  // The frequency estimate of the active OFDM waveform, relative to the 1500 Hz centre. Unit: Hz.
  double freq_offset_hz;
  // The sample clock offset of the active OFDM waveform. Unit: ppm.
  double clock_offset_ppm;
  // The last timing metric of the active OFDM waveform (0 to about 1).
  double timing_metric;
  // Unique word bit errors of the bits of the unique word, since the call started. Unit: %.
  double uw_ber_pct;
  // Frames whose LDPC decoder did not satisfy every check, since the call started.
  int64_t ldpc_failed_frames;
  // Modem frames decoded with sync.
  int64_t freedv_frames;
  // Codec frames sent to the speech output.
  int64_t speech_frames;
  // The text line in progress, or the last finished line.
  sdrkit_string callsign;
} sdrkit_freedv_fields;

// The options of `tx_kind` of `sdrkit_freedv_tx_fields`.
typedef enum sdrkit_freedv_tx_fields_tx_kind {
  // Voice, 700D
  SDRKIT_FREEDV_TX_FIELDS_TX_KIND_700_D = 0,
  // Voice, 700E
  SDRKIT_FREEDV_TX_FIELDS_TX_KIND_700_E = 1,
  // Voice, 1600
  SDRKIT_FREEDV_TX_FIELDS_TX_KIND_VOICE_1600 = 2,
} sdrkit_freedv_tx_fields_tx_kind;

// The result fields of the FreeDV sender.
typedef struct sdrkit_freedv_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency (the suppressed carrier): the listening frequency plus the transmit offset. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_kind` holds a value.
  bool has_tx_kind;
  // What the transmission sends, or the last one sent. A value of `sdrkit_freedv_tx_fields_tx_kind`.
  int32_t tx_kind;
  // The modem frames of the transmission, or of the last one, that are written.
  int64_t tx_frames;
} sdrkit_freedv_tx_fields;

// The options of `detected` of `sdrkit_dvoice_auto_fields`.
typedef enum sdrkit_dvoice_auto_fields_detected {
  // DMR
  SDRKIT_DVOICE_AUTO_FIELDS_DETECTED_DMR = 0,
  // NXDN
  SDRKIT_DVOICE_AUTO_FIELDS_DETECTED_NXDN = 1,
  // dPMR
  SDRKIT_DVOICE_AUTO_FIELDS_DETECTED_DPMR = 2,
  // P25
  SDRKIT_DVOICE_AUTO_FIELDS_DETECTED_P25 = 3,
  // Yaesu System Fusion
  SDRKIT_DVOICE_AUTO_FIELDS_DETECTED_YSF = 4,
  // M17
  SDRKIT_DVOICE_AUTO_FIELDS_DETECTED_M17 = 5,
} sdrkit_dvoice_auto_fields_detected;

// The result fields of the automatic detector.
typedef struct sdrkit_dvoice_auto_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `detected` holds a value.
  bool has_detected;
  // The stack decoded last. A value of `sdrkit_dvoice_auto_fields_detected`.
  int32_t detected;
} sdrkit_dvoice_auto_fields;

// The options of `stack` of `sdrkit_dvoice_auto_seen_row`.
typedef enum sdrkit_dvoice_auto_seen_row_stack {
  // DMR
  SDRKIT_DVOICE_AUTO_SEEN_ROW_STACK_DMR = 0,
  // NXDN
  SDRKIT_DVOICE_AUTO_SEEN_ROW_STACK_NXDN = 1,
  // dPMR
  SDRKIT_DVOICE_AUTO_SEEN_ROW_STACK_DPMR = 2,
  // P25
  SDRKIT_DVOICE_AUTO_SEEN_ROW_STACK_P25 = 3,
  // Yaesu System Fusion
  SDRKIT_DVOICE_AUTO_SEEN_ROW_STACK_YSF = 4,
  // M17
  SDRKIT_DVOICE_AUTO_SEEN_ROW_STACK_M17 = 5,
} sdrkit_dvoice_auto_seen_row_stack;

// A stack that the detector heard. The row key is the stack id.
typedef struct sdrkit_dvoice_auto_seen_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A stack the detector decoded. A value of `sdrkit_dvoice_auto_seen_row_stack`.
  int32_t stack;
  // Blocks decoded.
  int64_t blocks;
  // Signal time of the last decoded block, seconds. Unit: s.
  double last_s;
} sdrkit_dvoice_auto_seen_row;

// The result fields of DCR.
typedef struct sdrkit_dcr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames read after a sync.
  int64_t dcr_frames;
  // SACCH units that passed their CRC.
  int64_t sacch_ok;
  // SACCH units that failed their CRC.
  int64_t sacch_bad;
  // Frames whose RICH parity failed.
  int64_t rich_bad;
  // Call signs read from a PICH.
  int64_t pich_ok;
  // Data blocks that passed their CRC.
  int64_t data_ok;
  // Data blocks that failed their CRC.
  int64_t data_bad;
  // Voice frames of 20 ms in frames whose SACCH decoded.
  int64_t voice_frames;
  // Calls opened.
  int64_t calls;
  // The bits that the Viterbi decoder repaired, of the coded bits of the blocks that passed their CRC. Unit: %.
  double ber_pct;
  // True if `last_user_code` holds a value.
  bool has_last_user_code;
  // The user code of the last message that counted, 0 to 511.
  int64_t last_user_code;
  // True if `last_call_sign` holds a value.
  bool has_last_call_sign;
  // The last call sign read, 9 digits.
  sdrkit_string last_call_sign;
  // True if `channel` holds a value.
  bool has_channel;
  // The plan channel of the tuned frequency, or "not on the plan". No value without a plan.
  sdrkit_string channel;
  // The plan channels in the span of a scan.
  int64_t scan_channels;
  // The decoders of scanned channels that run.
  int64_t scan_decoders;
  // The decoders of scanned channels that decode.
  int64_t scan_decoding;
  // What the stack decodes of the voice.
  sdrkit_string note;
} sdrkit_dcr_fields;

// The options of `kind` of `sdrkit_dcr_event_row`.
typedef enum sdrkit_dcr_event_row_kind {
  // Call
  SDRKIT_DCR_EVENT_ROW_KIND_CALL = 0,
  // Call end
  SDRKIT_DCR_EVENT_ROW_KIND_CALL_END = 1,
  // Call sign
  SDRKIT_DCR_EVENT_ROW_KIND_CALL_SIGN = 2,
  // Data
  SDRKIT_DCR_EVENT_ROW_KIND_DATA = 3,
  // Manufacturer message
  SDRKIT_DCR_EVENT_ROW_KIND_MANUFACTURER_MESSAGE = 4,
  // Idle
  SDRKIT_DCR_EVENT_ROW_KIND_IDLE = 5,
  // Unknown message
  SDRKIT_DCR_EVENT_ROW_KIND_UNKNOWN_MESSAGE = 6,
} sdrkit_dcr_event_row_kind;

// A row of the DCR list: a call, a call end, a call sign, a data block or a message. The row key counts the rows.
typedef struct sdrkit_dcr_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time, seconds. Unit: s.
  double time_s;
  // What the row reports. A value of `sdrkit_dcr_event_row_kind`.
  int32_t kind;
  // Who: "UC n", a call sign, "TCH1" or "TCH2", after the plan label of a scanned channel.
  sdrkit_string identity;
  // The text.
  sdrkit_string text;
  // True if `channel` holds a value.
  bool has_channel;
  // The plan label of a scanned channel.
  sdrkit_string channel;
} sdrkit_dcr_event_row;

// The options of `urgency` of `sdrkit_dvoice_call_event`.
typedef enum sdrkit_dvoice_call_event_urgency {
  // None
  SDRKIT_DVOICE_CALL_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_DVOICE_CALL_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_DVOICE_CALL_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_DVOICE_CALL_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_dvoice_call_event_urgency;

// The options of `family` of `sdrkit_dvoice_call_event`.
typedef enum sdrkit_dvoice_call_event_family {
  // Aircraft
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DVOICE_CALL_EVENT_FAMILY_OTHER = 12,
} sdrkit_dvoice_call_event_family;

// The options of `kind` of `sdrkit_dvoice_call_event`.
typedef enum sdrkit_dvoice_call_event_kind {
  // Voice
  SDRKIT_DVOICE_CALL_EVENT_KIND_VOICE = 0,
  // Group voice
  SDRKIT_DVOICE_CALL_EVENT_KIND_GROUP_VOICE = 1,
  // Private voice
  SDRKIT_DVOICE_CALL_EVENT_KIND_PRIVATE_VOICE = 2,
  // All call
  SDRKIT_DVOICE_CALL_EVENT_KIND_ALL_CALL = 3,
  // Data
  SDRKIT_DVOICE_CALL_EVENT_KIND_DATA = 4,
  // Packet
  SDRKIT_DVOICE_CALL_EVENT_KIND_PACKET = 5,
  // Status
  SDRKIT_DVOICE_CALL_EVENT_KIND_STATUS = 6,
  // Short message
  SDRKIT_DVOICE_CALL_EVENT_KIND_SHORT_MESSAGE = 7,
  // Short data
  SDRKIT_DVOICE_CALL_EVENT_KIND_SHORT_DATA = 8,
  // Control
  SDRKIT_DVOICE_CALL_EVENT_KIND_CONTROL = 9,
  // Grant
  SDRKIT_DVOICE_CALL_EVENT_KIND_GRANT = 10,
} sdrkit_dvoice_call_event_kind;

// The options of `end` of `sdrkit_dvoice_call_event`.
typedef enum sdrkit_dvoice_call_event_end {
  // On air
  SDRKIT_DVOICE_CALL_EVENT_END_ACTIVE = 0,
  // Terminator
  SDRKIT_DVOICE_CALL_EVENT_END_TERMINATOR = 1,
  // Hang time
  SDRKIT_DVOICE_CALL_EVENT_END_HANG_TIME = 2,
  // Replaced
  SDRKIT_DVOICE_CALL_EVENT_END_REPLACED = 3,
  // Closed
  SDRKIT_DVOICE_CALL_EVENT_END_CLOSED = 4,
  // Single
  SDRKIT_DVOICE_CALL_EVENT_END_SINGLE = 5,
} sdrkit_dvoice_call_event_end;

// A call started, ended, or was a single transmission. The field `end` is `active` when it started.
typedef struct sdrkit_dvoice_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Who sends.
  sdrkit_string source;
  // Who is called.
  sdrkit_string target;
  // True if `channel` holds a value.
  bool has_channel;
  // The talkgroup of a group call, else the time slot.
  sdrkit_string channel;
  // True if `text` holds a value.
  bool has_text;
  // The text.
  sdrkit_string text;
  // How urgent it is. A value of `sdrkit_dvoice_call_event_urgency`.
  int32_t urgency;
  // The family of the event. A value of `sdrkit_dvoice_call_event_family`.
  int32_t family;
  // The content is encrypted.
  bool private_;
  // The id of a call.
  int64_t id;
  // What a transmission carries. A value of `sdrkit_dvoice_call_event_kind`.
  int32_t kind;
  // The target is a group.
  bool group;
  // Where a call is in its life. A value of `sdrkit_dvoice_call_event_end`.
  int32_t end;
  // True if `duration` holds a value.
  bool has_duration;
  // How long a call lasted. A length of time in seconds.
  double duration;
} sdrkit_dvoice_call_event;

// The options of `family` of `sdrkit_dvoice_message_event`.
typedef enum sdrkit_dvoice_message_event_family {
  // Aircraft
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DVOICE_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_dvoice_message_event_family;

// The options of `format` of `sdrkit_dvoice_message_event`.
typedef enum sdrkit_dvoice_message_event_format {
  // Short data
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_SHORT_DATA = 0,
  // Packet data
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_PACKET_DATA = 1,
  // Slow data
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_SLOW_DATA = 2,
  // SMS
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_SMS = 3,
  // META text
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_META_TEXT = 4,
  // FreeDV text
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_FREEDV_TEXT = 5,
  // System Fusion text
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_YSF_TEXT = 6,
  // Short message
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_SHORT_MESSAGE = 7,
  // Packet
  SDRKIT_DVOICE_MESSAGE_EVENT_FORMAT_PACKET = 8,
} sdrkit_dvoice_message_event_format;

// A text message.
typedef struct sdrkit_dvoice_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Who sends.
  sdrkit_string source;
  // Who is called.
  sdrkit_string target;
  // The text.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_dvoice_message_event_family`.
  int32_t family;
  // How a message was sent. A value of `sdrkit_dvoice_message_event_format`.
  int32_t format;
  // The target is a group.
  bool group;
} sdrkit_dvoice_message_event;

// The options of `family` of `sdrkit_dvoice_position_event`.
typedef enum sdrkit_dvoice_position_event_family {
  // Aircraft
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DVOICE_POSITION_EVENT_FAMILY_OTHER = 12,
} sdrkit_dvoice_position_event_family;

// The options of `format` of `sdrkit_dvoice_position_event`.
typedef enum sdrkit_dvoice_position_event_format {
  // LRRP
  SDRKIT_DVOICE_POSITION_EVENT_FORMAT_LRRP = 0,
  // NMEA
  SDRKIT_DVOICE_POSITION_EVENT_FORMAT_NMEA = 1,
  // D-PRS
  SDRKIT_DVOICE_POSITION_EVENT_FORMAT_DPRS = 2,
  // M17 GNSS
  SDRKIT_DVOICE_POSITION_EVENT_FORMAT_M17_GNSS = 3,
  // System Fusion GPS
  SDRKIT_DVOICE_POSITION_EVENT_FORMAT_YSF_GPS = 4,
  // DMR GPS
  SDRKIT_DVOICE_POSITION_EVENT_FORMAT_DMR_GPS = 5,
} sdrkit_dvoice_position_event_format;

// A position.
typedef struct sdrkit_dvoice_position_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Who sends.
  sdrkit_string source;
  // The position, latitude first.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height. Unit: m.
  double height_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course. Unit: °.
  double course_deg;
  // True if `text` holds a value.
  bool has_text;
  // The text.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_dvoice_position_event_family`.
  int32_t family;
  // How a position was sent. A value of `sdrkit_dvoice_position_event_format`.
  int32_t format;
} sdrkit_dvoice_position_event;

// The arguments of `remove`.
typedef struct sdrkit_dvoice_talkgroup_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the talkgroup.
  sdrkit_string row;
} sdrkit_dvoice_talkgroup_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVOICE_TALKGROUP_ARGS_INIT {sizeof(sdrkit_dvoice_talkgroup_args), {0}}

// The arguments of `ptt`.
typedef struct sdrkit_dvoice_ptt_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Push-to-talk pressed (on) or released (off).
  bool on;
} sdrkit_dvoice_ptt_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVOICE_PTT_ARGS_INIT {sizeof(sdrkit_dvoice_ptt_args), 0}

// The arguments of a text `send` of short data.
typedef struct sdrkit_dmr_data_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text of the message.
  sdrkit_string text;
} sdrkit_dmr_data_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DMR_DATA_SEND_ARGS_INIT {sizeof(sdrkit_dmr_data_send_args), {0}}

// The arguments of `send`.
typedef struct sdrkit_m17_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text of the SMS packet.
  sdrkit_string text;
} sdrkit_m17_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_M17_SEND_ARGS_INIT {sizeof(sdrkit_m17_send_args), {0}}

// The arguments of `send_data`.
typedef struct sdrkit_m17_send_data_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The protocol byte of the packet.
  int64_t protocol;
  // The body of the packet.
  sdrkit_bytes data;
} sdrkit_m17_send_data_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_M17_SEND_DATA_ARGS_INIT {sizeof(sdrkit_m17_send_data_args), 0, {0}}

// The arguments of the D-STAR `send`.
typedef struct sdrkit_dstar_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text of the slow data.
  sdrkit_string text;
} sdrkit_dstar_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DSTAR_SEND_ARGS_INIT {sizeof(sdrkit_dstar_send_args), {0}}

// Fills `rows` with the changed rows of the table "calls" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_dmr_calls_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_call_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dmr_call_details_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_call_detail_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dmr_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_dmr_positions_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_position_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dmr_sites_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_site_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dmr_grants_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_grant_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dmr_talkgroups_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_talkgroup_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dmr_history_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_history_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "dmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dmr_heard_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_heard_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dvoice.call"
// (Call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dvoice_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dvoice_call_event *event) {
  return sdrkit_typed_event(handle, n, "dvoice.call", "dvoice.CallEvent", event, sizeof(sdrkit_dvoice_call_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dvoice.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dvoice_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dvoice_message_event *event) {
  return sdrkit_typed_event(handle, n, "dvoice.message", "dvoice.MessageEvent", event, sizeof(sdrkit_dvoice_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dvoice.position"
// (Position). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dvoice_position_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dvoice_position_event *event) {
  return sdrkit_typed_event(handle, n, "dvoice.position", "dvoice.PositionEvent", event, sizeof(sdrkit_dvoice_position_event));
}

// Runs the command "clear" (Clear) of the stack "dmr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dmr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "dmr" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_dmr_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "dmr" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_dmr_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dmr" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dmr_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// Runs the command "ptt" (Push to talk) of the stack "dmr" on the session of `handle`. With on true, sends a voice call from the microphone input in direct mode, on the setting slot with the setting colour_code, from the setting radio_id to the setting called_id, or to the setting talkgroup when called_id is 0, until ptt with on false, halt or the time-out. The speech is coded with the half-rate coder, three 20 ms frames to a burst. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
static inline int32_t sdrkit_dmr_ptt(SDRKIT_HANDLE handle, const sdrkit_dvoice_ptt_args *args) {
  return sdrkit_typed_command(handle, "ptt", "dvoice.PttArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "dmr" on the session of `handle`. Sends the text as defined short data in direct mode, on the setting slot with the setting colour_code, from the setting radio_id to the setting called_id, or to the setting talkgroup when called_id is 0: up to 752 ISO 8859-1 characters, or 376 other characters as UTF-16LE. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_dmr_send(SDRKIT_HANDLE handle, const sdrkit_dmr_data_send_args *args) {
  return sdrkit_typed_command(handle, "send", "dvoice.DataSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "dmr" on the session of `handle`. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
static inline int32_t sdrkit_dmr_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dmr" are the sdrkit_dmr_gen_*() functions
// after the settings of its receiver.

// The setting "squelch_pct" (Error squelch) of the stack "dmr". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_dmr_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_dmr_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "dmr". Play only calls on the allow list.
static inline int32_t sdrkit_dmr_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_dmr_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "dmr". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dmr_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_dmr_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "dmr". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dmr_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_dmr_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "dmr". A call on this list takes the sound at once.
static inline int32_t sdrkit_dmr_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_dmr_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "dmr". Only this target is played.
static inline int32_t sdrkit_dmr_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_dmr_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "dmr". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_dmr_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_dmr_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "dmr". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_dmr_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_dmr_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "dmr". Unit: s.
static inline int32_t sdrkit_dmr_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_dmr_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "dmr". Off: decode and list, never play speech.
static inline int32_t sdrkit_dmr_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_dmr_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "radio_id" (Radio ID) of the stack "dmr". The source of every transmission: the ID registered to the call sign of the operator, 1 to 16776415. Nothing is sent while it is 0.
static inline int32_t sdrkit_dmr_get_radio_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "radio_id", value);
}
static inline int32_t sdrkit_dmr_set_radio_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "radio_id", value);
}

// The setting "called_id" (Radio ID called) of the stack "dmr". The radio a call or a text goes to. With 0 it goes to the setting talkgroup.
static inline int32_t sdrkit_dmr_get_called_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "called_id", value);
}
static inline int32_t sdrkit_dmr_set_called_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "called_id", value);
}

// The setting "talkgroup" (Talkgroup) of the stack "dmr". The talkgroup a call or a text goes to when the setting called_id is 0.
static inline int32_t sdrkit_dmr_get_talkgroup(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "talkgroup", value);
}
static inline int32_t sdrkit_dmr_set_talkgroup(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "talkgroup", value);
}

// The setting "colour_code" (Colour code) of the stack "dmr". The colour code in the slot type and the EMB of every burst, 0 to 15.
static inline int32_t sdrkit_dmr_get_colour_code(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "colour_code", value);
}
static inline int32_t sdrkit_dmr_set_colour_code(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "colour_code", value);
}

// The setting "slot" (Time slot) of the stack "dmr". A value of `sdrkit_dmr_slot`. The slot of direct mode: its sync patterns tell a receiver the slot.
static inline int32_t sdrkit_dmr_get_slot(SDRKIT_HANDLE handle, sdrkit_dmr_slot *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "slot", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dmr_slot)index;
  return code;
}
static inline int32_t sdrkit_dmr_set_slot(SDRKIT_HANDLE handle, sdrkit_dmr_slot value) {
  return sdrkit_typed_setting_set_enum(handle, "slot", (int32_t)value);
}

// The setting "tx_split" (Transmit offset) of the stack "dmr". A frequency in Hz. The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.
static inline int32_t sdrkit_dmr_get_tx_split(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_split", value);
}
static inline int32_t sdrkit_dmr_set_tx_split(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_split", value);
}

// The setting "tx_timeout" (Time-out) of the stack "dmr". A length of time in seconds. A voice stream ends after this time on air.
static inline int32_t sdrkit_dmr_get_tx_timeout(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_timeout", value);
}
static inline int32_t sdrkit_dmr_set_tx_timeout(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_timeout", value);
}

// The setting "gen_cc" (Colour code) of the test signal generator of the stack "dmr".
static inline int32_t sdrkit_dmr_gen_get_cc(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_cc", value);
}
static inline int32_t sdrkit_dmr_gen_set_cc(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_cc", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_p25_calls_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_call_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_p25_call_details_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_call_detail_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_p25_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_p25_positions_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_position_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25_sites_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_site_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25_grants_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_grant_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25_talkgroups_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_talkgroup_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "identifiers" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25_identifiers_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_identifier_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.IdentifierRow", "identifiers", rows, sizeof(sdrkit_dvoice_identifier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "adjacent_sites" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25_adjacent_sites_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_adjacent_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.AdjacentRow", "adjacent_sites", rows, sizeof(sdrkit_dvoice_adjacent_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_p25_history_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_history_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "p25" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_p25_heard_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_heard_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "p25" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_p25_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "p25" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_p25_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "p25" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_p25_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "p25" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_p25_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "p25" are the sdrkit_p25_gen_*() functions
// after the settings of its receiver.

// The setting "squelch_pct" (Error squelch) of the stack "p25". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_p25_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_p25_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "p25". Play only calls on the allow list.
static inline int32_t sdrkit_p25_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_p25_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "p25". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_p25_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_p25_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "p25". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_p25_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_p25_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "p25". A call on this list takes the sound at once.
static inline int32_t sdrkit_p25_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_p25_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "p25". Only this target is played.
static inline int32_t sdrkit_p25_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_p25_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "p25". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_p25_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_p25_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "p25". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_p25_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_p25_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "p25". Unit: s.
static inline int32_t sdrkit_p25_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_p25_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "p25". Off: decode and list, never play speech.
static inline int32_t sdrkit_p25_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_p25_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_nac" (NAC) of the test signal generator of the stack "p25".
static inline int32_t sdrkit_p25_gen_get_nac(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_nac", value);
}
static inline int32_t sdrkit_p25_gen_set_nac(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_nac", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_m17_calls_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_call_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_m17_call_details_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_call_detail_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_m17_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_m17_positions_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_position_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_m17_sites_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_site_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_m17_grants_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_grant_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_m17_talkgroups_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_talkgroup_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_m17_history_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_history_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "m17" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_m17_heard_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_heard_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "m17" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_m17_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "m17" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_m17_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "m17" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_m17_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "m17" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_m17_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// Runs the command "ptt" (Push to talk) of the stack "m17" on the session of `handle`. With on true, sends a voice stream from the microphone input (Codec 2, 3200 bit/s) from the setting callsign to the setting destination, until ptt with on false, halt or the time-out. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
static inline int32_t sdrkit_m17_ptt(SDRKIT_HANDLE handle, const sdrkit_dvoice_ptt_args *args) {
  return sdrkit_typed_command(handle, "ptt", "dvoice.PttArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "m17" on the session of `handle`. Sends the text as an SMS packet (protocol 5) from the setting callsign to the setting destination. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_m17_send(SDRKIT_HANDLE handle, const sdrkit_m17_send_args *args) {
  return sdrkit_typed_command(handle, "send", "dvoice.M17SendArgs", args, NULL, NULL, 0);
}

// Runs the command "send_data" (Send data) of the stack "m17" on the session of `handle`. Sends the bytes as a packet with the protocol byte, from the setting callsign to the setting destination. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_m17_send_data(SDRKIT_HANDLE handle, const sdrkit_m17_send_data_args *args) {
  return sdrkit_typed_command(handle, "send_data", "dvoice.M17SendDataArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "m17" on the session of `handle`. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
static inline int32_t sdrkit_m17_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "m17". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_m17_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_m17_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "m17". Play only calls on the allow list.
static inline int32_t sdrkit_m17_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_m17_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "m17". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_m17_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_m17_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "m17". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_m17_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_m17_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "m17". A call on this list takes the sound at once.
static inline int32_t sdrkit_m17_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_m17_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "m17". Only this target is played.
static inline int32_t sdrkit_m17_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_m17_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "m17". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_m17_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_m17_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "m17". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_m17_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_m17_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "m17". Unit: s.
static inline int32_t sdrkit_m17_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_m17_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "m17". Off: decode and list, never play speech.
static inline int32_t sdrkit_m17_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_m17_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "callsign" (Own call sign) of the stack "m17". The source address of every transmission: up to 9 characters of A to Z, 0 to 9, -, / and the full stop. Nothing is sent while it is empty.
static inline int32_t sdrkit_m17_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_m17_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "destination" (Destination) of the stack "m17". A call sign, or @ALL for every station (broadcast).
static inline int32_t sdrkit_m17_get_destination(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "destination", value);
}
static inline int32_t sdrkit_m17_set_destination(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "destination", value);
}

// The setting "can" (Channel access number) of the stack "m17". Sent in the TYPE field of the link setup frame.
static inline int32_t sdrkit_m17_get_can(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "can", value);
}
static inline int32_t sdrkit_m17_set_can(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "can", value);
}

// The setting "voice_mode" (Voice) of the stack "m17". A value of `sdrkit_m17_voice_mode`. Codec 2 at 3200 bit/s. The form of 1600 bit/s with data is not offered: the library has no encoder for it.
static inline int32_t sdrkit_m17_get_voice_mode(SDRKIT_HANDLE handle, sdrkit_m17_voice_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "voice_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_m17_voice_mode)index;
  return code;
}
static inline int32_t sdrkit_m17_set_voice_mode(SDRKIT_HANDLE handle, sdrkit_m17_voice_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "voice_mode", (int32_t)value);
}

// The setting "meta_text" (Text) of the stack "m17". Up to 52 characters, sent in the META field of the link setup frame of a voice stream, 13 characters a block.
static inline int32_t sdrkit_m17_get_meta_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "meta_text", value);
}
static inline int32_t sdrkit_m17_set_meta_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "meta_text", value);
}

// The setting "tx_split" (Transmit offset) of the stack "m17". A frequency in Hz. The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.
static inline int32_t sdrkit_m17_get_tx_split(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_split", value);
}
static inline int32_t sdrkit_m17_set_tx_split(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_split", value);
}

// The setting "tx_timeout" (Time-out) of the stack "m17". A length of time in seconds. A voice stream ends after this time on air.
static inline int32_t sdrkit_m17_get_tx_timeout(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_timeout", value);
}
static inline int32_t sdrkit_m17_set_tx_timeout(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_timeout", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_dstar_calls_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_call_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dstar_call_details_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_call_detail_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dstar_messages_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_message_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_dstar_positions_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_position_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dstar_sites_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_site_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dstar_grants_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_grant_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dstar_talkgroups_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_talkgroup_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dstar_history_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_history_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "dstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dstar_heard_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_heard_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dstar" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dstar_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "dstar" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_dstar_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "dstar" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_dstar_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dstar" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dstar_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "dstar" on the session of `handle`. Sends the radio header from the setting callsign to the setting your_call, and the text in the slow data of voice frames that carry AMBE silence: the library has no AMBE encoder. The transmission ends after every slow-data block went out once, or at halt. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_dstar_send(SDRKIT_HANDLE handle, const sdrkit_dstar_send_args *args) {
  return sdrkit_typed_command(handle, "send", "dvoice.DstarSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "dstar" on the session of `handle`. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
static inline int32_t sdrkit_dstar_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "dstar". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_dstar_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_dstar_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "dstar". Play only calls on the allow list.
static inline int32_t sdrkit_dstar_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_dstar_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "dstar". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dstar_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_dstar_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "dstar". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dstar_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_dstar_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "dstar". A call on this list takes the sound at once.
static inline int32_t sdrkit_dstar_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_dstar_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "dstar". Only this target is played.
static inline int32_t sdrkit_dstar_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_dstar_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "dstar". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_dstar_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_dstar_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "dstar". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_dstar_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_dstar_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "dstar". Unit: s.
static inline int32_t sdrkit_dstar_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_dstar_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "dstar". Off: decode and list, never play speech.
static inline int32_t sdrkit_dstar_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_dstar_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "callsign" (Own call sign) of the stack "dstar". MY of the radio header: up to 8 characters of A to Z, 0 to 9, / and space. Nothing is sent while it is empty.
static inline int32_t sdrkit_dstar_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_dstar_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "suffix" (Suffix) of the stack "dstar". Up to 4 characters after MY, for example the radio type.
static inline int32_t sdrkit_dstar_get_suffix(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "suffix", value);
}
static inline int32_t sdrkit_dstar_set_suffix(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "suffix", value);
}

// The setting "your_call" (Your call sign) of the stack "dstar". YOUR of the radio header: a call sign, CQCQCQ for everyone, or a command of a gateway; up to 8 characters.
static inline int32_t sdrkit_dstar_get_your_call(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "your_call", value);
}
static inline int32_t sdrkit_dstar_set_your_call(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "your_call", value);
}

// The setting "rpt1" (Repeater 1) of the stack "dstar". RPT1 of the radio header: DIRECT for simplex, or the repeater with its module letter in position 8.
static inline int32_t sdrkit_dstar_get_rpt1(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "rpt1", value);
}
static inline int32_t sdrkit_dstar_set_rpt1(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "rpt1", value);
}

// The setting "rpt2" (Repeater 2) of the stack "dstar". RPT2 of the radio header: DIRECT for simplex, or the repeater with G in position 8 for the gateway.
static inline int32_t sdrkit_dstar_get_rpt2(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "rpt2", value);
}
static inline int32_t sdrkit_dstar_set_rpt2(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "rpt2", value);
}

// The setting "via_repeater" (Via repeater) of the stack "dstar". Sets bit 6 (0x40) of flag byte 1: the transmission goes through a repeater.
static inline int32_t sdrkit_dstar_get_via_repeater(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "via_repeater", value);
}
static inline int32_t sdrkit_dstar_set_via_repeater(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "via_repeater", value);
}

// The setting "tx_split" (Transmit offset) of the stack "dstar". A frequency in Hz. The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.
static inline int32_t sdrkit_dstar_get_tx_split(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_split", value);
}
static inline int32_t sdrkit_dstar_set_tx_split(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_split", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_nxdn_calls_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_call_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_nxdn_call_details_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_call_detail_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_nxdn_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_nxdn_positions_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_position_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nxdn_sites_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_site_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nxdn_grants_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_grant_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nxdn_talkgroups_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_talkgroup_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_nxdn_history_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_history_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "nxdn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_nxdn_heard_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_heard_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "nxdn" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_nxdn_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "nxdn" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_nxdn_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "nxdn" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_nxdn_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "nxdn" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_nxdn_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "nxdn". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_nxdn_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_nxdn_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "nxdn". Play only calls on the allow list.
static inline int32_t sdrkit_nxdn_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_nxdn_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "nxdn". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_nxdn_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_nxdn_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "nxdn". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_nxdn_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_nxdn_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "nxdn". A call on this list takes the sound at once.
static inline int32_t sdrkit_nxdn_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_nxdn_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "nxdn". Only this target is played.
static inline int32_t sdrkit_nxdn_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_nxdn_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "nxdn". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_nxdn_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_nxdn_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "nxdn". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_nxdn_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_nxdn_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "nxdn". Unit: s.
static inline int32_t sdrkit_nxdn_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_nxdn_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "nxdn". Off: decode and list, never play speech.
static inline int32_t sdrkit_nxdn_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_nxdn_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_dpmr_calls_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_call_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dpmr_call_details_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_call_detail_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dpmr_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_dpmr_positions_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_position_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dpmr_sites_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_site_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dpmr_grants_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_grant_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dpmr_talkgroups_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_talkgroup_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dpmr_history_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_history_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "dpmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dpmr_heard_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_heard_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dpmr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dpmr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "dpmr" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_dpmr_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "dpmr" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_dpmr_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dpmr" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dpmr_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "dpmr". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_dpmr_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_dpmr_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "dpmr". Play only calls on the allow list.
static inline int32_t sdrkit_dpmr_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_dpmr_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "dpmr". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dpmr_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_dpmr_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "dpmr". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dpmr_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_dpmr_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "dpmr". A call on this list takes the sound at once.
static inline int32_t sdrkit_dpmr_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_dpmr_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "dpmr". Only this target is played.
static inline int32_t sdrkit_dpmr_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_dpmr_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "dpmr". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_dpmr_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_dpmr_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "dpmr". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_dpmr_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_dpmr_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "dpmr". Unit: s.
static inline int32_t sdrkit_dpmr_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_dpmr_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "dpmr". Off: decode and list, never play speech.
static inline int32_t sdrkit_dpmr_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_dpmr_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_ysf_calls_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_call_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_ysf_call_details_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_call_detail_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_ysf_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_ysf_positions_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_position_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ysf_sites_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_site_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ysf_grants_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_grant_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ysf_talkgroups_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_talkgroup_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_ysf_history_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_history_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "ysf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_ysf_heard_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_heard_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ysf" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ysf_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "ysf" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_ysf_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "ysf" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_ysf_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ysf" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ysf_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ysf" are the sdrkit_ysf_gen_*() functions
// after the settings of its receiver.

// The setting "squelch_pct" (Error squelch) of the stack "ysf". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_ysf_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_ysf_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "ysf". Play only calls on the allow list.
static inline int32_t sdrkit_ysf_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_ysf_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "ysf". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_ysf_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_ysf_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "ysf". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_ysf_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_ysf_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "ysf". A call on this list takes the sound at once.
static inline int32_t sdrkit_ysf_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_ysf_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "ysf". Only this target is played.
static inline int32_t sdrkit_ysf_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_ysf_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "ysf". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_ysf_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_ysf_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "ysf". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_ysf_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_ysf_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "ysf". Unit: s.
static inline int32_t sdrkit_ysf_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_ysf_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "ysf". Off: decode and list, never play speech.
static inline int32_t sdrkit_ysf_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ysf_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_voice" (Voice type) of the test signal generator of the stack "ysf". A value of `sdrkit_ysf_gen_voice`. The data type of the voice of the test signal: V/D mode 1, V/D mode 2 or voice full rate.
static inline int32_t sdrkit_ysf_gen_get_voice(SDRKIT_HANDLE handle, sdrkit_ysf_gen_voice *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_voice", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ysf_gen_voice)index;
  return code;
}
static inline int32_t sdrkit_ysf_gen_set_voice(SDRKIT_HANDLE handle, sdrkit_ysf_gen_voice value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_voice", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_p25p2_calls_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_call_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_p25p2_call_details_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_call_detail_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_p25p2_messages_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_message_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_p25p2_positions_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_position_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25p2_sites_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_site_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25p2_grants_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_grant_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_p25p2_talkgroups_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_talkgroup_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_p25p2_history_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_history_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "p25p2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_p25p2_heard_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_heard_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "p25p2" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_p25p2_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "p25p2" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_p25p2_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "p25p2" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_p25p2_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "p25p2" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_p25p2_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "p25p2". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_p25p2_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_p25p2_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "p25p2". Play only calls on the allow list.
static inline int32_t sdrkit_p25p2_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_p25p2_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "p25p2". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_p25p2_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_p25p2_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "p25p2". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_p25p2_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_p25p2_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "p25p2". A call on this list takes the sound at once.
static inline int32_t sdrkit_p25p2_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_p25p2_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "p25p2". Only this target is played.
static inline int32_t sdrkit_p25p2_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_p25p2_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "p25p2". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_p25p2_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_p25p2_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "p25p2". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_p25p2_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_p25p2_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "p25p2". Unit: s.
static inline int32_t sdrkit_p25p2_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_p25p2_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "p25p2". Off: decode and list, never play speech.
static inline int32_t sdrkit_p25p2_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_p25p2_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_tetrapol_calls_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_call_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_tetrapol_call_details_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_dvoice_call_detail_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_tetrapol_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_tetrapol_positions_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_position_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetrapol_sites_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_site_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetrapol_grants_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_grant_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetrapol_talkgroups_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dvoice_talkgroup_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_tetrapol_history_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_history_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "tetrapol" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_tetrapol_heard_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_heard_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "tetrapol" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_tetrapol_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "tetrapol" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_tetrapol_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "tetrapol" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_tetrapol_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "tetrapol" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_tetrapol_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "tetrapol". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_tetrapol_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_tetrapol_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "tetrapol". Play only calls on the allow list.
static inline int32_t sdrkit_tetrapol_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_tetrapol_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "tetrapol". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_tetrapol_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_tetrapol_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "tetrapol". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_tetrapol_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_tetrapol_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "tetrapol". A call on this list takes the sound at once.
static inline int32_t sdrkit_tetrapol_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_tetrapol_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "tetrapol". Only this target is played.
static inline int32_t sdrkit_tetrapol_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_tetrapol_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "tetrapol". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_tetrapol_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_tetrapol_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "tetrapol". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_tetrapol_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_tetrapol_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "tetrapol". Unit: s.
static inline int32_t sdrkit_tetrapol_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_tetrapol_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "tetrapol". Off: decode and list, never play speech.
static inline int32_t sdrkit_tetrapol_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_tetrapol_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_tetra_calls_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_call_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_tetra_call_details_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_call_detail_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_tetra_messages_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_message_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_tetra_positions_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_position_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetra_sites_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_site_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetra_grants_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_grant_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetra_talkgroups_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_talkgroup_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_tetra_history_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_history_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "tetra" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_tetra_heard_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_heard_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "tetra" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_tetra_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "tetra" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_tetra_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "tetra" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_tetra_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "tetra" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_tetra_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "tetra". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_tetra_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_tetra_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "tetra". Play only calls on the allow list.
static inline int32_t sdrkit_tetra_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_tetra_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "tetra". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_tetra_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_tetra_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "tetra". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_tetra_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_tetra_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "tetra". A call on this list takes the sound at once.
static inline int32_t sdrkit_tetra_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_tetra_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "tetra". Only this target is played.
static inline int32_t sdrkit_tetra_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_tetra_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "tetra". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_tetra_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_tetra_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "tetra". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_tetra_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_tetra_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "tetra". Unit: s.
static inline int32_t sdrkit_tetra_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_tetra_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "tetra". Off: decode and list, never play speech.
static inline int32_t sdrkit_tetra_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_tetra_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_tetra_dmo_calls_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_call_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_tetra_dmo_call_details_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_dvoice_call_detail_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_tetra_dmo_messages_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_message_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_tetra_dmo_positions_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dvoice_position_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetra_dmo_sites_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_site_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetra_dmo_grants_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_grant_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tetra_dmo_talkgroups_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_dvoice_talkgroup_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_tetra_dmo_history_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_history_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "tetra_dmo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_tetra_dmo_heard_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_heard_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "tetra_dmo" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_tetra_dmo_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "tetra_dmo" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_tetra_dmo_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "tetra_dmo" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_tetra_dmo_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "tetra_dmo" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_tetra_dmo_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "tetra_dmo" are the sdrkit_tetra_dmo_gen_*() functions
// after the settings of its receiver.

// The setting "squelch_pct" (Error squelch) of the stack "tetra_dmo". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_tetra_dmo_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_tetra_dmo_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "tetra_dmo". Play only calls on the allow list.
static inline int32_t sdrkit_tetra_dmo_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_tetra_dmo_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "tetra_dmo". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_tetra_dmo_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_tetra_dmo_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "tetra_dmo". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_tetra_dmo_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_tetra_dmo_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "tetra_dmo". A call on this list takes the sound at once.
static inline int32_t sdrkit_tetra_dmo_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_tetra_dmo_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "tetra_dmo". Only this target is played.
static inline int32_t sdrkit_tetra_dmo_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_tetra_dmo_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "tetra_dmo". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_tetra_dmo_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_tetra_dmo_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "tetra_dmo". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_tetra_dmo_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_tetra_dmo_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "tetra_dmo". Unit: s.
static inline int32_t sdrkit_tetra_dmo_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_tetra_dmo_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "tetra_dmo". Off: decode and list, never play speech.
static inline int32_t sdrkit_tetra_dmo_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_tetra_dmo_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_comm_type" (Test signal communication type) of the test signal generator of the stack "tetra_dmo".
static inline int32_t sdrkit_tetra_dmo_gen_get_comm_type(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_comm_type", value);
}
static inline int32_t sdrkit_tetra_dmo_gen_set_comm_type(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_comm_type", value);
}

// The setting "gen_group" (Test signal group call) of the test signal generator of the stack "tetra_dmo".
static inline int32_t sdrkit_tetra_dmo_gen_get_group(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_group", value);
}
static inline int32_t sdrkit_tetra_dmo_gen_set_group(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_group", value);
}

// The setting "gen_ai_enc" (Test signal encryption state) of the test signal generator of the stack "tetra_dmo".
static inline int32_t sdrkit_tetra_dmo_gen_get_ai_enc(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ai_enc", value);
}
static inline int32_t sdrkit_tetra_dmo_gen_set_ai_enc(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ai_enc", value);
}

// The setting "gen_e2e" (Test signal end-to-end encryption) of the test signal generator of the stack "tetra_dmo".
static inline int32_t sdrkit_tetra_dmo_gen_get_e2e(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_e2e", value);
}
static inline int32_t sdrkit_tetra_dmo_gen_set_e2e(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_e2e", value);
}

// The setting "gen_traffic_frames" (Test signal traffic bursts) of the test signal generator of the stack "tetra_dmo".
static inline int32_t sdrkit_tetra_dmo_gen_get_traffic_frames(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_traffic_frames", value);
}
static inline int32_t sdrkit_tetra_dmo_gen_set_traffic_frames(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_traffic_frames", value);
}

// The setting "gen_setup_repeats" (Test signal set-up bursts) of the test signal generator of the stack "tetra_dmo".
static inline int32_t sdrkit_tetra_dmo_gen_get_setup_repeats(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_setup_repeats", value);
}
static inline int32_t sdrkit_tetra_dmo_gen_set_setup_repeats(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_setup_repeats", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_freedv_calls_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_call_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_freedv_call_details_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dvoice_call_detail_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_freedv_messages_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dvoice_message_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_freedv_positions_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_position_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_freedv_sites_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_site_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_freedv_grants_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_grant_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_freedv_talkgroups_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_talkgroup_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_freedv_history_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_history_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "freedv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_freedv_heard_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_heard_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "freedv" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_freedv_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "freedv" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_freedv_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "freedv" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_freedv_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "freedv" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_freedv_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// Runs the command "ptt" (Push to talk) of the stack "freedv" on the session of `handle`. With on true, sends the microphone input on the waveform of the submode: Codec 2 1300 on 1600 in the submode 1600, else Codec 2 700C on 700D (700E in the submode 700E); with the setting callsign in the text channel, until ptt with on false, halt or the time-out. The submode RADE does not send. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
static inline int32_t sdrkit_freedv_ptt(SDRKIT_HANDLE handle, const sdrkit_dvoice_ptt_args *args) {
  return sdrkit_typed_command(handle, "ptt", "dvoice.PttArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "freedv" on the session of `handle`. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
static inline int32_t sdrkit_freedv_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "freedv" are the sdrkit_freedv_gen_*() functions
// after the settings of its receiver.

// The setting "squelch_pct" (Error squelch) of the stack "freedv". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_freedv_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_freedv_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "freedv". Play only calls on the allow list.
static inline int32_t sdrkit_freedv_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_freedv_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "freedv". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_freedv_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_freedv_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "freedv". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_freedv_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_freedv_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "freedv". A call on this list takes the sound at once.
static inline int32_t sdrkit_freedv_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_freedv_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "freedv". Only this target is played.
static inline int32_t sdrkit_freedv_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_freedv_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "freedv". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_freedv_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_freedv_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "freedv". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_freedv_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_freedv_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "freedv". Unit: s.
static inline int32_t sdrkit_freedv_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_freedv_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "freedv". Off: decode and list, never play speech.
static inline int32_t sdrkit_freedv_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_freedv_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "callsign" (Callsign text) of the stack "freedv". Sent in the text channel with a carriage return after it, again and again: up to 40 printable ASCII characters, normally the call sign of the station. Nothing is sent while it is empty.
static inline int32_t sdrkit_freedv_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_freedv_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "tx_clip" (Clipper) of the stack "freedv". The drive gain and a limit at half of full scale before the level, with the transmit filter after it: more average power for a little distortion of the carriers. It works on 1600, 700D and 700E.
static inline int32_t sdrkit_freedv_get_tx_clip(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_clip", value);
}
static inline int32_t sdrkit_freedv_set_tx_clip(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_clip", value);
}

// The setting "tx_filter" (Transmit filter) of the stack "freedv". A band-pass around 1500 Hz: about 725 Hz to 2275 Hz for 1600, 750 Hz to 2250 Hz for 700D, 500 Hz to 2500 Hz for 700E. The clipper turns it on as well.
static inline int32_t sdrkit_freedv_get_tx_filter(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_filter", value);
}
static inline int32_t sdrkit_freedv_set_tx_filter(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_filter", value);
}

// The setting "tx_split" (Transmit offset) of the stack "freedv". A frequency in Hz. The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.
static inline int32_t sdrkit_freedv_get_tx_split(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_split", value);
}
static inline int32_t sdrkit_freedv_set_tx_split(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_split", value);
}

// The setting "tx_timeout" (Time-out) of the stack "freedv". A length of time in seconds. A voice stream ends after this time on air.
static inline int32_t sdrkit_freedv_get_tx_timeout(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_timeout", value);
}
static inline int32_t sdrkit_freedv_set_tx_timeout(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_timeout", value);
}

// The setting "gen_clip" (Clipper) of the test signal generator of the stack "freedv". The 700D and 700E test signals pass the clipper and the transmit filter.
static inline int32_t sdrkit_freedv_gen_get_clip(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_clip", value);
}
static inline int32_t sdrkit_freedv_gen_set_clip(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_clip", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_dvoice_auto_calls_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_call_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dvoice_auto_call_details_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_dvoice_call_detail_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dvoice_auto_messages_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_dvoice_message_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_dvoice_auto_positions_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_dvoice_position_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvoice_auto_sites_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_site_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvoice_auto_grants_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvoice_grant_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvoice_auto_talkgroups_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_dvoice_talkgroup_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes_seen" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvoice_auto_modes_seen_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_dvoice_auto_seen_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SeenRow", "modes_seen", rows, sizeof(sdrkit_dvoice_auto_seen_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "identifiers" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvoice_auto_identifiers_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_dvoice_identifier_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.IdentifierRow", "identifiers", rows, sizeof(sdrkit_dvoice_identifier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "adjacent_sites" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvoice_auto_adjacent_sites_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_dvoice_adjacent_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.AdjacentRow", "adjacent_sites", rows, sizeof(sdrkit_dvoice_adjacent_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dvoice_auto_history_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dvoice_history_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "dvoice-auto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dvoice_auto_heard_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvoice_heard_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dvoice-auto" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dvoice_auto_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "dvoice-auto" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_dvoice_auto_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "dvoice-auto" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_dvoice_auto_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dvoice-auto" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dvoice_auto_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The setting "squelch_pct" (Error squelch) of the stack "dvoice-auto". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_dvoice_auto_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_dvoice_auto_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "dvoice-auto". Play only calls on the allow list.
static inline int32_t sdrkit_dvoice_auto_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_dvoice_auto_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "dvoice-auto". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dvoice_auto_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_dvoice_auto_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "dvoice-auto". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dvoice_auto_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_dvoice_auto_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "dvoice-auto". A call on this list takes the sound at once.
static inline int32_t sdrkit_dvoice_auto_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_dvoice_auto_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "dvoice-auto". Only this target is played.
static inline int32_t sdrkit_dvoice_auto_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_dvoice_auto_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "dvoice-auto". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_dvoice_auto_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_dvoice_auto_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "dvoice-auto". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_dvoice_auto_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_dvoice_auto_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "dvoice-auto". Unit: s.
static inline int32_t sdrkit_dvoice_auto_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_dvoice_auto_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "dvoice-auto". Off: decode and list, never play speech.
static inline int32_t sdrkit_dvoice_auto_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_dvoice_auto_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `alias` names a row.
static inline int32_t sdrkit_dcr_calls_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_call_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallRow", "calls", rows, sizeof(sdrkit_dvoice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "call_details" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dcr_call_details_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvoice_call_detail_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.CallDetailRow", "call_details", rows, sizeof(sdrkit_dvoice_call_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dcr_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvoice_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.MessageRow", "messages", rows, sizeof(sdrkit_dvoice_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "positions" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_dcr_positions_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvoice_position_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.PositionRow", "positions", rows, sizeof(sdrkit_dvoice_position_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dcr_sites_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_site_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.SiteRow", "sites", rows, sizeof(sdrkit_dvoice_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grants" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dcr_grants_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvoice_grant_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.GrantRow", "grants", rows, sizeof(sdrkit_dvoice_grant_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dcr_talkgroups_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dvoice_talkgroup_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_dvoice_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `identity` names a row.
static inline int32_t sdrkit_dcr_events_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dcr_event_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.DcrEventRow", "events", rows, sizeof(sdrkit_dcr_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "history" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_dcr_history_rows(SDRKIT_HANDLE handle,
                                             sdrkit_dvoice_history_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HistoryRow", "history", rows, sizeof(sdrkit_dvoice_history_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "dcr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `src` names a row.
static inline int32_t sdrkit_dcr_heard_rows(SDRKIT_HANDLE handle,
                                           sdrkit_dvoice_heard_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvoice.HeardRow", "heard", rows, sizeof(sdrkit_dvoice_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dcr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dcr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "hold_current" (Hold current) of the stack "dcr" on the session of `handle`. Plays only the target of the call that plays now, or of the newest call.
static inline int32_t sdrkit_dcr_hold_current(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "hold_current", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "release" (Release) of the stack "dcr" on the session of `handle`. Lets the held target go.
static inline int32_t sdrkit_dcr_release(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "release", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dcr" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dcr_remove(SDRKIT_HANDLE handle, const sdrkit_dvoice_talkgroup_args *args) {
  return sdrkit_typed_command(handle, "remove", "dvoice.TalkgroupArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dcr" are the sdrkit_dcr_gen_*() functions
// after the settings of its receiver.

// The setting "squelch_pct" (Error squelch) of the stack "dcr". Unit: %. Frames with a higher bit error rate are not played.
static inline int32_t sdrkit_dcr_get_squelch_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_pct", value);
}
static inline int32_t sdrkit_dcr_set_squelch_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_pct", value);
}

// The setting "allow_only" (Allowed only) of the stack "dcr". Play only calls on the allow list.
static inline int32_t sdrkit_dcr_get_allow_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_only", value);
}
static inline int32_t sdrkit_dcr_set_allow_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_only", value);
}

// The setting "allow" (Allow list) of the stack "dcr". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dcr_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_dcr_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "mute_list" (Mute list) of the stack "dcr". Numbers, ranges 100-199, callsigns and prefixes LY*.
static inline int32_t sdrkit_dcr_get_mute_list(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mute_list", value);
}
static inline int32_t sdrkit_dcr_set_mute_list(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mute_list", value);
}

// The setting "priority" (Priority list) of the stack "dcr". A call on this list takes the sound at once.
static inline int32_t sdrkit_dcr_get_priority(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "priority", value);
}
static inline int32_t sdrkit_dcr_set_priority(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "priority", value);
}

// The setting "hold" (Hold) of the stack "dcr". Only this target is played.
static inline int32_t sdrkit_dcr_get_hold(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hold", value);
}
static inline int32_t sdrkit_dcr_set_hold(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hold", value);
}

// The setting "labels" (Labels) of the stack "dcr". Lines 91=Worldwide, and RID 3120123=W1ABC Joe.
static inline int32_t sdrkit_dcr_get_labels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "labels", value);
}
static inline int32_t sdrkit_dcr_set_labels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "labels", value);
}

// The setting "hold_timeout_s" (Hold timeout) of the stack "dcr". Unit: s. The hold lets go after this long without a call on it (0: until released).
static inline int32_t sdrkit_dcr_get_hold_timeout_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", value);
}
static inline int32_t sdrkit_dcr_set_hold_timeout_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value);
}

// The setting "hang_s" (Hang time) of the stack "dcr". Unit: s.
static inline int32_t sdrkit_dcr_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_dcr_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "listen" (Listen) of the stack "dcr". Off: decode and list, never play speech.
static inline int32_t sdrkit_dcr_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_dcr_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "plan" (Channel plan) of the stack "dcr". The channels to watch: entries separated by ";", each a label, "=" and the frequency in Hz, for example "351 MHz ch 15=351287500". At most 512 entries. The host fills it from the DCR plan file. Empty: the tuned frequency only.
static inline int32_t sdrkit_dcr_get_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "plan", value);
}
static inline int32_t sdrkit_dcr_set_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "plan", value);
}

// The setting "scan" (Watch every plan channel in the span) of the stack "dcr". Decode every plan channel inside 42 % of the sample rate around the centre that carries a signal. It needs two plan channels in the span and 100 ksps or more.
static inline int32_t sdrkit_dcr_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_dcr_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "max_channels" (Channels decoded at once) of the stack "dcr".
static inline int32_t sdrkit_dcr_get_max_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_channels", value);
}
static inline int32_t sdrkit_dcr_set_max_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_channels", value);
}

// The setting "gen_content" (Content) of the test signal generator of the stack "dcr". A value of `sdrkit_dcr_gen_content`. A test signal of chapter 7 of the standard, eight calls, or manufacturer messages, data frames and a call sign.
static inline int32_t sdrkit_dcr_gen_get_content(SDRKIT_HANDLE handle, sdrkit_dcr_gen_content *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_content", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dcr_gen_content)index;
  return code;
}
static inline int32_t sdrkit_dcr_gen_set_content(SDRKIT_HANDLE handle, sdrkit_dcr_gen_content value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_content", (int32_t)value);
}

// The setting "gen_deviation_scale" (Deviation scale) of the test signal generator of the stack "dcr". The outer deviation over 945 Hz.
static inline int32_t sdrkit_dcr_gen_get_deviation_scale(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_scale", value);
}
static inline int32_t sdrkit_dcr_gen_set_deviation_scale(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_scale", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvoice_shared_fields_read(SDRKIT_HANDLE handle, sdrkit_dvoice_shared_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.SharedFields", fields, sizeof(sdrkit_dvoice_shared_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dmr_fields_read(SDRKIT_HANDLE handle, sdrkit_dmr_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DmrFields", fields, sizeof(sdrkit_dmr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvoice_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_dvoice_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.MonitorFields", fields, sizeof(sdrkit_dvoice_monitor_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvoice_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_dvoice_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DecoderFields", fields, sizeof(sdrkit_dvoice_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dmr_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_dmr_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DmrTxFields", fields, sizeof(sdrkit_dmr_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_p25_fields_read(SDRKIT_HANDLE handle, sdrkit_p25_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.P25Fields", fields, sizeof(sdrkit_p25_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_m17_fields_read(SDRKIT_HANDLE handle, sdrkit_m17_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.M17Fields", fields, sizeof(sdrkit_m17_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_m17_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_m17_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.M17TxFields", fields, sizeof(sdrkit_m17_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dstar_fields_read(SDRKIT_HANDLE handle, sdrkit_dstar_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DstarFields", fields, sizeof(sdrkit_dstar_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dstar_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_dstar_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DstarTxFields", fields, sizeof(sdrkit_dstar_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nxdn_fields_read(SDRKIT_HANDLE handle, sdrkit_nxdn_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.NxdnFields", fields, sizeof(sdrkit_nxdn_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dpmr_fields_read(SDRKIT_HANDLE handle, sdrkit_dpmr_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DpmrFields", fields, sizeof(sdrkit_dpmr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ysf_fields_read(SDRKIT_HANDLE handle, sdrkit_ysf_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.YsfFields", fields, sizeof(sdrkit_ysf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_p25p2_fields_read(SDRKIT_HANDLE handle, sdrkit_p25p2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.P25p2Fields", fields, sizeof(sdrkit_p25p2_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tetrapol_fields_read(SDRKIT_HANDLE handle, sdrkit_tetrapol_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.TetrapolFields", fields, sizeof(sdrkit_tetrapol_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tetra_fields_read(SDRKIT_HANDLE handle, sdrkit_tetra_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.TetraFields", fields, sizeof(sdrkit_tetra_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tetra_dmo_fields_read(SDRKIT_HANDLE handle, sdrkit_tetra_dmo_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.TetraDmoFields", fields, sizeof(sdrkit_tetra_dmo_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_freedv_fields_read(SDRKIT_HANDLE handle, sdrkit_freedv_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.FreedvFields", fields, sizeof(sdrkit_freedv_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_freedv_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_freedv_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.FreedvTxFields", fields, sizeof(sdrkit_freedv_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvoice_auto_fields_read(SDRKIT_HANDLE handle, sdrkit_dvoice_auto_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.AutoFields", fields, sizeof(sdrkit_dvoice_auto_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dcr_fields_read(SDRKIT_HANDLE handle, sdrkit_dcr_fields *fields) {
  return sdrkit_typed_fields(handle, "dvoice.DcrFields", fields, sizeof(sdrkit_dcr_fields));
}

#endif
