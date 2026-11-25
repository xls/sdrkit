/*
 * sdrkit: the typed interface of the stack family "dvbcs", version 2.0. MIT licence.
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

#ifndef SDRKIT_DVBCS_H
#define SDRKIT_DVBCS_H

#include "../sdrkit.h"
#include "mpegts.h"
#include "ipcast.h"
#include "dvbs2.h"

// The version of the family that this header describes.
#define SDRKIT_DVBCS_VERSION_MAJOR 2
#define SDRKIT_DVBCS_VERSION_MINOR 0

// The stack "satbeacon": its id, and the keys of its event types.
#define SDRKIT_STACK_SATBEACON "satbeacon"
#define SDRKIT_EVENT_BEACON_FRAME "beacon.frame"
#define SDRKIT_EVENT_BEACON_CARRIER "beacon.carrier"

// The stack "dvbc": its id, and the keys of its event types.
#define SDRKIT_STACK_DVBC "dvbc"

// The stack "dvbc2": its id, and the keys of its event types.
#define SDRKIT_STACK_DVBC2 "dvbc2"

// The stack "dvbs": its id, and the keys of its event types.
#define SDRKIT_STACK_DVBS "dvbs"

// The stack "isdbs": its id, and the keys of its event types.
#define SDRKIT_STACK_ISDBS "isdbs"

// The options of the setting "qam" of the stack "dvbc".
typedef enum sdrkit_dvbc_qam {
  // Automatic
  SDRKIT_DVBC_QAM_AUTO = 0,
  // 16-QAM
  SDRKIT_DVBC_QAM_16_QAM = 1,
  // 32-QAM
  SDRKIT_DVBC_QAM_32_QAM = 2,
  // 64-QAM
  SDRKIT_DVBC_QAM_64_QAM = 3,
  // 128-QAM
  SDRKIT_DVBC_QAM_128_QAM = 4,
  // 256-QAM
  SDRKIT_DVBC_QAM_256_QAM = 5,
} sdrkit_dvbc_qam;

// The options of the setting "gen_qam" of the stack "dvbc".
typedef enum sdrkit_dvbc_gen_qam {
  // 16-QAM
  SDRKIT_DVBC_GEN_QAM_16_QAM = 0,
  // 32-QAM
  SDRKIT_DVBC_GEN_QAM_32_QAM = 1,
  // 64-QAM
  SDRKIT_DVBC_GEN_QAM_64_QAM = 2,
  // 128-QAM
  SDRKIT_DVBC_GEN_QAM_128_QAM = 3,
  // 256-QAM
  SDRKIT_DVBC_GEN_QAM_256_QAM = 4,
} sdrkit_dvbc_gen_qam;

// The options of the setting "gen_payload" of the stack "dvbc".
typedef enum sdrkit_dvbc_gen_payload {
  // Test multiplex
  SDRKIT_DVBC_GEN_PAYLOAD_PROGRAMME = 0,
  // Numbered packets
  SDRKIT_DVBC_GEN_PAYLOAD_NUMBERED = 1,
} sdrkit_dvbc_gen_payload;

// The options of the setting "raster" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_raster {
  // 8 MHz
  SDRKIT_DVBC2_RASTER_8_MHZ = 0,
  // 6 MHz
  SDRKIT_DVBC2_RASTER_6_MHZ = 1,
} sdrkit_dvbc2_raster;

// The options of the setting "gen_modcod" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_modcod {
  // 256-QAM 5/6
  SDRKIT_DVBC2_GEN_MODCOD_256_5_6 = 0,
  // 16-QAM 4/5
  SDRKIT_DVBC2_GEN_MODCOD_16_4_5 = 1,
  // 64-QAM 2/3
  SDRKIT_DVBC2_GEN_MODCOD_64_2_3 = 2,
  // 256-QAM 3/4
  SDRKIT_DVBC2_GEN_MODCOD_256_3_4 = 3,
  // 64-QAM 9/10
  SDRKIT_DVBC2_GEN_MODCOD_64_9_10 = 4,
  // 256-QAM 9/10
  SDRKIT_DVBC2_GEN_MODCOD_256_9_10 = 5,
  // 1024-QAM 3/4
  SDRKIT_DVBC2_GEN_MODCOD_1024_3_4 = 6,
  // 4096-QAM 5/6
  SDRKIT_DVBC2_GEN_MODCOD_4096_5_6 = 7,
} sdrkit_dvbc2_gen_modcod;

// The options of the setting "gen_guard" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_guard {
  // 1/64
  SDRKIT_DVBC2_GEN_GUARD_1_64 = 0,
  // 1/128
  SDRKIT_DVBC2_GEN_GUARD_1_128 = 1,
} sdrkit_dvbc2_gen_guard;

// The options of the setting "gen_slice_type" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_slice_type {
  // Type 1
  SDRKIT_DVBC2_GEN_SLICE_TYPE_TYPE1 = 0,
  // Type 2, robust header
  SDRKIT_DVBC2_GEN_SLICE_TYPE_TYPE2_ROBUST = 1,
  // Type 2, high efficiency header
  SDRKIT_DVBC2_GEN_SLICE_TYPE_TYPE2_EFFICIENT = 2,
} sdrkit_dvbc2_gen_slice_type;

// The options of the setting "gen_ti_depth" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_ti_depth {
  // None
  SDRKIT_DVBC2_GEN_TI_DEPTH_NONE = 0,
  // 4 symbols
  SDRKIT_DVBC2_GEN_TI_DEPTH_4_SYMBOLS = 1,
  // 8 symbols
  SDRKIT_DVBC2_GEN_TI_DEPTH_8_SYMBOLS = 2,
  // 16 symbols
  SDRKIT_DVBC2_GEN_TI_DEPTH_16_SYMBOLS = 3,
} sdrkit_dvbc2_gen_ti_depth;

// The options of the setting "gen_l1_ti" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_l1_ti {
  // None
  SDRKIT_DVBC2_GEN_L1_TI_NONE = 0,
  // Best fit
  SDRKIT_DVBC2_GEN_L1_TI_BEST_FIT = 1,
  // 4 symbols
  SDRKIT_DVBC2_GEN_L1_TI_4_SYMBOLS = 2,
  // 8 symbols
  SDRKIT_DVBC2_GEN_L1_TI_8_SYMBOLS = 3,
} sdrkit_dvbc2_gen_l1_ti;

// The options of the setting "gen_system" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_system {
  // One slice, one PLP
  SDRKIT_DVBC2_GEN_SYSTEM_SINGLE = 0,
  // One slice, two PLPs
  SDRKIT_DVBC2_GEN_SYSTEM_TWO_PLPS = 1,
  // Data PLP and common PLP
  SDRKIT_DVBC2_GEN_SYSTEM_COMMON = 2,
  // PLP bundled over two slices
  SDRKIT_DVBC2_GEN_SYSTEM_BUNDLED = 3,
  // Two slices, two L1 blocks wide
  SDRKIT_DVBC2_GEN_SYSTEM_TWO_SLICES = 4,
  // A type 1 and a type 2 slice
  SDRKIT_DVBC2_GEN_SYSTEM_MIXED_SLICES = 5,
  // Two data PLPs and their common PLP
  SDRKIT_DVBC2_GEN_SYSTEM_GROUP = 6,
  // Group over two L1 blocks
  SDRKIT_DVBC2_GEN_SYSTEM_FAR_SLICE = 7,
} sdrkit_dvbc2_gen_system;

// The options of the setting "gen_stream" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_stream {
  // Transport stream
  SDRKIT_DVBC2_GEN_STREAM_TS = 0,
  // GSE
  SDRKIT_DVBC2_GEN_STREAM_GSE = 1,
  // GFPS
  SDRKIT_DVBC2_GEN_STREAM_GFPS = 2,
  // GCS
  SDRKIT_DVBC2_GEN_STREAM_GCS = 3,
} sdrkit_dvbc2_gen_stream;

// The options of the setting "gen_payload" of the stack "dvbc2".
typedef enum sdrkit_dvbc2_gen_payload {
  // Test multiplex
  SDRKIT_DVBC2_GEN_PAYLOAD_PROGRAMME = 0,
  // Numbered packets
  SDRKIT_DVBC2_GEN_PAYLOAD_NUMBERED = 1,
} sdrkit_dvbc2_gen_payload;

// The options of the setting "standard" of the stack "dvbs".
typedef enum sdrkit_dvbs_standard {
  // Auto
  SDRKIT_DVBS_STANDARD_AUTO = 0,
  // DVB-S only
  SDRKIT_DVBS_STANDARD_DVBS = 1,
  // DVB-S2 only
  SDRKIT_DVBS_STANDARD_DVBS2 = 2,
} sdrkit_dvbs_standard;

// The options of the setting "roll_off" of the stack "dvbs".
typedef enum sdrkit_dvbs_roll_off {
  // Auto
  SDRKIT_DVBS_ROLL_OFF_AUTO = 0,
  // 0.35
  SDRKIT_DVBS_ROLL_OFF_0_35 = 1,
  // 0.25
  SDRKIT_DVBS_ROLL_OFF_0_25 = 2,
  // 0.20
  SDRKIT_DVBS_ROLL_OFF_0_20 = 3,
  // 0.15
  SDRKIT_DVBS_ROLL_OFF_0_15 = 4,
  // 0.10
  SDRKIT_DVBS_ROLL_OFF_0_10 = 5,
  // 0.05
  SDRKIT_DVBS_ROLL_OFF_0_05 = 6,
} sdrkit_dvbs_roll_off;

// The options of the setting "modcod_rule" of the stack "dvbs".
typedef enum sdrkit_dvbs_modcod_rule {
  // Any
  SDRKIT_DVBS_MODCOD_RULE_ANY = 0,
  // Fixed
  SDRKIT_DVBS_MODCOD_RULE_FIXED = 1,
  // Set
  SDRKIT_DVBS_MODCOD_RULE_SET = 2,
} sdrkit_dvbs_modcod_rule;

// The options of the setting "frame_length" of the stack "dvbs".
typedef enum sdrkit_dvbs_frame_length {
  // Any
  SDRKIT_DVBS_FRAME_LENGTH_ANY = 0,
  // Normal
  SDRKIT_DVBS_FRAME_LENGTH_NORMAL = 1,
  // Short
  SDRKIT_DVBS_FRAME_LENGTH_SHORT = 2,
} sdrkit_dvbs_frame_length;

// The options of the setting "pilots" of the stack "dvbs".
typedef enum sdrkit_dvbs_pilots {
  // Any
  SDRKIT_DVBS_PILOTS_ANY = 0,
  // On
  SDRKIT_DVBS_PILOTS_ON = 1,
  // Off
  SDRKIT_DVBS_PILOTS_OFF = 2,
} sdrkit_dvbs_pilots;

// The options of the setting "tx_polarisation" of the stack "dvbs".
typedef enum sdrkit_dvbs_tx_polarisation {
  // Not set
  SDRKIT_DVBS_TX_POLARISATION_UNSET = 0,
  // Vertical
  SDRKIT_DVBS_TX_POLARISATION_V = 1,
  // Horizontal
  SDRKIT_DVBS_TX_POLARISATION_H = 2,
} sdrkit_dvbs_tx_polarisation;

// The options of the setting "lnb_supply" of the stack "dvbs".
typedef enum sdrkit_dvbs_lnb_supply {
  // Not set
  SDRKIT_DVBS_LNB_SUPPLY_UNSET = 0,
  // Vertical (about 13 V)
  SDRKIT_DVBS_LNB_SUPPLY_V = 1,
  // Horizontal (about 18 V)
  SDRKIT_DVBS_LNB_SUPPLY_H = 2,
} sdrkit_dvbs_lnb_supply;

// The options of the setting "gen_standard" of the stack "dvbs".
typedef enum sdrkit_dvbs_gen_standard {
  // DVB-S2
  SDRKIT_DVBS_GEN_STANDARD_DVBS2 = 0,
  // DVB-S
  SDRKIT_DVBS_GEN_STANDARD_DVBS = 1,
} sdrkit_dvbs_gen_standard;

// The options of the setting "gen_modulation" of the stack "dvbs".
typedef enum sdrkit_dvbs_gen_modulation {
  // QPSK
  SDRKIT_DVBS_GEN_MODULATION_Q_P_S_K = 0,
  // 8PSK
  SDRKIT_DVBS_GEN_MODULATION_8_P_S_K = 1,
  // 16APSK
  SDRKIT_DVBS_GEN_MODULATION_16_A_P_S_K = 2,
  // 32APSK
  SDRKIT_DVBS_GEN_MODULATION_32_A_P_S_K = 3,
} sdrkit_dvbs_gen_modulation;

// The options of the setting "gen_rate" of the stack "dvbs".
typedef enum sdrkit_dvbs_gen_rate {
  // 1/4
  SDRKIT_DVBS_GEN_RATE_1_4 = 0,
  // 1/3
  SDRKIT_DVBS_GEN_RATE_1_3 = 1,
  // 2/5
  SDRKIT_DVBS_GEN_RATE_2_5 = 2,
  // 1/2
  SDRKIT_DVBS_GEN_RATE_1_2 = 3,
  // 3/5
  SDRKIT_DVBS_GEN_RATE_3_5 = 4,
  // 2/3
  SDRKIT_DVBS_GEN_RATE_2_3 = 5,
  // 3/4
  SDRKIT_DVBS_GEN_RATE_3_4 = 6,
  // 4/5
  SDRKIT_DVBS_GEN_RATE_4_5 = 7,
  // 5/6
  SDRKIT_DVBS_GEN_RATE_5_6 = 8,
  // 7/8
  SDRKIT_DVBS_GEN_RATE_7_8 = 9,
  // 8/9
  SDRKIT_DVBS_GEN_RATE_8_9 = 10,
  // 9/10
  SDRKIT_DVBS_GEN_RATE_9_10 = 11,
} sdrkit_dvbs_gen_rate;

// The options of the setting "gen_rolloff" of the stack "dvbs".
typedef enum sdrkit_dvbs_gen_rolloff {
  // 0.35
  SDRKIT_DVBS_GEN_ROLLOFF_0_35 = 0,
  // 0.25
  SDRKIT_DVBS_GEN_ROLLOFF_0_25 = 1,
  // 0.20
  SDRKIT_DVBS_GEN_ROLLOFF_0_20 = 2,
} sdrkit_dvbs_gen_rolloff;

// The options of the setting "gen_payload" of the stack "dvbs".
typedef enum sdrkit_dvbs_gen_payload {
  // Test multiplex
  SDRKIT_DVBS_GEN_PAYLOAD_PROGRAMME = 0,
  // Numbered packets
  SDRKIT_DVBS_GEN_PAYLOAD_NUMBERED = 1,
} sdrkit_dvbs_gen_payload;

// The options of the setting "tx_rate" of the stack "dvbs".
typedef enum sdrkit_dvbs_tx_rate {
  // 1/2
  SDRKIT_DVBS_TX_RATE_1_2 = 0,
  // 2/3
  SDRKIT_DVBS_TX_RATE_2_3 = 1,
  // 3/4
  SDRKIT_DVBS_TX_RATE_3_4 = 2,
  // 5/6
  SDRKIT_DVBS_TX_RATE_5_6 = 3,
  // 7/8
  SDRKIT_DVBS_TX_RATE_7_8 = 4,
} sdrkit_dvbs_tx_rate;

// The options of the setting "tx_standard" of the stack "dvbs".
typedef enum sdrkit_dvbs_tx_standard {
  // DVB-S2
  SDRKIT_DVBS_TX_STANDARD_DVBS2 = 0,
  // DVB-S
  SDRKIT_DVBS_TX_STANDARD_DVBS = 1,
} sdrkit_dvbs_tx_standard;

// The options of the setting "tx_modcod" of the stack "dvbs".
typedef enum sdrkit_dvbs_tx_modcod {
  // QPSK 1/4
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_1_4 = 0,
  // QPSK 1/3
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_1_3 = 1,
  // QPSK 2/5
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_2_5 = 2,
  // QPSK 1/2
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_1_2 = 3,
  // QPSK 3/5
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_3_5 = 4,
  // QPSK 2/3
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_2_3 = 5,
  // QPSK 3/4
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_3_4 = 6,
  // QPSK 4/5
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_4_5 = 7,
  // QPSK 5/6
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_5_6 = 8,
  // QPSK 8/9
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_8_9 = 9,
  // QPSK 9/10
  SDRKIT_DVBS_TX_MODCOD_Q_P_S_K_9_10 = 10,
  // 8PSK 3/5
  SDRKIT_DVBS_TX_MODCOD_8_P_S_K_3_5 = 11,
  // 8PSK 2/3
  SDRKIT_DVBS_TX_MODCOD_8_P_S_K_2_3 = 12,
  // 8PSK 3/4
  SDRKIT_DVBS_TX_MODCOD_8_P_S_K_3_4 = 13,
  // 8PSK 5/6
  SDRKIT_DVBS_TX_MODCOD_8_P_S_K_5_6 = 14,
  // 8PSK 8/9
  SDRKIT_DVBS_TX_MODCOD_8_P_S_K_8_9 = 15,
  // 8PSK 9/10
  SDRKIT_DVBS_TX_MODCOD_8_P_S_K_9_10 = 16,
  // 16APSK 2/3
  SDRKIT_DVBS_TX_MODCOD_16_A_P_S_K_2_3 = 17,
  // 16APSK 3/4
  SDRKIT_DVBS_TX_MODCOD_16_A_P_S_K_3_4 = 18,
  // 16APSK 4/5
  SDRKIT_DVBS_TX_MODCOD_16_A_P_S_K_4_5 = 19,
  // 16APSK 5/6
  SDRKIT_DVBS_TX_MODCOD_16_A_P_S_K_5_6 = 20,
  // 16APSK 8/9
  SDRKIT_DVBS_TX_MODCOD_16_A_P_S_K_8_9 = 21,
  // 16APSK 9/10
  SDRKIT_DVBS_TX_MODCOD_16_A_P_S_K_9_10 = 22,
  // 32APSK 3/4
  SDRKIT_DVBS_TX_MODCOD_32_A_P_S_K_3_4 = 23,
  // 32APSK 4/5
  SDRKIT_DVBS_TX_MODCOD_32_A_P_S_K_4_5 = 24,
  // 32APSK 5/6
  SDRKIT_DVBS_TX_MODCOD_32_A_P_S_K_5_6 = 25,
  // 32APSK 8/9
  SDRKIT_DVBS_TX_MODCOD_32_A_P_S_K_8_9 = 26,
  // 32APSK 9/10
  SDRKIT_DVBS_TX_MODCOD_32_A_P_S_K_9_10 = 27,
} sdrkit_dvbs_tx_modcod;

// The options of the setting "tx_frame" of the stack "dvbs".
typedef enum sdrkit_dvbs_tx_frame {
  // Normal
  SDRKIT_DVBS_TX_FRAME_NORMAL = 0,
  // Short
  SDRKIT_DVBS_TX_FRAME_SHORT = 1,
} sdrkit_dvbs_tx_frame;

// The options of the setting "tx_rolloff" of the stack "dvbs".
typedef enum sdrkit_dvbs_tx_rolloff {
  // 0.35
  SDRKIT_DVBS_TX_ROLLOFF_0_35 = 0,
  // 0.25
  SDRKIT_DVBS_TX_ROLLOFF_0_25 = 1,
  // 0.20
  SDRKIT_DVBS_TX_ROLLOFF_0_20 = 2,
} sdrkit_dvbs_tx_rolloff;

// The options of the setting "gen_layout" of the stack "isdbs".
typedef enum sdrkit_isdbs_gen_layout {
  // TC8PSK 36, QPSK 3/4 8, BPSK 4
  SDRKIT_ISDBS_GEN_LAYOUT_TC8PSK_36_QPSK_3_4_8_BPSK_4 = 0,
  // TC8PSK 48
  SDRKIT_ISDBS_GEN_LAYOUT_TC8PSK_48 = 1,
  // QPSK 1/2 48
  SDRKIT_ISDBS_GEN_LAYOUT_QPSK_1_2_48 = 2,
} sdrkit_isdbs_gen_layout;

// The options of `state` of `sdrkit_satbeacon_fields`.
typedef enum sdrkit_satbeacon_fields_state {
  // No carrier
  SDRKIT_SATBEACON_FIELDS_STATE_NO_CARRIER = 0,
  // Carrier locked
  SDRKIT_SATBEACON_FIELDS_STATE_CARRIER_LOCKED = 1,
  // Bit clock locked
  SDRKIT_SATBEACON_FIELDS_STATE_CLOCK_LOCKED = 2,
  // Frames found
  SDRKIT_SATBEACON_FIELDS_STATE_FRAMES_FOUND = 3,
} sdrkit_satbeacon_fields_state;

// The options of `coding` of `sdrkit_satbeacon_fields`.
typedef enum sdrkit_satbeacon_fields_coding {
  // NRZ
  SDRKIT_SATBEACON_FIELDS_CODING_NRZ = 0,
  // Bi-phase
  SDRKIT_SATBEACON_FIELDS_CODING_BIPHASE = 1,
} sdrkit_satbeacon_fields_coding;

// The options of `sync_word` of `sdrkit_satbeacon_fields`.
typedef enum sdrkit_satbeacon_fields_sync_word {
  // FAF320 (IRIG 106)
  SDRKIT_SATBEACON_FIELDS_SYNC_WORD_FAF320 = 0,
  // FE6B2840 (IRIG 106)
  SDRKIT_SATBEACON_FIELDS_SYNC_WORD_FE6B2840 = 1,
  // EB90 (IRIG 106)
  SDRKIT_SATBEACON_FIELDS_SYNC_WORD_EB90 = 2,
  // 1ACFFC1D (CCSDS)
  SDRKIT_SATBEACON_FIELDS_SYNC_WORD_1ACFFC1D = 3,
} sdrkit_satbeacon_fields_sync_word;

// The result fields of `satbeacon`: the carrier, the subcarrier, the bit clock and the frame learner. A measurement of the subcarrier has no value without a subcarrier.
typedef struct sdrkit_satbeacon_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the decoder does now. A value of `sdrkit_satbeacon_fields_state`.
  int32_t state;
  // The signal time decoded. A length of time in seconds.
  double signal_time;
  // The work rate of the carrier loop. A frequency in Hz.
  double work_rate;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the centre. A frequency in Hz.
  double offset_hz;
  // True if `rf` holds a value.
  bool has_rf;
  // The RF frequency of the carrier. A frequency in Hz.
  double rf;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // The carrier to noise density. Unit: dB-Hz.
  double cn0_dbhz;
  // The drift of the carrier. Unit: Hz/min.
  double drift_hz_min;
  // The noise density. Unit: dBFS/Hz.
  double noise_density_dbfs;
  // The carrier loop is locked.
  bool carrier_locked;
  // The rms of the phase modulation. Unit: °.
  double pm_rms_deg;
  // True if `subcarrier` holds a value.
  bool has_subcarrier;
  // The subcarrier frequency. A frequency in Hz.
  double subcarrier;
  // True if `tones` holds a value.
  bool has_tones;
  // The humps of the subcarrier spectrum.
  int64_t tones;
  // True if `tone_spacing` holds a value.
  bool has_tone_spacing;
  // Their spacing. A frequency in Hz.
  double tone_spacing;
  // True if `subcarrier_snr_db` holds a value.
  bool has_subcarrier_snr_db;
  // The subcarrier over the noise of the phase modulation. Unit: dB.
  double subcarrier_snr_db;
  // True if `bit_rate_bps` holds a value.
  bool has_bit_rate_bps;
  // The bit rate. No value before the bit clock is found. Unit: bit/s.
  double bit_rate_bps;
  // True if `coding` holds a value.
  bool has_coding;
  // The line code. A value of `sdrkit_satbeacon_fields_coding`.
  int32_t coding;
  // The bit clock is locked.
  bool clock_locked;
  // True if `frame_bits` holds a value.
  bool has_frame_bits;
  // The frame length. No value before the frames are found. Unit: bits.
  int64_t frame_bits;
  // The agreement of the frames with the template.
  double frame_agreement;
  // Bits that do not change from frame to frame.
  int64_t fixed_bits;
  // True if `sync_word` holds a value.
  bool has_sync_word;
  // The sync word. A value of `sdrkit_satbeacon_fields_sync_word`.
  int32_t sync_word;
  // True if `counter_byte` holds a value.
  bool has_counter_byte;
  // The byte of the frame counter.
  int64_t counter_byte;
  // True if `counter_modulo` holds a value.
  bool has_counter_modulo;
  // The modulo of the frame counter.
  int64_t counter_modulo;
  // Bits received.
  int64_t bits;
  // Frames good.
  int64_t frames_ok;
  // Frames bad.
  int64_t frames_bad;
  // The template of a frame.
  sdrkit_bytes template_;
  // The mask of the fixed bits, one bit for each bit of the frame.
  sdrkit_bytes fixed_mask;
  // True if `pilot_spacing` holds a value.
  bool has_pilot_spacing;
  // The spacing of a second carrier above the beacon. A frequency in Hz.
  double pilot_spacing;
  // The lines of the spectrum.
  int64_t lines;
  // True if `first_frames` holds a value.
  bool has_first_frames;
  // The signal time when the first frames were found. A length of time in seconds.
  double first_frames;
} sdrkit_satbeacon_fields;

// A row of the narrow lines of the spectrum, strongest first. The row key is the place.
typedef struct sdrkit_satbeacon_line_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset of the line from the centre. A frequency in Hz.
  double offset;
  // Its level. Unit: dBFS.
  double level_dbfs;
  // Its height over the noise. Unit: dB.
  double over_noise_db;
} sdrkit_satbeacon_line_row;

// A row of the frames received. The row key is the frame number.
typedef struct sdrkit_satbeacon_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frame number.
  int64_t frame;
  // True if `counter` holds a value.
  bool has_counter;
  // The frame counter.
  int64_t counter;
  // The errors against the template.
  int64_t errors;
  // The frame is good.
  bool good;
  // The bytes of the frame.
  sdrkit_bytes bytes;
  // The signal time when the frame was found. Unit: s.
  double time_s;
} sdrkit_satbeacon_frame_row;

// The options of `kind` of `sdrkit_satbeacon_decoded_row`.
typedef enum sdrkit_satbeacon_decoded_row_kind {
  // Field
  SDRKIT_SATBEACON_DECODED_ROW_KIND_FIELD = 0,
  // Summary
  SDRKIT_SATBEACON_DECODED_ROW_KIND_SUMMARY = 1,
} sdrkit_satbeacon_decoded_row_kind;

// The options of `confidence` of `sdrkit_satbeacon_decoded_row`.
typedef enum sdrkit_satbeacon_decoded_row_confidence {
  // Decoded
  SDRKIT_SATBEACON_DECODED_ROW_CONFIDENCE_DECODED = 0,
  // Estimated
  SDRKIT_SATBEACON_DECODED_ROW_CONFIDENCE_ESTIMATED = 1,
  // Meaning not known
  SDRKIT_SATBEACON_DECODED_ROW_CONFIDENCE_UNKNOWN = 2,
} sdrkit_satbeacon_decoded_row_confidence;

// A row of the decoded view. The row key is the name and the bytes.
typedef struct sdrkit_satbeacon_decoded_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the field.
  sdrkit_string name;
  // What the line is. A value of `sdrkit_satbeacon_decoded_row_kind`.
  int32_t kind;
  // Its value, for people.
  sdrkit_string value;
  // Its bytes in the frame, for people.
  sdrkit_string bytes;
  // How sure the reading is. A value of `sdrkit_satbeacon_decoded_row_confidence`.
  int32_t confidence;
  // Why, for people.
  sdrkit_string reason;
} sdrkit_satbeacon_decoded_row;

// The options of `stage` of `sdrkit_dvbc_fields`.
typedef enum sdrkit_dvbc_fields_stage {
  // Measuring the spectrum
  SDRKIT_DVBC_FIELDS_STAGE_SPECTRUM = 0,
  // Blind equaliser (CMA)
  SDRKIT_DVBC_FIELDS_STAGE_CMA = 1,
  // Carrier frequency
  SDRKIT_DVBC_FIELDS_STAGE_FREQ = 2,
  // Carrier phase
  SDRKIT_DVBC_FIELDS_STAGE_PHASE = 3,
  // Trying the QAM order
  SDRKIT_DVBC_FIELDS_STAGE_DD = 4,
  // Checking the QAM order
  SDRKIT_DVBC_FIELDS_STAGE_TRIAL = 5,
  // Decoding
  SDRKIT_DVBC_FIELDS_STAGE_LOCKED = 6,
} sdrkit_dvbc_fields_stage;

// The options of `note` of `sdrkit_dvbc_fields`.
typedef enum sdrkit_dvbc_fields_note {
  // No carrier in the spectrum
  SDRKIT_DVBC_FIELDS_NOTE_NO_CARRIER = 0,
  // Symbol rate not usable for this input rate
  SDRKIT_DVBC_FIELDS_NOTE_RATE_NOT_USABLE = 1,
  // No lock at the manual symbol rate: the measured one differs
  SDRKIT_DVBC_FIELDS_NOTE_MANUAL_RATE_DIFFERS = 2,
  // No lock at the manual symbol rate: the signal can be too weak for this QAM order
  SDRKIT_DVBC_FIELDS_NOTE_MANUAL_RATE_NO_LOCK = 3,
} sdrkit_dvbc_fields_note;

// The options of `qam` of `sdrkit_dvbc_fields`.
typedef enum sdrkit_dvbc_fields_qam {
  // 16-QAM
  SDRKIT_DVBC_FIELDS_QAM_16_QAM = 0,
  // 32-QAM
  SDRKIT_DVBC_FIELDS_QAM_32_QAM = 1,
  // 64-QAM
  SDRKIT_DVBC_FIELDS_QAM_64_QAM = 2,
  // 128-QAM
  SDRKIT_DVBC_FIELDS_QAM_128_QAM = 3,
  // 256-QAM
  SDRKIT_DVBC_FIELDS_QAM_256_QAM = 4,
} sdrkit_dvbc_fields_qam;

// The options of `annex` of `sdrkit_dvbc_fields`.
typedef enum sdrkit_dvbc_fields_annex {
  // A (8 MHz)
  SDRKIT_DVBC_FIELDS_ANNEX_A = 0,
  // C (6 MHz)
  SDRKIT_DVBC_FIELDS_ANNEX_C = 1,
} sdrkit_dvbc_fields_annex;

// The result fields of `dvbc`: the carrier, the demodulator, the outer decoder and the locks. A measurement of the carrier has no value before a carrier is found.
typedef struct sdrkit_dvbc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the demodulator is doing. A value of `sdrkit_dvbc_fields_stage`.
  int32_t stage;
  // True if `note` holds a value.
  bool has_note;
  // The reason the demodulator does not lock, when known. A value of `sdrkit_dvbc_fields_note`.
  int32_t note;
  // True if `qam` holds a value.
  bool has_qam;
  // The QAM order in use. A value of `sdrkit_dvbc_fields_qam`.
  int32_t qam;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate in use. A frequency in Hz.
  double symbol_rate_hz;
  // True if `measured_rate` holds a value.
  bool has_measured_rate;
  // The symbol rate measured from the spectrum. A frequency in Hz.
  double measured_rate;
  // The manual symbol rate is in use.
  bool manual_rate;
  // True if `width` holds a value.
  bool has_width;
  // The -3 dB width of the carrier. A frequency in Hz.
  double width;
  // The annex of J.83. A value of `sdrkit_dvbc_fields_annex`.
  int32_t annex;
  // The roll-off of the annex.
  double roll_off;
  // The spectrum is inverted.
  bool inverted;
  // The error of the symbol clock. Unit: ppm.
  double clock_ppm;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset from the centre. A frequency in Hz.
  double offset_hz;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The modulation error ratio. Unit: dB.
  double mer_db;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // The fraction of bytes Reed-Solomon changed.
  double pre_rs;
  // Reed-Solomon words with no error.
  int64_t rs_clean;
  // Reed-Solomon words corrected.
  int64_t rs_corrected;
  // Reed-Solomon words that failed.
  int64_t rs_failed;
  // Byte re-alignments without a loss of lock.
  int64_t realigned;
  // True if `net_bitrate_bps` holds a value.
  bool has_net_bitrate_bps;
  // The net bit rate. No value while the receiver is not locked. Unit: bit/s.
  double net_bitrate_bps;
  // The carrier is found in the spectrum.
  bool spectrum_lock;
  // The symbol timing holds.
  bool timing_lock;
  // The equaliser has converged.
  bool equaliser_lock;
  // The carrier loop holds.
  bool carrier_lock;
  // The sync bytes are found.
  bool sync_lock;
  // The transport stream is valid.
  bool ts_lock;
  // Acquisitions started again.
  int64_t searches;
  // Frames good.
  int64_t frames_ok;
  // Frames bad.
  int64_t frames_bad;
} sdrkit_dvbc_fields;

// The options of `state` of `sdrkit_dvbc2_fields`.
typedef enum sdrkit_dvbc2_fields_state {
  // Searching
  SDRKIT_DVBC2_FIELDS_STATE_SEARCHING = 0,
  // Timing found
  SDRKIT_DVBC2_FIELDS_STATE_TIMING = 1,
  // Decoding the L1
  SDRKIT_DVBC2_FIELDS_STATE_L1 = 2,
  // Data
  SDRKIT_DVBC2_FIELDS_STATE_DATA = 3,
} sdrkit_dvbc2_fields_state;

// The options of `guard` of `sdrkit_dvbc2_fields`.
typedef enum sdrkit_dvbc2_fields_guard {
  // 1/64
  SDRKIT_DVBC2_FIELDS_GUARD_1_64 = 0,
  // 1/128
  SDRKIT_DVBC2_FIELDS_GUARD_1_128 = 1,
} sdrkit_dvbc2_fields_guard;

// The result fields of the receiver of `dvbc2`: the signal, the L1 signalling and the FEC. A value of the L1 has no value before the L1 is read.
typedef struct sdrkit_dvbc2_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the receiver does now. A value of `sdrkit_dvbc2_fields_state`.
  int32_t state;
  // True if `guard` holds a value.
  bool has_guard;
  // The guard interval. A value of `sdrkit_dvbc2_fields_guard`.
  int32_t guard;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset from the centre. No value while the receiver searches. A frequency in Hz.
  double offset_hz;
  // The error of the sample clock. Unit: ppm.
  double clock_ppm;
  // True if `window_carrier` holds a value.
  bool has_window_carrier;
  // The carrier at the centre of the receive window.
  int64_t window_carrier;
  // True if `window_hz` holds a value.
  bool has_window_hz;
  // The frequency of the centre of the receive window. A frequency in Hz.
  double window_hz;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // Es/N0. Unit: dB.
  double esn0_db;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The modulation error ratio. Unit: dB.
  double mer_db;
  // True if `network_id` holds a value.
  bool has_network_id;
  // The network id of the L1.
  int64_t network_id;
  // True if `system_id` holds a value.
  bool has_system_id;
  // The C2 system id of the L1.
  int64_t system_id;
  // True if `start_frequency_hz` holds a value.
  bool has_start_frequency_hz;
  // The start frequency of the C2 system. A frequency in Hz.
  double start_frequency_hz;
  // True if `bandwidth_hz` holds a value.
  bool has_bandwidth_hz;
  // The width of the C2 system. A frequency in Hz.
  double bandwidth_hz;
  // True if `change_counter` holds a value.
  bool has_change_counter;
  // The change counter of the L1.
  int64_t change_counter;
  // True if `ews` holds a value.
  bool has_ews;
  // The early warning flag of the L1.
  bool ews;
  // True if `c2_version` holds a value.
  bool has_c2_version;
  // The C2 version of the L1.
  int64_t c2_version;
  // True if `reserved_tones` holds a value.
  bool has_reserved_tones;
  // Reserved tones are in use.
  bool reserved_tones;
  // FEC blocks good.
  int64_t blocks_ok;
  // FEC blocks that failed.
  int64_t blocks_bad;
  // L1 blocks that failed.
  int64_t l1_bad;
  // Losses of the frame sync.
  int64_t sync_losses;
  // True if `pre_ber` holds a value.
  bool has_pre_ber;
  // The bit error ratio before the LDPC decoder.
  double pre_ber;
  // True if `mean_iterations` holds a value.
  bool has_mean_iterations;
  // LDPC iterations, recent mean.
  double mean_iterations;
  // C2 frames received.
  int64_t c2_frames;
  // True if `chosen_slice` holds a value.
  bool has_chosen_slice;
  // The data slice decoded.
  int64_t chosen_slice;
  // True if `chosen_plp` holds a value.
  bool has_chosen_plp;
  // The PLP decoded.
  int64_t chosen_plp;
  // True if `common_plp` holds a value.
  bool has_common_plp;
  // The common PLP of the PLP decoded.
  int64_t common_plp;
  // True if `bundled_slices` holds a value.
  bool has_bundled_slices;
  // The data slices that carry the PLP, when more than one.
  int64_t bundled_slices;
  // True if `retune_hz` holds a value.
  bool has_retune_hz;
  // The frequency to tune to for the chosen data slice, when it is outside the window. A frequency in Hz.
  double retune_hz;
  // True if `outside_slice` holds a value.
  bool has_outside_slice;
  // The data slice that the decoding needs and that reaches out of the receive window.
  int64_t outside_slice;
  // True if `outside_hz` holds a value.
  bool has_outside_hz;
  // How far that data slice reaches beyond the edge of the receive window. A frequency in Hz.
  double outside_hz;
  // Packets of a generic stream.
  int64_t generic_packets;
  // Bytes of a generic stream.
  int64_t generic_bytes;
  // A note on what the receiver decodes.
  sdrkit_string note;
} sdrkit_dvbc2_fields;

// The counts of the stream adaptation and the IP layer that `dvbc2` shows under its own keys.
typedef struct sdrkit_dvbc2_stream_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Transport packets of the chosen PLP.
  int64_t ts_packets;
  // GSE PDUs.
  int64_t pdus;
  // IP packets.
  int64_t ip_packets;
} sdrkit_dvbc2_stream_fields;

// The options of `slice_type` of `sdrkit_dvbc2_slice_row`.
typedef enum sdrkit_dvbc2_slice_row_slice_type {
  // Type 1
  SDRKIT_DVBC2_SLICE_ROW_SLICE_TYPE_TYPE1 = 0,
  // Type 2
  SDRKIT_DVBC2_SLICE_ROW_SLICE_TYPE_TYPE2 = 1,
} sdrkit_dvbc2_slice_row_slice_type;

// The options of `header_type` of `sdrkit_dvbc2_slice_row`.
typedef enum sdrkit_dvbc2_slice_row_header_type {
  // None
  SDRKIT_DVBC2_SLICE_ROW_HEADER_TYPE_NONE = 0,
  // Robust
  SDRKIT_DVBC2_SLICE_ROW_HEADER_TYPE_ROBUST = 1,
  // High efficiency
  SDRKIT_DVBC2_SLICE_ROW_HEADER_TYPE_EFFICIENT = 2,
} sdrkit_dvbc2_slice_row_header_type;

// A row of the data slices of the L1. The row key is DSLICE_ID.
typedef struct sdrkit_dvbc2_slice_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // DSLICE_ID.
  int64_t slice_id;
  // DSLICE_TYPE. A value of `sdrkit_dvbc2_slice_row_slice_type`.
  int32_t slice_type;
  // The FECFrame header of a type 2 slice. A value of `sdrkit_dvbc2_slice_row_header_type`.
  int32_t header_type;
  // The tuning position of the slice. A frequency in Hz.
  double tune_hz;
  // True if `start_hz` holds a value.
  bool has_start_hz;
  // The first carrier of the slice. A frequency in Hz.
  double start_hz;
  // True if `end_hz` holds a value.
  bool has_end_hz;
  // The last carrier of the slice. A frequency in Hz.
  double end_hz;
  // The depth of the time interleaver. Unit: symbols.
  int64_t ti_depth;
  // The PLPs of the slice.
  int64_t plp_count;
  // The slice is inside the receive window.
  bool in_window;
  // True if `width_hz` holds a value.
  bool has_width_hz;
  // The width of the slice. A frequency in Hz.
  double width_hz;
} sdrkit_dvbc2_slice_row;

// The options of `plp_type` of `sdrkit_dvbc2_plp_row`.
typedef enum sdrkit_dvbc2_plp_row_plp_type {
  // Common
  SDRKIT_DVBC2_PLP_ROW_PLP_TYPE_COMMON = 0,
  // Grouped data
  SDRKIT_DVBC2_PLP_ROW_PLP_TYPE_GROUPED = 1,
  // Normal data
  SDRKIT_DVBC2_PLP_ROW_PLP_TYPE_NORMAL = 2,
  // Reserved
  SDRKIT_DVBC2_PLP_ROW_PLP_TYPE_RESERVED = 3,
} sdrkit_dvbc2_plp_row_plp_type;

// The options of `payload` of `sdrkit_dvbc2_plp_row`.
typedef enum sdrkit_dvbc2_plp_row_payload {
  // GFPS
  SDRKIT_DVBC2_PLP_ROW_PAYLOAD_GFPS = 0,
  // GCS
  SDRKIT_DVBC2_PLP_ROW_PAYLOAD_GCS = 1,
  // GSE
  SDRKIT_DVBC2_PLP_ROW_PAYLOAD_GSE = 2,
  // TS
  SDRKIT_DVBC2_PLP_ROW_PAYLOAD_TS = 3,
  // Reserved
  SDRKIT_DVBC2_PLP_ROW_PAYLOAD_RESERVED = 4,
} sdrkit_dvbc2_plp_row_payload;

// The options of `modulation` of `sdrkit_dvbc2_plp_row`.
typedef enum sdrkit_dvbc2_plp_row_modulation {
  // 16-QAM
  SDRKIT_DVBC2_PLP_ROW_MODULATION_16_QAM = 0,
  // 64-QAM
  SDRKIT_DVBC2_PLP_ROW_MODULATION_64_QAM = 1,
  // 256-QAM
  SDRKIT_DVBC2_PLP_ROW_MODULATION_256_QAM = 2,
  // 1024-QAM
  SDRKIT_DVBC2_PLP_ROW_MODULATION_1024_QAM = 3,
  // 4096-QAM
  SDRKIT_DVBC2_PLP_ROW_MODULATION_4096_QAM = 4,
} sdrkit_dvbc2_plp_row_modulation;

// The options of `code_rate` of `sdrkit_dvbc2_plp_row`.
typedef enum sdrkit_dvbc2_plp_row_code_rate {
  // 2/3
  SDRKIT_DVBC2_PLP_ROW_CODE_RATE_2_3 = 0,
  // 3/4
  SDRKIT_DVBC2_PLP_ROW_CODE_RATE_3_4 = 1,
  // 4/5
  SDRKIT_DVBC2_PLP_ROW_CODE_RATE_4_5 = 2,
  // 5/6
  SDRKIT_DVBC2_PLP_ROW_CODE_RATE_5_6 = 3,
  // 9/10 (64K) or 8/9 (16K)
  SDRKIT_DVBC2_PLP_ROW_CODE_RATE_9_10 = 4,
} sdrkit_dvbc2_plp_row_code_rate;

// A row of the PLPs of the L1. The row key is `<slice>/<PLP_ID>`.
typedef struct sdrkit_dvbc2_plp_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The data slice of the PLP.
  int64_t plp_slice;
  // PLP_ID.
  int64_t plp_id;
  // PLP_TYPE. A value of `sdrkit_dvbc2_plp_row_plp_type`.
  int32_t plp_type;
  // PLP_PAYLOAD_TYPE. A value of `sdrkit_dvbc2_plp_row_payload`.
  int32_t payload;
  // True if `fec_frame_bits` holds a value.
  bool has_fec_frame_bits;
  // The length of the FEC frame. Unit: bits.
  int64_t fec_frame_bits;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The constellation. A value of `sdrkit_dvbc2_plp_row_modulation`.
  int32_t modulation;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The code rate. A value of `sdrkit_dvbc2_plp_row_code_rate`.
  int32_t code_rate;
  // True if `ts_id` holds a value.
  bool has_ts_id;
  // The transport stream id. No value for a PLP with PSI reprocessing.
  int64_t ts_id;
  // True if `on_id` holds a value.
  bool has_on_id;
  // The original network id. No value for a PLP with PSI reprocessing.
  int64_t on_id;
  // It is the PLP decoded.
  bool chosen;
  // True if `group` holds a value.
  bool has_group;
  // PLP_GROUP_ID. No value for a normal data PLP.
  int64_t group;
  // PLP_BUNDLED: the PLP is spread over several data slices.
  bool bundled;
} sdrkit_dvbc2_plp_row;

// A row of the notches of the L1. The row key is the number of the notch.
typedef struct sdrkit_dvbc2_notch_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `notch_start_hz` holds a value.
  bool has_notch_start_hz;
  // The first carrier of the notch. A frequency in Hz.
  double notch_start_hz;
  // True if `notch_end_hz` holds a value.
  bool has_notch_end_hz;
  // The last carrier of the notch. A frequency in Hz.
  double notch_end_hz;
} sdrkit_dvbc2_notch_row;

// The result fields of the sender `transmit` of `dvbs`.
typedef struct sdrkit_dvbs_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the centre of the carrier sent. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // The bit rate of the transport stream that the settings carry: the rate that the host feeds at most. Unit: bit/s.
  double tx_bitrate_bps;
  // The packets of the host sent in this transmission, or in the last one.
  int64_t tx_packets;
  // The null packets put in slots that found no packet of the host.
  int64_t tx_nulls;
  // The packets that wait to be sent.
  int64_t tx_queue;
  // The time that the waiting packets take at the useful bit rate. A length of time in seconds.
  double tx_queue_s;
  // The PCRs rewritten to the time of their slot.
  int64_t tx_pcr_restamped;
} sdrkit_dvbs_tx_fields;

// The options of `state` of `sdrkit_isdbs_fields`.
typedef enum sdrkit_isdbs_fields_state {
  // Searching
  SDRKIT_ISDBS_FIELDS_STATE_SEARCHING = 0,
  // Frame sync
  SDRKIT_ISDBS_FIELDS_STATE_FRAME_SYNC = 1,
  // TMCC decoded
  SDRKIT_ISDBS_FIELDS_STATE_TMCC = 2,
  // Transport stream
  SDRKIT_ISDBS_FIELDS_STATE_TS = 3,
} sdrkit_isdbs_fields_state;

// The result fields of `isdbs`: the demodulator, the TMCC and the outer decoder. The offset has no value before the frame sync is found.
typedef struct sdrkit_isdbs_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the receiver does now. A value of `sdrkit_isdbs_fields_state`.
  int32_t state;
  // The frame sync words are found.
  bool frame_sync;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset from the centre. A frequency in Hz.
  double offset_hz;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // Es/N0. Unit: dB.
  double esn0_db;
  // Frames received.
  int64_t frames;
  // Super-frames received.
  int64_t superframes;
  // Losses of the frame sync.
  int64_t sync_losses;
  // TMCC words good.
  int64_t tmcc_good;
  // TMCC words that failed.
  int64_t tmcc_bad;
  // True if `change_counter` holds a value.
  bool has_change_counter;
  // The change counter of the TMCC.
  int64_t change_counter;
  // Reed-Solomon words with no error.
  int64_t rs_clean;
  // Reed-Solomon words corrected.
  int64_t rs_corrected;
  // Reed-Solomon words that failed.
  int64_t rs_failed;
  // True if `stream` holds a value.
  bool has_stream;
  // The stream decoded.
  int64_t stream;
  // The emergency warning flag of the TMCC.
  bool ews;
  // The transport stream is valid.
  bool ts_lock;
} sdrkit_isdbs_fields;

// The options of `modulation` of `sdrkit_isdbs_stream_row`.
typedef enum sdrkit_isdbs_stream_row_modulation {
  // BPSK 1/2
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_BPSK_1_2 = 0,
  // QPSK 1/2
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_QPSK_1_2 = 1,
  // QPSK 2/3
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_QPSK_2_3 = 2,
  // QPSK 3/4
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_QPSK_3_4 = 3,
  // QPSK 5/6
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_QPSK_5_6 = 4,
  // QPSK 7/8
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_QPSK_7_8 = 5,
  // TC8PSK 2/3
  SDRKIT_ISDBS_STREAM_ROW_MODULATION_TC8PSK_2_3 = 6,
} sdrkit_isdbs_stream_row_modulation;

// A row of the streams of the transponder. The row key is the stream number.
typedef struct sdrkit_isdbs_stream_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The stream number in the TMCC.
  int64_t stream;
  // The transport stream id.
  int64_t ts_id;
  // The slots of the stream.
  int64_t slots;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The modulation and code. No value for a stream with mixed modulation. A value of `sdrkit_isdbs_stream_row_modulation`.
  int32_t modulation;
  // The slots of the stream differ in modulation.
  bool mixed;
  // The bit rate of the stream. Unit: bit/s.
  double bitrate_bps;
  // It is the stream decoded.
  bool decoded;
} sdrkit_isdbs_stream_row;

// The fields of a telemetry frame, when it is found.
typedef struct sdrkit_satbeacon_beacon_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The frame number.
  int64_t frame;
  // True if `counter` holds a value.
  bool has_counter;
  // The frame counter.
  int64_t counter;
  // The errors against the template.
  int64_t errors;
  // The frame is good.
  bool good;
  // The bytes of the frame.
  sdrkit_bytes bytes;
} sdrkit_satbeacon_beacon_frame_event;

// The fields of the carrier measurements of one second of signal.
typedef struct sdrkit_satbeacon_beacon_carrier_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time. A length of time in seconds.
  double signal_time;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the centre. A frequency in Hz.
  double offset_hz;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // The carrier to noise density. Unit: dB-Hz.
  double cn0_dbhz;
  // The drift of the carrier. Unit: Hz/min.
  double drift_hz_min;
  // True if `bit_rate_bps` holds a value.
  bool has_bit_rate_bps;
  // The bit rate. No value before the bit clock is found. Unit: bit/s.
  double bit_rate_bps;
  // True if `frame_bits` holds a value.
  bool has_frame_bits;
  // The frame length. No value before the frames are found. Unit: bits.
  int64_t frame_bits;
} sdrkit_satbeacon_beacon_carrier_event;

// The arguments of the `append` command of a sender that takes transport packets.
typedef struct sdrkit_dvbs_tx_append_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The packets.
  sdrkit_bytes packets;
} sdrkit_dvbs_tx_append_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBS_TX_APPEND_ARGS_INIT {sizeof(sdrkit_dvbs_tx_append_args), {0}}

// Fills `rows` with the changed rows of the table "lines" of the stack "satbeacon" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_dbfs`, highest first.
static inline int32_t sdrkit_satbeacon_lines_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satbeacon_line_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.SatbeaconLineRow", "lines", rows, sizeof(sdrkit_satbeacon_line_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "satbeacon" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_satbeacon_frames_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satbeacon_frame_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.SatbeaconFrameRow", "frames", rows, sizeof(sdrkit_satbeacon_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoded" of the stack "satbeacon" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_satbeacon_decoded_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_satbeacon_decoded_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.SatbeaconDecodedRow", "decoded", rows, sizeof(sdrkit_satbeacon_decoded_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "beacon.frame"
// (Telemetry frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_satbeacon_beacon_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_satbeacon_beacon_frame_event *event) {
  return sdrkit_typed_event(handle, n, "beacon.frame", "dvbcs.SatbeaconFrameEvent", event, sizeof(sdrkit_satbeacon_beacon_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "beacon.carrier"
// (Beacon carrier). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_satbeacon_beacon_carrier_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_satbeacon_beacon_carrier_event *event) {
  return sdrkit_typed_event(handle, n, "beacon.carrier", "dvbcs.SatbeaconCarrierEvent", event, sizeof(sdrkit_satbeacon_beacon_carrier_event));
}

// Runs the command "clear" (Clear) of the stack "satbeacon" on the session of `handle`. Forgets the frames learned.
static inline int32_t sdrkit_satbeacon_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "satbeacon" are the sdrkit_satbeacon_gen_*() functions
// after the settings of its receiver.

// The setting "gen_bitrate" (Bit rate) of the test signal generator of the stack "satbeacon". Unit: bit/s.
static inline int32_t sdrkit_satbeacon_gen_get_bitrate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bitrate", value);
}
static inline int32_t sdrkit_satbeacon_gen_set_bitrate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bitrate", value);
}

// The setting "gen_subcarrier_khz" (Subcarrier) of the test signal generator of the stack "satbeacon". Unit: kHz.
static inline int32_t sdrkit_satbeacon_gen_get_subcarrier_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_subcarrier_khz", value);
}
static inline int32_t sdrkit_satbeacon_gen_set_subcarrier_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_subcarrier_khz", value);
}

// The setting "gen_nrz" (NRZ instead of bi-phase) of the test signal generator of the stack "satbeacon".
static inline int32_t sdrkit_satbeacon_gen_get_nrz(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nrz", value);
}
static inline int32_t sdrkit_satbeacon_gen_set_nrz(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nrz", value);
}

// The setting "gen_carrier_only" (Carrier only) of the test signal generator of the stack "satbeacon".
static inline int32_t sdrkit_satbeacon_gen_get_carrier_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_carrier_only", value);
}
static inline int32_t sdrkit_satbeacon_gen_set_carrier_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_carrier_only", value);
}

// The setting "gen_pilot" (Pilot carrier) of the test signal generator of the stack "satbeacon". A second carrier 545.642 kHz above the beacon, 8 dB below it.
static inline int32_t sdrkit_satbeacon_gen_get_pilot(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_pilot", value);
}
static inline int32_t sdrkit_satbeacon_gen_set_pilot(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_pilot", value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_services_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mpegts_ts_service_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_tracks_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_track_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_guide_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mpegts_ts_guide_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_guide_rating_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_teletext_page_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_mpegts_teletext_subpage_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_pid_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvbc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_ts_section_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dvbc" on the session of `handle`. Forgets every count.
static inline int32_t sdrkit_dvbc_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvbc" are the sdrkit_dvbc_gen_*() functions
// after the settings of its receiver.

// The setting "symbol_rate" (Symbol rate) of the stack "dvbc". A frequency in Hz. 0: measured from the signal. A manual rate that does not lock gives way to the measured one.
static inline int32_t sdrkit_dvbc_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_dvbc_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "qam" (QAM) of the stack "dvbc". A value of `sdrkit_dvbc_qam`.
static inline int32_t sdrkit_dvbc_get_qam(SDRKIT_HANDLE handle, sdrkit_dvbc_qam *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "qam", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc_qam)index;
  return code;
}
static inline int32_t sdrkit_dvbc_set_qam(SDRKIT_HANDLE handle, sdrkit_dvbc_qam value) {
  return sdrkit_typed_setting_set_enum(handle, "qam", (int32_t)value);
}

// The setting "annex_c" (Annex C) of the stack "dvbc". ITU-T J.83 annex C (Japan, 6 MHz channels): roll-off 0.13 instead of 0.15.
static inline int32_t sdrkit_dvbc_get_annex_c(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "annex_c", value);
}
static inline int32_t sdrkit_dvbc_set_annex_c(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "annex_c", value);
}

// The setting "decode_video" (Decode video) of the stack "dvbc". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvbc_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvbc_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "gen_qam" (QAM) of the test signal generator of the stack "dvbc". A value of `sdrkit_dvbc_gen_qam`.
static inline int32_t sdrkit_dvbc_gen_get_qam(SDRKIT_HANDLE handle, sdrkit_dvbc_gen_qam *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_qam", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc_gen_qam)index;
  return code;
}
static inline int32_t sdrkit_dvbc_gen_set_qam(SDRKIT_HANDLE handle, sdrkit_dvbc_gen_qam value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_qam", (int32_t)value);
}

// The setting "gen_annex_c" (Annex C) of the test signal generator of the stack "dvbc".
static inline int32_t sdrkit_dvbc_gen_get_annex_c(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_annex_c", value);
}
static inline int32_t sdrkit_dvbc_gen_set_annex_c(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_annex_c", value);
}

// The setting "gen_symbol_rate" (Symbol rate) of the test signal generator of the stack "dvbc". A frequency in Hz. 0: 6.952 Msym/s (annex A) or 5.274 Msym/s (annex C).
static inline int32_t sdrkit_dvbc_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_dvbc_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_payload" (Payload) of the test signal generator of the stack "dvbc". A value of `sdrkit_dvbc_gen_payload`.
static inline int32_t sdrkit_dvbc_gen_get_payload(SDRKIT_HANDLE handle, sdrkit_dvbc_gen_payload *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_payload", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc_gen_payload)index;
  return code;
}
static inline int32_t sdrkit_dvbc_gen_set_payload(SDRKIT_HANDLE handle, sdrkit_dvbc_gen_payload value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_payload", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "slices" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_slices_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvbc2_slice_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.Dvbc2SliceRow", "slices", rows, sizeof(sdrkit_dvbc2_slice_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "plps" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_plps_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvbc2_plp_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.Dvbc2PlpRow", "plps", rows, sizeof(sdrkit_dvbc2_plp_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "notches" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_notches_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvbc2_notch_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.Dvbc2NotchRow", "notches", rows, sizeof(sdrkit_dvbc2_notch_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_services_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mpegts_ts_service_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_tracks_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mpegts_ts_track_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_guide_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_guide_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_guide_rating_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_teletext_page_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_mpegts_teletext_subpage_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_pid_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvbc2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbc2_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_section_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dvbc2" on the session of `handle`. Forgets every count.
static inline int32_t sdrkit_dvbc2_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvbc2" are the sdrkit_dvbc2_gen_*() functions
// after the settings of its receiver.

// The setting "plp" (PLP) of the stack "dvbc2". PLP_ID of the PLP to decode, as the table of PLPs lists it; -1: the first data PLP of the first data slice inside the receive window. The common PLP of its group is decoded with it.
static inline int32_t sdrkit_dvbc2_get_plp(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "plp", value);
}
static inline int32_t sdrkit_dvbc2_set_plp(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "plp", value);
}

// The setting "slice" (Data slice) of the stack "dvbc2". DSLICE_ID of the data slice; -1: the first slice inside the receive window that carries the PLP.
static inline int32_t sdrkit_dvbc2_get_slice(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "slice", value);
}
static inline int32_t sdrkit_dvbc2_set_slice(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "slice", value);
}

// The setting "centre_known" (Centre frequency known) of the stack "dvbc2". Off: the absolute carrier index is found by a full search over every carrier position (about a second once).
static inline int32_t sdrkit_dvbc2_get_centre_known(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "centre_known", value);
}
static inline int32_t sdrkit_dvbc2_set_centre_known(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "centre_known", value);
}

// The setting "raster" (Channel raster) of the stack "dvbc2". A value of `sdrkit_dvbc2_raster`. 8 MHz: 64/7 Msps, carriers 2232 Hz apart; 6 MHz: 48/7 Msps, 1674 Hz. Transmitter and receiver must agree.
static inline int32_t sdrkit_dvbc2_get_raster(SDRKIT_HANDLE handle, sdrkit_dvbc2_raster *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "raster", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_raster)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_set_raster(SDRKIT_HANDLE handle, sdrkit_dvbc2_raster value) {
  return sdrkit_typed_setting_set_enum(handle, "raster", (int32_t)value);
}

// The setting "ldpc_iterations" (LDPC iterations) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_ldpc_iterations(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "ldpc_iterations", value);
}
static inline int32_t sdrkit_dvbc2_set_ldpc_iterations(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "ldpc_iterations", value);
}

// The setting "timing_metric" (Timing metric) of the stack "dvbc2". The mean of |C| / E of the guard correlation folded over 8 symbol periods.
static inline int32_t sdrkit_dvbc2_get_timing_metric(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "timing_metric", value);
}
static inline int32_t sdrkit_dvbc2_set_timing_metric(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "timing_metric", value);
}

// The setting "preamble_ratio" (Preamble ratio) of the stack "dvbc2". The energy of the strongest scattered pilot phase against the median: about 5 on a data symbol.
static inline int32_t sdrkit_dvbc2_get_preamble_ratio(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "preamble_ratio", value);
}
static inline int32_t sdrkit_dvbc2_set_preamble_ratio(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "preamble_ratio", value);
}

// The setting "index_metric" (Carrier index metric) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_index_metric(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "index_metric", value);
}
static inline int32_t sdrkit_dvbc2_set_index_metric(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "index_metric", value);
}

// The setting "index_span" (Carrier index span) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_index_span(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "index_span", value);
}
static inline int32_t sdrkit_dvbc2_set_index_span(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "index_span", value);
}

// The setting "header_candidate" (Header candidate) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_header_candidate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "header_candidate", value);
}
static inline int32_t sdrkit_dvbc2_set_header_candidate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "header_candidate", value);
}

// The setting "header_agree" (Header agreement) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_header_agree(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "header_agree", value);
}
static inline int32_t sdrkit_dvbc2_set_header_agree(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "header_agree", value);
}

// The setting "header_misses" (Header misses) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_header_misses(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "header_misses", value);
}
static inline int32_t sdrkit_dvbc2_set_header_misses(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "header_misses", value);
}

// The setting "block_misses" (Block misses) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_block_misses(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "block_misses", value);
}
static inline int32_t sdrkit_dvbc2_set_block_misses(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "block_misses", value);
}

// The setting "preamble_misses" (Preamble misses) of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_get_preamble_misses(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "preamble_misses", value);
}
static inline int32_t sdrkit_dvbc2_set_preamble_misses(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "preamble_misses", value);
}

// The setting "ti_full_width" (Time interleaver over every carrier) of the stack "dvbc2". On: N_DSLICE_WIDTH = K_DS,max - K_DS,min + 1, one column per carrier from K_DS,min to K_DS,max (clause 9.4.1 makes K_DS,max the last carrier of the slice). Off: K_DS,max - K_DS,min as clause 9.4.4 prints it, and the last carrier of the slice carries no data.
static inline int32_t sdrkit_dvbc2_get_ti_full_width(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "ti_full_width", value);
}
static inline int32_t sdrkit_dvbc2_set_ti_full_width(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "ti_full_width", value);
}

// The setting "rm_scrambling" (Header scrambling word) of the stack "dvbc2". w^RM of clause 7.2.2.3 as a number, w^RM_0 the most significant bit; default 0x1D8D3BF3. Transmitter and receiver must agree.
static inline int32_t sdrkit_dvbc2_get_rm_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "rm_scrambling", value);
}
static inline int32_t sdrkit_dvbc2_set_rm_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "rm_scrambling", value);
}

// The setting "bundle_hold" (Bundled PLP hold) of the stack "dvbc2". Unit: frames. The BBFRAMEs of a PLP bundled over several slices wait here until they can go out in the order of their ISCR.
static inline int32_t sdrkit_dvbc2_get_bundle_hold(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "bundle_hold", value);
}
static inline int32_t sdrkit_dvbc2_set_bundle_hold(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "bundle_hold", value);
}

// The setting "decode_video" (Decode video) of the stack "dvbc2". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvbc2_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvbc2_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "gen_frequency" (Centre frequency) of the test signal generator of the stack "dvbc2". A frequency in Hz. The system is centred on the carrier below this frequency that is a multiple of D_X.
static inline int32_t sdrkit_dvbc2_gen_get_frequency(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_frequency", value);
}
static inline int32_t sdrkit_dvbc2_gen_set_frequency(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_frequency", value);
}

// The setting "gen_modcod" (Coding) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_modcod`.
static inline int32_t sdrkit_dvbc2_gen_get_modcod(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_modcod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_modcod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_modcod)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_modcod(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_modcod value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_modcod", (int32_t)value);
}

// The setting "gen_short_frames" (16K FEC frames) of the test signal generator of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_gen_get_short_frames(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_short_frames", value);
}
static inline int32_t sdrkit_dvbc2_gen_set_short_frames(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_short_frames", value);
}

// The setting "gen_guard" (Guard interval) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_guard`.
static inline int32_t sdrkit_dvbc2_gen_get_guard(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_guard)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_guard(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_guard", (int32_t)value);
}

// The setting "gen_slice_type" (Data slice type) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_slice_type`.
static inline int32_t sdrkit_dvbc2_gen_get_slice_type(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_slice_type *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_slice_type", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_slice_type)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_slice_type(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_slice_type value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_slice_type", (int32_t)value);
}

// The setting "gen_ti_depth" (Time interleaving) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_ti_depth`.
static inline int32_t sdrkit_dvbc2_gen_get_ti_depth(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_ti_depth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_ti_depth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_ti_depth)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_ti_depth(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_ti_depth value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_ti_depth", (int32_t)value);
}

// The setting "gen_l1_ti" (L1 time interleaving) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_l1_ti`.
static inline int32_t sdrkit_dvbc2_gen_get_l1_ti(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_l1_ti *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_l1_ti", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_l1_ti)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_l1_ti(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_l1_ti value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_l1_ti", (int32_t)value);
}

// The setting "gen_notch" (Narrowband notch) of the test signal generator of the stack "dvbc2". 35 carriers at guard 1/64, 23 at 1/128.
static inline int32_t sdrkit_dvbc2_gen_get_notch(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_notch", value);
}
static inline int32_t sdrkit_dvbc2_gen_set_notch(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_notch", value);
}

// The setting "gen_reserved_tones" (Reserved tones) of the test signal generator of the stack "dvbc2".
static inline int32_t sdrkit_dvbc2_gen_get_reserved_tones(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_reserved_tones", value);
}
static inline int32_t sdrkit_dvbc2_gen_set_reserved_tones(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_reserved_tones", value);
}

// The setting "gen_system" (Slices and PLPs) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_system`. two_plps and common use a type 2 slice; bundled, group and far_slice use type 2 slices only; mixed_slices has one slice of each type. group and far_slice carry numbered packets. two_slices and far_slice are 15.2 MHz wide (8 MHz raster): two_slices is centred on gen_frequency and needs 20 Msps to be seen whole; far_slice has the centre of its lower L1 block there, and the window at the tuning position of the slice of PLP 2 holds that slice and the common PLP.
static inline int32_t sdrkit_dvbc2_gen_get_system(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_system)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_system(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_system value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_system", (int32_t)value);
}

// The setting "gen_stream" (Stream) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_stream`.
static inline int32_t sdrkit_dvbc2_gen_get_stream(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_stream *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_stream", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_stream)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_stream(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_stream value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_stream", (int32_t)value);
}

// The setting "gen_hem" (High efficiency mode) of the test signal generator of the stack "dvbc2". Transport packets without sync byte and CRC-8, or GSE, in BBFRAMEs of MODE 1.
static inline int32_t sdrkit_dvbc2_gen_get_hem(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_hem", value);
}
static inline int32_t sdrkit_dvbc2_gen_set_hem(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_hem", value);
}

// The setting "gen_payload" (Payload) of the test signal generator of the stack "dvbc2". A value of `sdrkit_dvbc2_gen_payload`.
static inline int32_t sdrkit_dvbc2_gen_get_payload(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_payload *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_payload", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbc2_gen_payload)index;
  return code;
}
static inline int32_t sdrkit_dvbc2_gen_set_payload(SDRKIT_HANDLE handle, sdrkit_dvbc2_gen_payload value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_payload", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_carriers_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dvbs2_carrier_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2CarrierRow", "carriers", rows, sizeof(sdrkit_dvbs2_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_streams_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dvbs2_stream_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamRow", "streams", rows, sizeof(sdrkit_dvbs2_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stream_modcods" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_stream_modcods_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dvbs2_stream_modcod_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamModcodRow", "stream_modcods", rows, sizeof(sdrkit_dvbs2_stream_modcod_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_services_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mpegts_ts_service_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_tracks_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_track_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_guide_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mpegts_ts_guide_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_guide_rating_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_teletext_page_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_mpegts_teletext_subpage_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_pid_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbs_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_ts_section_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dvbs" on the session of `handle`. Forgets every count.
static inline int32_t sdrkit_dvbs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "start" (Start sending) of the stack "dvbs" on the session of `handle`. Sends a DVB-S signal (EN 300 421) or a DVB-S2 signal (EN 302 307-1), as tx_standard says, with the tx_* settings, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_dvbs_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "dvbs" on the session of `handle`. Ends the transmission after the group of eight packets on air (DVB-S) or the PLFRAME on air (DVB-S2) with a fall of 5 ms. The packets that wait are dropped.
static inline int32_t sdrkit_dvbs_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "append" (Add packets) of the stack "dvbs" on the session of `handle`. Adds transport packets to the queue of the stream sent. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.
static inline int32_t sdrkit_dvbs_append(SDRKIT_HANDLE handle, const sdrkit_dvbs_tx_append_args *args) {
  return sdrkit_typed_command(handle, "append", "dvbcs.TxAppendArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvbs" are the sdrkit_dvbs_gen_*() functions
// after the settings of its receiver.

// The setting "symbol_rate" (Symbol rate) of the stack "dvbs". A frequency in Hz. 0: automatic.
static inline int32_t sdrkit_dvbs_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_dvbs_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "standard" (Standard) of the stack "dvbs". A value of `sdrkit_dvbs_standard`. Auto tries DVB-S2 and DVB-S. Choose one to skip the other search on a weak signal.
static inline int32_t sdrkit_dvbs_get_standard(SDRKIT_HANDLE handle, sdrkit_dvbs_standard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "standard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_standard)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_standard(SDRKIT_HANDLE handle, sdrkit_dvbs_standard value) {
  return sdrkit_typed_setting_set_enum(handle, "standard", (int32_t)value);
}

// The setting "roll_off" (Roll-off) of the stack "dvbs". A value of `sdrkit_dvbs_roll_off`.
static inline int32_t sdrkit_dvbs_get_roll_off(SDRKIT_HANDLE handle, sdrkit_dvbs_roll_off *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "roll_off", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_roll_off)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_roll_off(SDRKIT_HANDLE handle, sdrkit_dvbs_roll_off value) {
  return sdrkit_typed_setting_set_enum(handle, "roll_off", (int32_t)value);
}

// The setting "isi" (Input stream) of the stack "dvbs". -1: the first stream seen.
static inline int32_t sdrkit_dvbs_get_isi(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "isi", value);
}
static inline int32_t sdrkit_dvbs_set_isi(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "isi", value);
}

// The setting "pl_scrambling" (PL scrambling code) of the stack "dvbs".
static inline int32_t sdrkit_dvbs_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pl_scrambling", value);
}
static inline int32_t sdrkit_dvbs_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value);
}

// The setting "carrier_pick" (Carrier) of the stack "dvbs". Offset in Hz, empty for automatic (set by clicking a carrier in the survey).
static inline int32_t sdrkit_dvbs_get_carrier_pick(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "carrier_pick", value);
}
static inline int32_t sdrkit_dvbs_set_carrier_pick(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "carrier_pick", value);
}

// The setting "modcod_rule" (MODCODs taken) of the stack "dvbs". A value of `sdrkit_dvbs_modcod_rule`. Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.
static inline int32_t sdrkit_dvbs_get_modcod_rule(SDRKIT_HANDLE handle, sdrkit_dvbs_modcod_rule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_modcod_rule)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_modcod_rule(SDRKIT_HANDLE handle, sdrkit_dvbs_modcod_rule value) {
  return sdrkit_typed_setting_set_enum(handle, "modcod_rule", (int32_t)value);
}

// The setting "modcod" (MODCOD) of the stack "dvbs".
static inline int32_t sdrkit_dvbs_get_modcod(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "modcod", value);
}
static inline int32_t sdrkit_dvbs_set_modcod(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "modcod", value);
}

// The setting "modcods" (MODCOD set) of the stack "dvbs". MODCOD numbers with commas between them, for the rule "set".
static inline int32_t sdrkit_dvbs_get_modcods(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "modcods", value);
}
static inline int32_t sdrkit_dvbs_set_modcods(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "modcods", value);
}

// The setting "frame_length" (Frame length) of the stack "dvbs". A value of `sdrkit_dvbs_frame_length`. Frames of the other length are counted and skipped.
static inline int32_t sdrkit_dvbs_get_frame_length(SDRKIT_HANDLE handle, sdrkit_dvbs_frame_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_frame_length)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_frame_length(SDRKIT_HANDLE handle, sdrkit_dvbs_frame_length value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_length", (int32_t)value);
}

// The setting "pilots" (Pilots) of the stack "dvbs". A value of `sdrkit_dvbs_pilots`. A check only: a frame with other pilots is counted and decoded.
static inline int32_t sdrkit_dvbs_get_pilots(SDRKIT_HANDLE handle, sdrkit_dvbs_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_pilots)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_pilots(SDRKIT_HANDLE handle, sdrkit_dvbs_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "pilots", (int32_t)value);
}

// The setting "ip_pcap" (PCAP output) of the stack "dvbs". Send every IP packet as PCAP file bytes. The host writes the file.
static inline int32_t sdrkit_dvbs_get_ip_pcap(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "ip_pcap", value);
}
static inline int32_t sdrkit_dvbs_set_ip_pcap(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "ip_pcap", value);
}

// The setting "tx_polarisation" (Transponder) of the stack "dvbs". A value of `sdrkit_dvbs_tx_polarisation`.
static inline int32_t sdrkit_dvbs_get_tx_polarisation(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_polarisation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_polarisation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_tx_polarisation)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_tx_polarisation(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_polarisation value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_polarisation", (int32_t)value);
}

// The setting "lnb_supply" (LNB supply) of the stack "dvbs". A value of `sdrkit_dvbs_lnb_supply`.
static inline int32_t sdrkit_dvbs_get_lnb_supply(SDRKIT_HANDLE handle, sdrkit_dvbs_lnb_supply *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "lnb_supply", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_lnb_supply)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_lnb_supply(SDRKIT_HANDLE handle, sdrkit_dvbs_lnb_supply value) {
  return sdrkit_typed_setting_set_enum(handle, "lnb_supply", (int32_t)value);
}

// The setting "decode_video" (Decode video) of the stack "dvbs". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvbs_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvbs_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "tx_symbol_rate" (Symbol rate) of the stack "dvbs". A frequency in Hz. The symbol rate sent. The session's rate must be at least 1.3 × (1 + roll-off) times it: 1.755 for roll-off 0.35.
static inline int32_t sdrkit_dvbs_get_tx_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_symbol_rate", value);
}
static inline int32_t sdrkit_dvbs_set_tx_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_symbol_rate", value);
}

// The setting "tx_rate" (Code rate) of the stack "dvbs". A value of `sdrkit_dvbs_tx_rate`. DVB-S: the code rate of the convolutional code, 1/2, 2/3, 3/4, 5/6 or 7/8. DVB-S2 takes its code rate from tx_modcod.
static inline int32_t sdrkit_dvbs_get_tx_rate(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_tx_rate)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_tx_rate(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_rate", (int32_t)value);
}

// The setting "tx_inversion" (Spectral inversion) of the stack "dvbs". Sends the spectrum mirrored (Q negated).
static inline int32_t sdrkit_dvbs_get_tx_inversion(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_inversion", value);
}
static inline int32_t sdrkit_dvbs_set_tx_inversion(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_inversion", value);
}

// The setting "tx_standard" (Standard) of the stack "dvbs". A value of `sdrkit_dvbs_tx_standard`. The standard sent: DVB-S (EN 300 421) or DVB-S2 (EN 302 307-1).
static inline int32_t sdrkit_dvbs_get_tx_standard(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_standard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_standard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_tx_standard)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_tx_standard(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_standard value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_standard", (int32_t)value);
}

// The setting "tx_modcod" (MODCOD) of the stack "dvbs". A value of `sdrkit_dvbs_tx_modcod`. DVB-S2: the modulation and the code rate of every frame.
static inline int32_t sdrkit_dvbs_get_tx_modcod(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_modcod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_modcod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_tx_modcod)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_tx_modcod(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_modcod value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_modcod", (int32_t)value);
}

// The setting "tx_frame" (FEC frame) of the stack "dvbs". A value of `sdrkit_dvbs_tx_frame`. DVB-S2: normal frames (64 800 bits) or short frames (16 200 bits). Code rate 9/10 has no short frame.
static inline int32_t sdrkit_dvbs_get_tx_frame(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_frame *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_frame", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_tx_frame)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_tx_frame(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_frame value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_frame", (int32_t)value);
}

// The setting "tx_pilots" (Pilots) of the stack "dvbs". DVB-S2: a block of 36 pilot symbols after every 16 slots.
static inline int32_t sdrkit_dvbs_get_tx_pilots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_pilots", value);
}
static inline int32_t sdrkit_dvbs_set_tx_pilots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_pilots", value);
}

// The setting "tx_rolloff" (Roll-off) of the stack "dvbs". A value of `sdrkit_dvbs_tx_rolloff`. DVB-S2: the roll-off of the pulse. DVB-S has 0.35.
static inline int32_t sdrkit_dvbs_get_tx_rolloff(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_rolloff *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_rolloff", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_tx_rolloff)index;
  return code;
}
static inline int32_t sdrkit_dvbs_set_tx_rolloff(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_rolloff value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_rolloff", (int32_t)value);
}

// The setting "tx_pl_scrambling" (PL scrambling code) of the stack "dvbs". DVB-S2: the number of the Gold code of the PL scrambling. 0 for broadcast.
static inline int32_t sdrkit_dvbs_get_tx_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_pl_scrambling", value);
}
static inline int32_t sdrkit_dvbs_set_tx_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_pl_scrambling", value);
}

// The setting "gen_standard" (Standard) of the test signal generator of the stack "dvbs". A value of `sdrkit_dvbs_gen_standard`.
static inline int32_t sdrkit_dvbs_gen_get_standard(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_standard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_standard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_gen_standard)index;
  return code;
}
static inline int32_t sdrkit_dvbs_gen_set_standard(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_standard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_standard", (int32_t)value);
}

// The setting "gen_modulation" (Modulation) of the test signal generator of the stack "dvbs". A value of `sdrkit_dvbs_gen_modulation`.
static inline int32_t sdrkit_dvbs_gen_get_modulation(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_gen_modulation)index;
  return code;
}
static inline int32_t sdrkit_dvbs_gen_set_modulation(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_modulation", (int32_t)value);
}

// The setting "gen_rate" (Code rate) of the test signal generator of the stack "dvbs". A value of `sdrkit_dvbs_gen_rate`.
static inline int32_t sdrkit_dvbs_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_dvbs_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// The setting "gen_short" (Short frames) of the test signal generator of the stack "dvbs".
static inline int32_t sdrkit_dvbs_gen_get_short(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_short", value);
}
static inline int32_t sdrkit_dvbs_gen_set_short(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_short", value);
}

// The setting "gen_pilots" (Pilots) of the test signal generator of the stack "dvbs".
static inline int32_t sdrkit_dvbs_gen_get_pilots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_pilots", value);
}
static inline int32_t sdrkit_dvbs_gen_set_pilots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_pilots", value);
}

// The setting "gen_rolloff" (Roll-off) of the test signal generator of the stack "dvbs". A value of `sdrkit_dvbs_gen_rolloff`.
static inline int32_t sdrkit_dvbs_gen_get_rolloff(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_rolloff *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rolloff", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_gen_rolloff)index;
  return code;
}
static inline int32_t sdrkit_dvbs_gen_set_rolloff(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_rolloff value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rolloff", (int32_t)value);
}

// The setting "gen_symbol_rate" (Symbol rate) of the test signal generator of the stack "dvbs". A frequency in Hz. 0: 5 Msym/s or what the sample rate allows.
static inline int32_t sdrkit_dvbs_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_dvbs_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_pl_scrambling" (PL scrambling code) of the test signal generator of the stack "dvbs".
static inline int32_t sdrkit_dvbs_gen_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_pl_scrambling", value);
}
static inline int32_t sdrkit_dvbs_gen_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_pl_scrambling", value);
}

// The setting "gen_dummy_every" (A dummy frame every) of the test signal generator of the stack "dvbs". Unit: frames.
static inline int32_t sdrkit_dvbs_gen_get_dummy_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_dummy_every", value);
}
static inline int32_t sdrkit_dvbs_gen_set_dummy_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_dummy_every", value);
}

// The setting "gen_gse" (GSE (IP) instead of a transport stream) of the test signal generator of the stack "dvbs".
static inline int32_t sdrkit_dvbs_gen_get_gse(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_gse", value);
}
static inline int32_t sdrkit_dvbs_gen_set_gse(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_gse", value);
}

// The setting "gen_multistream" (Two input streams (TS and GSE)) of the test signal generator of the stack "dvbs".
static inline int32_t sdrkit_dvbs_gen_get_multistream(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_multistream", value);
}
static inline int32_t sdrkit_dvbs_gen_set_multistream(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_multistream", value);
}

// The setting "gen_payload" (Payload) of the test signal generator of the stack "dvbs". A value of `sdrkit_dvbs_gen_payload`.
static inline int32_t sdrkit_dvbs_gen_get_payload(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_payload *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_payload", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbs_gen_payload)index;
  return code;
}
static inline int32_t sdrkit_dvbs_gen_set_payload(SDRKIT_HANDLE handle, sdrkit_dvbs_gen_payload value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_payload", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_streams_rows(SDRKIT_HANDLE handle,
                                               sdrkit_isdbs_stream_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbcs.IsdbsStreamRow", "streams", rows, sizeof(sdrkit_isdbs_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_services_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mpegts_ts_service_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_tracks_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mpegts_ts_track_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_guide_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_guide_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_guide_rating_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_teletext_page_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_mpegts_teletext_subpage_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_pid_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "isdbs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbs_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_section_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "isdbs" on the session of `handle`. Forgets every count.
static inline int32_t sdrkit_isdbs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "isdbs" are the sdrkit_isdbs_gen_*() functions
// after the settings of its receiver.

// The setting "stream" (Stream) of the stack "isdbs". -1: the stream with the most slots.
static inline int32_t sdrkit_isdbs_get_stream(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "stream", value);
}
static inline int32_t sdrkit_isdbs_set_stream(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "stream", value);
}

// The setting "decode_video" (Decode video) of the stack "isdbs". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_isdbs_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_isdbs_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "gen_layout" (Layout) of the test signal generator of the stack "isdbs". A value of `sdrkit_isdbs_gen_layout`.
static inline int32_t sdrkit_isdbs_gen_get_layout(SDRKIT_HANDLE handle, sdrkit_isdbs_gen_layout *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_layout", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbs_gen_layout)index;
  return code;
}
static inline int32_t sdrkit_isdbs_gen_set_layout(SDRKIT_HANDLE handle, sdrkit_isdbs_gen_layout value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_layout", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satbeacon_fields_read(SDRKIT_HANDLE handle, sdrkit_satbeacon_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbcs.SatbeaconFields", fields, sizeof(sdrkit_satbeacon_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbc_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbc_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbcs.DvbcFields", fields, sizeof(sdrkit_dvbc_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbc2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbc2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbcs.Dvbc2Fields", fields, sizeof(sdrkit_dvbc2_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbc2_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbc2_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbcs.Dvbc2StreamFields", fields, sizeof(sdrkit_dvbc2_stream_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbs_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbcs.DvbsTxFields", fields, sizeof(sdrkit_dvbs_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_isdbs_fields_read(SDRKIT_HANDLE handle, sdrkit_isdbs_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbcs.IsdbsFields", fields, sizeof(sdrkit_isdbs_fields));
}

// Fills `fields` with the result fields of the component "monitor" of the stack "dvbc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "monitor.<key>".
static inline int32_t sdrkit_dvbc_monitor_ts_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsMonitorFields@monitor", fields, sizeof(sdrkit_mpegts_ts_monitor_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvbc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvbc_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvbc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvbc_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "dvbc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_dvbc_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "dvbc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_dvbc_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

// Fills `fields` with the result fields of the component "monitor" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "monitor.<key>".
static inline int32_t sdrkit_dvbc2_monitor_ts_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsMonitorFields@monitor", fields, sizeof(sdrkit_mpegts_ts_monitor_fields));
}

// Fills `fields` with the result fields of the component "bbframe" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "bbframe.<key>".
static inline int32_t sdrkit_dvbc2_bbframe_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@bbframe", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

// Fills `fields` with the result fields of the component "ip" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ip.<key>".
static inline int32_t sdrkit_dvbc2_ip_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_ip_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.IpFields@ip", fields, sizeof(sdrkit_ipcast_ip_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvbc2_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvbc2_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_dvbc2_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "dvbc2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_dvbc2_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

// Fills `fields` with the result fields of the component "monitor" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "monitor.<key>".
static inline int32_t sdrkit_dvbs_monitor_ts_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsMonitorFields@monitor", fields, sizeof(sdrkit_mpegts_ts_monitor_fields));
}

// Fills `fields` with the result fields of the component "bbframe" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "bbframe.<key>".
static inline int32_t sdrkit_dvbs_bbframe_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@bbframe", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

// Fills `fields` with the result fields of the component "ip" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ip.<key>".
static inline int32_t sdrkit_dvbs_ip_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_ip_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.IpFields@ip", fields, sizeof(sdrkit_ipcast_ip_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvbs_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvbs_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_dvbs_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "dvbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_dvbs_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

// Fills `fields` with the result fields of the component "monitor" of the stack "isdbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "monitor.<key>".
static inline int32_t sdrkit_isdbs_monitor_ts_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsMonitorFields@monitor", fields, sizeof(sdrkit_mpegts_ts_monitor_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "isdbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_isdbs_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "isdbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_isdbs_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "isdbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_isdbs_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "isdbs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_isdbs_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

#endif
