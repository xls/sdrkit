/*
 * sdrkit: the typed interface of the stack family "wsjt", version 4.0. MIT licence.
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

#ifndef SDRKIT_WSJT_H
#define SDRKIT_WSJT_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_WSJT_VERSION_MAJOR 4
#define SDRKIT_WSJT_VERSION_MINOR 0

// The stack "wsjt": its id, and the keys of its event types.
#define SDRKIT_STACK_WSJT "wsjt"
#define SDRKIT_EVENT_WSJT_DECODE "wsjt.decode"
#define SDRKIT_EVENT_WSJT_SPOT "wsjt.spot"
#define SDRKIT_EVENT_JS8_FRAME "js8.frame"
#define SDRKIT_EVENT_JS8_MESSAGE "js8.message"
#define SDRKIT_EVENT_WSJT_TX "wsjt.tx"

// The stack "ft8": its id, and the keys of its event types.
#define SDRKIT_STACK_FT8 "ft8"

// The stack "ft4": its id, and the keys of its event types.
#define SDRKIT_STACK_FT4 "ft4"

// The stack "ft2": its id, and the keys of its event types.
#define SDRKIT_STACK_FT2 "ft2"

// The stack "wspr": its id, and the keys of its event types.
#define SDRKIT_STACK_WSPR "wspr"

// The stack "jt65": its id, and the keys of its event types.
#define SDRKIT_STACK_JT65 "jt65"

// The stack "jt9": its id, and the keys of its event types.
#define SDRKIT_STACK_JT9 "jt9"

// The stack "jt4": its id, and the keys of its event types.
#define SDRKIT_STACK_JT4 "jt4"

// The stack "q65": its id, and the keys of its event types.
#define SDRKIT_STACK_Q65 "q65"

// The stack "fst4": its id, and the keys of its event types.
#define SDRKIT_STACK_FST4 "fst4"

// The stack "fst4w": its id, and the keys of its event types.
#define SDRKIT_STACK_FST4W "fst4w"

// The stack "msk144": its id, and the keys of its event types.
#define SDRKIT_STACK_MSK144 "msk144"

// The stack "js8": its id, and the keys of its event types.
#define SDRKIT_STACK_JS8 "js8"

// The stack "fsk441": its id, and the keys of its event types.
#define SDRKIT_STACK_FSK441 "fsk441"

// The stack "jt6m": its id, and the keys of its event types.
#define SDRKIT_STACK_JT6M "jt6m"

// The stack "iscat": its id, and the keys of its event types.
#define SDRKIT_STACK_ISCAT "iscat"

// The stack "pi4": its id, and the keys of its event types.
#define SDRKIT_STACK_PI4 "pi4"

// The options of the setting "jt65_submode" of the stack "wsjt".
typedef enum sdrkit_wsjt_jt65_submode {
  // A (2.7 Hz)
  SDRKIT_WSJT_JT65_SUBMODE_A = 0,
  // B (5.4 Hz)
  SDRKIT_WSJT_JT65_SUBMODE_B = 1,
  // C (10.8 Hz)
  SDRKIT_WSJT_JT65_SUBMODE_C = 2,
} sdrkit_wsjt_jt65_submode;

// The options of the setting "jt4_submode" of the stack "wsjt".
typedef enum sdrkit_wsjt_jt4_submode {
  // A (4.4 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_A = 0,
  // B (8.75 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_B = 1,
  // C (17.5 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_C = 2,
  // D (35 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_D = 3,
  // E (70 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_E = 4,
  // F (140 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_F = 5,
  // G (280 Hz)
  SDRKIT_WSJT_JT4_SUBMODE_G = 6,
} sdrkit_wsjt_jt4_submode;

// The options of the setting "q65_submode" of the stack "wsjt".
typedef enum sdrkit_wsjt_q65_submode {
  // A
  SDRKIT_WSJT_Q65_SUBMODE_A = 0,
  // B
  SDRKIT_WSJT_Q65_SUBMODE_B = 1,
  // C
  SDRKIT_WSJT_Q65_SUBMODE_C = 2,
  // D
  SDRKIT_WSJT_Q65_SUBMODE_D = 3,
  // E
  SDRKIT_WSJT_Q65_SUBMODE_E = 4,
} sdrkit_wsjt_q65_submode;

// The options of the setting "demod" of the stack "wsjt".
typedef enum sdrkit_wsjt_demod {
  // Auto
  SDRKIT_WSJT_DEMOD_AUTO = 0,
  // USB
  SDRKIT_WSJT_DEMOD_USB = 1,
  // LSB
  SDRKIT_WSJT_DEMOD_LSB = 2,
  // NFM
  SDRKIT_WSJT_DEMOD_NFM = 3,
  // AM
  SDRKIT_WSJT_DEMOD_AM = 4,
} sdrkit_wsjt_demod;

// The options of the setting "time_source" of the stack "wsjt".
typedef enum sdrkit_wsjt_time_source {
  // System clock
  SDRKIT_WSJT_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_WSJT_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_WSJT_TIME_SOURCE_SEARCH = 2,
} sdrkit_wsjt_time_source;

// The options of the setting "qso_mode" of the stack "wsjt".
typedef enum sdrkit_wsjt_qso_mode {
  // FT8
  SDRKIT_WSJT_QSO_MODE_FT8 = 0,
  // FT4
  SDRKIT_WSJT_QSO_MODE_FT4 = 1,
  // FT2
  SDRKIT_WSJT_QSO_MODE_FT2 = 2,
  // JT65
  SDRKIT_WSJT_QSO_MODE_JT65 = 3,
  // JT9
  SDRKIT_WSJT_QSO_MODE_JT9 = 4,
  // JT4
  SDRKIT_WSJT_QSO_MODE_JT4 = 5,
} sdrkit_wsjt_qso_mode;

// The options of the setting "js8_speed" of the stack "wsjt".
typedef enum sdrkit_wsjt_js8_speed {
  // Normal
  SDRKIT_WSJT_JS8_SPEED_NORMAL = 0,
  // Fast
  SDRKIT_WSJT_JS8_SPEED_FAST = 1,
  // Turbo
  SDRKIT_WSJT_JS8_SPEED_TURBO = 2,
} sdrkit_wsjt_js8_speed;

// The options of the setting "demod" of the stack "ft8".
typedef enum sdrkit_ft8_demod {
  // Auto
  SDRKIT_FT8_DEMOD_AUTO = 0,
  // USB
  SDRKIT_FT8_DEMOD_USB = 1,
  // LSB
  SDRKIT_FT8_DEMOD_LSB = 2,
  // NFM
  SDRKIT_FT8_DEMOD_NFM = 3,
  // AM
  SDRKIT_FT8_DEMOD_AM = 4,
} sdrkit_ft8_demod;

// The options of the setting "time_source" of the stack "ft8".
typedef enum sdrkit_ft8_time_source {
  // System clock
  SDRKIT_FT8_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_FT8_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_FT8_TIME_SOURCE_SEARCH = 2,
} sdrkit_ft8_time_source;

// The options of the setting "demod" of the stack "ft4".
typedef enum sdrkit_ft4_demod {
  // Auto
  SDRKIT_FT4_DEMOD_AUTO = 0,
  // USB
  SDRKIT_FT4_DEMOD_USB = 1,
  // LSB
  SDRKIT_FT4_DEMOD_LSB = 2,
  // NFM
  SDRKIT_FT4_DEMOD_NFM = 3,
  // AM
  SDRKIT_FT4_DEMOD_AM = 4,
} sdrkit_ft4_demod;

// The options of the setting "time_source" of the stack "ft4".
typedef enum sdrkit_ft4_time_source {
  // System clock
  SDRKIT_FT4_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_FT4_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_FT4_TIME_SOURCE_SEARCH = 2,
} sdrkit_ft4_time_source;

// The options of the setting "demod" of the stack "ft2".
typedef enum sdrkit_ft2_demod {
  // Auto
  SDRKIT_FT2_DEMOD_AUTO = 0,
  // USB
  SDRKIT_FT2_DEMOD_USB = 1,
  // LSB
  SDRKIT_FT2_DEMOD_LSB = 2,
  // NFM
  SDRKIT_FT2_DEMOD_NFM = 3,
  // AM
  SDRKIT_FT2_DEMOD_AM = 4,
} sdrkit_ft2_demod;

// The options of the setting "time_source" of the stack "ft2".
typedef enum sdrkit_ft2_time_source {
  // System clock
  SDRKIT_FT2_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_FT2_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_FT2_TIME_SOURCE_SEARCH = 2,
} sdrkit_ft2_time_source;

// The options of the setting "demod" of the stack "wspr".
typedef enum sdrkit_wspr_demod {
  // Auto
  SDRKIT_WSPR_DEMOD_AUTO = 0,
  // USB
  SDRKIT_WSPR_DEMOD_USB = 1,
  // LSB
  SDRKIT_WSPR_DEMOD_LSB = 2,
  // NFM
  SDRKIT_WSPR_DEMOD_NFM = 3,
  // AM
  SDRKIT_WSPR_DEMOD_AM = 4,
} sdrkit_wspr_demod;

// The options of the setting "time_source" of the stack "wspr".
typedef enum sdrkit_wspr_time_source {
  // System clock
  SDRKIT_WSPR_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_WSPR_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_WSPR_TIME_SOURCE_SEARCH = 2,
} sdrkit_wspr_time_source;

// The options of the setting "jt65_submode" of the stack "jt65".
typedef enum sdrkit_jt65_submode {
  // A (2.7 Hz)
  SDRKIT_JT65_SUBMODE_A = 0,
  // B (5.4 Hz)
  SDRKIT_JT65_SUBMODE_B = 1,
  // C (10.8 Hz)
  SDRKIT_JT65_SUBMODE_C = 2,
} sdrkit_jt65_submode;

// The options of the setting "demod" of the stack "jt65".
typedef enum sdrkit_jt65_demod {
  // Auto
  SDRKIT_JT65_DEMOD_AUTO = 0,
  // USB
  SDRKIT_JT65_DEMOD_USB = 1,
  // LSB
  SDRKIT_JT65_DEMOD_LSB = 2,
  // NFM
  SDRKIT_JT65_DEMOD_NFM = 3,
  // AM
  SDRKIT_JT65_DEMOD_AM = 4,
} sdrkit_jt65_demod;

// The options of the setting "time_source" of the stack "jt65".
typedef enum sdrkit_jt65_time_source {
  // System clock
  SDRKIT_JT65_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_JT65_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_JT65_TIME_SOURCE_SEARCH = 2,
} sdrkit_jt65_time_source;

// The options of the setting "demod" of the stack "jt9".
typedef enum sdrkit_jt9_demod {
  // Auto
  SDRKIT_JT9_DEMOD_AUTO = 0,
  // USB
  SDRKIT_JT9_DEMOD_USB = 1,
  // LSB
  SDRKIT_JT9_DEMOD_LSB = 2,
  // NFM
  SDRKIT_JT9_DEMOD_NFM = 3,
  // AM
  SDRKIT_JT9_DEMOD_AM = 4,
} sdrkit_jt9_demod;

// The options of the setting "time_source" of the stack "jt9".
typedef enum sdrkit_jt9_time_source {
  // System clock
  SDRKIT_JT9_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_JT9_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_JT9_TIME_SOURCE_SEARCH = 2,
} sdrkit_jt9_time_source;

// The options of the setting "jt4_submode" of the stack "jt4".
typedef enum sdrkit_jt4_submode {
  // A (4.4 Hz)
  SDRKIT_JT4_SUBMODE_A = 0,
  // B (8.75 Hz)
  SDRKIT_JT4_SUBMODE_B = 1,
  // C (17.5 Hz)
  SDRKIT_JT4_SUBMODE_C = 2,
  // D (35 Hz)
  SDRKIT_JT4_SUBMODE_D = 3,
  // E (70 Hz)
  SDRKIT_JT4_SUBMODE_E = 4,
  // F (140 Hz)
  SDRKIT_JT4_SUBMODE_F = 5,
  // G (280 Hz)
  SDRKIT_JT4_SUBMODE_G = 6,
} sdrkit_jt4_submode;

// The options of the setting "demod" of the stack "jt4".
typedef enum sdrkit_jt4_demod {
  // Auto
  SDRKIT_JT4_DEMOD_AUTO = 0,
  // USB
  SDRKIT_JT4_DEMOD_USB = 1,
  // LSB
  SDRKIT_JT4_DEMOD_LSB = 2,
  // NFM
  SDRKIT_JT4_DEMOD_NFM = 3,
  // AM
  SDRKIT_JT4_DEMOD_AM = 4,
} sdrkit_jt4_demod;

// The options of the setting "time_source" of the stack "jt4".
typedef enum sdrkit_jt4_time_source {
  // System clock
  SDRKIT_JT4_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_JT4_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_JT4_TIME_SOURCE_SEARCH = 2,
} sdrkit_jt4_time_source;

// The options of the setting "q65_submode" of the stack "q65".
typedef enum sdrkit_q65_submode {
  // A
  SDRKIT_Q65_SUBMODE_A = 0,
  // B
  SDRKIT_Q65_SUBMODE_B = 1,
  // C
  SDRKIT_Q65_SUBMODE_C = 2,
  // D
  SDRKIT_Q65_SUBMODE_D = 3,
  // E
  SDRKIT_Q65_SUBMODE_E = 4,
} sdrkit_q65_submode;

// The options of the setting "demod" of the stack "q65".
typedef enum sdrkit_q65_demod {
  // Auto
  SDRKIT_Q65_DEMOD_AUTO = 0,
  // USB
  SDRKIT_Q65_DEMOD_USB = 1,
  // LSB
  SDRKIT_Q65_DEMOD_LSB = 2,
  // NFM
  SDRKIT_Q65_DEMOD_NFM = 3,
  // AM
  SDRKIT_Q65_DEMOD_AM = 4,
} sdrkit_q65_demod;

// The options of the setting "time_source" of the stack "q65".
typedef enum sdrkit_q65_time_source {
  // System clock
  SDRKIT_Q65_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_Q65_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_Q65_TIME_SOURCE_SEARCH = 2,
} sdrkit_q65_time_source;

// The options of the setting "demod" of the stack "fst4".
typedef enum sdrkit_fst4_demod {
  // Auto
  SDRKIT_FST4_DEMOD_AUTO = 0,
  // USB
  SDRKIT_FST4_DEMOD_USB = 1,
  // LSB
  SDRKIT_FST4_DEMOD_LSB = 2,
  // NFM
  SDRKIT_FST4_DEMOD_NFM = 3,
  // AM
  SDRKIT_FST4_DEMOD_AM = 4,
} sdrkit_fst4_demod;

// The options of the setting "time_source" of the stack "fst4".
typedef enum sdrkit_fst4_time_source {
  // System clock
  SDRKIT_FST4_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_FST4_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_FST4_TIME_SOURCE_SEARCH = 2,
} sdrkit_fst4_time_source;

// The options of the setting "demod" of the stack "fst4w".
typedef enum sdrkit_fst4w_demod {
  // Auto
  SDRKIT_FST4W_DEMOD_AUTO = 0,
  // USB
  SDRKIT_FST4W_DEMOD_USB = 1,
  // LSB
  SDRKIT_FST4W_DEMOD_LSB = 2,
  // NFM
  SDRKIT_FST4W_DEMOD_NFM = 3,
  // AM
  SDRKIT_FST4W_DEMOD_AM = 4,
} sdrkit_fst4w_demod;

// The options of the setting "time_source" of the stack "fst4w".
typedef enum sdrkit_fst4w_time_source {
  // System clock
  SDRKIT_FST4W_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_FST4W_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_FST4W_TIME_SOURCE_SEARCH = 2,
} sdrkit_fst4w_time_source;

// The options of the setting "demod" of the stack "msk144".
typedef enum sdrkit_msk144_demod {
  // Auto
  SDRKIT_MSK144_DEMOD_AUTO = 0,
  // USB
  SDRKIT_MSK144_DEMOD_USB = 1,
  // LSB
  SDRKIT_MSK144_DEMOD_LSB = 2,
  // NFM
  SDRKIT_MSK144_DEMOD_NFM = 3,
  // AM
  SDRKIT_MSK144_DEMOD_AM = 4,
} sdrkit_msk144_demod;

// The options of the setting "time_source" of the stack "msk144".
typedef enum sdrkit_msk144_time_source {
  // System clock
  SDRKIT_MSK144_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_MSK144_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_MSK144_TIME_SOURCE_SEARCH = 2,
} sdrkit_msk144_time_source;

// The options of the setting "demod" of the stack "js8".
typedef enum sdrkit_js8_demod {
  // Auto
  SDRKIT_JS8_DEMOD_AUTO = 0,
  // USB
  SDRKIT_JS8_DEMOD_USB = 1,
  // LSB
  SDRKIT_JS8_DEMOD_LSB = 2,
  // NFM
  SDRKIT_JS8_DEMOD_NFM = 3,
  // AM
  SDRKIT_JS8_DEMOD_AM = 4,
} sdrkit_js8_demod;

// The options of the setting "time_source" of the stack "js8".
typedef enum sdrkit_js8_time_source {
  // System clock
  SDRKIT_JS8_TIME_SOURCE_SYSTEM = 0,
  // Recording start
  SDRKIT_JS8_TIME_SOURCE_START = 1,
  // Slot search
  SDRKIT_JS8_TIME_SOURCE_SEARCH = 2,
} sdrkit_js8_time_source;

// The options of the setting "js8_speed" of the stack "js8".
typedef enum sdrkit_js8_speed {
  // Normal
  SDRKIT_JS8_SPEED_NORMAL = 0,
  // Fast
  SDRKIT_JS8_SPEED_FAST = 1,
  // Turbo
  SDRKIT_JS8_SPEED_TURBO = 2,
} sdrkit_js8_speed;

// The options of the setting "demod" of the stack "fsk441".
typedef enum sdrkit_fsk441_demod {
  // Auto
  SDRKIT_FSK441_DEMOD_AUTO = 0,
  // USB
  SDRKIT_FSK441_DEMOD_USB = 1,
  // LSB
  SDRKIT_FSK441_DEMOD_LSB = 2,
  // NFM
  SDRKIT_FSK441_DEMOD_NFM = 3,
  // AM
  SDRKIT_FSK441_DEMOD_AM = 4,
} sdrkit_fsk441_demod;

// The options of the setting "demod" of the stack "jt6m".
typedef enum sdrkit_jt6m_demod {
  // Auto
  SDRKIT_JT6M_DEMOD_AUTO = 0,
  // USB
  SDRKIT_JT6M_DEMOD_USB = 1,
  // LSB
  SDRKIT_JT6M_DEMOD_LSB = 2,
  // NFM
  SDRKIT_JT6M_DEMOD_NFM = 3,
  // AM
  SDRKIT_JT6M_DEMOD_AM = 4,
} sdrkit_jt6m_demod;

// The options of the setting "submode" of the stack "iscat".
typedef enum sdrkit_iscat_submode {
  // Automatic
  SDRKIT_ISCAT_SUBMODE_AUTO = 0,
  // ISCAT-A
  SDRKIT_ISCAT_SUBMODE_A = 1,
  // ISCAT-B
  SDRKIT_ISCAT_SUBMODE_B = 2,
} sdrkit_iscat_submode;

// The options of the setting "demod" of the stack "iscat".
typedef enum sdrkit_iscat_demod {
  // Auto
  SDRKIT_ISCAT_DEMOD_AUTO = 0,
  // USB
  SDRKIT_ISCAT_DEMOD_USB = 1,
  // LSB
  SDRKIT_ISCAT_DEMOD_LSB = 2,
  // NFM
  SDRKIT_ISCAT_DEMOD_NFM = 3,
  // AM
  SDRKIT_ISCAT_DEMOD_AM = 4,
} sdrkit_iscat_demod;

// The options of the setting "gen_variant" of the stack "iscat".
typedef enum sdrkit_iscat_gen_variant {
  // ISCAT-A
  SDRKIT_ISCAT_GEN_VARIANT_A = 0,
  // ISCAT-B
  SDRKIT_ISCAT_GEN_VARIANT_B = 1,
} sdrkit_iscat_gen_variant;

// The options of the setting "k" of the stack "pi4".
typedef enum sdrkit_pi4_k {
  // PI4 (K = 40)
  SDRKIT_PI4_K_PI4 = 0,
  // PI4-80 (K = 80)
  SDRKIT_PI4_K_PI4_80 = 1,
  // PI4-96 (K = 96)
  SDRKIT_PI4_K_PI4_96 = 2,
  // PI4-120 (K = 120)
  SDRKIT_PI4_K_PI4_120 = 3,
} sdrkit_pi4_k;

// The options of the setting "demod" of the stack "pi4".
typedef enum sdrkit_pi4_demod {
  // Auto
  SDRKIT_PI4_DEMOD_AUTO = 0,
  // USB
  SDRKIT_PI4_DEMOD_USB = 1,
  // LSB
  SDRKIT_PI4_DEMOD_LSB = 2,
  // NFM
  SDRKIT_PI4_DEMOD_NFM = 3,
  // AM
  SDRKIT_PI4_DEMOD_AM = 4,
} sdrkit_pi4_demod;

// The options of the setting "gen_k" of the stack "pi4".
typedef enum sdrkit_pi4_gen_k {
  // PI4 (K = 40)
  SDRKIT_PI4_GEN_K_PI4 = 0,
  // PI4-80 (K = 80)
  SDRKIT_PI4_GEN_K_PI4_80 = 1,
  // PI4-96 (K = 96)
  SDRKIT_PI4_GEN_K_PI4_96 = 2,
  // PI4-120 (K = 120)
  SDRKIT_PI4_GEN_K_PI4_120 = 3,
} sdrkit_pi4_gen_k;

// The options of `time_source` of `sdrkit_wsjt_fields`.
typedef enum sdrkit_wsjt_fields_time_source {
  // System clock
  SDRKIT_WSJT_FIELDS_TIME_SOURCE_SYSTEM_CLOCK = 0,
  // Recording start
  SDRKIT_WSJT_FIELDS_TIME_SOURCE_RECORDING_START = 1,
  // Searching the slots
  SDRKIT_WSJT_FIELDS_TIME_SOURCE_SEARCHING = 2,
  // Slots found from the signals
  SDRKIT_WSJT_FIELDS_TIME_SOURCE_FOUND_FROM_SIGNALS = 3,
} sdrkit_wsjt_fields_time_source;

// The options of `js8_state` of `sdrkit_wsjt_fields`.
typedef enum sdrkit_wsjt_fields_js8_state {
  // Listening
  SDRKIT_WSJT_FIELDS_JS8_STATE_LISTENING = 0,
  // Slots decoded
  SDRKIT_WSJT_FIELDS_JS8_STATE_SLOTS = 1,
  // Frames
  SDRKIT_WSJT_FIELDS_JS8_STATE_FRAMES = 2,
} sdrkit_wsjt_fields_js8_state;

// The result fields of a slot stack: the slot clock, the clock estimate from the decodes, and the counts since Clear.
typedef struct sdrkit_wsjt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `time_source` holds a value.
  bool has_time_source;
  // The time source of the slot clock. It has no value before the clock reports. A value of `sdrkit_wsjt_fields_time_source`.
  int32_t time_source;
  // The UTC of the newest audio. Seconds since 1970 (UTC).
  double utc;
  // True if `clock_error_s` holds a value.
  bool has_clock_error_s;
  // The median DT of the recent decodes. It has no value without a recent decode. Unit: s.
  double clock_error_s;
  // True if `clock_spread_s` holds a value.
  bool has_clock_spread_s;
  // The spread of the recent DTs. It has no value without a recent decode. Unit: s.
  double clock_spread_s;
  // The decodes that the clock estimate uses.
  int64_t clock_decodes;
  // The clock is more than 1 s off.
  bool clock_warning;
  // Decodes since Clear.
  int64_t decodes_total;
  // JS8 frames since Clear.
  int64_t js8_frames;
  // JS8 candidates that did not decode since Clear.
  int64_t js8_failed;
  // What JS8 has heard. A value of `sdrkit_wsjt_fields_js8_state`.
  int32_t js8_state;
  // True if `js8_waterfall_seq` holds a value.
  bool has_js8_waterfall_seq;
  // The sequence number of the newest line of the JS8 waterfall. It has no value before the first line.
  int64_t js8_waterfall_seq;
} sdrkit_wsjt_fields;

// The counts of the decoder of one mode (`dec_<mode>`).
typedef struct sdrkit_wsjt_slot_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The slots decoded.
  int64_t slots;
  // The decodes.
  int64_t decodes;
  // The time that the last slot took to decode. Unit: ms.
  double decode_ms;
} sdrkit_wsjt_slot_decoder_fields;

// The result fields of whole-span decoding and the multi-band monitor (`wide`).
typedef struct sdrkit_wsjt_wide_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The dials decoded.
  int64_t channels;
  // The share of real time that decoding takes. Unit: %.
  double load_pct;
  // Decoding does not keep up.
  bool too_slow;
} sdrkit_wsjt_wide_fields;

// The options of `how` of `sdrkit_wsjt_decode_row`.
typedef enum sdrkit_wsjt_decode_row_how {
  // This slot
  SDRKIT_WSJT_DECODE_ROW_HOW_ALONE = 0,
  // (combined)
  SDRKIT_WSJT_DECODE_ROW_HOW_COMBINED = 1,
  // (a-priori)
  SDRKIT_WSJT_DECODE_ROW_HOW_A_PRIORI = 2,
} sdrkit_wsjt_decode_row_how;

// A row of the decode table. The row key is the place of the decode in the log.
typedef struct sdrkit_wsjt_decode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the slot. Seconds since 1970 (UTC).
  double utc;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // The time offset from the slot start. Unit: s.
  double dt_s;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // True if `band` holds a value.
  bool has_band;
  // The amateur band of the dial, if the dial is in one.
  sdrkit_string band;
  // True if `dial_hz` holds a value.
  bool has_dial_hz;
  // The dial frequency, if the slot has one. A frequency in Hz.
  double dial_hz;
  // The mode with its sub-mode and period ("JT65B").
  sdrkit_string mode;
  // The decoded message.
  sdrkit_string message;
  // The decode was found on the other sideband.
  bool mirrored;
  // How the decode was made. A value of `sdrkit_wsjt_decode_row_how`.
  int32_t how;
  // The decode is from the first part of a slot. The decode of the whole slot replaces it.
  bool early;
} sdrkit_wsjt_decode_row;

// A row of the WSPR and FST4W spots. The row key is the place of the spot in the log.
typedef struct sdrkit_wspr_spot_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the slot. Seconds since 1970 (UTC).
  double utc;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // The time offset from the slot start. Unit: s.
  double dt_s;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The callsign of the beacon.
  sdrkit_string callsign;
  // The locator of the beacon.
  sdrkit_string locator;
  // The power that the beacon states. Unit: dBm.
  int64_t power_dbm;
  // The decoded message.
  sdrkit_string message;
} sdrkit_wspr_spot_row;

// The options of `state` of `sdrkit_wsjt_mode_row`.
typedef enum sdrkit_wsjt_mode_row_state {
  // Decoding
  SDRKIT_WSJT_MODE_ROW_STATE_DECODING = 0,
  // No decoder
  SDRKIT_WSJT_MODE_ROW_STATE_NO_DECODER = 1,
} sdrkit_wsjt_mode_row_state;

// A row of the modes table: the counters and the state of one mode heard, or of one JS8 speed. The row key is the mode id, or `js8n`, `js8f` and `js8t`.
typedef struct sdrkit_wsjt_mode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The mode with its sub-mode and period.
  sdrkit_string mode;
  // The T/R period. A length of time in seconds.
  double period;
  // The slots decoded.
  int64_t slots;
  // Decodes since Clear.
  int64_t decodes;
  // The decodes of the last slot.
  int64_t last_count;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the last slot decoded. It has no value before the first slot. Seconds since 1970 (UTC).
  double utc;
  // The time that the last slot took to decode. Unit: ms.
  double decode_ms;
  // Whether the mode decodes. A value of `sdrkit_wsjt_mode_row_state`.
  int32_t state;
} sdrkit_wsjt_mode_row;

// The options of `mode` of `sdrkit_wsjt_station_row`.
typedef enum sdrkit_wsjt_station_row_mode {
  // FT8
  SDRKIT_WSJT_STATION_ROW_MODE_FT8 = 0,
  // FT4
  SDRKIT_WSJT_STATION_ROW_MODE_FT4 = 1,
  // FT2
  SDRKIT_WSJT_STATION_ROW_MODE_FT2 = 2,
  // WSPR
  SDRKIT_WSJT_STATION_ROW_MODE_WSPR = 3,
  // JT65
  SDRKIT_WSJT_STATION_ROW_MODE_JT65 = 4,
  // JT9
  SDRKIT_WSJT_STATION_ROW_MODE_JT9 = 5,
  // JT4
  SDRKIT_WSJT_STATION_ROW_MODE_JT4 = 6,
  // Q65
  SDRKIT_WSJT_STATION_ROW_MODE_Q65 = 7,
  // FST4
  SDRKIT_WSJT_STATION_ROW_MODE_FST4 = 8,
  // FST4W
  SDRKIT_WSJT_STATION_ROW_MODE_FST4W = 9,
  // MSK144
  SDRKIT_WSJT_STATION_ROW_MODE_MSK144 = 10,
  // JS8
  SDRKIT_WSJT_STATION_ROW_MODE_JS8 = 11,
} sdrkit_wsjt_station_row_mode;

// A row of the station table: a station heard in any mode. The row key is the callsign.
typedef struct sdrkit_wsjt_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign.
  sdrkit_string callsign;
  // True if `locator` holds a value.
  bool has_locator;
  // The locator, if one was heard.
  sdrkit_string locator;
  // The mode of the newest decode. A value of `sdrkit_wsjt_station_row_mode`.
  int32_t mode;
  // The SNR of the newest decode. Unit: dB.
  double snr_db;
  // The radio frequency of the newest decode. A frequency in Hz.
  double freq_hz;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last decode. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_wsjt_station_row;

// The options of `submode` of `sdrkit_js8_frame_row`.
typedef enum sdrkit_js8_frame_row_submode {
  // Normal
  SDRKIT_JS8_FRAME_ROW_SUBMODE_NORMAL = 0,
  // Fast
  SDRKIT_JS8_FRAME_ROW_SUBMODE_FAST = 1,
  // Turbo
  SDRKIT_JS8_FRAME_ROW_SUBMODE_TURBO = 2,
} sdrkit_js8_frame_row_submode;

// A row of the JS8 frames. The row key is the place of the frame in the log.
typedef struct sdrkit_js8_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the slot. Seconds since 1970 (UTC).
  double utc;
  // True if `submode` holds a value.
  bool has_submode;
  // The speed of the frame. A value of `sdrkit_js8_frame_row_submode`.
  int32_t submode;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // The time offset from the slot start. Unit: s.
  double dt_s;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The text of the frame.
  sdrkit_string frame;
} sdrkit_js8_frame_row;

// A row of the JS8 messages. The row key is the place of the message in the log.
typedef struct sdrkit_js8_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double utc;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The message.
  sdrkit_string message;
} sdrkit_js8_message_row;

// A row of the JS8 stations. The row key is the callsign.
typedef struct sdrkit_js8_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign.
  sdrkit_string callsign;
  // True if `locator` holds a value.
  bool has_locator;
  // The locator, if one was heard.
  sdrkit_string locator;
  // The SNR of the newest frame. Unit: dB.
  double snr_db;
  // The audio frequency of the newest frame. A frequency in Hz.
  double audio_hz;
  // The UTC of the newest frame. Seconds since 1970 (UTC).
  double utc;
  // The newest text of the station.
  sdrkit_string last_text;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_js8_station_row;

// The options of `mode` of `sdrkit_wsjt_multiband_row`.
typedef enum sdrkit_wsjt_multiband_row_mode {
  // FT8
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_FT8 = 0,
  // FT4
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_FT4 = 1,
  // FT2
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_FT2 = 2,
  // WSPR
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_WSPR = 3,
  // JT65
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_JT65 = 4,
  // JT9
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_JT9 = 5,
  // JT4
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_JT4 = 6,
  // Q65
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_Q65 = 7,
  // FST4
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_FST4 = 8,
  // FST4W
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_FST4W = 9,
  // MSK144
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_MSK144 = 10,
  // JS8
  SDRKIT_WSJT_MULTIBAND_ROW_MODE_JS8 = 11,
} sdrkit_wsjt_multiband_row_mode;

// A row of the multi-band monitor: one band and mode of the selection. The row key is the band index in two digits and the mode index.
typedef struct sdrkit_wsjt_multiband_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The band.
  sdrkit_string band;
  // The mode. A value of `sdrkit_wsjt_multiband_row_mode`.
  int32_t mode;
  // True if `dial_hz` holds a value.
  bool has_dial_hz;
  // The dial of the mode on the band, if it has one. A frequency in Hz.
  double dial_hz;
  // A decoder runs on the dial.
  bool running;
  // The decodes of the dial.
  int64_t decodes;
  // The capture covers the dial.
  bool covered;
  // True if `span_needed_hz` holds a value.
  bool has_span_needed_hz;
  // The span that the capture needs around its centre to cover the dial. It has no value for a covered dial. A frequency in Hz.
  double span_needed_hz;
} sdrkit_wsjt_multiband_row;

// The result fields of band hopping (`hop`).
typedef struct sdrkit_wsjt_hop_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `band` holds a value.
  bool has_band;
  // The band of the present hop slot. It has no value when no band is planned or hopping is off.
  sdrkit_string band;
  // True if `next_utc` holds a value.
  bool has_next_utc;
  // The UTC of the next band change. It has no value while hopping is off. Seconds since 1970 (UTC).
  double next_utc;
} sdrkit_wsjt_hop_fields;

// The result fields that each of `fsk441`, `jt6m`, `iscat` and `pi4` shows.
typedef struct sdrkit_wsjt_scatter_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The decodes since the last clear.
  int64_t decodes_total;
  // What no public document confirms.
  sdrkit_string note;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level. These stacks do not measure it: it has no value. Unit: dBFS.
  double level_dbfs;
} sdrkit_wsjt_scatter_fields;

// The result fields that `fsk441` adds.
typedef struct sdrkit_fsk441_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The level of the burst finder: the last frame over the noise. Unit: dB.
  double burst_level_db;
  // The bursts found.
  int64_t bursts;
} sdrkit_fsk441_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_wsjt_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_wsjt_decoder_fields;

// A row of the decode table of `fsk441`. The row key is the number of the decode.
typedef struct sdrkit_fsk441_decode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time of the start of the decode. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the start of the decode, from the first sample. Unit: s.
  double time_s;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string message;
  // The contrast of the decode. It is no calibrated SNR. Unit: dB.
  double contrast_db;
  // The length of the burst. Unit: ms.
  double duration_ms;
} sdrkit_fsk441_decode_row;

// The result fields that `jt6m` adds. No field has a value before the first ping.
typedef struct sdrkit_jt6m_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `folded` holds a value.
  bool has_folded;
  // The folded message of the newest window that gave one.
  sdrkit_string folded;
  // True if `fold_period` holds a value.
  bool has_fold_period;
  // The period of the folded message, in characters.
  int64_t fold_period;
  // True if `tuning_hz` holds a value.
  bool has_tuning_hz;
  // The offset of the sync tone of the newest ping. Unit: Hz.
  double tuning_hz;
} sdrkit_jt6m_fields;

// A row of the ping table of `jt6m`. The row key is the number of the ping.
typedef struct sdrkit_jt6m_decode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time of the start of the decode. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the start of the decode, from the first sample. Unit: s.
  double time_s;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string message;
  // The length of the ping. Unit: s.
  double duration_s;
  // The clear frames of the ping.
  int64_t clear_frames;
} sdrkit_jt6m_decode_row;

// The result fields that `iscat` adds.
typedef struct sdrkit_iscat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `variant` holds a value.
  bool has_variant;
  // The variant of the newest decode. No value before the first decode.
  sdrkit_string variant;
} sdrkit_iscat_fields;

// A row of the decode table of `iscat`. The row key is the number of the decode.
typedef struct sdrkit_iscat_decode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time of the start of the decode. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the start of the decode, from the first sample. Unit: s.
  double time_s;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string message;
  // The variant of the decode.
  sdrkit_string variant;
  // The sync ratio r.
  double sync_ratio;
  // The whole frames used.
  int64_t frames;
  // The characters shown as `?`.
  int64_t unclear;
} sdrkit_iscat_decode_row;

// A row of the decode table of `pi4`. The row key is the number of the decode.
typedef struct sdrkit_pi4_decode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time of the start of the decode. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the start of the decode, from the first sample. Unit: s.
  double time_s;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string message;
  // The start of the decode against the minute. Unit: s.
  double dt_s;
  // The carrier frequency: tone 0 plus half a spacing. Unit: Hz.
  double carrier_hz;
  // The sync value c.
  double sync;
  // The tone spacing factor of the decode.
  int64_t k;
} sdrkit_pi4_decode_row;

// The items that each decoder component of the stack "wsjt" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_wsjt_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_ft8` holds a value.
  bool has_dec_ft8;
  // The items that the component "dec_ft8" dropped.
  int64_t dec_ft8;
  // True if `dec_ft4` holds a value.
  bool has_dec_ft4;
  // The items that the component "dec_ft4" dropped.
  int64_t dec_ft4;
  // True if `dec_ft2` holds a value.
  bool has_dec_ft2;
  // The items that the component "dec_ft2" dropped.
  int64_t dec_ft2;
  // True if `dec_wspr` holds a value.
  bool has_dec_wspr;
  // The items that the component "dec_wspr" dropped.
  int64_t dec_wspr;
  // True if `dec_jt65` holds a value.
  bool has_dec_jt65;
  // The items that the component "dec_jt65" dropped.
  int64_t dec_jt65;
  // True if `dec_jt9` holds a value.
  bool has_dec_jt9;
  // The items that the component "dec_jt9" dropped.
  int64_t dec_jt9;
  // True if `dec_jt4` holds a value.
  bool has_dec_jt4;
  // The items that the component "dec_jt4" dropped.
  int64_t dec_jt4;
  // True if `dec_q65` holds a value.
  bool has_dec_q65;
  // The items that the component "dec_q65" dropped.
  int64_t dec_q65;
  // True if `dec_fst4` holds a value.
  bool has_dec_fst4;
  // The items that the component "dec_fst4" dropped.
  int64_t dec_fst4;
  // True if `dec_fst4w` holds a value.
  bool has_dec_fst4w;
  // The items that the component "dec_fst4w" dropped.
  int64_t dec_fst4w;
  // True if `dec_msk144` holds a value.
  bool has_dec_msk144;
  // The items that the component "dec_msk144" dropped.
  int64_t dec_msk144;
  // True if `dec_js8` holds a value.
  bool has_dec_js8;
  // The items that the component "dec_js8" dropped.
  int64_t dec_js8;
} sdrkit_wsjt_dropped;

// The items that each decoder component of the stack "ft8" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ft8_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_ft8` holds a value.
  bool has_dec_ft8;
  // The items that the component "dec_ft8" dropped.
  int64_t dec_ft8;
} sdrkit_ft8_dropped;

// The items that each decoder component of the stack "ft4" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ft4_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_ft4` holds a value.
  bool has_dec_ft4;
  // The items that the component "dec_ft4" dropped.
  int64_t dec_ft4;
} sdrkit_ft4_dropped;

// The items that each decoder component of the stack "ft2" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ft2_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_ft2` holds a value.
  bool has_dec_ft2;
  // The items that the component "dec_ft2" dropped.
  int64_t dec_ft2;
} sdrkit_ft2_dropped;

// The items that each decoder component of the stack "wspr" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_wspr_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_wspr` holds a value.
  bool has_dec_wspr;
  // The items that the component "dec_wspr" dropped.
  int64_t dec_wspr;
} sdrkit_wspr_dropped;

// The items that each decoder component of the stack "jt65" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_jt65_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_jt65` holds a value.
  bool has_dec_jt65;
  // The items that the component "dec_jt65" dropped.
  int64_t dec_jt65;
} sdrkit_jt65_dropped;

// The items that each decoder component of the stack "jt9" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_jt9_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_jt9` holds a value.
  bool has_dec_jt9;
  // The items that the component "dec_jt9" dropped.
  int64_t dec_jt9;
} sdrkit_jt9_dropped;

// The items that each decoder component of the stack "jt4" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_jt4_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_jt4` holds a value.
  bool has_dec_jt4;
  // The items that the component "dec_jt4" dropped.
  int64_t dec_jt4;
} sdrkit_jt4_dropped;

// The items that each decoder component of the stack "q65" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_q65_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_q65` holds a value.
  bool has_dec_q65;
  // The items that the component "dec_q65" dropped.
  int64_t dec_q65;
} sdrkit_q65_dropped;

// The items that each decoder component of the stack "fst4" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_fst4_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_fst4` holds a value.
  bool has_dec_fst4;
  // The items that the component "dec_fst4" dropped.
  int64_t dec_fst4;
} sdrkit_fst4_dropped;

// The items that each decoder component of the stack "fst4w" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_fst4w_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_fst4w` holds a value.
  bool has_dec_fst4w;
  // The items that the component "dec_fst4w" dropped.
  int64_t dec_fst4w;
} sdrkit_fst4w_dropped;

// The items that each decoder component of the stack "msk144" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_msk144_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_msk144` holds a value.
  bool has_dec_msk144;
  // The items that the component "dec_msk144" dropped.
  int64_t dec_msk144;
} sdrkit_msk144_dropped;

// The items that each decoder component of the stack "js8" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_js8_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dec_js8` holds a value.
  bool has_dec_js8;
  // The items that the component "dec_js8" dropped.
  int64_t dec_js8;
} sdrkit_js8_dropped;

// The options of `mode` of `sdrkit_wsjt_decode_event`.
typedef enum sdrkit_wsjt_decode_event_mode {
  // FT8
  SDRKIT_WSJT_DECODE_EVENT_MODE_FT8 = 0,
  // FT4
  SDRKIT_WSJT_DECODE_EVENT_MODE_FT4 = 1,
  // FT2
  SDRKIT_WSJT_DECODE_EVENT_MODE_FT2 = 2,
  // WSPR
  SDRKIT_WSJT_DECODE_EVENT_MODE_WSPR = 3,
  // JT65
  SDRKIT_WSJT_DECODE_EVENT_MODE_JT65 = 4,
  // JT9
  SDRKIT_WSJT_DECODE_EVENT_MODE_JT9 = 5,
  // JT4
  SDRKIT_WSJT_DECODE_EVENT_MODE_JT4 = 6,
  // Q65
  SDRKIT_WSJT_DECODE_EVENT_MODE_Q65 = 7,
  // FST4
  SDRKIT_WSJT_DECODE_EVENT_MODE_FST4 = 8,
  // FST4W
  SDRKIT_WSJT_DECODE_EVENT_MODE_FST4W = 9,
  // MSK144
  SDRKIT_WSJT_DECODE_EVENT_MODE_MSK144 = 10,
  // JS8
  SDRKIT_WSJT_DECODE_EVENT_MODE_JS8 = 11,
} sdrkit_wsjt_decode_event_mode;

// The options of `how` of `sdrkit_wsjt_decode_event`.
typedef enum sdrkit_wsjt_decode_event_how {
  // This slot
  SDRKIT_WSJT_DECODE_EVENT_HOW_ALONE = 0,
  // (combined)
  SDRKIT_WSJT_DECODE_EVENT_HOW_COMBINED = 1,
  // (a-priori)
  SDRKIT_WSJT_DECODE_EVENT_HOW_A_PRIORI = 2,
} sdrkit_wsjt_decode_event_how;

// The options of `family` of `sdrkit_wsjt_decode_event`.
typedef enum sdrkit_wsjt_decode_event_family {
  // Aircraft
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WSJT_DECODE_EVENT_FAMILY_OTHER = 12,
} sdrkit_wsjt_decode_event_family;

// The fields of a decode of any mode but JS8.
typedef struct sdrkit_wsjt_decode_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The mode. A value of `sdrkit_wsjt_decode_event_mode`.
  int32_t mode;
  // The decoded message.
  sdrkit_string text;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // The time offset from the slot start. Unit: s.
  double dt_s;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The calling station, if the message names one.
  sdrkit_string callsign;
  // True if `locator` holds a value.
  bool has_locator;
  // The locator, if the message holds one.
  sdrkit_string locator;
  // The message is a CQ.
  bool cq;
  // The decode was found on the other sideband.
  bool mirrored;
  // How the decode was made. A value of `sdrkit_wsjt_decode_event_how`.
  int32_t how;
  // True if `power_dbm` holds a value.
  bool has_power_dbm;
  // The power that a WSPR or FST4W station states. Unit: dBm.
  int64_t power_dbm;
  // True if `source` holds a value.
  bool has_source;
  // The calling station, if the message names one.
  sdrkit_string source;
  // True if `position` holds a value.
  bool has_position;
  // The centre of the locator, if the message holds one.
  sdrkit_position position;
  // The family of the source: a message. A value of `sdrkit_wsjt_decode_event_family`.
  int32_t family;
} sdrkit_wsjt_decode_event;

// The options of `family` of `sdrkit_wsjt_spot_event`.
typedef enum sdrkit_wsjt_spot_event_family {
  // Aircraft
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WSJT_SPOT_EVENT_FAMILY_OTHER = 12,
} sdrkit_wsjt_spot_event_family;

// The fields of a station heard, with the standard spot fields.
typedef struct sdrkit_wsjt_spot_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The callsign.
  sdrkit_string callsign;
  // True if `locator` holds a value.
  bool has_locator;
  // The locator, if one was heard.
  sdrkit_string locator;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // The radio frequency. A frequency in Hz.
  double freq_hz_;
  // The emission as spot networks name it.
  sdrkit_string mode;
  // The callsign.
  sdrkit_string source;
  // The family of the source: a message. A value of `sdrkit_wsjt_spot_event_family`.
  int32_t family;
} sdrkit_wsjt_spot_event;

// The options of `submode` of `sdrkit_js8_frame_event`.
typedef enum sdrkit_js8_frame_event_submode {
  // Normal
  SDRKIT_JS8_FRAME_EVENT_SUBMODE_NORMAL = 0,
  // Fast
  SDRKIT_JS8_FRAME_EVENT_SUBMODE_FAST = 1,
  // Turbo
  SDRKIT_JS8_FRAME_EVENT_SUBMODE_TURBO = 2,
} sdrkit_js8_frame_event_submode;

// The options of `family` of `sdrkit_js8_frame_event`.
typedef enum sdrkit_js8_frame_event_family {
  // Aircraft
  SDRKIT_JS8_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_JS8_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_JS8_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_JS8_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_JS8_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_JS8_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_JS8_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_JS8_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_JS8_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_JS8_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_JS8_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_JS8_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_JS8_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_js8_frame_event_family;

// The fields of a JS8 frame.
typedef struct sdrkit_js8_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the frame.
  sdrkit_string text;
  // The twelve characters of the frame.
  sdrkit_string chars;
  // The SNR in 2500 Hz. Unit: dB.
  double snr_db;
  // True if `submode` holds a value.
  bool has_submode;
  // The speed of the frame. A value of `sdrkit_js8_frame_event_submode`.
  int32_t submode;
  // True if `source` holds a value.
  bool has_source;
  // The sending station, if the frame names one.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The station or group addressed, if the frame names one.
  sdrkit_string target;
  // The family of the source: a message. A value of `sdrkit_js8_frame_event_family`.
  int32_t family;
} sdrkit_js8_frame_event;

// The options of `family` of `sdrkit_js8_message_event`.
typedef enum sdrkit_js8_message_event_family {
  // Aircraft
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_JS8_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_js8_message_event_family;

// The fields of a JS8 message, sent when its last frame came or when it stopped.
typedef struct sdrkit_js8_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The message.
  sdrkit_string text;
  // The last frame of the message came.
  bool complete;
  // True if `source` holds a value.
  bool has_source;
  // The sending station, if it is known.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The station or group addressed, if it is known.
  sdrkit_string target;
  // The family of the source: a message. A value of `sdrkit_js8_message_event_family`.
  int32_t family;
} sdrkit_js8_message_event;

// The options of `mode` of `sdrkit_wsjt_tx_event`.
typedef enum sdrkit_wsjt_tx_event_mode {
  // FT8
  SDRKIT_WSJT_TX_EVENT_MODE_FT8 = 0,
  // FT4
  SDRKIT_WSJT_TX_EVENT_MODE_FT4 = 1,
  // FT2
  SDRKIT_WSJT_TX_EVENT_MODE_FT2 = 2,
  // WSPR
  SDRKIT_WSJT_TX_EVENT_MODE_WSPR = 3,
  // JT65
  SDRKIT_WSJT_TX_EVENT_MODE_JT65 = 4,
  // JT9
  SDRKIT_WSJT_TX_EVENT_MODE_JT9 = 5,
  // JT4
  SDRKIT_WSJT_TX_EVENT_MODE_JT4 = 6,
  // Q65
  SDRKIT_WSJT_TX_EVENT_MODE_Q65 = 7,
  // FST4
  SDRKIT_WSJT_TX_EVENT_MODE_FST4 = 8,
  // FST4W
  SDRKIT_WSJT_TX_EVENT_MODE_FST4W = 9,
  // MSK144
  SDRKIT_WSJT_TX_EVENT_MODE_MSK144 = 10,
  // JS8
  SDRKIT_WSJT_TX_EVENT_MODE_JS8 = 11,
} sdrkit_wsjt_tx_event_mode;

// The options of `family` of `sdrkit_wsjt_tx_event`.
typedef enum sdrkit_wsjt_tx_event_family {
  // Aircraft
  SDRKIT_WSJT_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WSJT_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WSJT_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WSJT_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WSJT_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WSJT_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WSJT_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WSJT_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WSJT_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WSJT_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WSJT_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WSJT_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WSJT_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_wsjt_tx_event_family;

// The fields of a message to send.
typedef struct sdrkit_wsjt_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The mode. A value of `sdrkit_wsjt_tx_event_mode`.
  int32_t mode;
  // The message.
  sdrkit_string text;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // The UTC of the start of the transmit slot. Seconds since 1970 (UTC).
  double slot;
  // The T/R period. A length of time in seconds.
  double period;
  // The family of the source: a message. A value of `sdrkit_wsjt_tx_event_family`.
  int32_t family;
} sdrkit_wsjt_tx_event;

// The options of `family` of `sdrkit_fsk441_decode_event`.
typedef enum sdrkit_fsk441_decode_event_family {
  // Aircraft
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FSK441_DECODE_EVENT_FAMILY_OTHER = 12,
} sdrkit_fsk441_decode_event_family;

// The fields of a decode of `fsk441`.
typedef struct sdrkit_fsk441_decode_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string text;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The first callsign of the text.
  sdrkit_string callsign;
  // True if `source` holds a value.
  bool has_source;
  // The first callsign of the text.
  sdrkit_string source;
  // The family of the source: a message. A value of `sdrkit_fsk441_decode_event_family`.
  int32_t family;
  // The contrast of the decode. It is no calibrated SNR. Unit: dB.
  double contrast_db;
  // The length of the burst. Unit: ms.
  double duration_ms;
} sdrkit_fsk441_decode_event;

// The options of `family` of `sdrkit_jt6m_decode_event`.
typedef enum sdrkit_jt6m_decode_event_family {
  // Aircraft
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_JT6M_DECODE_EVENT_FAMILY_OTHER = 12,
} sdrkit_jt6m_decode_event_family;

// The fields of a ping of `jt6m`.
typedef struct sdrkit_jt6m_decode_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string text;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The first callsign of the text.
  sdrkit_string callsign;
  // True if `source` holds a value.
  bool has_source;
  // The first callsign of the text.
  sdrkit_string source;
  // The family of the source: a message. A value of `sdrkit_jt6m_decode_event_family`.
  int32_t family;
  // The length of the ping. Unit: s.
  double duration_s;
  // The clear frames of the ping.
  int64_t clear_frames;
} sdrkit_jt6m_decode_event;

// The options of `family` of `sdrkit_iscat_decode_event`.
typedef enum sdrkit_iscat_decode_event_family {
  // Aircraft
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ISCAT_DECODE_EVENT_FAMILY_OTHER = 12,
} sdrkit_iscat_decode_event_family;

// The fields of a decode of `iscat`.
typedef struct sdrkit_iscat_decode_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string text;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The first callsign of the text.
  sdrkit_string callsign;
  // True if `source` holds a value.
  bool has_source;
  // The first callsign of the text.
  sdrkit_string source;
  // The family of the source: a message. A value of `sdrkit_iscat_decode_event_family`.
  int32_t family;
  // The variant of the decode.
  sdrkit_string variant;
  // The sync ratio r.
  double sync_ratio;
  // The whole frames used.
  int64_t frames;
  // The characters shown as `?`.
  int64_t unclear;
} sdrkit_iscat_decode_event;

// The options of `family` of `sdrkit_pi4_decode_event`.
typedef enum sdrkit_pi4_decode_event_family {
  // Aircraft
  SDRKIT_PI4_DECODE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PI4_DECODE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PI4_DECODE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PI4_DECODE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PI4_DECODE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PI4_DECODE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PI4_DECODE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PI4_DECODE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PI4_DECODE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PI4_DECODE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PI4_DECODE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PI4_DECODE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PI4_DECODE_EVENT_FAMILY_OTHER = 12,
} sdrkit_pi4_decode_event_family;

// The fields of a decode of `pi4`.
typedef struct sdrkit_pi4_decode_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The mode or its variant.
  sdrkit_string mode;
  // The decoded text.
  sdrkit_string text;
  // The SNR. Unit: dB.
  double snr_db;
  // The audio frequency. A frequency in Hz.
  double audio_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The first callsign of the text.
  sdrkit_string callsign;
  // True if `source` holds a value.
  bool has_source;
  // The first callsign of the text.
  sdrkit_string source;
  // The family of the source: a message. A value of `sdrkit_pi4_decode_event_family`.
  int32_t family;
  // The start of the decode against the minute. Unit: s.
  double dt_s;
  // The carrier frequency: tone 0 plus half a spacing. Unit: Hz.
  double carrier_hz;
  // The sync value c.
  double sync;
  // The tone spacing factor of the decode.
  int64_t k;
} sdrkit_pi4_decode_event;

// The arguments of `remove`.
typedef struct sdrkit_wsjt_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_wsjt_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WSJT_STATION_ARGS_INIT {sizeof(sdrkit_wsjt_station_args), {0}}

// The arguments of `send`.
typedef struct sdrkit_wsjt_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The message to send: `HB`, `CQ`, `TO CMD [NUM]` (a command such as `SNR?`), `TO TEXT` or `VIA>DEST TEXT`.
  sdrkit_string text;
} sdrkit_wsjt_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WSJT_SEND_ARGS_INIT {sizeof(sdrkit_wsjt_send_args), {0}}

// Fills `rows` with the changed rows of the table "decodes" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_decodes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_decode_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wspr_spot_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_modes_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wsjt_mode_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_wsjt_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_js8_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_js8_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_js8_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_js8_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wide.multiband" of the stack "wsjt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wsjt_wide_multiband_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_wsjt_multiband_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.MultibandRow", "wide.multiband", rows, sizeof(sdrkit_wsjt_multiband_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.decode"
// (Decode). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wsjt_decode_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wsjt_decode_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.decode", "wsjt.DecodeEvent", event, sizeof(sdrkit_wsjt_decode_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.spot"
// (Station heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wsjt_spot_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wsjt_spot_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.spot", "wsjt.SpotEvent", event, sizeof(sdrkit_wsjt_spot_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "js8.frame"
// (JS8 frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_js8_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_js8_frame_event *event) {
  return sdrkit_typed_event(handle, n, "js8.frame", "wsjt.Js8FrameEvent", event, sizeof(sdrkit_js8_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "js8.message"
// (JS8 message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_js8_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_js8_message_event *event) {
  return sdrkit_typed_event(handle, n, "js8.message", "wsjt.Js8MessageEvent", event, sizeof(sdrkit_js8_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.tx"
// (Message to send). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wsjt_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wsjt_tx_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.tx", "wsjt.TxEvent", event, sizeof(sdrkit_wsjt_tx_event));
}

// Runs the command "clear" (Clear) of the stack "wsjt" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_wsjt_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "wsjt" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_wsjt_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "wsjt" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_wsjt_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "beacon" (Start the WSPR beacon) of the stack "wsjt" on the session of `handle`. Sends the call, locator and power of the station in the beacon slots. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_wsjt_beacon(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "beacon", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "wsjt" on the session of `handle`. Queues a JS8 message. My call and locator come from the station. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_wsjt_send(SDRKIT_HANDLE handle, const sdrkit_wsjt_send_args *args) {
  return sdrkit_typed_command(handle, "send", "wsjt.SendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "wsjt" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_wsjt_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wsjt" are the sdrkit_wsjt_gen_*() functions
// after the settings of its receiver.

// The setting "enable_ft8" (Decode FT8 (15 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_ft8(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_ft8", value);
}
static inline int32_t sdrkit_wsjt_set_enable_ft8(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_ft8", value);
}

// The setting "enable_ft4" (Decode FT4 (7.5 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_ft4(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_ft4", value);
}
static inline int32_t sdrkit_wsjt_set_enable_ft4(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_ft4", value);
}

// The setting "enable_ft2" (Decode FT2 (3.75 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_ft2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_ft2", value);
}
static inline int32_t sdrkit_wsjt_set_enable_ft2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_ft2", value);
}

// The setting "enable_wspr" (Decode WSPR (120 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_wspr(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_wspr", value);
}
static inline int32_t sdrkit_wsjt_set_enable_wspr(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_wspr", value);
}

// The setting "enable_jt65" (Decode JT65 (60 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_jt65(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_jt65", value);
}
static inline int32_t sdrkit_wsjt_set_enable_jt65(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_jt65", value);
}

// The setting "enable_jt9" (Decode JT9 (60 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_jt9(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_jt9", value);
}
static inline int32_t sdrkit_wsjt_set_enable_jt9(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_jt9", value);
}

// The setting "enable_jt4" (Decode JT4 (60 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_jt4(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_jt4", value);
}
static inline int32_t sdrkit_wsjt_set_enable_jt4(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_jt4", value);
}

// The setting "enable_q65" (Decode Q65 (60 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_q65(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_q65", value);
}
static inline int32_t sdrkit_wsjt_set_enable_q65(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_q65", value);
}

// The setting "enable_fst4" (Decode FST4 (60 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_fst4(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_fst4", value);
}
static inline int32_t sdrkit_wsjt_set_enable_fst4(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_fst4", value);
}

// The setting "enable_fst4w" (Decode FST4W (120 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_fst4w(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_fst4w", value);
}
static inline int32_t sdrkit_wsjt_set_enable_fst4w(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_fst4w", value);
}

// The setting "enable_msk144" (Decode MSK144 (15 s slots)) of the stack "wsjt". Takes effect at the next slot.
static inline int32_t sdrkit_wsjt_get_enable_msk144(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "enable_msk144", value);
}
static inline int32_t sdrkit_wsjt_set_enable_msk144(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "enable_msk144", value);
}

// The setting "jt65_submode" (JT65 sub-mode) of the stack "wsjt". A value of `sdrkit_wsjt_jt65_submode`.
static inline int32_t sdrkit_wsjt_get_jt65_submode(SDRKIT_HANDLE handle, sdrkit_wsjt_jt65_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "jt65_submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_jt65_submode)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_jt65_submode(SDRKIT_HANDLE handle, sdrkit_wsjt_jt65_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "jt65_submode", (int32_t)value);
}

// The setting "jt4_submode" (JT4 sub-mode) of the stack "wsjt". A value of `sdrkit_wsjt_jt4_submode`.
static inline int32_t sdrkit_wsjt_get_jt4_submode(SDRKIT_HANDLE handle, sdrkit_wsjt_jt4_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "jt4_submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_jt4_submode)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_jt4_submode(SDRKIT_HANDLE handle, sdrkit_wsjt_jt4_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "jt4_submode", (int32_t)value);
}

// The setting "q65_submode" (Q65 sub-mode) of the stack "wsjt". A value of `sdrkit_wsjt_q65_submode`.
static inline int32_t sdrkit_wsjt_get_q65_submode(SDRKIT_HANDLE handle, sdrkit_wsjt_q65_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "q65_submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_q65_submode)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_q65_submode(SDRKIT_HANDLE handle, sdrkit_wsjt_q65_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "q65_submode", (int32_t)value);
}

// The setting "q65_period" (Q65 T/R period) of the stack "wsjt". A length of time in seconds. One of 15, 30, 60, 120, 300 s.
static inline int32_t sdrkit_wsjt_get_q65_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "q65_period", value);
}
static inline int32_t sdrkit_wsjt_set_q65_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "q65_period", value);
}

// The setting "fst4_period" (FST4 T/R period) of the stack "wsjt". A length of time in seconds. One of 15, 30, 60, 120, 300, 900, 1800 s.
static inline int32_t sdrkit_wsjt_get_fst4_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fst4_period", value);
}
static inline int32_t sdrkit_wsjt_set_fst4_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fst4_period", value);
}

// The setting "fst4w_period" (FST4W T/R period) of the stack "wsjt". A length of time in seconds. One of 15, 30, 60, 120, 300, 900, 1800 s.
static inline int32_t sdrkit_wsjt_get_fst4w_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fst4w_period", value);
}
static inline int32_t sdrkit_wsjt_set_fst4w_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fst4w_period", value);
}

// The setting "msk144_period" (MSK144 T/R period) of the stack "wsjt". A length of time in seconds. One of 15, 30 s.
static inline int32_t sdrkit_wsjt_get_msk144_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "msk144_period", value);
}
static inline int32_t sdrkit_wsjt_set_msk144_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "msk144_period", value);
}

// The setting "js8_normal" (JS8 Normal (15 s)) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_js8_normal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "js8_normal", value);
}
static inline int32_t sdrkit_wsjt_set_js8_normal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "js8_normal", value);
}

// The setting "js8_fast" (JS8 Fast (10 s)) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_js8_fast(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "js8_fast", value);
}
static inline int32_t sdrkit_wsjt_set_js8_fast(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "js8_fast", value);
}

// The setting "js8_turbo" (JS8 Turbo (6 s)) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_js8_turbo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "js8_turbo", value);
}
static inline int32_t sdrkit_wsjt_set_js8_turbo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "js8_turbo", value);
}

// The setting "mirrored" (Upside down too) of the stack "wsjt". Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR).
static inline int32_t sdrkit_wsjt_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mirrored", value);
}
static inline int32_t sdrkit_wsjt_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mirrored", value);
}

// The setting "deep" (Deep decoding) of the stack "wsjt". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_wsjt_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_wsjt_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "early_decode" (Early decode) of the stack "wsjt". FT8 and FT4: decode the slot once more when 85 % of the transmission is in, while the decoder is idle. Early decodes are shown at once and replaced by the decodes of the whole slot; they are not counted.
static inline int32_t sdrkit_wsjt_get_early_decode(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "early_decode", value);
}
static inline int32_t sdrkit_wsjt_set_early_decode(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "early_decode", value);
}

// The setting "demod" (Demodulator) of the stack "wsjt". A value of `sdrkit_wsjt_demod`.
static inline int32_t sdrkit_wsjt_get_demod(SDRKIT_HANDLE handle, sdrkit_wsjt_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_demod)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_demod(SDRKIT_HANDLE handle, sdrkit_wsjt_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "wsjt". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_wsjt_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_wsjt_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "wsjt". A value of `sdrkit_wsjt_time_source`.
static inline int32_t sdrkit_wsjt_get_time_source(SDRKIT_HANDLE handle, sdrkit_wsjt_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_time_source)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_time_source(SDRKIT_HANDLE handle, sdrkit_wsjt_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "wsjt". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_wsjt_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_wsjt_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "wsjt". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_wsjt_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_wsjt_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "dials" (Dials) of the stack "wsjt". The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by ";", for example "ft8=14074000". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial.
static inline int32_t sdrkit_wsjt_get_dials(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "dials", value);
}
static inline int32_t sdrkit_wsjt_set_dials(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "dials", value);
}

// The setting "wide" (Decode the whole span) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide", value);
}
static inline int32_t sdrkit_wsjt_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide", value);
}

// The setting "wide_span" (Span) of the stack "wsjt". A frequency in Hz.
static inline int32_t sdrkit_wsjt_get_wide_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wide_span", value);
}
static inline int32_t sdrkit_wsjt_set_wide_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wide_span", value);
}

// The setting "multiband" (Decode all selected at once) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_multiband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "multiband", value);
}
static inline int32_t sdrkit_wsjt_set_multiband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "multiband", value);
}

// The setting "multiband_select" (Bands and modes) of the stack "wsjt". The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR.
static inline int32_t sdrkit_wsjt_get_multiband_select(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "multiband_select", value);
}
static inline int32_t sdrkit_wsjt_set_multiband_select(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "multiband_select", value);
}

// The setting "spots" (Spot events) of the stack "wsjt". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_wsjt_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_wsjt_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_mode" (Mode) of the stack "wsjt". A value of `sdrkit_wsjt_qso_mode`.
static inline int32_t sdrkit_wsjt_get_qso_mode(SDRKIT_HANDLE handle, sdrkit_wsjt_qso_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "qso_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_qso_mode)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_qso_mode(SDRKIT_HANDLE handle, sdrkit_wsjt_qso_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "qso_mode", (int32_t)value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_wsjt_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_wsjt_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_wsjt_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "wsjt". A frequency in Hz.
static inline int32_t sdrkit_wsjt_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_wsjt_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_wsjt_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "wsjt". A length of time in seconds.
static inline int32_t sdrkit_wsjt_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_wsjt_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "beacon_dbm" (Power) of the stack "wsjt". Unit: dBm. WSPR sends 0 to 60 dBm, ending in 0, 3 or 7.
static inline int32_t sdrkit_wsjt_get_beacon_dbm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beacon_dbm", value);
}
static inline int32_t sdrkit_wsjt_set_beacon_dbm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beacon_dbm", value);
}

// The setting "beacon_every" (One slot in N) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_beacon_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beacon_every", value);
}
static inline int32_t sdrkit_wsjt_set_beacon_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beacon_every", value);
}

// The setting "beacon_offset" (Offset) of the stack "wsjt".
static inline int32_t sdrkit_wsjt_get_beacon_offset(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beacon_offset", value);
}
static inline int32_t sdrkit_wsjt_set_beacon_offset(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beacon_offset", value);
}

// The setting "beacon_hz" (Audio frequency) of the stack "wsjt". A frequency in Hz.
static inline int32_t sdrkit_wsjt_get_beacon_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "beacon_hz", value);
}
static inline int32_t sdrkit_wsjt_set_beacon_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "beacon_hz", value);
}

// The setting "js8_speed" (Speed) of the stack "wsjt". A value of `sdrkit_wsjt_js8_speed`.
static inline int32_t sdrkit_wsjt_get_js8_speed(SDRKIT_HANDLE handle, sdrkit_wsjt_js8_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "js8_speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wsjt_js8_speed)index;
  return code;
}
static inline int32_t sdrkit_wsjt_set_js8_speed(SDRKIT_HANDLE handle, sdrkit_wsjt_js8_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "js8_speed", (int32_t)value);
}

// The setting "js8_hz" (Audio frequency) of the stack "wsjt". A frequency in Hz.
static inline int32_t sdrkit_wsjt_get_js8_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "js8_hz", value);
}
static inline int32_t sdrkit_wsjt_set_js8_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "js8_hz", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "wsjt".
static inline int32_t sdrkit_wsjt_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_wsjt_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "wsjt".
static inline int32_t sdrkit_wsjt_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_wsjt_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "wsjt".
static inline int32_t sdrkit_wsjt_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_wsjt_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "wsjt". A frequency in Hz.
static inline int32_t sdrkit_wsjt_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_wsjt_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_js8_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_js8_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_js8_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_js8_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wide.multiband" of the stack "ft8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft8_wide_multiband_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_wsjt_multiband_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.MultibandRow", "wide.multiband", rows, sizeof(sdrkit_wsjt_multiband_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ft8" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ft8_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ft8" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ft8_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "ft8" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_ft8_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "ft8" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_ft8_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ft8" are the sdrkit_ft8_gen_*() functions
// after the settings of its receiver.

// The setting "mirrored" (Upside down too) of the stack "ft8". Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR).
static inline int32_t sdrkit_ft8_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mirrored", value);
}
static inline int32_t sdrkit_ft8_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mirrored", value);
}

// The setting "deep" (Deep decoding) of the stack "ft8". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_ft8_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_ft8_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "early_decode" (Early decode) of the stack "ft8". FT8 and FT4: decode the slot once more when 85 % of the transmission is in, while the decoder is idle. Early decodes are shown at once and replaced by the decodes of the whole slot; they are not counted.
static inline int32_t sdrkit_ft8_get_early_decode(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "early_decode", value);
}
static inline int32_t sdrkit_ft8_set_early_decode(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "early_decode", value);
}

// The setting "demod" (Demodulator) of the stack "ft8". A value of `sdrkit_ft8_demod`.
static inline int32_t sdrkit_ft8_get_demod(SDRKIT_HANDLE handle, sdrkit_ft8_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ft8_demod)index;
  return code;
}
static inline int32_t sdrkit_ft8_set_demod(SDRKIT_HANDLE handle, sdrkit_ft8_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "ft8". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_ft8_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ft8_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "ft8". A value of `sdrkit_ft8_time_source`.
static inline int32_t sdrkit_ft8_get_time_source(SDRKIT_HANDLE handle, sdrkit_ft8_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ft8_time_source)index;
  return code;
}
static inline int32_t sdrkit_ft8_set_time_source(SDRKIT_HANDLE handle, sdrkit_ft8_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "ft8". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_ft8_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_ft8_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "ft8". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_ft8_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_ft8_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "dials" (Dials) of the stack "ft8". The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by ";", for example "ft8=14074000". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial.
static inline int32_t sdrkit_ft8_get_dials(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "dials", value);
}
static inline int32_t sdrkit_ft8_set_dials(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "dials", value);
}

// The setting "wide" (Decode the whole span) of the stack "ft8".
static inline int32_t sdrkit_ft8_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide", value);
}
static inline int32_t sdrkit_ft8_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide", value);
}

// The setting "wide_span" (Span) of the stack "ft8". A frequency in Hz.
static inline int32_t sdrkit_ft8_get_wide_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wide_span", value);
}
static inline int32_t sdrkit_ft8_set_wide_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wide_span", value);
}

// The setting "multiband" (Decode all selected at once) of the stack "ft8".
static inline int32_t sdrkit_ft8_get_multiband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "multiband", value);
}
static inline int32_t sdrkit_ft8_set_multiband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "multiband", value);
}

// The setting "multiband_select" (Bands and modes) of the stack "ft8". The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR.
static inline int32_t sdrkit_ft8_get_multiband_select(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "multiband_select", value);
}
static inline int32_t sdrkit_ft8_set_multiband_select(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "multiband_select", value);
}

// The setting "hop.enabled" (Band hopping) of the stack "ft8". At the start of each 2-minute slot, asks the host to tune to the dial of the band of the plan. Off while the whole span is decoded.
static inline int32_t sdrkit_ft8_get_hop_enabled(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.enabled", value);
}
static inline int32_t sdrkit_ft8_set_hop_enabled(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.enabled", value);
}

// The setting "hop.coordinated" (Coordinated hopping) of the stack "ft8". Every station takes the same band of the cycle in the same slot.
static inline int32_t sdrkit_ft8_get_hop_coordinated(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.coordinated", value);
}
static inline int32_t sdrkit_ft8_set_hop_coordinated(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value);
}

// The setting "hop.hours" (Bands of each hour) of the stack "ft8". 24 hexadecimal masks separated by commas, UTC hour 0 first. Bit b of a mask is band b of the band list.
static inline int32_t sdrkit_ft8_get_hop_hours(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.hours", value);
}
static inline int32_t sdrkit_ft8_set_hop_hours(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.hours", value);
}

// The setting "hop.bands" (Band list) of the stack "ft8". The band names in bit order, separated by commas (for example from data/wsjt/band_hopping.csv). Empty: no band.
static inline int32_t sdrkit_ft8_get_hop_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.bands", value);
}
static inline int32_t sdrkit_ft8_set_hop_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.bands", value);
}

// The setting "hop.cycle" (Coordinated cycle) of the stack "ft8". The bands of the coordinated cycle in their order, by the names of the band list.
static inline int32_t sdrkit_ft8_get_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.cycle", value);
}
static inline int32_t sdrkit_ft8_set_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.cycle", value);
}

// The setting "spots" (Spot events) of the stack "ft8". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_ft8_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_ft8_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "ft8".
static inline int32_t sdrkit_ft8_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_ft8_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "ft8".
static inline int32_t sdrkit_ft8_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_ft8_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "ft8".
static inline int32_t sdrkit_ft8_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_ft8_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "ft8". A frequency in Hz.
static inline int32_t sdrkit_ft8_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_ft8_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "ft8".
static inline int32_t sdrkit_ft8_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_ft8_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "ft8". A length of time in seconds.
static inline int32_t sdrkit_ft8_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_ft8_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "ft8".
static inline int32_t sdrkit_ft8_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_ft8_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "ft8".
static inline int32_t sdrkit_ft8_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_ft8_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "ft8".
static inline int32_t sdrkit_ft8_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_ft8_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "ft8". A frequency in Hz.
static inline int32_t sdrkit_ft8_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_ft8_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_js8_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_js8_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_js8_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_js8_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wide.multiband" of the stack "ft4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft4_wide_multiband_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_wsjt_multiband_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.MultibandRow", "wide.multiband", rows, sizeof(sdrkit_wsjt_multiband_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ft4" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ft4_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ft4" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ft4_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "ft4" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_ft4_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "ft4" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_ft4_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ft4" are the sdrkit_ft4_gen_*() functions
// after the settings of its receiver.

// The setting "mirrored" (Upside down too) of the stack "ft4". Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR).
static inline int32_t sdrkit_ft4_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mirrored", value);
}
static inline int32_t sdrkit_ft4_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mirrored", value);
}

// The setting "deep" (Deep decoding) of the stack "ft4". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_ft4_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_ft4_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "early_decode" (Early decode) of the stack "ft4". FT8 and FT4: decode the slot once more when 85 % of the transmission is in, while the decoder is idle. Early decodes are shown at once and replaced by the decodes of the whole slot; they are not counted.
static inline int32_t sdrkit_ft4_get_early_decode(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "early_decode", value);
}
static inline int32_t sdrkit_ft4_set_early_decode(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "early_decode", value);
}

// The setting "demod" (Demodulator) of the stack "ft4". A value of `sdrkit_ft4_demod`.
static inline int32_t sdrkit_ft4_get_demod(SDRKIT_HANDLE handle, sdrkit_ft4_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ft4_demod)index;
  return code;
}
static inline int32_t sdrkit_ft4_set_demod(SDRKIT_HANDLE handle, sdrkit_ft4_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "ft4". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_ft4_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ft4_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "ft4". A value of `sdrkit_ft4_time_source`.
static inline int32_t sdrkit_ft4_get_time_source(SDRKIT_HANDLE handle, sdrkit_ft4_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ft4_time_source)index;
  return code;
}
static inline int32_t sdrkit_ft4_set_time_source(SDRKIT_HANDLE handle, sdrkit_ft4_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "ft4". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_ft4_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_ft4_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "ft4". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_ft4_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_ft4_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "dials" (Dials) of the stack "ft4". The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by ";", for example "ft8=14074000". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial.
static inline int32_t sdrkit_ft4_get_dials(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "dials", value);
}
static inline int32_t sdrkit_ft4_set_dials(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "dials", value);
}

// The setting "wide" (Decode the whole span) of the stack "ft4".
static inline int32_t sdrkit_ft4_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide", value);
}
static inline int32_t sdrkit_ft4_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide", value);
}

// The setting "wide_span" (Span) of the stack "ft4". A frequency in Hz.
static inline int32_t sdrkit_ft4_get_wide_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wide_span", value);
}
static inline int32_t sdrkit_ft4_set_wide_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wide_span", value);
}

// The setting "multiband" (Decode all selected at once) of the stack "ft4".
static inline int32_t sdrkit_ft4_get_multiband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "multiband", value);
}
static inline int32_t sdrkit_ft4_set_multiband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "multiband", value);
}

// The setting "multiband_select" (Bands and modes) of the stack "ft4". The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR.
static inline int32_t sdrkit_ft4_get_multiband_select(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "multiband_select", value);
}
static inline int32_t sdrkit_ft4_set_multiband_select(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "multiband_select", value);
}

// The setting "hop.enabled" (Band hopping) of the stack "ft4". At the start of each 2-minute slot, asks the host to tune to the dial of the band of the plan. Off while the whole span is decoded.
static inline int32_t sdrkit_ft4_get_hop_enabled(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.enabled", value);
}
static inline int32_t sdrkit_ft4_set_hop_enabled(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.enabled", value);
}

// The setting "hop.coordinated" (Coordinated hopping) of the stack "ft4". Every station takes the same band of the cycle in the same slot.
static inline int32_t sdrkit_ft4_get_hop_coordinated(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.coordinated", value);
}
static inline int32_t sdrkit_ft4_set_hop_coordinated(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value);
}

// The setting "hop.hours" (Bands of each hour) of the stack "ft4". 24 hexadecimal masks separated by commas, UTC hour 0 first. Bit b of a mask is band b of the band list.
static inline int32_t sdrkit_ft4_get_hop_hours(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.hours", value);
}
static inline int32_t sdrkit_ft4_set_hop_hours(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.hours", value);
}

// The setting "hop.bands" (Band list) of the stack "ft4". The band names in bit order, separated by commas (for example from data/wsjt/band_hopping.csv). Empty: no band.
static inline int32_t sdrkit_ft4_get_hop_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.bands", value);
}
static inline int32_t sdrkit_ft4_set_hop_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.bands", value);
}

// The setting "hop.cycle" (Coordinated cycle) of the stack "ft4". The bands of the coordinated cycle in their order, by the names of the band list.
static inline int32_t sdrkit_ft4_get_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.cycle", value);
}
static inline int32_t sdrkit_ft4_set_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.cycle", value);
}

// The setting "spots" (Spot events) of the stack "ft4". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_ft4_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_ft4_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "ft4".
static inline int32_t sdrkit_ft4_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_ft4_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "ft4".
static inline int32_t sdrkit_ft4_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_ft4_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "ft4".
static inline int32_t sdrkit_ft4_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_ft4_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "ft4". A frequency in Hz.
static inline int32_t sdrkit_ft4_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_ft4_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "ft4".
static inline int32_t sdrkit_ft4_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_ft4_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "ft4". A length of time in seconds.
static inline int32_t sdrkit_ft4_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_ft4_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "ft4".
static inline int32_t sdrkit_ft4_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_ft4_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "ft4".
static inline int32_t sdrkit_ft4_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_ft4_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "ft4".
static inline int32_t sdrkit_ft4_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_ft4_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "ft4". A frequency in Hz.
static inline int32_t sdrkit_ft4_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_ft4_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_js8_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_js8_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_js8_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_js8_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wide.multiband" of the stack "ft2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ft2_wide_multiband_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_wsjt_multiband_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.MultibandRow", "wide.multiband", rows, sizeof(sdrkit_wsjt_multiband_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ft2" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ft2_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ft2" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ft2_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "ft2" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_ft2_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "ft2" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_ft2_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ft2" are the sdrkit_ft2_gen_*() functions
// after the settings of its receiver.

// The setting "mirrored" (Upside down too) of the stack "ft2". Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR).
static inline int32_t sdrkit_ft2_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mirrored", value);
}
static inline int32_t sdrkit_ft2_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mirrored", value);
}

// The setting "deep" (Deep decoding) of the stack "ft2". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_ft2_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_ft2_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "ft2". A value of `sdrkit_ft2_demod`.
static inline int32_t sdrkit_ft2_get_demod(SDRKIT_HANDLE handle, sdrkit_ft2_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ft2_demod)index;
  return code;
}
static inline int32_t sdrkit_ft2_set_demod(SDRKIT_HANDLE handle, sdrkit_ft2_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "ft2". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_ft2_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ft2_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "ft2". A value of `sdrkit_ft2_time_source`.
static inline int32_t sdrkit_ft2_get_time_source(SDRKIT_HANDLE handle, sdrkit_ft2_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ft2_time_source)index;
  return code;
}
static inline int32_t sdrkit_ft2_set_time_source(SDRKIT_HANDLE handle, sdrkit_ft2_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "ft2". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_ft2_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_ft2_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "ft2". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_ft2_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_ft2_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "dials" (Dials) of the stack "ft2". The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by ";", for example "ft8=14074000". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial.
static inline int32_t sdrkit_ft2_get_dials(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "dials", value);
}
static inline int32_t sdrkit_ft2_set_dials(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "dials", value);
}

// The setting "wide" (Decode the whole span) of the stack "ft2".
static inline int32_t sdrkit_ft2_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide", value);
}
static inline int32_t sdrkit_ft2_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide", value);
}

// The setting "wide_span" (Span) of the stack "ft2". A frequency in Hz.
static inline int32_t sdrkit_ft2_get_wide_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wide_span", value);
}
static inline int32_t sdrkit_ft2_set_wide_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wide_span", value);
}

// The setting "multiband" (Decode all selected at once) of the stack "ft2".
static inline int32_t sdrkit_ft2_get_multiband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "multiband", value);
}
static inline int32_t sdrkit_ft2_set_multiband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "multiband", value);
}

// The setting "multiband_select" (Bands and modes) of the stack "ft2". The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR.
static inline int32_t sdrkit_ft2_get_multiband_select(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "multiband_select", value);
}
static inline int32_t sdrkit_ft2_set_multiband_select(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "multiband_select", value);
}

// The setting "hop.enabled" (Band hopping) of the stack "ft2". At the start of each 2-minute slot, asks the host to tune to the dial of the band of the plan. Off while the whole span is decoded.
static inline int32_t sdrkit_ft2_get_hop_enabled(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.enabled", value);
}
static inline int32_t sdrkit_ft2_set_hop_enabled(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.enabled", value);
}

// The setting "hop.coordinated" (Coordinated hopping) of the stack "ft2". Every station takes the same band of the cycle in the same slot.
static inline int32_t sdrkit_ft2_get_hop_coordinated(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.coordinated", value);
}
static inline int32_t sdrkit_ft2_set_hop_coordinated(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value);
}

// The setting "hop.hours" (Bands of each hour) of the stack "ft2". 24 hexadecimal masks separated by commas, UTC hour 0 first. Bit b of a mask is band b of the band list.
static inline int32_t sdrkit_ft2_get_hop_hours(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.hours", value);
}
static inline int32_t sdrkit_ft2_set_hop_hours(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.hours", value);
}

// The setting "hop.bands" (Band list) of the stack "ft2". The band names in bit order, separated by commas (for example from data/wsjt/band_hopping.csv). Empty: no band.
static inline int32_t sdrkit_ft2_get_hop_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.bands", value);
}
static inline int32_t sdrkit_ft2_set_hop_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.bands", value);
}

// The setting "hop.cycle" (Coordinated cycle) of the stack "ft2". The bands of the coordinated cycle in their order, by the names of the band list.
static inline int32_t sdrkit_ft2_get_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.cycle", value);
}
static inline int32_t sdrkit_ft2_set_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.cycle", value);
}

// The setting "spots" (Spot events) of the stack "ft2". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_ft2_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_ft2_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "ft2".
static inline int32_t sdrkit_ft2_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_ft2_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "ft2".
static inline int32_t sdrkit_ft2_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_ft2_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "ft2".
static inline int32_t sdrkit_ft2_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_ft2_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "ft2". A frequency in Hz.
static inline int32_t sdrkit_ft2_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_ft2_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "ft2".
static inline int32_t sdrkit_ft2_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_ft2_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "ft2". A length of time in seconds.
static inline int32_t sdrkit_ft2_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_ft2_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "ft2".
static inline int32_t sdrkit_ft2_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_ft2_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "ft2".
static inline int32_t sdrkit_ft2_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_ft2_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "ft2".
static inline int32_t sdrkit_ft2_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_ft2_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "ft2". A frequency in Hz.
static inline int32_t sdrkit_ft2_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_ft2_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_decodes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_decode_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_spots_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wspr_spot_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_modes_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wsjt_mode_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_wsjt_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_js8_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_js8_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_js8_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_js8_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wide.multiband" of the stack "wspr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wspr_wide_multiband_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_wsjt_multiband_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.MultibandRow", "wide.multiband", rows, sizeof(sdrkit_wsjt_multiband_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "wspr" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_wspr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "wspr" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_wspr_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "beacon" (Start the WSPR beacon) of the stack "wspr" on the session of `handle`. Sends the call, locator and power of the station in the beacon slots. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_wspr_beacon(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "beacon", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "wspr" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_wspr_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wspr" are the sdrkit_wspr_gen_*() functions
// after the settings of its receiver.

// The setting "mirrored" (Upside down too) of the stack "wspr". Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR).
static inline int32_t sdrkit_wspr_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mirrored", value);
}
static inline int32_t sdrkit_wspr_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mirrored", value);
}

// The setting "deep" (Deep decoding) of the stack "wspr". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_wspr_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_wspr_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "wspr". A value of `sdrkit_wspr_demod`.
static inline int32_t sdrkit_wspr_get_demod(SDRKIT_HANDLE handle, sdrkit_wspr_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wspr_demod)index;
  return code;
}
static inline int32_t sdrkit_wspr_set_demod(SDRKIT_HANDLE handle, sdrkit_wspr_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "wspr". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_wspr_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_wspr_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "wspr". A value of `sdrkit_wspr_time_source`.
static inline int32_t sdrkit_wspr_get_time_source(SDRKIT_HANDLE handle, sdrkit_wspr_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wspr_time_source)index;
  return code;
}
static inline int32_t sdrkit_wspr_set_time_source(SDRKIT_HANDLE handle, sdrkit_wspr_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "wspr". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_wspr_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_wspr_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "wspr". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_wspr_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_wspr_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "dials" (Dials) of the stack "wspr". The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by ";", for example "ft8=14074000". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial.
static inline int32_t sdrkit_wspr_get_dials(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "dials", value);
}
static inline int32_t sdrkit_wspr_set_dials(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "dials", value);
}

// The setting "wide" (Decode the whole span) of the stack "wspr".
static inline int32_t sdrkit_wspr_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide", value);
}
static inline int32_t sdrkit_wspr_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide", value);
}

// The setting "wide_span" (Span) of the stack "wspr". A frequency in Hz.
static inline int32_t sdrkit_wspr_get_wide_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wide_span", value);
}
static inline int32_t sdrkit_wspr_set_wide_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wide_span", value);
}

// The setting "multiband" (Decode all selected at once) of the stack "wspr".
static inline int32_t sdrkit_wspr_get_multiband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "multiband", value);
}
static inline int32_t sdrkit_wspr_set_multiband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "multiband", value);
}

// The setting "multiband_select" (Bands and modes) of the stack "wspr". The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR.
static inline int32_t sdrkit_wspr_get_multiband_select(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "multiband_select", value);
}
static inline int32_t sdrkit_wspr_set_multiband_select(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "multiband_select", value);
}

// The setting "hop.enabled" (Band hopping) of the stack "wspr". At the start of each 2-minute slot, asks the host to tune to the dial of the band of the plan. Off while the whole span is decoded.
static inline int32_t sdrkit_wspr_get_hop_enabled(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.enabled", value);
}
static inline int32_t sdrkit_wspr_set_hop_enabled(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.enabled", value);
}

// The setting "hop.coordinated" (Coordinated hopping) of the stack "wspr". Every station takes the same band of the cycle in the same slot.
static inline int32_t sdrkit_wspr_get_hop_coordinated(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop.coordinated", value);
}
static inline int32_t sdrkit_wspr_set_hop_coordinated(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop.coordinated", value);
}

// The setting "hop.hours" (Bands of each hour) of the stack "wspr". 24 hexadecimal masks separated by commas, UTC hour 0 first. Bit b of a mask is band b of the band list.
static inline int32_t sdrkit_wspr_get_hop_hours(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.hours", value);
}
static inline int32_t sdrkit_wspr_set_hop_hours(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.hours", value);
}

// The setting "hop.bands" (Band list) of the stack "wspr". The band names in bit order, separated by commas (for example from data/wsjt/band_hopping.csv). Empty: no band.
static inline int32_t sdrkit_wspr_get_hop_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.bands", value);
}
static inline int32_t sdrkit_wspr_set_hop_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.bands", value);
}

// The setting "hop.cycle" (Coordinated cycle) of the stack "wspr". The bands of the coordinated cycle in their order, by the names of the band list.
static inline int32_t sdrkit_wspr_get_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hop.cycle", value);
}
static inline int32_t sdrkit_wspr_set_hop_cycle(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hop.cycle", value);
}

// The setting "spots" (Spot events) of the stack "wspr". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_wspr_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_wspr_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "beacon_dbm" (Power) of the stack "wspr". Unit: dBm. WSPR sends 0 to 60 dBm, ending in 0, 3 or 7.
static inline int32_t sdrkit_wspr_get_beacon_dbm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beacon_dbm", value);
}
static inline int32_t sdrkit_wspr_set_beacon_dbm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beacon_dbm", value);
}

// The setting "beacon_every" (One slot in N) of the stack "wspr".
static inline int32_t sdrkit_wspr_get_beacon_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beacon_every", value);
}
static inline int32_t sdrkit_wspr_set_beacon_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beacon_every", value);
}

// The setting "beacon_offset" (Offset) of the stack "wspr".
static inline int32_t sdrkit_wspr_get_beacon_offset(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beacon_offset", value);
}
static inline int32_t sdrkit_wspr_set_beacon_offset(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beacon_offset", value);
}

// The setting "beacon_hz" (Audio frequency) of the stack "wspr". A frequency in Hz.
static inline int32_t sdrkit_wspr_get_beacon_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "beacon_hz", value);
}
static inline int32_t sdrkit_wspr_set_beacon_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "beacon_hz", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "wspr".
static inline int32_t sdrkit_wspr_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_wspr_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "wspr".
static inline int32_t sdrkit_wspr_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_wspr_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "wspr".
static inline int32_t sdrkit_wspr_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_wspr_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "wspr". A frequency in Hz.
static inline int32_t sdrkit_wspr_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_wspr_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_decodes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_decode_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wspr_spot_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_modes_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wsjt_mode_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_wsjt_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_js8_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_js8_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_js8_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "jt65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt65_js8_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "jt65" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_jt65_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "jt65" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_jt65_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "jt65" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_jt65_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "jt65" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_jt65_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "jt65" are the sdrkit_jt65_gen_*() functions
// after the settings of its receiver.

// The setting "jt65_submode" (JT65 sub-mode) of the stack "jt65". A value of `sdrkit_jt65_submode`.
static inline int32_t sdrkit_jt65_get_jt65_submode(SDRKIT_HANDLE handle, sdrkit_jt65_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "jt65_submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt65_submode)index;
  return code;
}
static inline int32_t sdrkit_jt65_set_jt65_submode(SDRKIT_HANDLE handle, sdrkit_jt65_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "jt65_submode", (int32_t)value);
}

// The setting "deep" (Deep decoding) of the stack "jt65". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_jt65_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_jt65_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "jt65". A value of `sdrkit_jt65_demod`.
static inline int32_t sdrkit_jt65_get_demod(SDRKIT_HANDLE handle, sdrkit_jt65_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt65_demod)index;
  return code;
}
static inline int32_t sdrkit_jt65_set_demod(SDRKIT_HANDLE handle, sdrkit_jt65_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "jt65". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_jt65_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_jt65_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "jt65". A value of `sdrkit_jt65_time_source`.
static inline int32_t sdrkit_jt65_get_time_source(SDRKIT_HANDLE handle, sdrkit_jt65_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt65_time_source)index;
  return code;
}
static inline int32_t sdrkit_jt65_set_time_source(SDRKIT_HANDLE handle, sdrkit_jt65_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "jt65". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_jt65_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_jt65_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "jt65". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_jt65_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_jt65_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "jt65". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_jt65_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_jt65_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "jt65".
static inline int32_t sdrkit_jt65_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_jt65_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "jt65".
static inline int32_t sdrkit_jt65_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_jt65_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "jt65".
static inline int32_t sdrkit_jt65_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_jt65_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "jt65". A frequency in Hz.
static inline int32_t sdrkit_jt65_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_jt65_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "jt65".
static inline int32_t sdrkit_jt65_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_jt65_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "jt65". A length of time in seconds.
static inline int32_t sdrkit_jt65_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_jt65_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "jt65".
static inline int32_t sdrkit_jt65_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_jt65_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "jt65".
static inline int32_t sdrkit_jt65_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_jt65_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "jt65".
static inline int32_t sdrkit_jt65_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_jt65_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "jt65". A frequency in Hz.
static inline int32_t sdrkit_jt65_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_jt65_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_js8_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_js8_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_js8_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "jt9" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt9_js8_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "jt9" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_jt9_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "jt9" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_jt9_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "jt9" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_jt9_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "jt9" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_jt9_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "jt9" are the sdrkit_jt9_gen_*() functions
// after the settings of its receiver.

// The setting "deep" (Deep decoding) of the stack "jt9". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_jt9_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_jt9_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "jt9". A value of `sdrkit_jt9_demod`.
static inline int32_t sdrkit_jt9_get_demod(SDRKIT_HANDLE handle, sdrkit_jt9_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt9_demod)index;
  return code;
}
static inline int32_t sdrkit_jt9_set_demod(SDRKIT_HANDLE handle, sdrkit_jt9_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "jt9". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_jt9_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_jt9_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "jt9". A value of `sdrkit_jt9_time_source`.
static inline int32_t sdrkit_jt9_get_time_source(SDRKIT_HANDLE handle, sdrkit_jt9_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt9_time_source)index;
  return code;
}
static inline int32_t sdrkit_jt9_set_time_source(SDRKIT_HANDLE handle, sdrkit_jt9_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "jt9". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_jt9_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_jt9_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "jt9". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_jt9_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_jt9_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "jt9". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_jt9_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_jt9_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "jt9".
static inline int32_t sdrkit_jt9_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_jt9_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "jt9".
static inline int32_t sdrkit_jt9_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_jt9_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "jt9".
static inline int32_t sdrkit_jt9_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_jt9_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "jt9". A frequency in Hz.
static inline int32_t sdrkit_jt9_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_jt9_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "jt9".
static inline int32_t sdrkit_jt9_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_jt9_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "jt9". A length of time in seconds.
static inline int32_t sdrkit_jt9_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_jt9_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "jt9".
static inline int32_t sdrkit_jt9_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_jt9_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "jt9".
static inline int32_t sdrkit_jt9_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_jt9_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "jt9".
static inline int32_t sdrkit_jt9_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_jt9_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "jt9". A frequency in Hz.
static inline int32_t sdrkit_jt9_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_jt9_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_js8_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_js8_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_js8_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "jt4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt4_js8_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "jt4" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_jt4_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "jt4" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_jt4_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "cq" (Call CQ) of the stack "jt4" on the session of `handle`. Starts calling CQ in the next transmit slot. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_jt4_cq(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "cq", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "jt4" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_jt4_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "jt4" are the sdrkit_jt4_gen_*() functions
// after the settings of its receiver.

// The setting "jt4_submode" (JT4 sub-mode) of the stack "jt4". A value of `sdrkit_jt4_submode`.
static inline int32_t sdrkit_jt4_get_jt4_submode(SDRKIT_HANDLE handle, sdrkit_jt4_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "jt4_submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt4_submode)index;
  return code;
}
static inline int32_t sdrkit_jt4_set_jt4_submode(SDRKIT_HANDLE handle, sdrkit_jt4_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "jt4_submode", (int32_t)value);
}

// The setting "deep" (Deep decoding) of the stack "jt4". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_jt4_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_jt4_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "jt4". A value of `sdrkit_jt4_demod`.
static inline int32_t sdrkit_jt4_get_demod(SDRKIT_HANDLE handle, sdrkit_jt4_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt4_demod)index;
  return code;
}
static inline int32_t sdrkit_jt4_set_demod(SDRKIT_HANDLE handle, sdrkit_jt4_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "jt4". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_jt4_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_jt4_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "jt4". A value of `sdrkit_jt4_time_source`.
static inline int32_t sdrkit_jt4_get_time_source(SDRKIT_HANDLE handle, sdrkit_jt4_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt4_time_source)index;
  return code;
}
static inline int32_t sdrkit_jt4_set_time_source(SDRKIT_HANDLE handle, sdrkit_jt4_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "jt4". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_jt4_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_jt4_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "jt4". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_jt4_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_jt4_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "jt4". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_jt4_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_jt4_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "qso_even" (Transmit in even slots) of the stack "jt4".
static inline int32_t sdrkit_jt4_get_qso_even(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_even", value);
}
static inline int32_t sdrkit_jt4_set_qso_even(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_even", value);
}

// The setting "qso_auto" (Auto-sequence) of the stack "jt4".
static inline int32_t sdrkit_jt4_get_qso_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_auto", value);
}
static inline int32_t sdrkit_jt4_set_qso_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_auto", value);
}

// The setting "qso_hold" (Hold TX frequency) of the stack "jt4".
static inline int32_t sdrkit_jt4_get_qso_hold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "qso_hold", value);
}
static inline int32_t sdrkit_jt4_set_qso_hold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "qso_hold", value);
}

// The setting "qso_hz" (TX audio frequency) of the stack "jt4". A frequency in Hz.
static inline int32_t sdrkit_jt4_get_qso_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_hz", value);
}
static inline int32_t sdrkit_jt4_set_qso_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_hz", value);
}

// The setting "qso_retries" (Retry limit) of the stack "jt4".
static inline int32_t sdrkit_jt4_get_qso_retries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "qso_retries", value);
}
static inline int32_t sdrkit_jt4_set_qso_retries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "qso_retries", value);
}

// The setting "qso_watchdog" (Watchdog) of the stack "jt4". A length of time in seconds.
static inline int32_t sdrkit_jt4_get_qso_watchdog(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "qso_watchdog", value);
}
static inline int32_t sdrkit_jt4_set_qso_watchdog(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "qso_watchdog", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "jt4".
static inline int32_t sdrkit_jt4_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_jt4_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "jt4".
static inline int32_t sdrkit_jt4_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_jt4_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "jt4".
static inline int32_t sdrkit_jt4_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_jt4_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "jt4". A frequency in Hz.
static inline int32_t sdrkit_jt4_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_jt4_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_js8_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_js8_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_js8_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "q65" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_q65_js8_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "q65" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_q65_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "q65" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_q65_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "q65" are the sdrkit_q65_gen_*() functions
// after the settings of its receiver.

// The setting "q65_submode" (Q65 sub-mode) of the stack "q65". A value of `sdrkit_q65_submode`.
static inline int32_t sdrkit_q65_get_q65_submode(SDRKIT_HANDLE handle, sdrkit_q65_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "q65_submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_q65_submode)index;
  return code;
}
static inline int32_t sdrkit_q65_set_q65_submode(SDRKIT_HANDLE handle, sdrkit_q65_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "q65_submode", (int32_t)value);
}

// The setting "q65_period" (Q65 T/R period) of the stack "q65". A length of time in seconds. One of 15, 30, 60, 120, 300 s.
static inline int32_t sdrkit_q65_get_q65_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "q65_period", value);
}
static inline int32_t sdrkit_q65_set_q65_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "q65_period", value);
}

// The setting "deep" (Deep decoding) of the stack "q65". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_q65_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_q65_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "q65". A value of `sdrkit_q65_demod`.
static inline int32_t sdrkit_q65_get_demod(SDRKIT_HANDLE handle, sdrkit_q65_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_q65_demod)index;
  return code;
}
static inline int32_t sdrkit_q65_set_demod(SDRKIT_HANDLE handle, sdrkit_q65_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "q65". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_q65_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_q65_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "q65". A value of `sdrkit_q65_time_source`.
static inline int32_t sdrkit_q65_get_time_source(SDRKIT_HANDLE handle, sdrkit_q65_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_q65_time_source)index;
  return code;
}
static inline int32_t sdrkit_q65_set_time_source(SDRKIT_HANDLE handle, sdrkit_q65_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "q65". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_q65_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_q65_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "q65". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_q65_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_q65_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "q65". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_q65_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_q65_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "q65".
static inline int32_t sdrkit_q65_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_q65_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "q65".
static inline int32_t sdrkit_q65_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_q65_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "q65".
static inline int32_t sdrkit_q65_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_q65_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "q65". A frequency in Hz.
static inline int32_t sdrkit_q65_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_q65_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_decodes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_decode_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wspr_spot_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_modes_rows(SDRKIT_HANDLE handle,
                                            sdrkit_wsjt_mode_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_wsjt_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_js8_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_js8_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_js8_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "fst4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4_js8_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "fst4" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fst4_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "fst4" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_fst4_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fst4" are the sdrkit_fst4_gen_*() functions
// after the settings of its receiver.

// The setting "fst4_period" (FST4 T/R period) of the stack "fst4". A length of time in seconds. One of 15, 30, 60, 120, 300, 900, 1800 s.
static inline int32_t sdrkit_fst4_get_fst4_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fst4_period", value);
}
static inline int32_t sdrkit_fst4_set_fst4_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fst4_period", value);
}

// The setting "deep" (Deep decoding) of the stack "fst4". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_fst4_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_fst4_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "fst4". A value of `sdrkit_fst4_demod`.
static inline int32_t sdrkit_fst4_get_demod(SDRKIT_HANDLE handle, sdrkit_fst4_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fst4_demod)index;
  return code;
}
static inline int32_t sdrkit_fst4_set_demod(SDRKIT_HANDLE handle, sdrkit_fst4_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "fst4". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_fst4_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_fst4_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "fst4". A value of `sdrkit_fst4_time_source`.
static inline int32_t sdrkit_fst4_get_time_source(SDRKIT_HANDLE handle, sdrkit_fst4_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fst4_time_source)index;
  return code;
}
static inline int32_t sdrkit_fst4_set_time_source(SDRKIT_HANDLE handle, sdrkit_fst4_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "fst4". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_fst4_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_fst4_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "fst4". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_fst4_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_fst4_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "fst4". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_fst4_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_fst4_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "fst4".
static inline int32_t sdrkit_fst4_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_fst4_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "fst4".
static inline int32_t sdrkit_fst4_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_fst4_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "fst4".
static inline int32_t sdrkit_fst4_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_fst4_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "fst4". A frequency in Hz.
static inline int32_t sdrkit_fst4_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_fst4_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_decodes_rows(SDRKIT_HANDLE handle,
                                               sdrkit_wsjt_decode_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_wspr_spot_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_modes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_mode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_stations_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wsjt_station_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_js8_frames_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_js8_frame_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_js8_messages_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_js8_message_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "fst4w" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fst4w_js8_stations_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_js8_station_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "fst4w" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fst4w_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "fst4w" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_fst4w_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fst4w" are the sdrkit_fst4w_gen_*() functions
// after the settings of its receiver.

// The setting "fst4w_period" (FST4W T/R period) of the stack "fst4w". A length of time in seconds. One of 15, 30, 60, 120, 300, 900, 1800 s.
static inline int32_t sdrkit_fst4w_get_fst4w_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fst4w_period", value);
}
static inline int32_t sdrkit_fst4w_set_fst4w_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fst4w_period", value);
}

// The setting "deep" (Deep decoding) of the stack "fst4w". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_fst4w_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_fst4w_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "fst4w". A value of `sdrkit_fst4w_demod`.
static inline int32_t sdrkit_fst4w_get_demod(SDRKIT_HANDLE handle, sdrkit_fst4w_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fst4w_demod)index;
  return code;
}
static inline int32_t sdrkit_fst4w_set_demod(SDRKIT_HANDLE handle, sdrkit_fst4w_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "fst4w". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_fst4w_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_fst4w_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "fst4w". A value of `sdrkit_fst4w_time_source`.
static inline int32_t sdrkit_fst4w_get_time_source(SDRKIT_HANDLE handle, sdrkit_fst4w_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fst4w_time_source)index;
  return code;
}
static inline int32_t sdrkit_fst4w_set_time_source(SDRKIT_HANDLE handle, sdrkit_fst4w_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "fst4w". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_fst4w_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_fst4w_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "fst4w". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_fst4w_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_fst4w_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "fst4w". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_fst4w_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_fst4w_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "fst4w".
static inline int32_t sdrkit_fst4w_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_fst4w_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "fst4w".
static inline int32_t sdrkit_fst4w_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_fst4w_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "fst4w".
static inline int32_t sdrkit_fst4w_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_fst4w_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "fst4w". A frequency in Hz.
static inline int32_t sdrkit_fst4w_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_fst4w_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_decodes_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wsjt_decode_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_wspr_spot_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_modes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_wsjt_mode_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_stations_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wsjt_station_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_js8_frames_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_js8_frame_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_js8_messages_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_js8_message_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "msk144" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msk144_js8_stations_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_js8_station_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "msk144" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_msk144_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "msk144" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_msk144_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "msk144" are the sdrkit_msk144_gen_*() functions
// after the settings of its receiver.

// The setting "msk144_period" (MSK144 T/R period) of the stack "msk144". A length of time in seconds. One of 15, 30 s.
static inline int32_t sdrkit_msk144_get_msk144_period(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "msk144_period", value);
}
static inline int32_t sdrkit_msk144_set_msk144_period(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "msk144_period", value);
}

// The setting "deep" (Deep decoding) of the stack "msk144". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_msk144_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_msk144_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "msk144". A value of `sdrkit_msk144_demod`.
static inline int32_t sdrkit_msk144_get_demod(SDRKIT_HANDLE handle, sdrkit_msk144_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_msk144_demod)index;
  return code;
}
static inline int32_t sdrkit_msk144_set_demod(SDRKIT_HANDLE handle, sdrkit_msk144_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "msk144". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_msk144_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_msk144_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "msk144". A value of `sdrkit_msk144_time_source`.
static inline int32_t sdrkit_msk144_get_time_source(SDRKIT_HANDLE handle, sdrkit_msk144_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_msk144_time_source)index;
  return code;
}
static inline int32_t sdrkit_msk144_set_time_source(SDRKIT_HANDLE handle, sdrkit_msk144_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "msk144". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_msk144_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_msk144_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "msk144". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_msk144_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_msk144_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "msk144". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_msk144_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_msk144_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "msk144".
static inline int32_t sdrkit_msk144_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_msk144_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "msk144".
static inline int32_t sdrkit_msk144_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_msk144_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "msk144".
static inline int32_t sdrkit_msk144_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_msk144_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "msk144". A frequency in Hz.
static inline int32_t sdrkit_msk144_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_msk144_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_wsjt_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.DecodeRow", "decodes", rows, sizeof(sdrkit_wsjt_decode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "wspr_spots" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_wspr_spots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_wspr_spot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.WsprSpotRow", "wspr_spots", rows, sizeof(sdrkit_wspr_spot_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modes" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_modes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_wsjt_mode_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.ModeRow", "modes", rows, sizeof(sdrkit_wsjt_mode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_wsjt_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_wsjt_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.StationRow", "stations", rows, sizeof(sdrkit_wsjt_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_frames" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_frames_rows(SDRKIT_HANDLE handle,
                                            sdrkit_js8_frame_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8FrameRow", "js8_frames", rows, sizeof(sdrkit_js8_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_messages" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_js8_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8MessageRow", "js8_messages", rows, sizeof(sdrkit_js8_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "js8_stations" of the stack "js8" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_js8_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_js8_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Js8StationRow", "js8_stations", rows, sizeof(sdrkit_js8_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "js8" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_js8_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "js8" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_js8_remove(SDRKIT_HANDLE handle, const sdrkit_wsjt_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "wsjt.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "js8" on the session of `handle`. Queues a JS8 message. My call and locator come from the station. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_js8_send(SDRKIT_HANDLE handle, const sdrkit_wsjt_send_args *args) {
  return sdrkit_typed_command(handle, "send", "wsjt.SendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "js8" on the session of `handle`. Stops transmitting at once.
static inline int32_t sdrkit_js8_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "js8" are the sdrkit_js8_gen_*() functions
// after the settings of its receiver.

// The setting "js8_normal" (JS8 Normal (15 s)) of the stack "js8".
static inline int32_t sdrkit_js8_get_js8_normal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "js8_normal", value);
}
static inline int32_t sdrkit_js8_set_js8_normal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "js8_normal", value);
}

// The setting "js8_fast" (JS8 Fast (10 s)) of the stack "js8".
static inline int32_t sdrkit_js8_get_js8_fast(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "js8_fast", value);
}
static inline int32_t sdrkit_js8_set_js8_fast(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "js8_fast", value);
}

// The setting "js8_turbo" (JS8 Turbo (6 s)) of the stack "js8".
static inline int32_t sdrkit_js8_get_js8_turbo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "js8_turbo", value);
}
static inline int32_t sdrkit_js8_set_js8_turbo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "js8_turbo", value);
}

// The setting "deep" (Deep decoding) of the stack "js8". A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band.
static inline int32_t sdrkit_js8_get_deep(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deep", value);
}
static inline int32_t sdrkit_js8_set_deep(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deep", value);
}

// The setting "demod" (Demodulator) of the stack "js8". A value of `sdrkit_js8_demod`.
static inline int32_t sdrkit_js8_get_demod(SDRKIT_HANDLE handle, sdrkit_js8_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_js8_demod)index;
  return code;
}
static inline int32_t sdrkit_js8_set_demod(SDRKIT_HANDLE handle, sdrkit_js8_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "js8". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_js8_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_js8_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "time_source" (Slot clock) of the stack "js8". A value of `sdrkit_js8_time_source`.
static inline int32_t sdrkit_js8_get_time_source(SDRKIT_HANDLE handle, sdrkit_js8_time_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "time_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_js8_time_source)index;
  return code;
}
static inline int32_t sdrkit_js8_set_time_source(SDRKIT_HANDLE handle, sdrkit_js8_time_source value) {
  return sdrkit_typed_setting_set_enum(handle, "time_source", (int32_t)value);
}

// The setting "time_start" (Recording start (UTC)) of the stack "js8". An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.
static inline int32_t sdrkit_js8_get_time_start(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "time_start", value);
}
static inline int32_t sdrkit_js8_set_time_start(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "time_start", value);
}

// The setting "band_edges" (Band edges) of the stack "js8". The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.
static inline int32_t sdrkit_js8_get_band_edges(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_edges", value);
}
static inline int32_t sdrkit_js8_set_band_edges(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_edges", value);
}

// The setting "spots" (Spot events) of the stack "js8". Makes a spot event of every decode with a calling station, for the host to forward.
static inline int32_t sdrkit_js8_get_spots(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spots", value);
}
static inline int32_t sdrkit_js8_set_spots(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spots", value);
}

// The setting "js8_speed" (Speed) of the stack "js8". A value of `sdrkit_js8_speed`.
static inline int32_t sdrkit_js8_get_js8_speed(SDRKIT_HANDLE handle, sdrkit_js8_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "js8_speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_js8_speed)index;
  return code;
}
static inline int32_t sdrkit_js8_set_js8_speed(SDRKIT_HANDLE handle, sdrkit_js8_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "js8_speed", (int32_t)value);
}

// The setting "js8_hz" (Audio frequency) of the stack "js8". A frequency in Hz.
static inline int32_t sdrkit_js8_get_js8_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "js8_hz", value);
}
static inline int32_t sdrkit_js8_set_js8_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "js8_hz", value);
}

// The setting "gen_mirrored" (Test signal upside down) of the test signal generator of the stack "js8".
static inline int32_t sdrkit_js8_gen_get_mirrored(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_mirrored", value);
}
static inline int32_t sdrkit_js8_gen_set_mirrored(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_mirrored", value);
}

// The setting "gen_live" (Test signal in step with the clock) of the test signal generator of the stack "js8".
static inline int32_t sdrkit_js8_gen_get_live(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_live", value);
}
static inline int32_t sdrkit_js8_gen_set_live(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_live", value);
}

// The setting "gen_single" (Single station message) of the test signal generator of the stack "js8".
static inline int32_t sdrkit_js8_gen_get_single(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_single", value);
}
static inline int32_t sdrkit_js8_gen_set_single(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_single", value);
}

// The setting "gen_hz" (Single station frequency) of the test signal generator of the stack "js8". A frequency in Hz.
static inline int32_t sdrkit_js8_gen_get_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_hz", value);
}
static inline int32_t sdrkit_js8_gen_set_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_hz", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "fsk441" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsk441_decodes_rows(SDRKIT_HANDLE handle,
                                                sdrkit_fsk441_decode_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Fsk441DecodeRow", "decodes", rows, sizeof(sdrkit_fsk441_decode_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.decode"
// (Decode). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fsk441_decode_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fsk441_decode_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.decode", "wsjt.Fsk441DecodeEvent", event, sizeof(sdrkit_fsk441_decode_event));
}

// Runs the command "clear" (Clear) of the stack "fsk441" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fsk441_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fsk441" are the sdrkit_fsk441_gen_*() functions
// after the settings of its receiver.

// The setting "tol_hz" (Frequency tolerance) of the stack "fsk441". A frequency in Hz. How far the tones may lie from 882 to 2205 Hz.
static inline int32_t sdrkit_fsk441_get_tol_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tol_hz", value);
}
static inline int32_t sdrkit_fsk441_set_tol_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tol_hz", value);
}

// The setting "burst_db" (Burst threshold) of the stack "fsk441". Unit: dB. A frame of audio this far above the noise starts a burst.
static inline int32_t sdrkit_fsk441_get_burst_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "burst_db", value);
}
static inline int32_t sdrkit_fsk441_set_burst_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "burst_db", value);
}

// The setting "demod" (Demodulator) of the stack "fsk441". A value of `sdrkit_fsk441_demod`.
static inline int32_t sdrkit_fsk441_get_demod(SDRKIT_HANDLE handle, sdrkit_fsk441_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk441_demod)index;
  return code;
}
static inline int32_t sdrkit_fsk441_set_demod(SDRKIT_HANDLE handle, sdrkit_fsk441_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "fsk441". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_fsk441_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_fsk441_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_text" (Test text) of the test signal generator of the stack "fsk441". The message of the test signal. Empty: the stack's own test text.
static inline int32_t sdrkit_fsk441_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_fsk441_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "jt6m" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jt6m_decodes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_jt6m_decode_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Jt6mDecodeRow", "decodes", rows, sizeof(sdrkit_jt6m_decode_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.decode"
// (Ping). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_jt6m_decode_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_jt6m_decode_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.decode", "wsjt.Jt6mDecodeEvent", event, sizeof(sdrkit_jt6m_decode_event));
}

// Runs the command "clear" (Clear) of the stack "jt6m" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_jt6m_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "jt6m" are the sdrkit_jt6m_gen_*() functions
// after the settings of its receiver.

// The setting "tol_hz" (Frequency tolerance) of the stack "jt6m". A frequency in Hz. How far the sync tone may lie from 1076.66 Hz.
static inline int32_t sdrkit_jt6m_get_tol_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tol_hz", value);
}
static inline int32_t sdrkit_jt6m_set_tol_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tol_hz", value);
}

// The setting "demod" (Demodulator) of the stack "jt6m". A value of `sdrkit_jt6m_demod`.
static inline int32_t sdrkit_jt6m_get_demod(SDRKIT_HANDLE handle, sdrkit_jt6m_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_jt6m_demod)index;
  return code;
}
static inline int32_t sdrkit_jt6m_set_demod(SDRKIT_HANDLE handle, sdrkit_jt6m_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "jt6m". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_jt6m_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_jt6m_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_text" (Test text) of the test signal generator of the stack "jt6m". The message of the test signal. Empty: the stack's own test text.
static inline int32_t sdrkit_jt6m_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_jt6m_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// The setting "gen_continuous" (Continuous test signal) of the test signal generator of the stack "jt6m". Off: pings of 0.8, 1.6 and 0.6 s in a loop of 24 s. On: the message without a break.
static inline int32_t sdrkit_jt6m_gen_get_continuous(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_continuous", value);
}
static inline int32_t sdrkit_jt6m_gen_set_continuous(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_continuous", value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "iscat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_iscat_decodes_rows(SDRKIT_HANDLE handle,
                                               sdrkit_iscat_decode_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.IscatDecodeRow", "decodes", rows, sizeof(sdrkit_iscat_decode_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.decode"
// (Decode). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_iscat_decode_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_iscat_decode_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.decode", "wsjt.IscatDecodeEvent", event, sizeof(sdrkit_iscat_decode_event));
}

// Runs the command "clear" (Clear) of the stack "iscat" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_iscat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "iscat" are the sdrkit_iscat_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Variant) of the stack "iscat". A value of `sdrkit_iscat_submode`. Automatic tries ISCAT-A and ISCAT-B on each reading and keeps the stronger sync.
static inline int32_t sdrkit_iscat_get_submode(SDRKIT_HANDLE handle, sdrkit_iscat_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_iscat_submode)index;
  return code;
}
static inline int32_t sdrkit_iscat_set_submode(SDRKIT_HANDLE handle, sdrkit_iscat_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "tol_hz" (Frequency tolerance) of the stack "iscat". A frequency in Hz. How far tone 0 may lie from its nominal frequency.
static inline int32_t sdrkit_iscat_get_tol_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tol_hz", value);
}
static inline int32_t sdrkit_iscat_set_tol_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tol_hz", value);
}

// The setting "demod" (Demodulator) of the stack "iscat". A value of `sdrkit_iscat_demod`.
static inline int32_t sdrkit_iscat_get_demod(SDRKIT_HANDLE handle, sdrkit_iscat_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_iscat_demod)index;
  return code;
}
static inline int32_t sdrkit_iscat_set_demod(SDRKIT_HANDLE handle, sdrkit_iscat_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "iscat". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_iscat_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_iscat_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_text" (Test text) of the test signal generator of the stack "iscat". The message of the test signal. Empty: the stack's own test text.
static inline int32_t sdrkit_iscat_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_iscat_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// The setting "gen_variant" (Test signal variant) of the test signal generator of the stack "iscat". A value of `sdrkit_iscat_gen_variant`.
static inline int32_t sdrkit_iscat_gen_get_variant(SDRKIT_HANDLE handle, sdrkit_iscat_gen_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_iscat_gen_variant)index;
  return code;
}
static inline int32_t sdrkit_iscat_gen_set_variant(SDRKIT_HANDLE handle, sdrkit_iscat_gen_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_variant", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "decodes" of the stack "pi4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pi4_decodes_rows(SDRKIT_HANDLE handle,
                                             sdrkit_pi4_decode_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "wsjt.Pi4DecodeRow", "decodes", rows, sizeof(sdrkit_pi4_decode_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wsjt.decode"
// (Decode). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_pi4_decode_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_pi4_decode_event *event) {
  return sdrkit_typed_event(handle, n, "wsjt.decode", "wsjt.Pi4DecodeEvent", event, sizeof(sdrkit_pi4_decode_event));
}

// Runs the command "clear" (Clear) of the stack "pi4" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_pi4_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "pi4" are the sdrkit_pi4_gen_*() functions
// after the settings of its receiver.

// The setting "k" (Tone spacing factor) of the stack "pi4". A value of `sdrkit_pi4_k`. PI4, PI4-80, PI4-96 or PI4-120. Which one a beacon uses depends on its band and channel spacing.
static inline int32_t sdrkit_pi4_get_k(SDRKIT_HANDLE handle, sdrkit_pi4_k *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "k", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pi4_k)index;
  return code;
}
static inline int32_t sdrkit_pi4_set_k(SDRKIT_HANDLE handle, sdrkit_pi4_k value) {
  return sdrkit_typed_setting_set_enum(handle, "k", (int32_t)value);
}

// The setting "f_lo_hz" (Lowest tone-0 frequency) of the stack "pi4". A frequency in Hz.
static inline int32_t sdrkit_pi4_get_f_lo_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "f_lo_hz", value);
}
static inline int32_t sdrkit_pi4_set_f_lo_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "f_lo_hz", value);
}

// The setting "f_hi_hz" (Highest tone-0 frequency) of the stack "pi4". A frequency in Hz. Clipped so that tone 3 lies below 3800 Hz.
static inline int32_t sdrkit_pi4_get_f_hi_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "f_hi_hz", value);
}
static inline int32_t sdrkit_pi4_set_f_hi_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "f_hi_hz", value);
}

// The setting "demod" (Demodulator) of the stack "pi4". A value of `sdrkit_pi4_demod`.
static inline int32_t sdrkit_pi4_get_demod(SDRKIT_HANDLE handle, sdrkit_pi4_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pi4_demod)index;
  return code;
}
static inline int32_t sdrkit_pi4_set_demod(SDRKIT_HANDLE handle, sdrkit_pi4_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "pi4". Copies the 8 kHz audio to the speaker. Decoding is the same either way.
static inline int32_t sdrkit_pi4_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_pi4_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_text" (Test text) of the test signal generator of the stack "pi4". The message of the test signal. Empty: the stack's own test text.
static inline int32_t sdrkit_pi4_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_pi4_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// The setting "gen_k" (Test signal tone spacing factor) of the test signal generator of the stack "pi4". A value of `sdrkit_pi4_gen_k`.
static inline int32_t sdrkit_pi4_gen_get_k(SDRKIT_HANDLE handle, sdrkit_pi4_gen_k *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_k", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pi4_gen_k)index;
  return code;
}
static inline int32_t sdrkit_pi4_gen_set_k(SDRKIT_HANDLE handle, sdrkit_pi4_gen_k value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_k", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wsjt_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.WsjtFields", fields, sizeof(sdrkit_wsjt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wsjt_scatter_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_scatter_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.ScatterFields", fields, sizeof(sdrkit_wsjt_scatter_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fsk441_fields_read(SDRKIT_HANDLE handle, sdrkit_fsk441_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.Fsk441Fields", fields, sizeof(sdrkit_fsk441_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wsjt_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.DecoderFields", fields, sizeof(sdrkit_wsjt_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_jt6m_fields_read(SDRKIT_HANDLE handle, sdrkit_jt6m_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.Jt6mFields", fields, sizeof(sdrkit_jt6m_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_iscat_fields_read(SDRKIT_HANDLE handle, sdrkit_iscat_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.IscatFields", fields, sizeof(sdrkit_iscat_fields));
}

// Fills `fields` with the result fields of the component "dec_ft8" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_ft8.<key>".
static inline int32_t sdrkit_wsjt_dec_ft8_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_ft8", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_ft4" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_ft4.<key>".
static inline int32_t sdrkit_wsjt_dec_ft4_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_ft4", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_ft2" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_ft2.<key>".
static inline int32_t sdrkit_wsjt_dec_ft2_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_ft2", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_wspr" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_wspr.<key>".
static inline int32_t sdrkit_wsjt_dec_wspr_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_wspr", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_jt65" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_jt65.<key>".
static inline int32_t sdrkit_wsjt_dec_jt65_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_jt65", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_jt9" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_jt9.<key>".
static inline int32_t sdrkit_wsjt_dec_jt9_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_jt9", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_jt4" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_jt4.<key>".
static inline int32_t sdrkit_wsjt_dec_jt4_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_jt4", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_q65" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_q65.<key>".
static inline int32_t sdrkit_wsjt_dec_q65_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_q65", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_fst4" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_fst4.<key>".
static inline int32_t sdrkit_wsjt_dec_fst4_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_fst4", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_fst4w" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_fst4w.<key>".
static inline int32_t sdrkit_wsjt_dec_fst4w_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_fst4w", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_msk144" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_msk144.<key>".
static inline int32_t sdrkit_wsjt_dec_msk144_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_msk144", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "dec_js8" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_js8.<key>".
static inline int32_t sdrkit_wsjt_dec_js8_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_js8", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "wide" of the stack "wsjt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "wide.<key>".
static inline int32_t sdrkit_wsjt_wide_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_wide_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.WideFields@wide", fields, sizeof(sdrkit_wsjt_wide_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "wsjt" in the
// snapshot of `handle`.
static inline int32_t sdrkit_wsjt_dropped_read(SDRKIT_HANDLE handle, sdrkit_wsjt_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.wsjt.dropped", dropped, sizeof(sdrkit_wsjt_dropped));
}

// Fills `fields` with the result fields of the component "dec_ft8" of the stack "ft8" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_ft8.<key>".
static inline int32_t sdrkit_ft8_dec_ft8_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_ft8", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "wide" of the stack "ft8" in the snapshot of
// `handle`. A generic consumer reads them under the keys "wide.<key>".
static inline int32_t sdrkit_ft8_wide_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_wide_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.WideFields@wide", fields, sizeof(sdrkit_wsjt_wide_fields));
}

// Fills `fields` with the result fields of the component "hop" of the stack "ft8" in the snapshot of
// `handle`. A generic consumer reads them under the keys "hop.<key>".
static inline int32_t sdrkit_ft8_hop_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_hop_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.HopFields@hop", fields, sizeof(sdrkit_wsjt_hop_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ft8" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ft8_dropped_read(SDRKIT_HANDLE handle, sdrkit_ft8_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.ft8.dropped", dropped, sizeof(sdrkit_ft8_dropped));
}

// Fills `fields` with the result fields of the component "dec_ft4" of the stack "ft4" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_ft4.<key>".
static inline int32_t sdrkit_ft4_dec_ft4_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_ft4", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "wide" of the stack "ft4" in the snapshot of
// `handle`. A generic consumer reads them under the keys "wide.<key>".
static inline int32_t sdrkit_ft4_wide_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_wide_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.WideFields@wide", fields, sizeof(sdrkit_wsjt_wide_fields));
}

// Fills `fields` with the result fields of the component "hop" of the stack "ft4" in the snapshot of
// `handle`. A generic consumer reads them under the keys "hop.<key>".
static inline int32_t sdrkit_ft4_hop_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_hop_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.HopFields@hop", fields, sizeof(sdrkit_wsjt_hop_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ft4" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ft4_dropped_read(SDRKIT_HANDLE handle, sdrkit_ft4_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.ft4.dropped", dropped, sizeof(sdrkit_ft4_dropped));
}

// Fills `fields` with the result fields of the component "dec_ft2" of the stack "ft2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_ft2.<key>".
static inline int32_t sdrkit_ft2_dec_ft2_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_ft2", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "wide" of the stack "ft2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "wide.<key>".
static inline int32_t sdrkit_ft2_wide_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_wide_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.WideFields@wide", fields, sizeof(sdrkit_wsjt_wide_fields));
}

// Fills `fields` with the result fields of the component "hop" of the stack "ft2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "hop.<key>".
static inline int32_t sdrkit_ft2_hop_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_hop_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.HopFields@hop", fields, sizeof(sdrkit_wsjt_hop_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ft2" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ft2_dropped_read(SDRKIT_HANDLE handle, sdrkit_ft2_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.ft2.dropped", dropped, sizeof(sdrkit_ft2_dropped));
}

// Fills `fields` with the result fields of the component "dec_wspr" of the stack "wspr" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_wspr.<key>".
static inline int32_t sdrkit_wspr_dec_wspr_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_wspr", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `fields` with the result fields of the component "wide" of the stack "wspr" in the snapshot of
// `handle`. A generic consumer reads them under the keys "wide.<key>".
static inline int32_t sdrkit_wspr_wide_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_wide_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.WideFields@wide", fields, sizeof(sdrkit_wsjt_wide_fields));
}

// Fills `fields` with the result fields of the component "hop" of the stack "wspr" in the snapshot of
// `handle`. A generic consumer reads them under the keys "hop.<key>".
static inline int32_t sdrkit_wspr_hop_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_hop_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.HopFields@hop", fields, sizeof(sdrkit_wsjt_hop_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "wspr" in the
// snapshot of `handle`.
static inline int32_t sdrkit_wspr_dropped_read(SDRKIT_HANDLE handle, sdrkit_wspr_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.wspr.dropped", dropped, sizeof(sdrkit_wspr_dropped));
}

// Fills `fields` with the result fields of the component "dec_jt65" of the stack "jt65" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_jt65.<key>".
static inline int32_t sdrkit_jt65_dec_jt65_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_jt65", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "jt65" in the
// snapshot of `handle`.
static inline int32_t sdrkit_jt65_dropped_read(SDRKIT_HANDLE handle, sdrkit_jt65_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.jt65.dropped", dropped, sizeof(sdrkit_jt65_dropped));
}

// Fills `fields` with the result fields of the component "dec_jt9" of the stack "jt9" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_jt9.<key>".
static inline int32_t sdrkit_jt9_dec_jt9_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_jt9", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "jt9" in the
// snapshot of `handle`.
static inline int32_t sdrkit_jt9_dropped_read(SDRKIT_HANDLE handle, sdrkit_jt9_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.jt9.dropped", dropped, sizeof(sdrkit_jt9_dropped));
}

// Fills `fields` with the result fields of the component "dec_jt4" of the stack "jt4" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_jt4.<key>".
static inline int32_t sdrkit_jt4_dec_jt4_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_jt4", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "jt4" in the
// snapshot of `handle`.
static inline int32_t sdrkit_jt4_dropped_read(SDRKIT_HANDLE handle, sdrkit_jt4_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.jt4.dropped", dropped, sizeof(sdrkit_jt4_dropped));
}

// Fills `fields` with the result fields of the component "dec_q65" of the stack "q65" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_q65.<key>".
static inline int32_t sdrkit_q65_dec_q65_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_q65", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "q65" in the
// snapshot of `handle`.
static inline int32_t sdrkit_q65_dropped_read(SDRKIT_HANDLE handle, sdrkit_q65_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.q65.dropped", dropped, sizeof(sdrkit_q65_dropped));
}

// Fills `fields` with the result fields of the component "dec_fst4" of the stack "fst4" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_fst4.<key>".
static inline int32_t sdrkit_fst4_dec_fst4_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_fst4", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "fst4" in the
// snapshot of `handle`.
static inline int32_t sdrkit_fst4_dropped_read(SDRKIT_HANDLE handle, sdrkit_fst4_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.fst4.dropped", dropped, sizeof(sdrkit_fst4_dropped));
}

// Fills `fields` with the result fields of the component "dec_fst4w" of the stack "fst4w" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_fst4w.<key>".
static inline int32_t sdrkit_fst4w_dec_fst4w_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_fst4w", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "fst4w" in the
// snapshot of `handle`.
static inline int32_t sdrkit_fst4w_dropped_read(SDRKIT_HANDLE handle, sdrkit_fst4w_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.fst4w.dropped", dropped, sizeof(sdrkit_fst4w_dropped));
}

// Fills `fields` with the result fields of the component "dec_msk144" of the stack "msk144" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_msk144.<key>".
static inline int32_t sdrkit_msk144_dec_msk144_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_msk144", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "msk144" in the
// snapshot of `handle`.
static inline int32_t sdrkit_msk144_dropped_read(SDRKIT_HANDLE handle, sdrkit_msk144_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.msk144.dropped", dropped, sizeof(sdrkit_msk144_dropped));
}

// Fills `fields` with the result fields of the component "dec_js8" of the stack "js8" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dec_js8.<key>".
static inline int32_t sdrkit_js8_dec_js8_slot_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_wsjt_slot_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "wsjt.SlotDecoderFields@dec_js8", fields, sizeof(sdrkit_wsjt_slot_decoder_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "js8" in the
// snapshot of `handle`.
static inline int32_t sdrkit_js8_dropped_read(SDRKIT_HANDLE handle, sdrkit_js8_dropped *dropped) {
  return sdrkit_typed_fields(handle, "wsjt.js8.dropped", dropped, sizeof(sdrkit_js8_dropped));
}

#endif
