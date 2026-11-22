/*
 * sdrkit: the typed interface of the stack family "dvbt", version 2.6. MIT licence.
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

#ifndef SDRKIT_DVBT_H
#define SDRKIT_DVBT_H

#include "../sdrkit.h"
#include "mpegts.h"

// The version of the family that this header describes.
#define SDRKIT_DVBT_VERSION_MAJOR 2
#define SDRKIT_DVBT_VERSION_MINOR 6

// The stack "dvbt": its id, and the keys of its event types.
#define SDRKIT_STACK_DVBT "dvbt"

// The stack "dvb_terrestrial": its id, and the keys of its event types.
#define SDRKIT_STACK_DVB_TERRESTRIAL "dvb_terrestrial"

// The stack "dvbsh": its id, and the keys of its event types.
#define SDRKIT_STACK_DVBSH "dvbsh"

// The options of the setting "bandwidth" of the stack "dvbt".
typedef enum sdrkit_dvbt_bandwidth {
  // Auto
  SDRKIT_DVBT_BANDWIDTH_AUTO = 0,
  // 8 MHz
  SDRKIT_DVBT_BANDWIDTH_8_MHZ = 1,
  // 7 MHz
  SDRKIT_DVBT_BANDWIDTH_7_MHZ = 2,
  // 6 MHz
  SDRKIT_DVBT_BANDWIDTH_6_MHZ = 3,
  // 5 MHz
  SDRKIT_DVBT_BANDWIDTH_5_MHZ = 4,
} sdrkit_dvbt_bandwidth;

// The options of the setting "stream" of the stack "dvbt".
typedef enum sdrkit_dvbt_stream {
  // High priority
  SDRKIT_DVBT_STREAM_HP = 0,
  // Low priority
  SDRKIT_DVBT_STREAM_LP = 1,
} sdrkit_dvbt_stream;

// The options of the setting "viterbi" of the stack "dvbt".
typedef enum sdrkit_dvbt_viterbi {
  // Auto
  SDRKIT_DVBT_VITERBI_AUTO = 0,
  // Exact
  SDRKIT_DVBT_VITERBI_EXACT = 1,
  // Fast (8-bit)
  SDRKIT_DVBT_VITERBI_FAST = 2,
} sdrkit_dvbt_viterbi;

// The options of the setting "standard" of the stack "dvbt".
typedef enum sdrkit_dvbt_standard {
  // Auto (DVB-T2 / DVB-T)
  SDRKIT_DVBT_STANDARD_AUTO = 0,
  // DVB-T only
  SDRKIT_DVBT_STANDARD_DVBT = 1,
} sdrkit_dvbt_standard;

// The options of the setting "gen_mode" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_mode {
  // 2K
  SDRKIT_DVBT_GEN_MODE_2_K = 0,
  // 8K
  SDRKIT_DVBT_GEN_MODE_8_K = 1,
} sdrkit_dvbt_gen_mode;

// The options of the setting "gen_guard" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_guard {
  // 1/32
  SDRKIT_DVBT_GEN_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBT_GEN_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBT_GEN_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBT_GEN_GUARD_1_4 = 3,
} sdrkit_dvbt_gen_guard;

// The options of the setting "gen_constellation" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_constellation {
  // QPSK
  SDRKIT_DVBT_GEN_CONSTELLATION_Q_P_S_K = 0,
  // 16-QAM
  SDRKIT_DVBT_GEN_CONSTELLATION_16_Q_A_M = 1,
  // 64-QAM
  SDRKIT_DVBT_GEN_CONSTELLATION_64_Q_A_M = 2,
} sdrkit_dvbt_gen_constellation;

// The options of the setting "gen_hierarchy" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_hierarchy {
  // None
  SDRKIT_DVBT_GEN_HIERARCHY_NONE = 0,
  // α = 1
  SDRKIT_DVBT_GEN_HIERARCHY_1 = 1,
  // α = 2
  SDRKIT_DVBT_GEN_HIERARCHY_2 = 2,
  // α = 4
  SDRKIT_DVBT_GEN_HIERARCHY_4 = 3,
} sdrkit_dvbt_gen_hierarchy;

// The options of the setting "gen_rate_hp" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_rate_hp {
  // 1/2
  SDRKIT_DVBT_GEN_RATE_HP_1_2 = 0,
  // 2/3
  SDRKIT_DVBT_GEN_RATE_HP_2_3 = 1,
  // 3/4
  SDRKIT_DVBT_GEN_RATE_HP_3_4 = 2,
  // 5/6
  SDRKIT_DVBT_GEN_RATE_HP_5_6 = 3,
  // 7/8
  SDRKIT_DVBT_GEN_RATE_HP_7_8 = 4,
} sdrkit_dvbt_gen_rate_hp;

// The options of the setting "gen_rate_lp" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_rate_lp {
  // 1/2
  SDRKIT_DVBT_GEN_RATE_LP_1_2 = 0,
  // 2/3
  SDRKIT_DVBT_GEN_RATE_LP_2_3 = 1,
  // 3/4
  SDRKIT_DVBT_GEN_RATE_LP_3_4 = 2,
  // 5/6
  SDRKIT_DVBT_GEN_RATE_LP_5_6 = 3,
  // 7/8
  SDRKIT_DVBT_GEN_RATE_LP_7_8 = 4,
} sdrkit_dvbt_gen_rate_lp;

// The options of the setting "gen_bandwidth" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_bandwidth {
  // Auto
  SDRKIT_DVBT_GEN_BANDWIDTH_AUTO = 0,
  // 8 MHz
  SDRKIT_DVBT_GEN_BANDWIDTH_8_MHZ = 1,
  // 7 MHz
  SDRKIT_DVBT_GEN_BANDWIDTH_7_MHZ = 2,
  // 6 MHz
  SDRKIT_DVBT_GEN_BANDWIDTH_6_MHZ = 3,
  // 5 MHz
  SDRKIT_DVBT_GEN_BANDWIDTH_5_MHZ = 4,
} sdrkit_dvbt_gen_bandwidth;

// The options of the setting "gen_length" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_length {
  // 23
  SDRKIT_DVBT_GEN_LENGTH_23 = 0,
  // 31
  SDRKIT_DVBT_GEN_LENGTH_31 = 1,
} sdrkit_dvbt_gen_length;

// The options of the setting "gen_source" of the stack "dvbt".
typedef enum sdrkit_dvbt_gen_source {
  // Built-in multiplex
  SDRKIT_DVBT_GEN_SOURCE_BUILTIN = 0,
  // Counted test pattern
  SDRKIT_DVBT_GEN_SOURCE_PATTERN = 1,
  // Null packets
  SDRKIT_DVBT_GEN_SOURCE_NULL = 2,
} sdrkit_dvbt_gen_source;

// The options of the setting "tx_bandwidth" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_bandwidth {
  // 5 MHz
  SDRKIT_DVBT_TX_BANDWIDTH_5_MHZ = 0,
  // 6 MHz
  SDRKIT_DVBT_TX_BANDWIDTH_6_MHZ = 1,
  // 7 MHz
  SDRKIT_DVBT_TX_BANDWIDTH_7_MHZ = 2,
  // 8 MHz
  SDRKIT_DVBT_TX_BANDWIDTH_8_MHZ = 3,
} sdrkit_dvbt_tx_bandwidth;

// The options of the setting "tx_mode" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_mode {
  // 2K
  SDRKIT_DVBT_TX_MODE_2_K = 0,
  // 8K
  SDRKIT_DVBT_TX_MODE_8_K = 1,
} sdrkit_dvbt_tx_mode;

// The options of the setting "tx_guard" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_guard {
  // 1/32
  SDRKIT_DVBT_TX_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBT_TX_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBT_TX_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBT_TX_GUARD_1_4 = 3,
} sdrkit_dvbt_tx_guard;

// The options of the setting "tx_constellation" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_constellation {
  // QPSK
  SDRKIT_DVBT_TX_CONSTELLATION_Q_P_S_K = 0,
  // 16-QAM
  SDRKIT_DVBT_TX_CONSTELLATION_16_Q_A_M = 1,
  // 64-QAM
  SDRKIT_DVBT_TX_CONSTELLATION_64_Q_A_M = 2,
} sdrkit_dvbt_tx_constellation;

// The options of the setting "tx_hierarchy" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_hierarchy {
  // None
  SDRKIT_DVBT_TX_HIERARCHY_NONE = 0,
  // α = 1
  SDRKIT_DVBT_TX_HIERARCHY_1 = 1,
  // α = 2
  SDRKIT_DVBT_TX_HIERARCHY_2 = 2,
  // α = 4
  SDRKIT_DVBT_TX_HIERARCHY_4 = 3,
} sdrkit_dvbt_tx_hierarchy;

// The options of the setting "tx_rate_hp" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_rate_hp {
  // 1/2
  SDRKIT_DVBT_TX_RATE_HP_1_2 = 0,
  // 2/3
  SDRKIT_DVBT_TX_RATE_HP_2_3 = 1,
  // 3/4
  SDRKIT_DVBT_TX_RATE_HP_3_4 = 2,
  // 5/6
  SDRKIT_DVBT_TX_RATE_HP_5_6 = 3,
  // 7/8
  SDRKIT_DVBT_TX_RATE_HP_7_8 = 4,
} sdrkit_dvbt_tx_rate_hp;

// The options of the setting "tx_rate_lp" of the stack "dvbt".
typedef enum sdrkit_dvbt_tx_rate_lp {
  // 1/2
  SDRKIT_DVBT_TX_RATE_LP_1_2 = 0,
  // 2/3
  SDRKIT_DVBT_TX_RATE_LP_2_3 = 1,
  // 3/4
  SDRKIT_DVBT_TX_RATE_LP_3_4 = 2,
  // 5/6
  SDRKIT_DVBT_TX_RATE_LP_5_6 = 3,
  // 7/8
  SDRKIT_DVBT_TX_RATE_LP_7_8 = 4,
} sdrkit_dvbt_tx_rate_lp;

// The options of the setting "bandwidth" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_bandwidth {
  // Auto
  SDRKIT_DVB_TERRESTRIAL_BANDWIDTH_AUTO = 0,
  // 8 MHz
  SDRKIT_DVB_TERRESTRIAL_BANDWIDTH_8_MHZ = 1,
  // 7 MHz
  SDRKIT_DVB_TERRESTRIAL_BANDWIDTH_7_MHZ = 2,
  // 6 MHz
  SDRKIT_DVB_TERRESTRIAL_BANDWIDTH_6_MHZ = 3,
  // 5 MHz
  SDRKIT_DVB_TERRESTRIAL_BANDWIDTH_5_MHZ = 4,
} sdrkit_dvb_terrestrial_bandwidth;

// The options of the setting "stream" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_stream {
  // High priority
  SDRKIT_DVB_TERRESTRIAL_STREAM_HP = 0,
  // Low priority
  SDRKIT_DVB_TERRESTRIAL_STREAM_LP = 1,
} sdrkit_dvb_terrestrial_stream;

// The options of the setting "viterbi" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_viterbi {
  // Auto
  SDRKIT_DVB_TERRESTRIAL_VITERBI_AUTO = 0,
  // Exact
  SDRKIT_DVB_TERRESTRIAL_VITERBI_EXACT = 1,
  // Fast (8-bit)
  SDRKIT_DVB_TERRESTRIAL_VITERBI_FAST = 2,
} sdrkit_dvb_terrestrial_viterbi;

// The options of the setting "standard" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_standard {
  // Auto (DVB-T2 / DVB-T)
  SDRKIT_DVB_TERRESTRIAL_STANDARD_AUTO = 0,
  // DVB-T only
  SDRKIT_DVB_TERRESTRIAL_STANDARD_DVBT = 1,
} sdrkit_dvb_terrestrial_standard;

// The options of the setting "gen_mode" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_mode {
  // 2K
  SDRKIT_DVB_TERRESTRIAL_GEN_MODE_2_K = 0,
  // 8K
  SDRKIT_DVB_TERRESTRIAL_GEN_MODE_8_K = 1,
} sdrkit_dvb_terrestrial_gen_mode;

// The options of the setting "gen_guard" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_guard {
  // 1/32
  SDRKIT_DVB_TERRESTRIAL_GEN_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVB_TERRESTRIAL_GEN_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVB_TERRESTRIAL_GEN_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVB_TERRESTRIAL_GEN_GUARD_1_4 = 3,
} sdrkit_dvb_terrestrial_gen_guard;

// The options of the setting "gen_constellation" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_constellation {
  // QPSK
  SDRKIT_DVB_TERRESTRIAL_GEN_CONSTELLATION_Q_P_S_K = 0,
  // 16-QAM
  SDRKIT_DVB_TERRESTRIAL_GEN_CONSTELLATION_16_Q_A_M = 1,
  // 64-QAM
  SDRKIT_DVB_TERRESTRIAL_GEN_CONSTELLATION_64_Q_A_M = 2,
} sdrkit_dvb_terrestrial_gen_constellation;

// The options of the setting "gen_hierarchy" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_hierarchy {
  // None
  SDRKIT_DVB_TERRESTRIAL_GEN_HIERARCHY_NONE = 0,
  // α = 1
  SDRKIT_DVB_TERRESTRIAL_GEN_HIERARCHY_1 = 1,
  // α = 2
  SDRKIT_DVB_TERRESTRIAL_GEN_HIERARCHY_2 = 2,
  // α = 4
  SDRKIT_DVB_TERRESTRIAL_GEN_HIERARCHY_4 = 3,
} sdrkit_dvb_terrestrial_gen_hierarchy;

// The options of the setting "gen_rate_hp" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_rate_hp {
  // 1/2
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_HP_1_2 = 0,
  // 2/3
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_HP_2_3 = 1,
  // 3/4
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_HP_3_4 = 2,
  // 5/6
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_HP_5_6 = 3,
  // 7/8
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_HP_7_8 = 4,
} sdrkit_dvb_terrestrial_gen_rate_hp;

// The options of the setting "gen_rate_lp" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_rate_lp {
  // 1/2
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_LP_1_2 = 0,
  // 2/3
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_LP_2_3 = 1,
  // 3/4
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_LP_3_4 = 2,
  // 5/6
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_LP_5_6 = 3,
  // 7/8
  SDRKIT_DVB_TERRESTRIAL_GEN_RATE_LP_7_8 = 4,
} sdrkit_dvb_terrestrial_gen_rate_lp;

// The options of the setting "gen_bandwidth" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_bandwidth {
  // Auto
  SDRKIT_DVB_TERRESTRIAL_GEN_BANDWIDTH_AUTO = 0,
  // 8 MHz
  SDRKIT_DVB_TERRESTRIAL_GEN_BANDWIDTH_8_MHZ = 1,
  // 7 MHz
  SDRKIT_DVB_TERRESTRIAL_GEN_BANDWIDTH_7_MHZ = 2,
  // 6 MHz
  SDRKIT_DVB_TERRESTRIAL_GEN_BANDWIDTH_6_MHZ = 3,
  // 5 MHz
  SDRKIT_DVB_TERRESTRIAL_GEN_BANDWIDTH_5_MHZ = 4,
} sdrkit_dvb_terrestrial_gen_bandwidth;

// The options of the setting "gen_length" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_length {
  // 23
  SDRKIT_DVB_TERRESTRIAL_GEN_LENGTH_23 = 0,
  // 31
  SDRKIT_DVB_TERRESTRIAL_GEN_LENGTH_31 = 1,
} sdrkit_dvb_terrestrial_gen_length;

// The options of the setting "gen_source" of the stack "dvb_terrestrial".
typedef enum sdrkit_dvb_terrestrial_gen_source {
  // Built-in multiplex
  SDRKIT_DVB_TERRESTRIAL_GEN_SOURCE_BUILTIN = 0,
  // Counted test pattern
  SDRKIT_DVB_TERRESTRIAL_GEN_SOURCE_PATTERN = 1,
  // Null packets
  SDRKIT_DVB_TERRESTRIAL_GEN_SOURCE_NULL = 2,
} sdrkit_dvb_terrestrial_gen_source;

// The options of the setting "width" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_width {
  // Automatic
  SDRKIT_DVBSH_WIDTH_AUTO = 0,
  // 5 MHz
  SDRKIT_DVBSH_WIDTH_5_MHZ = 1,
  // 8 MHz
  SDRKIT_DVBSH_WIDTH_8_MHZ = 2,
  // 7 MHz
  SDRKIT_DVBSH_WIDTH_7_MHZ = 3,
  // 6 MHz
  SDRKIT_DVBSH_WIDTH_6_MHZ = 4,
  // 1.7 MHz
  SDRKIT_DVBSH_WIDTH_1_7 = 5,
} sdrkit_dvbsh_width;

// The options of the setting "gen_width" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_width {
  // Automatic
  SDRKIT_DVBSH_GEN_WIDTH_AUTO = 0,
  // 5 MHz
  SDRKIT_DVBSH_GEN_WIDTH_5_MHZ = 1,
  // 8 MHz
  SDRKIT_DVBSH_GEN_WIDTH_8_MHZ = 2,
  // 7 MHz
  SDRKIT_DVBSH_GEN_WIDTH_7_MHZ = 3,
  // 6 MHz
  SDRKIT_DVBSH_GEN_WIDTH_6_MHZ = 4,
  // 1.7 MHz
  SDRKIT_DVBSH_GEN_WIDTH_1_7 = 5,
} sdrkit_dvbsh_gen_width;

// The options of the setting "gen_mode" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_mode {
  // 1K
  SDRKIT_DVBSH_GEN_MODE_1K = 0,
  // 2K
  SDRKIT_DVBSH_GEN_MODE_2K = 1,
  // 4K
  SDRKIT_DVBSH_GEN_MODE_4K = 2,
  // 8K
  SDRKIT_DVBSH_GEN_MODE_8K = 3,
} sdrkit_dvbsh_gen_mode;

// The options of the setting "gen_constellation" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_constellation {
  // QPSK
  SDRKIT_DVBSH_GEN_CONSTELLATION_QPSK = 0,
  // 16-QAM
  SDRKIT_DVBSH_GEN_CONSTELLATION_16QAM = 1,
} sdrkit_dvbsh_gen_constellation;

// The options of the setting "gen_hierarchy" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_hierarchy {
  // None
  SDRKIT_DVBSH_GEN_HIERARCHY_NONE = 0,
  // α = 1
  SDRKIT_DVBSH_GEN_HIERARCHY_1 = 1,
  // α = 2
  SDRKIT_DVBSH_GEN_HIERARCHY_2 = 2,
  // α = 4
  SDRKIT_DVBSH_GEN_HIERARCHY_4 = 3,
} sdrkit_dvbsh_gen_hierarchy;

// The options of the setting "gen_guard" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_guard {
  // 1/32
  SDRKIT_DVBSH_GEN_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBSH_GEN_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBSH_GEN_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBSH_GEN_GUARD_1_4 = 3,
} sdrkit_dvbsh_gen_guard;

// The options of the setting "gen_rate" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_rate {
  // 1/5
  SDRKIT_DVBSH_GEN_RATE_1_5 = 0,
  // 2/9
  SDRKIT_DVBSH_GEN_RATE_2_9 = 1,
  // 1/4
  SDRKIT_DVBSH_GEN_RATE_1_4 = 2,
  // 2/7
  SDRKIT_DVBSH_GEN_RATE_2_7 = 3,
  // 1/3
  SDRKIT_DVBSH_GEN_RATE_1_3 = 4,
  // 1/3 complementary
  SDRKIT_DVBSH_GEN_RATE_1_3C = 5,
  // 2/5
  SDRKIT_DVBSH_GEN_RATE_2_5 = 6,
  // 2/5 complementary
  SDRKIT_DVBSH_GEN_RATE_2_5C = 7,
  // 1/2
  SDRKIT_DVBSH_GEN_RATE_1_2 = 8,
  // 1/2 complementary
  SDRKIT_DVBSH_GEN_RATE_1_2C = 9,
  // 2/3
  SDRKIT_DVBSH_GEN_RATE_2_3 = 10,
  // 2/3 complementary
  SDRKIT_DVBSH_GEN_RATE_2_3C = 11,
} sdrkit_dvbsh_gen_rate;

// The options of the setting "gen_rate_lp" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_rate_lp {
  // 1/5
  SDRKIT_DVBSH_GEN_RATE_LP_1_5 = 0,
  // 2/9
  SDRKIT_DVBSH_GEN_RATE_LP_2_9 = 1,
  // 1/4
  SDRKIT_DVBSH_GEN_RATE_LP_1_4 = 2,
  // 2/7
  SDRKIT_DVBSH_GEN_RATE_LP_2_7 = 3,
  // 1/3
  SDRKIT_DVBSH_GEN_RATE_LP_1_3 = 4,
  // 1/3 complementary
  SDRKIT_DVBSH_GEN_RATE_LP_1_3C = 5,
  // 2/5
  SDRKIT_DVBSH_GEN_RATE_LP_2_5 = 6,
  // 2/5 complementary
  SDRKIT_DVBSH_GEN_RATE_LP_2_5C = 7,
  // 1/2
  SDRKIT_DVBSH_GEN_RATE_LP_1_2 = 8,
  // 1/2 complementary
  SDRKIT_DVBSH_GEN_RATE_LP_1_2C = 9,
  // 2/3
  SDRKIT_DVBSH_GEN_RATE_LP_2_3 = 10,
  // 2/3 complementary
  SDRKIT_DVBSH_GEN_RATE_LP_2_3C = 11,
} sdrkit_dvbsh_gen_rate_lp;

// The options of the setting "waveform" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_waveform {
  // Automatic
  SDRKIT_DVBSH_WAVEFORM_AUTO = 0,
  // OFDM
  SDRKIT_DVBSH_WAVEFORM_OFDM = 1,
  // TDM
  SDRKIT_DVBSH_WAVEFORM_TDM = 2,
} sdrkit_dvbsh_waveform;

// The options of the setting "gen_waveform" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_waveform {
  // OFDM
  SDRKIT_DVBSH_GEN_WAVEFORM_OFDM = 0,
  // TDM QPSK
  SDRKIT_DVBSH_GEN_WAVEFORM_TDM_QPSK = 1,
  // TDM 8PSK
  SDRKIT_DVBSH_GEN_WAVEFORM_TDM_8PSK = 2,
  // TDM 16APSK
  SDRKIT_DVBSH_GEN_WAVEFORM_TDM_16APSK = 3,
} sdrkit_dvbsh_gen_waveform;

// The options of the setting "gen_rolloff" of the stack "dvbsh".
typedef enum sdrkit_dvbsh_gen_rolloff {
  // 0.15
  SDRKIT_DVBSH_GEN_ROLLOFF_0_15 = 0,
  // 0.25
  SDRKIT_DVBSH_GEN_ROLLOFF_0_25 = 1,
  // 0.35
  SDRKIT_DVBSH_GEN_ROLLOFF_0_35 = 2,
} sdrkit_dvbsh_gen_rolloff;

// The options of `bandwidth_from` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_bandwidth_from {
  // Setting
  SDRKIT_DVBT_FIELDS_BANDWIDTH_FROM_SETTING = 0,
  // Guess
  SDRKIT_DVBT_FIELDS_BANDWIDTH_FROM_GUESS = 1,
  // Measured
  SDRKIT_DVBT_FIELDS_BANDWIDTH_FROM_MEASURED = 2,
  // Tried
  SDRKIT_DVBT_FIELDS_BANDWIDTH_FROM_TRIED = 3,
} sdrkit_dvbt_fields_bandwidth_from;

// The options of `fft` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_fft {
  // 2K
  SDRKIT_DVBT_FIELDS_FFT_2_K = 0,
  // 8K
  SDRKIT_DVBT_FIELDS_FFT_8_K = 1,
} sdrkit_dvbt_fields_fft;

// The options of `guard` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_guard {
  // 1/32
  SDRKIT_DVBT_FIELDS_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBT_FIELDS_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBT_FIELDS_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBT_FIELDS_GUARD_1_4 = 3,
} sdrkit_dvbt_fields_guard;

// The options of `constellation` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_constellation {
  // QPSK
  SDRKIT_DVBT_FIELDS_CONSTELLATION_Q_P_S_K = 0,
  // 16-QAM
  SDRKIT_DVBT_FIELDS_CONSTELLATION_16_Q_A_M = 1,
  // 64-QAM
  SDRKIT_DVBT_FIELDS_CONSTELLATION_64_Q_A_M = 2,
} sdrkit_dvbt_fields_constellation;

// The options of `hierarchy` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_hierarchy {
  // None
  SDRKIT_DVBT_FIELDS_HIERARCHY_NONE = 0,
  // α = 1
  SDRKIT_DVBT_FIELDS_HIERARCHY_1 = 1,
  // α = 2
  SDRKIT_DVBT_FIELDS_HIERARCHY_2 = 2,
  // α = 4
  SDRKIT_DVBT_FIELDS_HIERARCHY_4 = 3,
} sdrkit_dvbt_fields_hierarchy;

// The options of `code_rate_hp` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_code_rate_hp {
  // 1/2
  SDRKIT_DVBT_FIELDS_CODE_RATE_HP_1_2 = 0,
  // 2/3
  SDRKIT_DVBT_FIELDS_CODE_RATE_HP_2_3 = 1,
  // 3/4
  SDRKIT_DVBT_FIELDS_CODE_RATE_HP_3_4 = 2,
  // 5/6
  SDRKIT_DVBT_FIELDS_CODE_RATE_HP_5_6 = 3,
  // 7/8
  SDRKIT_DVBT_FIELDS_CODE_RATE_HP_7_8 = 4,
} sdrkit_dvbt_fields_code_rate_hp;

// The options of `code_rate_lp` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_code_rate_lp {
  // 1/2
  SDRKIT_DVBT_FIELDS_CODE_RATE_LP_1_2 = 0,
  // 2/3
  SDRKIT_DVBT_FIELDS_CODE_RATE_LP_2_3 = 1,
  // 3/4
  SDRKIT_DVBT_FIELDS_CODE_RATE_LP_3_4 = 2,
  // 5/6
  SDRKIT_DVBT_FIELDS_CODE_RATE_LP_5_6 = 3,
  // 7/8
  SDRKIT_DVBT_FIELDS_CODE_RATE_LP_7_8 = 4,
} sdrkit_dvbt_fields_code_rate_lp;

// The options of `stream` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_stream {
  // High priority
  SDRKIT_DVBT_FIELDS_STREAM_HP = 0,
  // Low priority
  SDRKIT_DVBT_FIELDS_STREAM_LP = 1,
} sdrkit_dvbt_fields_stream;

// The options of `quality_label` of `sdrkit_dvbt_fields`.
typedef enum sdrkit_dvbt_fields_quality_label {
  // Excellent
  SDRKIT_DVBT_FIELDS_QUALITY_LABEL_EXCELLENT = 0,
  // Good
  SDRKIT_DVBT_FIELDS_QUALITY_LABEL_GOOD = 1,
  // Marginal
  SDRKIT_DVBT_FIELDS_QUALITY_LABEL_MARGINAL = 2,
  // Poor
  SDRKIT_DVBT_FIELDS_QUALITY_LABEL_POOR = 3,
  // No lock
  SDRKIT_DVBT_FIELDS_QUALITY_LABEL_NO_LOCK = 4,
} sdrkit_dvbt_fields_quality_label;

// The result fields of `dvbt`, which `report` writes: the input, the OFDM parameters, the TPS, the error correction and the quality. A value of the TPS has no value while the TPS is not locked; a value of the FFT mode has no value before the mode is found.
typedef struct sdrkit_dvbt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The detection level, 0 to 3.
  int64_t detect_level;
  // The channel width in use. A frequency in Hz.
  double channel_width;
  // The `bandwidth` setting is Auto.
  bool bandwidth_auto;
  // True if `bandwidth_from` holds a value.
  bool has_bandwidth_from;
  // Where the width in use came from. A value of `sdrkit_dvbt_fields_bandwidth_from`.
  int32_t bandwidth_from;
  // True if `measured_width` holds a value.
  bool has_measured_width;
  // The measured occupied width. A frequency in Hz.
  double measured_width;
  // True if `occupancy_db` holds a value.
  bool has_occupancy_db;
  // The level of the channel above the floor outside it. Unit: dB.
  double occupancy_db;
  // The input rate. A frequency in Hz.
  double input_rate;
  // The natural rate of the width in use. A frequency in Hz.
  double natural_rate;
  // The input is resampled.
  bool resampling;
  // The input rate is enough for the width.
  bool rate_ok;
  // True if `fft` holds a value.
  bool has_fft;
  // The FFT mode. A value of `sdrkit_dvbt_fields_fft`.
  int32_t fft;
  // True if `fft_size` holds a value.
  bool has_fft_size;
  // The FFT size.
  int64_t fft_size;
  // True if `guard` holds a value.
  bool has_guard;
  // The guard interval. A value of `sdrkit_dvbt_fields_guard`.
  int32_t guard;
  // True if `guard_samples` holds a value.
  bool has_guard_samples;
  // The guard samples.
  int64_t guard_samples;
  // True if `carriers` holds a value.
  bool has_carriers;
  // The carriers.
  int64_t carriers;
  // True if `constellation` holds a value.
  bool has_constellation;
  // The constellation. A value of `sdrkit_dvbt_fields_constellation`.
  int32_t constellation;
  // True if `hierarchy` holds a value.
  bool has_hierarchy;
  // The hierarchy. A value of `sdrkit_dvbt_fields_hierarchy`.
  int32_t hierarchy;
  // True if `code_rate_hp` holds a value.
  bool has_code_rate_hp;
  // The HP code rate. A value of `sdrkit_dvbt_fields_code_rate_hp`.
  int32_t code_rate_hp;
  // True if `code_rate_lp` holds a value.
  bool has_code_rate_lp;
  // The LP code rate. A value of `sdrkit_dvbt_fields_code_rate_lp`.
  int32_t code_rate_lp;
  // The stream decoded. A value of `sdrkit_dvbt_fields_stream`.
  int32_t stream;
  // True if `cell_id` holds a value.
  bool has_cell_id;
  // The cell id. No value until two frames agree.
  int64_t cell_id;
  // True if `tps_length` holds a value.
  bool has_tps_length;
  // The TPS length indicator.
  int64_t tps_length;
  // True if `dvbh_flags` holds a value.
  bool has_dvbh_flags;
  // The DVB-H signalling bits.
  int64_t dvbh_flags;
  // True if `frame` holds a value.
  bool has_frame;
  // The frame number, 0 to 3.
  int64_t frame;
  // True if `symbol` holds a value.
  bool has_symbol;
  // The symbol in the frame, 0 to 67.
  int64_t symbol;
  // True if `frame_duration` holds a value.
  bool has_frame_duration;
  // The duration of a frame. A length of time in seconds.
  double frame_duration;
  // True if `net_bitrate_bps` holds a value.
  bool has_net_bitrate_bps;
  // The net bit rate of the stream decoded. Unit: bit/s.
  double net_bitrate_bps;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset from the centre. A frequency in Hz.
  double offset_hz;
  // True if `integer_offset` holds a value.
  bool has_integer_offset;
  // The integer carrier offset. Unit: carriers.
  int64_t integer_offset;
  // The shift of the channel locator. A frequency in Hz.
  double locator_hz;
  // The error of the sample clock. Unit: ppm.
  double clock_ppm;
  // The spectrum is mirrored.
  bool spectrum_inverted;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled. Unit: dB.
  double snr_db;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The modulation error ratio. No value before the channel estimate is filled. Unit: dB.
  double mer_db;
  // The BER before Viterbi over the last second: the received hard bits that differ from the decoded bits, coded again.
  double ber_pre_viterbi;
  // The BER after Viterbi over the last second: the bits that Reed-Solomon corrected over the bits of the blocks that it decoded. A block that failed is not in it and is counted in `rs_failed`. 0 while no block decoded.
  double ber_post_viterbi;
  // The Viterbi margin.
  double viterbi_margin;
  // True if `puncture_phase` holds a value.
  bool has_puncture_phase;
  // The puncturing phase.
  int64_t puncture_phase;
  // RS blocks handled: those without error, those corrected and those that failed.
  int64_t rs_blocks;
  // RS blocks without error.
  int64_t rs_clean;
  // RS blocks with 1 to 8 bytes wrong, which were corrected. Their corrected bits are the BER after Viterbi.
  int64_t rs_corrected;
  // RS blocks that could not be corrected (more than 8 bytes wrong). Each gives a packet with the transport_error_indicator set. They do not enter the BER after Viterbi.
  int64_t rs_failed;
  // Bytes that were corrected in the RS blocks corrected.
  int64_t rs_bytes_corrected;
  // Transport packets delivered, with those of the blocks that failed.
  int64_t packets;
  // The packet alignment holds.
  bool fec_sync;
  // Resynchronisations.
  int64_t resyncs;
  // True if `since_tps` holds a value.
  bool has_since_tps;
  // The signal time since the last good TPS frame. No value before the first good frame. A length of time in seconds.
  double since_tps;
  // The quality score, 0 to 100. Unit: %.
  double quality_pct;
  // The quality class. A value of `sdrkit_dvbt_fields_quality_label`.
  int32_t quality_label;
  // True if `required_cn_db` holds a value.
  bool has_required_cn_db;
  // The C/N that the constellation and code rate need. Unit: dB.
  double required_cn_db;
  // True if `margin_db` holds a value.
  bool has_margin_db;
  // The SNR above the required C/N. Unit: dB.
  double margin_db;
  // True if `unsupported` holds a value.
  bool has_unsupported;
  // The features that TPS signals and the receiver does not decode.
  sdrkit_string unsupported;
  // The channel estimate is filled.
  bool data_valid;
  // True if `first_packet` holds a value.
  bool has_first_packet;
  // The signal time of the first good packet. A length of time in seconds.
  double first_packet;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. No value before it is measured. Unit: dBFS.
  double level_dbfs;
} sdrkit_dvbt_fields;

// The options of `player_status` of `sdrkit_dvbt_multiplex_fields`.
typedef enum sdrkit_dvbt_multiplex_fields_player_status {
  // Starting
  SDRKIT_DVBT_MULTIPLEX_FIELDS_PLAYER_STATUS_STARTING = 0,
  // Playing
  SDRKIT_DVBT_MULTIPLEX_FIELDS_PLAYER_STATUS_PLAYING = 1,
  // Scrambled
  SDRKIT_DVBT_MULTIPLEX_FIELDS_PLAYER_STATUS_SCRAMBLED = 2,
  // No service
  SDRKIT_DVBT_MULTIPLEX_FIELDS_PLAYER_STATUS_NO_SERVICE = 3,
} sdrkit_dvbt_multiplex_fields_player_status;

// The fields of the transport stream layer that `dvbt` shows under its own keys: the network and the time of the stream (`psi_si`), and the service that plays (`ts_demux`).
typedef struct sdrkit_dvbt_multiplex_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The network name.
  sdrkit_string network_name;
  // network_id.
  int64_t network_id;
  // The number of services.
  int64_t service_count;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the stream. No value before the stream gave the time. Seconds since 1970 (UTC).
  double utc;
  // True if `selected_service` holds a value.
  bool has_selected_service;
  // The service that plays. No value while none plays.
  int64_t selected_service;
  // What the player does. A value of `sdrkit_dvbt_multiplex_fields_player_status`.
  int32_t player_status;
  // The bit rate of the multiplex. Unit: kbit/s.
  double mux_kbps;
} sdrkit_dvbt_multiplex_fields;

// The result fields of the sender `transmit` of `dvbt`.
typedef struct sdrkit_dvbt_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the centre of the channel sent. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // The bit rate of the transport stream that the settings carry (of the high-priority stream in a hierarchical multiplex): the rate that the host feeds at most. Unit: bit/s.
  double tx_bitrate_bps;
  // True if `tx_bitrate_lp_bps` holds a value.
  bool has_tx_bitrate_lp_bps;
  // The bit rate of the low-priority stream. No value in a multiplex that is not hierarchical. Unit: bit/s.
  double tx_bitrate_lp_bps;
  // The packets of the host sent in this transmission, or in the last one.
  int64_t tx_packets;
  // The null packets put in slots that found no packet of the host.
  int64_t tx_nulls;
  // The packets that wait to be sent.
  int64_t tx_queue;
  // The packets of the low-priority stream that wait to be sent.
  int64_t tx_queue_lp;
  // The time that the waiting packets take at the useful bit rate. A length of time in seconds.
  double tx_queue_s;
  // The PCRs rewritten to the time of their slot.
  int64_t tx_pcr_restamped;
} sdrkit_dvbt_tx_fields;

// The result fields of the synchroniser `sync`. No value before the FFT mode and the guard interval are found.
typedef struct sdrkit_dvbt_sync_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `timing` holds a value.
  bool has_timing;
  // The timing error of the last symbol. Unit: samples.
  double timing;
  // True if `rho` holds a value.
  bool has_rho;
  // The correlation coefficient of the last symbol.
  double rho;
  // True if `frac` holds a value.
  bool has_frac;
  // The fractional carrier offset in use. Unit: carriers.
  double frac;
} sdrkit_dvbt_sync_fields;

// The result fields of the FFT stage `fft`.
typedef struct sdrkit_dvbt_fft_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `integer_offset` holds a value.
  bool has_integer_offset;
  // The integer carrier offset. No value before it is found. Unit: carriers.
  int64_t integer_offset;
  // The spectrum is mirrored.
  bool spectrum_inverted;
} sdrkit_dvbt_fft_fields;

// The result fields of the TPS decoder `tps`.
typedef struct sdrkit_dvbt_tps_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // TPS frames decoded.
  int64_t tps_frames_ok;
  // TPS frames that failed.
  int64_t tps_frames_failed;
} sdrkit_dvbt_tps_fields;

// The result fields of the equaliser `eq`.
typedef struct sdrkit_dvbt_eq_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled. Unit: dB.
  double snr_db;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The modulation error ratio. No value before the channel estimate is filled. Unit: dB.
  double mer_db;
  // The time-averaging gain of the channel estimate.
  double alpha;
  // The centre of the delay window. Unit: samples.
  double delay_window_centre;
  // The half width of the delay window. Unit: samples.
  double delay_window_half;
  // The measured delay spread. Unit: samples.
  double delay_spread;
} sdrkit_dvbt_eq_fields;

// The result fields of the error correction `fec`.
typedef struct sdrkit_dvbt_fec_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Transport packets delivered, with those of the blocks that failed.
  int64_t frames_ok;
  // Reed-Solomon blocks that could not be corrected. Each gives a packet with the transport_error_indicator set.
  int64_t frames_bad;
} sdrkit_dvbt_fec_fields;

// The options of `standard` of `sdrkit_dvb_terrestrial_fields`.
typedef enum sdrkit_dvb_terrestrial_fields_standard {
  // DVB-T2
  SDRKIT_DVB_TERRESTRIAL_FIELDS_STANDARD_DVBT2 = 0,
  // DVB-T
  SDRKIT_DVB_TERRESTRIAL_FIELDS_STANDARD_DVBT = 1,
} sdrkit_dvb_terrestrial_fields_standard;

// The result set that the receiver of `dvb_terrestrial` adds to the results of the DVB-T receiver.
typedef struct sdrkit_dvb_terrestrial_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The standard that decodes now. A value of `sdrkit_dvb_terrestrial_fields_standard`.
  int32_t standard;
} sdrkit_dvb_terrestrial_fields;

// The options of `waveform` of `sdrkit_dvbsh_fields`.
typedef enum sdrkit_dvbsh_fields_waveform {
  // OFDM
  SDRKIT_DVBSH_FIELDS_WAVEFORM_OFDM = 0,
  // TDM
  SDRKIT_DVBSH_FIELDS_WAVEFORM_TDM = 1,
} sdrkit_dvbsh_fields_waveform;

// The result fields of `dvbsh`, which `data` writes: the OFDM parameters, the SH frame and the turbo code. A value of the TPS has no value while the TPS is not locked.
typedef struct sdrkit_dvbsh_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // OFDM or TDM. A value of `sdrkit_dvbsh_fields_waveform`.
  int32_t waveform;
  // The channel width in use. Unit: MHz.
  double width_mhz;
  // 0 searching, 1 OFDM symbols found, 2 TPS, 3 parameters, 4 transport stream flowing.
  int64_t lock;
  // True if `cfo_hz` holds a value.
  bool has_cfo_hz;
  // The carrier offset. No value before the FFT mode is found. A frequency in Hz.
  double cfo_hz;
  // True if `fft_size` holds a value.
  bool has_fft_size;
  // The FFT size.
  int64_t fft_size;
  // True if `guard_samples` holds a value.
  bool has_guard_samples;
  // The guard samples.
  int64_t guard_samples;
  // True if `mode` holds a value.
  bool has_mode;
  // The OFDM mode from TPS.
  sdrkit_string mode;
  // True if `guard` holds a value.
  bool has_guard;
  // The guard interval from TPS.
  sdrkit_string guard;
  // True if `constellation` holds a value.
  bool has_constellation;
  // The constellation from TPS.
  sdrkit_string constellation;
  // True if `hierarchy` holds a value.
  bool has_hierarchy;
  // The hierarchy from TPS.
  sdrkit_string hierarchy;
  // True if `rate_hp` holds a value.
  bool has_rate_hp;
  // The code rate of the high-priority stream ("1/3", "1/3c" for complementary).
  sdrkit_string rate_hp;
  // True if `rate_lp` holds a value.
  bool has_rate_lp;
  // The code rate of the low-priority stream of a hierarchical multiplex.
  sdrkit_string rate_lp;
  // The stream decoded, HP or LP.
  sdrkit_string stream;
  // True if `interleaver` holds a value.
  bool has_interleaver;
  // The interleaver profile, "cm / lt / ns / sd / nli".
  sdrkit_string interleaver;
  // True if `interleaver_s` holds a value.
  bool has_interleaver_s;
  // The length of the time interleaver. A length of time in seconds.
  double interleaver_s;
  // True if `interleaver_mb` holds a value.
  bool has_interleaver_mb;
  // The memory that the time de-interleaver holds. Unit: MB.
  double interleaver_mb;
  // True if `refused` holds a value.
  bool has_refused;
  // Why the data path does not run (an interleaver too long for this receiver, a reserved pattern). No value while it runs.
  sdrkit_string refused;
  // True if `cell_id` holds a value.
  bool has_cell_id;
  // The 16-bit cell id from TPS.
  int64_t cell_id;
  // True if `superframe` holds a value.
  bool has_superframe;
  // The super-frame number in the SH frame.
  int64_t superframe;
  // Code words decoded.
  int64_t codewords_ok;
  // Code words not decoded.
  int64_t codewords_bad;
  // Packets with a good CRC.
  int64_t packets_ok;
  // Packets passed with the transport_error_indicator.
  int64_t packets_bad;
  // True if `turbo_iterations` holds a value.
  bool has_turbo_iterations;
  // The mean turbo iterations of the decoded code words. No value before a code word decodes.
  double turbo_iterations;
  // True if `first_packet_s` holds a value.
  bool has_first_packet_s;
  // The signal time of the first good packet. A length of time in seconds.
  double first_packet_s;
  // What the stack decodes and which of its values no public document confirms.
  sdrkit_string note;
  // The error of the sample clock. Unit: ppm.
  double clock_ppm;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. No value before it is measured. Unit: dBFS.
  double level_dbfs;
  // True if `tdm_modulation` holds a value.
  bool has_tdm_modulation;
  // The modulation of the TDM form, which the SOF shows.
  sdrkit_string tdm_modulation;
  // True if `tdm_symbol_rate` holds a value.
  bool has_tdm_symbol_rate;
  // The symbol rate of the TDM form that is tried or received. Unit: sym/s.
  double tdm_symbol_rate;
  // True if `tdm_rolloff` holds a value.
  bool has_tdm_rolloff;
  // The roll-off of the TDM form, from the row of the symbol rate. No value before slots are found.
  double tdm_rolloff;
  // True if `tdm_frame_width_cu` holds a value.
  bool has_tdm_frame_width_cu;
  // The width of the TDM SH frame from its signalling field. Unit: CU.
  int64_t tdm_frame_width_cu;
  // True if `tdm_mer_db` holds a value.
  bool has_tdm_mer_db;
  // The power of a pilot symbol of the TDM form over the noise in the symbol rate. Unit: dB.
  double tdm_mer_db;
  // Slots of the TDM form erased for want of pilots.
  int64_t tdm_slots_erased;
  // Signalling fields of the TDM form that did not decode.
  int64_t tdm_signalling_failed;
} sdrkit_dvbsh_fields;

// The fields of the OFDM chain that `dvbsh` shows under its own keys: the SNR of the equaliser and the counts of the TPS decoder.
typedef struct sdrkit_dvbsh_chain_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled. Unit: dB.
  double snr_db;
  // TPS frames decoded.
  int64_t tps_frames_ok;
  // TPS frames that failed.
  int64_t tps_frames_failed;
} sdrkit_dvbsh_chain_fields;

// The arguments of `service.select`.
typedef struct sdrkit_dvbt_service_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the service in `services`. Empty: the first service that can play.
  sdrkit_string row;
} sdrkit_dvbt_service_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT_SERVICE_SELECT_ARGS_INIT {sizeof(sdrkit_dvbt_service_select_args), {0}}

// The arguments of `teletext.select`.
typedef struct sdrkit_dvbt_teletext_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the page in `teletext_pages`.
  sdrkit_string row;
} sdrkit_dvbt_teletext_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT_TELETEXT_SELECT_ARGS_INIT {sizeof(sdrkit_dvbt_teletext_select_args), {0}}

// The arguments of `track.select`.
typedef struct sdrkit_dvbt_track_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the track in `tracks`.
  sdrkit_string row;
} sdrkit_dvbt_track_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT_TRACK_SELECT_ARGS_INIT {sizeof(sdrkit_dvbt_track_select_args), {0}}

// The arguments of the `append` command of a sender that takes transport packets.
typedef struct sdrkit_dvbt_tx_append_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The packets.
  sdrkit_bytes packets;
} sdrkit_dvbt_tx_append_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT_TX_APPEND_ARGS_INIT {sizeof(sdrkit_dvbt_tx_append_args), {0}}

// Fills `rows` with the changed rows of the table "services" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_services_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mpegts_ts_service_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_tracks_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_track_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_guide_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mpegts_ts_guide_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_guide_rating_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_teletext_page_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_mpegts_teletext_subpage_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_pid_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_ts_section_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "dvbt" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_dvbt_service_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "dvbt.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "teletext.select" (Select a teletext page) of the stack "dvbt" on the session of `handle`. Shows a teletext page.
static inline int32_t sdrkit_dvbt_teletext_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_teletext_select_args *args) {
  return sdrkit_typed_command(handle, "teletext.select", "dvbt.TeletextSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "dvbt" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_dvbt_track_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "dvbt.TrackSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "start" (Start sending) of the stack "dvbt" on the session of `handle`. Sends a DVB-T signal with the tx_* settings from the start of a super-frame, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_dvbt_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "dvbt" on the session of `handle`. Ends the transmission after the frame on air with a fall of 5 ms. The packets that wait are dropped.
static inline int32_t sdrkit_dvbt_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "append" (Add packets) of the stack "dvbt" on the session of `handle`. Adds transport packets to the queue of the stream, or of the high-priority stream of a hierarchical multiplex. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.
static inline int32_t sdrkit_dvbt_append(SDRKIT_HANDLE handle, const sdrkit_dvbt_tx_append_args *args) {
  return sdrkit_typed_command(handle, "append", "dvbt.TxAppendArgs", args, NULL, NULL, 0);
}

// Runs the command "append_lp" (Add packets (LP)) of the stack "dvbt" on the session of `handle`. Adds transport packets to the queue of the low-priority stream of a hierarchical multiplex. All or none.
static inline int32_t sdrkit_dvbt_append_lp(SDRKIT_HANDLE handle, const sdrkit_dvbt_tx_append_args *args) {
  return sdrkit_typed_command(handle, "append_lp", "dvbt.TxAppendArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvbt" are the sdrkit_dvbt_gen_*() functions
// after the settings of its receiver.

// The setting "bandwidth" (Channel bandwidth) of the stack "dvbt". A value of `sdrkit_dvbt_bandwidth`. The width of the channel. Auto measures the width from the spectrum, then tries the other widths when nothing locks.
static inline int32_t sdrkit_dvbt_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "stream" (Hierarchical stream) of the stack "dvbt". A value of `sdrkit_dvbt_stream`. Which stream of a hierarchical multiplex is decoded. A multiplex that is not hierarchical has only one.
static inline int32_t sdrkit_dvbt_get_stream(SDRKIT_HANDLE handle, sdrkit_dvbt_stream *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "stream", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_stream)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_stream(SDRKIT_HANDLE handle, sdrkit_dvbt_stream value) {
  return sdrkit_typed_setting_set_enum(handle, "stream", (int32_t)value);
}

// The setting "viterbi" (Viterbi decoder) of the stack "dvbt". A value of `sdrkit_dvbt_viterbi`.
static inline int32_t sdrkit_dvbt_get_viterbi(SDRKIT_HANDLE handle, sdrkit_dvbt_viterbi *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "viterbi", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_viterbi)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_viterbi(SDRKIT_HANDLE handle, sdrkit_dvbt_viterbi value) {
  return sdrkit_typed_setting_set_enum(handle, "viterbi", (int32_t)value);
}

// The setting "time_interp" (Time interpolation of pilots) of the stack "dvbt". Off uses the newest pilot: lower latency, worse with Doppler.
static inline int32_t sdrkit_dvbt_get_time_interp(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "time_interp", value);
}
static inline int32_t sdrkit_dvbt_set_time_interp(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "time_interp", value);
}

// The setting "interferer_erase" (Narrowband interferer weighting) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_interferer_erase(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "interferer_erase", value);
}
static inline int32_t sdrkit_dvbt_set_interferer_erase(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "interferer_erase", value);
}

// The setting "standard" (Standard) of the stack "dvbt". A value of `sdrkit_dvbt_standard`.
static inline int32_t sdrkit_dvbt_get_standard(SDRKIT_HANDLE handle, sdrkit_dvbt_standard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "standard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_standard)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_standard(SDRKIT_HANDLE handle, sdrkit_dvbt_standard value) {
  return sdrkit_typed_setting_set_enum(handle, "standard", (int32_t)value);
}

// The setting "channels" (Channels) of the stack "dvbt". The centre frequencies in Hz of the channels in use, separated by ";". At most 256 entries. A radio tuned to none of them asks the host to tune to the lowest. Empty: no list.
static inline int32_t sdrkit_dvbt_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_dvbt_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "audio_lang" (Audio language) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_audio_lang(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "audio_lang", value);
}
static inline int32_t sdrkit_dvbt_set_audio_lang(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "audio_lang", value);
}

// The setting "audio_description" (Audio description) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_audio_description(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_description", value);
}
static inline int32_t sdrkit_dvbt_set_audio_description(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_description", value);
}

// The setting "downmix" (Stereo downmix) of the stack "dvbt". Off: 5.1 sound leaves with all its channels, in the order that the audio information states.
static inline int32_t sdrkit_dvbt_get_downmix(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "downmix", value);
}
static inline int32_t sdrkit_dvbt_set_downmix(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "downmix", value);
}

// The setting "decode_video" (Decode video) of the stack "dvbt". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvbt_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvbt_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "subtitle_lang" (Subtitle language) of the stack "dvbt". Empty: subtitles off.
static inline int32_t sdrkit_dvbt_get_subtitle_lang(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "subtitle_lang", value);
}
static inline int32_t sdrkit_dvbt_set_subtitle_lang(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "subtitle_lang", value);
}

// The setting "hard_of_hearing" (Subtitles for the hard of hearing) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_hard_of_hearing(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hard_of_hearing", value);
}
static inline int32_t sdrkit_dvbt_set_hard_of_hearing(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hard_of_hearing", value);
}

// The setting "country" (Country) of the stack "dvbt". ISO 3166 alpha-3 code (NOR, AUS), for channel numbers and the local time.
static inline int32_t sdrkit_dvbt_get_country(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "country", value);
}
static inline int32_t sdrkit_dvbt_set_country(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "country", value);
}

// The setting "guide_lang" (Guide language) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_guide_lang(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "guide_lang", value);
}
static inline int32_t sdrkit_dvbt_set_guide_lang(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "guide_lang", value);
}

// The setting "test_flat_csi" (Flat noise weighting) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_test_flat_csi(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "test_flat_csi", value);
}
static inline int32_t sdrkit_dvbt_set_test_flat_csi(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "test_flat_csi", value);
}

// The setting "test_full_window" (Full delay window) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_test_full_window(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "test_full_window", value);
}
static inline int32_t sdrkit_dvbt_set_test_full_window(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "test_full_window", value);
}

// The setting "test_peak_only" (Window from the correlation peak) of the stack "dvbt".
static inline int32_t sdrkit_dvbt_get_test_peak_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "test_peak_only", value);
}
static inline int32_t sdrkit_dvbt_set_test_peak_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "test_peak_only", value);
}

// The setting "tx_bandwidth" (Channel width) of the stack "dvbt". A value of `sdrkit_dvbt_tx_bandwidth`. The width of the channel sent: 8, 7, 6 or 5 MHz. It sets the elementary rate of the signal.
static inline int32_t sdrkit_dvbt_get_tx_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_bandwidth", (int32_t)value);
}

// The setting "tx_mode" (FFT mode) of the stack "dvbt". A value of `sdrkit_dvbt_tx_mode`. 2K or 8K carriers.
static inline int32_t sdrkit_dvbt_get_tx_mode(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_mode)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_mode(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_mode", (int32_t)value);
}

// The setting "tx_guard" (Guard interval) of the stack "dvbt". A value of `sdrkit_dvbt_tx_guard`.
static inline int32_t sdrkit_dvbt_get_tx_guard(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_guard)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_guard(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_guard", (int32_t)value);
}

// The setting "tx_constellation" (Constellation) of the stack "dvbt". A value of `sdrkit_dvbt_tx_constellation`.
static inline int32_t sdrkit_dvbt_get_tx_constellation(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_constellation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_constellation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_constellation)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_constellation(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_constellation value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_constellation", (int32_t)value);
}

// The setting "tx_hierarchy" (Hierarchy) of the stack "dvbt". A value of `sdrkit_dvbt_tx_hierarchy`. None, or a hierarchical multiplex with the factor α: the low-priority stream then takes the packets of append_lp.
static inline int32_t sdrkit_dvbt_get_tx_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_hierarchy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_hierarchy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_hierarchy)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_hierarchy value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_hierarchy", (int32_t)value);
}

// The setting "tx_rate_hp" (Code rate) of the stack "dvbt". A value of `sdrkit_dvbt_tx_rate_hp`. The code rate of the stream, or of the high-priority stream of a hierarchical multiplex.
static inline int32_t sdrkit_dvbt_get_tx_rate_hp(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_rate_hp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_rate_hp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_rate_hp)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_rate_hp(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_rate_hp value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_rate_hp", (int32_t)value);
}

// The setting "tx_rate_lp" (Code rate (LP)) of the stack "dvbt". A value of `sdrkit_dvbt_tx_rate_lp`. The code rate of the low-priority stream of a hierarchical multiplex. TPS carries it in every multiplex.
static inline int32_t sdrkit_dvbt_get_tx_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_rate_lp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_rate_lp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_tx_rate_lp)index;
  return code;
}
static inline int32_t sdrkit_dvbt_set_tx_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_rate_lp value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_rate_lp", (int32_t)value);
}

// The setting "tx_cell_id" (Cell id) of the stack "dvbt". Sent in TPS with the length indicator 31. 0: no cell id, length indicator 23.
static inline int32_t sdrkit_dvbt_get_tx_cell_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_cell_id", value);
}
static inline int32_t sdrkit_dvbt_set_tx_cell_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_cell_id", value);
}

// The setting "gen_mode" (Mode) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_mode`.
static inline int32_t sdrkit_dvbt_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_guard" (Guard interval) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_guard`.
static inline int32_t sdrkit_dvbt_gen_get_guard(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_guard)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_guard(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_guard", (int32_t)value);
}

// The setting "gen_constellation" (Constellation) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_constellation`.
static inline int32_t sdrkit_dvbt_gen_get_constellation(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_constellation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_constellation)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_constellation(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_constellation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_constellation", (int32_t)value);
}

// The setting "gen_hierarchy" (Hierarchy) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_hierarchy`.
static inline int32_t sdrkit_dvbt_gen_get_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_hierarchy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_hierarchy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_hierarchy)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_hierarchy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_hierarchy", (int32_t)value);
}

// The setting "gen_rate_hp" (HP code rate) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_rate_hp`.
static inline int32_t sdrkit_dvbt_gen_get_rate_hp(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_rate_hp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate_hp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_rate_hp)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_rate_hp(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_rate_hp value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate_hp", (int32_t)value);
}

// The setting "gen_rate_lp" (LP code rate) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_rate_lp`.
static inline int32_t sdrkit_dvbt_gen_get_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_rate_lp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate_lp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_rate_lp)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_rate_lp value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate_lp", (int32_t)value);
}

// The setting "gen_bandwidth" (Bandwidth) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_bandwidth`.
static inline int32_t sdrkit_dvbt_gen_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bandwidth", (int32_t)value);
}

// The setting "gen_cell_id" (Cell id) of the test signal generator of the stack "dvbt".
static inline int32_t sdrkit_dvbt_gen_get_cell_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_cell_id", value);
}
static inline int32_t sdrkit_dvbt_gen_set_cell_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_cell_id", value);
}

// The setting "gen_length" (TPS length indicator) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_length`.
static inline int32_t sdrkit_dvbt_gen_get_length(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_length)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_length(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_length value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_length", (int32_t)value);
}

// The setting "gen_phase" (Puncturing start phase) of the test signal generator of the stack "dvbt".
static inline int32_t sdrkit_dvbt_gen_get_phase(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_phase", value);
}
static inline int32_t sdrkit_dvbt_gen_set_phase(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_phase", value);
}

// The setting "gen_service" (Service id) of the test signal generator of the stack "dvbt".
static inline int32_t sdrkit_dvbt_gen_get_service(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_service", value);
}
static inline int32_t sdrkit_dvbt_gen_set_service(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_service", value);
}

// The setting "gen_name" (Service name) of the test signal generator of the stack "dvbt".
static inline int32_t sdrkit_dvbt_gen_get_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_name", value);
}
static inline int32_t sdrkit_dvbt_gen_set_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_name", value);
}

// The setting "gen_source" (Packets) of the test signal generator of the stack "dvbt". A value of `sdrkit_dvbt_gen_source`.
static inline int32_t sdrkit_dvbt_gen_get_source(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt_gen_source)index;
  return code;
}
static inline int32_t sdrkit_dvbt_gen_set_source(SDRKIT_HANDLE handle, sdrkit_dvbt_gen_source value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_source", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_services_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_mpegts_ts_service_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_tracks_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_mpegts_ts_track_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_guide_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_guide_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                               sdrkit_mpegts_ts_guide_rating_row *rows,
                                                               uint32_t capacity,
                                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                                sdrkit_mpegts_teletext_page_row *rows,
                                                                uint32_t capacity,
                                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                                   sdrkit_mpegts_teletext_subpage_row *rows,
                                                                   uint32_t capacity,
                                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                               sdrkit_mpegts_ts_pid_row *rows,
                                                               uint32_t capacity,
                                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvb_terrestrial" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvb_terrestrial_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                                 sdrkit_mpegts_ts_section_row *rows,
                                                                 uint32_t capacity,
                                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "dvb_terrestrial" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_dvb_terrestrial_service_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "dvbt.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "teletext.select" (Select a teletext page) of the stack "dvb_terrestrial" on the session of `handle`. Shows a teletext page.
static inline int32_t sdrkit_dvb_terrestrial_teletext_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_teletext_select_args *args) {
  return sdrkit_typed_command(handle, "teletext.select", "dvbt.TeletextSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "dvb_terrestrial" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_dvb_terrestrial_track_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "dvbt.TrackSelectArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvb_terrestrial" are the sdrkit_dvb_terrestrial_gen_*() functions
// after the settings of its receiver.

// The setting "bandwidth" (Channel bandwidth) of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_bandwidth`. The width of the channel. Auto measures the width from the spectrum, then tries the other widths when nothing locks.
static inline int32_t sdrkit_dvb_terrestrial_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "stream" (Hierarchical stream) of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_stream`. Which stream of a hierarchical multiplex is decoded. A multiplex that is not hierarchical has only one.
static inline int32_t sdrkit_dvb_terrestrial_get_stream(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_stream *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "stream", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_stream)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_set_stream(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_stream value) {
  return sdrkit_typed_setting_set_enum(handle, "stream", (int32_t)value);
}

// The setting "viterbi" (Viterbi decoder) of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_viterbi`.
static inline int32_t sdrkit_dvb_terrestrial_get_viterbi(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_viterbi *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "viterbi", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_viterbi)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_set_viterbi(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_viterbi value) {
  return sdrkit_typed_setting_set_enum(handle, "viterbi", (int32_t)value);
}

// The setting "time_interp" (Time interpolation of pilots) of the stack "dvb_terrestrial". Off uses the newest pilot: lower latency, worse with Doppler.
static inline int32_t sdrkit_dvb_terrestrial_get_time_interp(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "time_interp", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_time_interp(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "time_interp", value);
}

// The setting "interferer_erase" (Narrowband interferer weighting) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_interferer_erase(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "interferer_erase", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_interferer_erase(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "interferer_erase", value);
}

// The setting "standard" (Standard) of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_standard`.
static inline int32_t sdrkit_dvb_terrestrial_get_standard(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_standard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "standard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_standard)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_set_standard(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_standard value) {
  return sdrkit_typed_setting_set_enum(handle, "standard", (int32_t)value);
}

// The setting "channels" (Channels) of the stack "dvb_terrestrial". The centre frequencies in Hz of the channels in use, separated by ";". At most 256 entries. A radio tuned to none of them asks the host to tune to the lowest. Empty: no list.
static inline int32_t sdrkit_dvb_terrestrial_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "audio_lang" (Audio language) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_audio_lang(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "audio_lang", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_audio_lang(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "audio_lang", value);
}

// The setting "audio_description" (Audio description) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_audio_description(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_description", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_audio_description(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_description", value);
}

// The setting "downmix" (Stereo downmix) of the stack "dvb_terrestrial". Off: 5.1 sound leaves with all its channels, in the order that the audio information states.
static inline int32_t sdrkit_dvb_terrestrial_get_downmix(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "downmix", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_downmix(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "downmix", value);
}

// The setting "decode_video" (Decode video) of the stack "dvb_terrestrial". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvb_terrestrial_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "subtitle_lang" (Subtitle language) of the stack "dvb_terrestrial". Empty: subtitles off.
static inline int32_t sdrkit_dvb_terrestrial_get_subtitle_lang(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "subtitle_lang", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_subtitle_lang(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "subtitle_lang", value);
}

// The setting "hard_of_hearing" (Subtitles for the hard of hearing) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_hard_of_hearing(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hard_of_hearing", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_hard_of_hearing(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hard_of_hearing", value);
}

// The setting "country" (Country) of the stack "dvb_terrestrial". ISO 3166 alpha-3 code (NOR, AUS), for channel numbers and the local time.
static inline int32_t sdrkit_dvb_terrestrial_get_country(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "country", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_country(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "country", value);
}

// The setting "guide_lang" (Guide language) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_guide_lang(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "guide_lang", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_guide_lang(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "guide_lang", value);
}

// The setting "test_flat_csi" (Flat noise weighting) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_test_flat_csi(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "test_flat_csi", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_test_flat_csi(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "test_flat_csi", value);
}

// The setting "test_full_window" (Full delay window) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_test_full_window(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "test_full_window", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_test_full_window(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "test_full_window", value);
}

// The setting "test_peak_only" (Window from the correlation peak) of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_get_test_peak_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "test_peak_only", value);
}
static inline int32_t sdrkit_dvb_terrestrial_set_test_peak_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "test_peak_only", value);
}

// The setting "gen_mode" (Mode) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_mode`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_guard" (Guard interval) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_guard`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_guard(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_guard)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_guard(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_guard", (int32_t)value);
}

// The setting "gen_constellation" (Constellation) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_constellation`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_constellation(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_constellation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_constellation)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_constellation(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_constellation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_constellation", (int32_t)value);
}

// The setting "gen_hierarchy" (Hierarchy) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_hierarchy`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_hierarchy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_hierarchy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_hierarchy)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_hierarchy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_hierarchy", (int32_t)value);
}

// The setting "gen_rate_hp" (HP code rate) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_rate_hp`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_rate_hp(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_rate_hp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate_hp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_rate_hp)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_rate_hp(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_rate_hp value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate_hp", (int32_t)value);
}

// The setting "gen_rate_lp" (LP code rate) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_rate_lp`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_rate_lp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate_lp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_rate_lp)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_rate_lp value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate_lp", (int32_t)value);
}

// The setting "gen_bandwidth" (Bandwidth) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_bandwidth`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bandwidth", (int32_t)value);
}

// The setting "gen_cell_id" (Cell id) of the test signal generator of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_gen_get_cell_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_cell_id", value);
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_cell_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_cell_id", value);
}

// The setting "gen_length" (TPS length indicator) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_length`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_length(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_length)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_length(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_length value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_length", (int32_t)value);
}

// The setting "gen_phase" (Puncturing start phase) of the test signal generator of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_gen_get_phase(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_phase", value);
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_phase(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_phase", value);
}

// The setting "gen_service" (Service id) of the test signal generator of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_gen_get_service(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_service", value);
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_service(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_service", value);
}

// The setting "gen_name" (Service name) of the test signal generator of the stack "dvb_terrestrial".
static inline int32_t sdrkit_dvb_terrestrial_gen_get_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_name", value);
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_name", value);
}

// The setting "gen_source" (Packets) of the test signal generator of the stack "dvb_terrestrial". A value of `sdrkit_dvb_terrestrial_gen_source`.
static inline int32_t sdrkit_dvb_terrestrial_gen_get_source(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvb_terrestrial_gen_source)index;
  return code;
}
static inline int32_t sdrkit_dvb_terrestrial_gen_set_source(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_gen_source value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_source", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dvbsh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbsh_services_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mpegts_ts_service_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvbsh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbsh_tracks_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mpegts_ts_track_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvbsh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbsh_guide_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_guide_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvbsh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbsh_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_guide_rating_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvbsh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbsh_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_pid_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvbsh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbsh_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_section_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "dvbsh" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_dvbsh_service_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "dvbt.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "dvbsh" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_dvbsh_track_select(SDRKIT_HANDLE handle, const sdrkit_dvbt_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "dvbt.TrackSelectArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvbsh" are the sdrkit_dvbsh_gen_*() functions
// after the settings of its receiver.

// The setting "width" (Channel width) of the stack "dvbsh". A value of `sdrkit_dvbsh_width`.
static inline int32_t sdrkit_dvbsh_get_width(SDRKIT_HANDLE handle, sdrkit_dvbsh_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_width)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_set_width(SDRKIT_HANDLE handle, sdrkit_dvbsh_width value) {
  return sdrkit_typed_setting_set_enum(handle, "width", (int32_t)value);
}

// The setting "low_priority" (Low-priority stream) of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_get_low_priority(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "low_priority", value);
}
static inline int32_t sdrkit_dvbsh_set_low_priority(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "low_priority", value);
}

// The setting "max_interleaver_mb" (Time de-interleaver memory limit) of the stack "dvbsh". Unit: MB.
static inline int32_t sdrkit_dvbsh_get_max_interleaver_mb(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_interleaver_mb", value);
}
static inline int32_t sdrkit_dvbsh_set_max_interleaver_mb(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_interleaver_mb", value);
}

// The setting "turbo_iterations" (Turbo iterations) of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_get_turbo_iterations(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "turbo_iterations", value);
}
static inline int32_t sdrkit_dvbsh_set_turbo_iterations(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "turbo_iterations", value);
}

// The setting "decode_video" (Decode video) of the stack "dvbsh". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvbsh_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvbsh_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "waveform" (Waveform) of the stack "dvbsh". A value of `sdrkit_dvbsh_waveform`.
static inline int32_t sdrkit_dvbsh_get_waveform(SDRKIT_HANDLE handle, sdrkit_dvbsh_waveform *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "waveform", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_waveform)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_set_waveform(SDRKIT_HANDLE handle, sdrkit_dvbsh_waveform value) {
  return sdrkit_typed_setting_set_enum(handle, "waveform", (int32_t)value);
}

// The setting "tdm_gold" (TDM scrambling code number) of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_get_tdm_gold(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tdm_gold", value);
}
static inline int32_t sdrkit_dvbsh_set_tdm_gold(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tdm_gold", value);
}

// The setting "tdm_symbol_rate" (TDM symbol rate) of the stack "dvbsh". Unit: sym/s.
static inline int32_t sdrkit_dvbsh_get_tdm_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tdm_symbol_rate", value);
}
static inline int32_t sdrkit_dvbsh_set_tdm_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tdm_symbol_rate", value);
}

// The setting "gen_width" (Test signal width) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_width`.
static inline int32_t sdrkit_dvbsh_gen_get_width(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_width)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_width(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_width value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_width", (int32_t)value);
}

// The setting "gen_mode" (Test signal FFT size) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_mode`.
static inline int32_t sdrkit_dvbsh_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_constellation" (Test signal constellation) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_constellation`.
static inline int32_t sdrkit_dvbsh_gen_get_constellation(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_constellation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_constellation)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_constellation(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_constellation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_constellation", (int32_t)value);
}

// The setting "gen_hierarchy" (Test signal hierarchy) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_hierarchy`.
static inline int32_t sdrkit_dvbsh_gen_get_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_hierarchy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_hierarchy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_hierarchy)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_hierarchy(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_hierarchy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_hierarchy", (int32_t)value);
}

// The setting "gen_guard" (Test signal guard interval) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_guard`.
static inline int32_t sdrkit_dvbsh_gen_get_guard(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_guard)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_guard(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_guard", (int32_t)value);
}

// The setting "gen_rate" (Test signal code rate) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_rate`.
static inline int32_t sdrkit_dvbsh_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// The setting "gen_rate_lp" (Test signal LP code rate) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_rate_lp`.
static inline int32_t sdrkit_dvbsh_gen_get_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_rate_lp *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate_lp", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_rate_lp)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_rate_lp(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_rate_lp value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate_lp", (int32_t)value);
}

// The setting "gen_ti_cm" (Test signal common_multiplier) of the test signal generator of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_gen_get_ti_cm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ti_cm", value);
}
static inline int32_t sdrkit_dvbsh_gen_set_ti_cm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ti_cm", value);
}

// The setting "gen_ti_lt" (Test signal nof_late_taps) of the test signal generator of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_gen_get_ti_lt(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ti_lt", value);
}
static inline int32_t sdrkit_dvbsh_gen_set_ti_lt(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ti_lt", value);
}

// The setting "gen_ti_ns" (Test signal nof_slices) of the test signal generator of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_gen_get_ti_ns(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ti_ns", value);
}
static inline int32_t sdrkit_dvbsh_gen_set_ti_ns(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ti_ns", value);
}

// The setting "gen_ti_sd" (Test signal slice_distance) of the test signal generator of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_gen_get_ti_sd(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ti_sd", value);
}
static inline int32_t sdrkit_dvbsh_gen_set_ti_sd(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ti_sd", value);
}

// The setting "gen_ti_nli" (Test signal non_late_increment) of the test signal generator of the stack "dvbsh".
static inline int32_t sdrkit_dvbsh_gen_get_ti_nli(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ti_nli", value);
}
static inline int32_t sdrkit_dvbsh_gen_set_ti_nli(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ti_nli", value);
}

// The setting "gen_waveform" (Test signal waveform) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_waveform`.
static inline int32_t sdrkit_dvbsh_gen_get_waveform(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_waveform *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_waveform", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_waveform)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_waveform(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_waveform value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_waveform", (int32_t)value);
}

// The setting "gen_rolloff" (Test signal TDM roll-off) of the test signal generator of the stack "dvbsh". A value of `sdrkit_dvbsh_gen_rolloff`.
static inline int32_t sdrkit_dvbsh_gen_get_rolloff(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_rolloff *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rolloff", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbsh_gen_rolloff)index;
  return code;
}
static inline int32_t sdrkit_dvbsh_gen_set_rolloff(SDRKIT_HANDLE handle, sdrkit_dvbsh_gen_rolloff value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rolloff", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbt_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.DvbtFields", fields, sizeof(sdrkit_dvbt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbt_multiplex_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_multiplex_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.DvbtMultiplexFields", fields, sizeof(sdrkit_dvbt_multiplex_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbt_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.DvbtTxFields", fields, sizeof(sdrkit_dvbt_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvb_terrestrial_fields_read(SDRKIT_HANDLE handle, sdrkit_dvb_terrestrial_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.DvbTerrestrialFields", fields, sizeof(sdrkit_dvb_terrestrial_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbsh_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbsh_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.DvbshFields", fields, sizeof(sdrkit_dvbsh_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbsh_chain_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbsh_chain_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.DvbshChainFields", fields, sizeof(sdrkit_dvbsh_chain_fields));
}

// Fills `fields` with the result fields of the component "sync" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "sync.<key>".
static inline int32_t sdrkit_dvbt_sync_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_sync_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.SyncFields@sync", fields, sizeof(sdrkit_dvbt_sync_fields));
}

// Fills `fields` with the result fields of the component "fft" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "fft.<key>".
static inline int32_t sdrkit_dvbt_fft_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_fft_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.FftFields@fft", fields, sizeof(sdrkit_dvbt_fft_fields));
}

// Fills `fields` with the result fields of the component "tps" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "tps.<key>".
static inline int32_t sdrkit_dvbt_tps_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_tps_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.TpsFields@tps", fields, sizeof(sdrkit_dvbt_tps_fields));
}

// Fills `fields` with the result fields of the component "eq" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "eq.<key>".
static inline int32_t sdrkit_dvbt_eq_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_eq_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.EqFields@eq", fields, sizeof(sdrkit_dvbt_eq_fields));
}

// Fills `fields` with the result fields of the component "fec" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "fec.<key>".
static inline int32_t sdrkit_dvbt_fec_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_fec_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.FecFields@fec", fields, sizeof(sdrkit_dvbt_fec_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvbt_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvbt_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_dvbt_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "dvbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_dvbt_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

// Fills `fields` with the result fields of the component "sync" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "sync.<key>".
static inline int32_t sdrkit_dvb_terrestrial_sync_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_sync_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.SyncFields@sync", fields, sizeof(sdrkit_dvbt_sync_fields));
}

// Fills `fields` with the result fields of the component "fft" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "fft.<key>".
static inline int32_t sdrkit_dvb_terrestrial_fft_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_fft_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.FftFields@fft", fields, sizeof(sdrkit_dvbt_fft_fields));
}

// Fills `fields` with the result fields of the component "tps" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "tps.<key>".
static inline int32_t sdrkit_dvb_terrestrial_tps_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_tps_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.TpsFields@tps", fields, sizeof(sdrkit_dvbt_tps_fields));
}

// Fills `fields` with the result fields of the component "eq" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "eq.<key>".
static inline int32_t sdrkit_dvb_terrestrial_eq_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_eq_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.EqFields@eq", fields, sizeof(sdrkit_dvbt_eq_fields));
}

// Fills `fields` with the result fields of the component "fec" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "fec.<key>".
static inline int32_t sdrkit_dvb_terrestrial_fec_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_fec_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.FecFields@fec", fields, sizeof(sdrkit_dvbt_fec_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvb_terrestrial_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvb_terrestrial_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_dvb_terrestrial_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "dvb_terrestrial" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_dvb_terrestrial_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

// Fills `fields` with the result fields of the component "sync" of the stack "dvbsh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "sync.<key>".
static inline int32_t sdrkit_dvbsh_sync_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_sync_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.SyncFields@sync", fields, sizeof(sdrkit_dvbt_sync_fields));
}

// Fills `fields` with the result fields of the component "fft" of the stack "dvbsh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "fft.<key>".
static inline int32_t sdrkit_dvbsh_fft_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_fft_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.FftFields@fft", fields, sizeof(sdrkit_dvbt_fft_fields));
}

// Fills `fields` with the result fields of the component "tps" of the stack "dvbsh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "tps.<key>".
static inline int32_t sdrkit_dvbsh_tps_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_tps_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.TpsFields@tps", fields, sizeof(sdrkit_dvbt_tps_fields));
}

// Fills `fields` with the result fields of the component "eq" of the stack "dvbsh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "eq.<key>".
static inline int32_t sdrkit_dvbsh_eq_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt_eq_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt.EqFields@eq", fields, sizeof(sdrkit_dvbt_eq_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvbsh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvbsh_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvbsh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvbsh_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

#endif
