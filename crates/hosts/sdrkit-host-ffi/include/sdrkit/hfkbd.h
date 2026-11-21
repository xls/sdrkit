/*
 * sdrkit: the typed interface of the stack family "hfkbd", version 2.6. MIT licence.
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

#ifndef SDRKIT_HFKBD_H
#define SDRKIT_HFKBD_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_HFKBD_VERSION_MAJOR 2
#define SDRKIT_HFKBD_VERSION_MINOR 6

// The stack "rtty": its id, and the keys of its event types.
#define SDRKIT_STACK_RTTY "rtty"
#define SDRKIT_EVENT_HFKBD_SPOT "hfkbd.spot"
#define SDRKIT_EVENT_HFKBD_TEXT "hfkbd.text"
#define SDRKIT_EVENT_HFKBD_TX "hfkbd.tx"

// The stack "psk": its id, and the keys of its event types.
#define SDRKIT_STACK_PSK "psk"

// The stack "cw": its id, and the keys of its event types.
#define SDRKIT_STACK_CW "cw"

// The stack "olivia": its id, and the keys of its event types.
#define SDRKIT_STACK_OLIVIA "olivia"

// The stack "contestia": its id, and the keys of its event types.
#define SDRKIT_STACK_CONTESTIA "contestia"

// The stack "mfsk": its id, and the keys of its event types.
#define SDRKIT_STACK_MFSK "mfsk"
#define SDRKIT_EVENT_MFSK_PICTURE "mfsk.picture"

// The stack "dominoex": its id, and the keys of its event types.
#define SDRKIT_STACK_DOMINOEX "dominoex"

// The stack "thor": its id, and the keys of its event types.
#define SDRKIT_STACK_THOR "thor"

// The stack "hell": its id, and the keys of its event types.
#define SDRKIT_STACK_HELL "hell"

// The stack "mt63": its id, and the keys of its event types.
#define SDRKIT_STACK_MT63 "mt63"

// The stack "fsq": its id, and the keys of its event types.
#define SDRKIT_STACK_FSQ "fsq"
#define SDRKIT_EVENT_FSQ_MESSAGE "fsq.message"

// The stack "ifkp": its id, and the keys of its event types.
#define SDRKIT_STACK_IFKP "ifkp"

// The stack "sstv": its id, and the keys of its event types.
#define SDRKIT_STACK_SSTV "sstv"
#define SDRKIT_EVENT_SSTV_PICTURE "sstv.picture"
#define SDRKIT_EVENT_SSTV_TX "sstv.tx"

// The stack "sitor": its id, and the keys of its event types.
#define SDRKIT_STACK_SITOR "sitor"
#define SDRKIT_EVENT_SITOR_ARQ "sitor.arq"

// The stack "ardop": its id, and the keys of its event types.
#define SDRKIT_STACK_ARDOP "ardop"
#define SDRKIT_EVENT_ARDOP_FRAME "ardop.frame"
#define SDRKIT_EVENT_ARDOP_LINK "ardop.link"

// The stack "ros": its id, and the keys of its event types.
#define SDRKIT_STACK_ROS "ros"
#define SDRKIT_EVENT_ROS_DETECTION "ros.detection"

// The stack "throb": its id, and the keys of its event types.
#define SDRKIT_STACK_THROB "throb"
#define SDRKIT_EVENT_THROB_TEXT "throb.text"

// The stack "qrss": its id, and the keys of its event types.
#define SDRKIT_STACK_QRSS "qrss"
#define SDRKIT_EVENT_QRSS_TEXT "qrss.text"

// The stack "cp16": its id, and the keys of its event types.
#define SDRKIT_STACK_CP16 "cp16"
#define SDRKIT_EVENT_CP16_CHAR "cp16.char"
#define SDRKIT_EVENT_CP16_TX "cp16.tx"

// The stack "winmor": its id, and the keys of its event types.
#define SDRKIT_STACK_WINMOR "winmor"
#define SDRKIT_EVENT_WINMOR_PILOT "winmor.pilot"

// The options of the setting "baud" of the stack "rtty".
typedef enum sdrkit_rtty_baud {
  // 45.45 Bd
  SDRKIT_RTTY_BAUD_45_45 = 0,
  // 50 Bd
  SDRKIT_RTTY_BAUD_50_BD = 1,
  // 75 Bd
  SDRKIT_RTTY_BAUD_75_BD = 2,
  // 100 Bd
  SDRKIT_RTTY_BAUD_100_BD = 3,
} sdrkit_rtty_baud;

// The options of the setting "shift" of the stack "rtty".
typedef enum sdrkit_rtty_shift {
  // 170 Hz
  SDRKIT_RTTY_SHIFT_170_HZ = 0,
  // 200 Hz
  SDRKIT_RTTY_SHIFT_200_HZ = 1,
  // 425 Hz
  SDRKIT_RTTY_SHIFT_425_HZ = 2,
  // 450 Hz
  SDRKIT_RTTY_SHIFT_450_HZ = 3,
  // 850 Hz
  SDRKIT_RTTY_SHIFT_850_HZ = 4,
} sdrkit_rtty_shift;

// The options of the setting "demod" of the stack "rtty".
typedef enum sdrkit_rtty_demod {
  // USB
  SDRKIT_RTTY_DEMOD_USB = 0,
  // LSB
  SDRKIT_RTTY_DEMOD_LSB = 1,
  // NFM
  SDRKIT_RTTY_DEMOD_NFM = 2,
  // AM
  SDRKIT_RTTY_DEMOD_AM = 3,
} sdrkit_rtty_demod;

// The options of the setting "modulation" of the stack "psk".
typedef enum sdrkit_psk_modulation {
  // BPSK31
  SDRKIT_PSK_MODULATION_B_P_S_K31 = 0,
  // BPSK63
  SDRKIT_PSK_MODULATION_B_P_S_K63 = 1,
  // QPSK31
  SDRKIT_PSK_MODULATION_Q_P_S_K31 = 2,
  // QPSK63
  SDRKIT_PSK_MODULATION_Q_P_S_K63 = 3,
  // BPSK125
  SDRKIT_PSK_MODULATION_B_P_S_K125 = 4,
  // BPSK250
  SDRKIT_PSK_MODULATION_B_P_S_K250 = 5,
  // QPSK125
  SDRKIT_PSK_MODULATION_Q_P_S_K125 = 6,
} sdrkit_psk_modulation;

// The options of the setting "decoder" of the stack "cw".
typedef enum sdrkit_cw_decoder {
  // Adaptive
  SDRKIT_CW_DECODER_ADAPTIVE = 0,
  // Clustering
  SDRKIT_CW_DECODER_CLASSIC = 1,
} sdrkit_cw_decoder;

// The options of the setting "filter" of the stack "cw".
typedef enum sdrkit_cw_filter {
  // 50 Hz
  SDRKIT_CW_FILTER_50HZ = 0,
  // 100 Hz
  SDRKIT_CW_FILTER_100HZ = 1,
  // 250 Hz
  SDRKIT_CW_FILTER_250HZ = 2,
  // 500 Hz
  SDRKIT_CW_FILTER_500HZ = 3,
} sdrkit_cw_filter;

// The options of the setting "listen" of the stack "cw".
typedef enum sdrkit_cw_listen {
  // Off
  SDRKIT_CW_LISTEN_OFF = 0,
  // Rebuilt tone
  SDRKIT_CW_LISTEN_TONE = 1,
  // Real signal
  SDRKIT_CW_LISTEN_SIGNAL = 2,
} sdrkit_cw_listen;

// The options of the setting "span" of the stack "cw".
typedef enum sdrkit_cw_span {
  // 14 kHz
  SDRKIT_CW_SPAN_14K = 0,
  // 48 kHz
  SDRKIT_CW_SPAN_48K = 1,
  // 96 kHz
  SDRKIT_CW_SPAN_96K = 2,
  // 192 kHz
  SDRKIT_CW_SPAN_192K = 3,
} sdrkit_cw_span;

// The options of the setting "tones" of the stack "olivia".
typedef enum sdrkit_olivia_tones {
  // 2
  SDRKIT_OLIVIA_TONES_2 = 0,
  // 4
  SDRKIT_OLIVIA_TONES_4 = 1,
  // 8
  SDRKIT_OLIVIA_TONES_8 = 2,
  // 16
  SDRKIT_OLIVIA_TONES_16 = 3,
  // 32
  SDRKIT_OLIVIA_TONES_32 = 4,
  // 64
  SDRKIT_OLIVIA_TONES_64 = 5,
  // 128
  SDRKIT_OLIVIA_TONES_128 = 6,
  // 256
  SDRKIT_OLIVIA_TONES_256 = 7,
} sdrkit_olivia_tones;

// The options of the setting "bandwidth" of the stack "olivia".
typedef enum sdrkit_olivia_bandwidth {
  // 125 Hz
  SDRKIT_OLIVIA_BANDWIDTH_125_HZ = 0,
  // 250 Hz
  SDRKIT_OLIVIA_BANDWIDTH_250_HZ = 1,
  // 500 Hz
  SDRKIT_OLIVIA_BANDWIDTH_500_HZ = 2,
  // 1000 Hz
  SDRKIT_OLIVIA_BANDWIDTH_1000_HZ = 3,
  // 2000 Hz
  SDRKIT_OLIVIA_BANDWIDTH_2000_HZ = 4,
} sdrkit_olivia_bandwidth;

// The options of the setting "demod" of the stack "olivia".
typedef enum sdrkit_olivia_demod {
  // USB
  SDRKIT_OLIVIA_DEMOD_USB = 0,
  // LSB
  SDRKIT_OLIVIA_DEMOD_LSB = 1,
  // NFM
  SDRKIT_OLIVIA_DEMOD_NFM = 2,
  // AM
  SDRKIT_OLIVIA_DEMOD_AM = 3,
} sdrkit_olivia_demod;

// The options of the setting "tones" of the stack "contestia".
typedef enum sdrkit_contestia_tones {
  // 2
  SDRKIT_CONTESTIA_TONES_2 = 0,
  // 4
  SDRKIT_CONTESTIA_TONES_4 = 1,
  // 8
  SDRKIT_CONTESTIA_TONES_8 = 2,
  // 16
  SDRKIT_CONTESTIA_TONES_16 = 3,
  // 32
  SDRKIT_CONTESTIA_TONES_32 = 4,
  // 64
  SDRKIT_CONTESTIA_TONES_64 = 5,
  // 128
  SDRKIT_CONTESTIA_TONES_128 = 6,
  // 256
  SDRKIT_CONTESTIA_TONES_256 = 7,
} sdrkit_contestia_tones;

// The options of the setting "bandwidth" of the stack "contestia".
typedef enum sdrkit_contestia_bandwidth {
  // 125 Hz
  SDRKIT_CONTESTIA_BANDWIDTH_125_HZ = 0,
  // 250 Hz
  SDRKIT_CONTESTIA_BANDWIDTH_250_HZ = 1,
  // 500 Hz
  SDRKIT_CONTESTIA_BANDWIDTH_500_HZ = 2,
  // 1000 Hz
  SDRKIT_CONTESTIA_BANDWIDTH_1000_HZ = 3,
  // 2000 Hz
  SDRKIT_CONTESTIA_BANDWIDTH_2000_HZ = 4,
} sdrkit_contestia_bandwidth;

// The options of the setting "demod" of the stack "contestia".
typedef enum sdrkit_contestia_demod {
  // USB
  SDRKIT_CONTESTIA_DEMOD_USB = 0,
  // LSB
  SDRKIT_CONTESTIA_DEMOD_LSB = 1,
  // NFM
  SDRKIT_CONTESTIA_DEMOD_NFM = 2,
  // AM
  SDRKIT_CONTESTIA_DEMOD_AM = 3,
} sdrkit_contestia_demod;

// The options of the setting "variant" of the stack "mfsk".
typedef enum sdrkit_mfsk_variant {
  // MFSK4
  SDRKIT_MFSK_VARIANT_M_F_S_K4 = 0,
  // MFSK8
  SDRKIT_MFSK_VARIANT_M_F_S_K8 = 1,
  // MFSK11
  SDRKIT_MFSK_VARIANT_M_F_S_K11 = 2,
  // MFSK16
  SDRKIT_MFSK_VARIANT_M_F_S_K16 = 3,
  // MFSK22
  SDRKIT_MFSK_VARIANT_M_F_S_K22 = 4,
  // MFSK31
  SDRKIT_MFSK_VARIANT_M_F_S_K31 = 5,
  // MFSK32
  SDRKIT_MFSK_VARIANT_M_F_S_K32 = 6,
  // MFSK64
  SDRKIT_MFSK_VARIANT_M_F_S_K64 = 7,
  // MFSK128
  SDRKIT_MFSK_VARIANT_M_F_S_K128 = 8,
  // MFSK64L
  SDRKIT_MFSK_VARIANT_M_F_S_K64_L = 9,
  // MFSK128L
  SDRKIT_MFSK_VARIANT_M_F_S_K128_L = 10,
} sdrkit_mfsk_variant;

// The options of the setting "demod" of the stack "mfsk".
typedef enum sdrkit_mfsk_demod {
  // USB
  SDRKIT_MFSK_DEMOD_USB = 0,
  // LSB
  SDRKIT_MFSK_DEMOD_LSB = 1,
  // NFM
  SDRKIT_MFSK_DEMOD_NFM = 2,
  // AM
  SDRKIT_MFSK_DEMOD_AM = 3,
} sdrkit_mfsk_demod;

// The options of the setting "variant" of the stack "dominoex".
typedef enum sdrkit_dominoex_variant {
  // DominoEX 4
  SDRKIT_DOMINOEX_VARIANT_DOMINO_E_X_4 = 0,
  // DominoEX 5
  SDRKIT_DOMINOEX_VARIANT_DOMINO_E_X_5 = 1,
  // DominoEX 8
  SDRKIT_DOMINOEX_VARIANT_DOMINO_E_X_8 = 2,
  // DominoEX 11
  SDRKIT_DOMINOEX_VARIANT_DOMINO_E_X_11 = 3,
  // DominoEX 16
  SDRKIT_DOMINOEX_VARIANT_DOMINO_E_X_16 = 4,
  // DominoEX 22
  SDRKIT_DOMINOEX_VARIANT_DOMINO_E_X_22 = 5,
} sdrkit_dominoex_variant;

// The options of the setting "demod" of the stack "dominoex".
typedef enum sdrkit_dominoex_demod {
  // USB
  SDRKIT_DOMINOEX_DEMOD_USB = 0,
  // LSB
  SDRKIT_DOMINOEX_DEMOD_LSB = 1,
  // NFM
  SDRKIT_DOMINOEX_DEMOD_NFM = 2,
  // AM
  SDRKIT_DOMINOEX_DEMOD_AM = 3,
} sdrkit_dominoex_demod;

// The options of the setting "variant" of the stack "thor".
typedef enum sdrkit_thor_variant {
  // THOR 4
  SDRKIT_THOR_VARIANT_T_H_O_R_4 = 0,
  // THOR 5
  SDRKIT_THOR_VARIANT_T_H_O_R_5 = 1,
  // THOR 8
  SDRKIT_THOR_VARIANT_T_H_O_R_8 = 2,
  // THOR 11
  SDRKIT_THOR_VARIANT_T_H_O_R_11 = 3,
  // THOR 16
  SDRKIT_THOR_VARIANT_T_H_O_R_16 = 4,
  // THOR 22
  SDRKIT_THOR_VARIANT_T_H_O_R_22 = 5,
  // THOR 25x4
  SDRKIT_THOR_VARIANT_T_H_O_R_25X4 = 6,
  // THOR 50x1
  SDRKIT_THOR_VARIANT_T_H_O_R_50X1 = 7,
  // THOR 50x2
  SDRKIT_THOR_VARIANT_T_H_O_R_50X2 = 8,
  // THOR 100
  SDRKIT_THOR_VARIANT_T_H_O_R_100 = 9,
} sdrkit_thor_variant;

// The options of the setting "demod" of the stack "thor".
typedef enum sdrkit_thor_demod {
  // USB
  SDRKIT_THOR_DEMOD_USB = 0,
  // LSB
  SDRKIT_THOR_DEMOD_LSB = 1,
  // NFM
  SDRKIT_THOR_DEMOD_NFM = 2,
  // AM
  SDRKIT_THOR_DEMOD_AM = 3,
} sdrkit_thor_demod;

// The options of the setting "variant" of the stack "hell".
typedef enum sdrkit_hell_variant {
  // Feld Hell
  SDRKIT_HELL_VARIANT_FELD_HELL = 0,
  // Slow Hell
  SDRKIT_HELL_VARIANT_SLOW_HELL = 1,
  // Hell 80
  SDRKIT_HELL_VARIANT_HELL_80 = 2,
  // HellX5
  SDRKIT_HELL_VARIANT_HELL_X5 = 3,
  // HellX9
  SDRKIT_HELL_VARIANT_HELL_X9 = 4,
  // FSK Hell 245
  SDRKIT_HELL_VARIANT_F_S_K_HELL_245 = 5,
  // FSK Hell 105
  SDRKIT_HELL_VARIANT_F_S_K_HELL_105 = 6,
} sdrkit_hell_variant;

// The options of the setting "demod" of the stack "hell".
typedef enum sdrkit_hell_demod {
  // USB
  SDRKIT_HELL_DEMOD_USB = 0,
  // LSB
  SDRKIT_HELL_DEMOD_LSB = 1,
  // NFM
  SDRKIT_HELL_DEMOD_NFM = 2,
  // AM
  SDRKIT_HELL_DEMOD_AM = 3,
} sdrkit_hell_demod;

// The options of the setting "bandwidth" of the stack "mt63".
typedef enum sdrkit_mt63_bandwidth {
  // Auto
  SDRKIT_MT63_BANDWIDTH_AUTO = 0,
  // 500 Hz
  SDRKIT_MT63_BANDWIDTH_500_HZ = 1,
  // 1000 Hz
  SDRKIT_MT63_BANDWIDTH_1000_HZ = 2,
  // 2000 Hz
  SDRKIT_MT63_BANDWIDTH_2000_HZ = 3,
} sdrkit_mt63_bandwidth;

// The options of the setting "interleave" of the stack "mt63".
typedef enum sdrkit_mt63_interleave {
  // Auto
  SDRKIT_MT63_INTERLEAVE_AUTO = 0,
  // Short
  SDRKIT_MT63_INTERLEAVE_SHORT = 1,
  // Long
  SDRKIT_MT63_INTERLEAVE_LONG = 2,
} sdrkit_mt63_interleave;

// The options of the setting "speed" of the stack "fsq".
typedef enum sdrkit_fsq_speed {
  // Auto
  SDRKIT_FSQ_SPEED_AUTO = 0,
  // 1.5
  SDRKIT_FSQ_SPEED_1_5 = 1,
  // 2
  SDRKIT_FSQ_SPEED_2 = 2,
  // 3
  SDRKIT_FSQ_SPEED_3 = 3,
  // 4.5
  SDRKIT_FSQ_SPEED_4_5 = 4,
  // 6
  SDRKIT_FSQ_SPEED_6 = 5,
} sdrkit_fsq_speed;

// The options of the setting "speed" of the stack "ifkp".
typedef enum sdrkit_ifkp_speed {
  // Auto
  SDRKIT_IFKP_SPEED_AUTO = 0,
  // 0.5
  SDRKIT_IFKP_SPEED_0_5 = 1,
  // 1.0
  SDRKIT_IFKP_SPEED_1_0 = 2,
  // 2.0
  SDRKIT_IFKP_SPEED_2_0 = 3,
} sdrkit_ifkp_speed;

// The options of the setting "demod" of the stack "sstv".
typedef enum sdrkit_sstv_demod {
  // USB
  SDRKIT_SSTV_DEMOD_USB = 0,
  // LSB
  SDRKIT_SSTV_DEMOD_LSB = 1,
  // NFM
  SDRKIT_SSTV_DEMOD_NFM = 2,
  // AM
  SDRKIT_SSTV_DEMOD_AM = 3,
} sdrkit_sstv_demod;

// The options of the setting "gen_mode" of the stack "sstv".
typedef enum sdrkit_sstv_gen_mode {
  // Robot 36
  SDRKIT_SSTV_GEN_MODE_ROBOT_36 = 0,
  // Robot 72
  SDRKIT_SSTV_GEN_MODE_ROBOT_72 = 1,
  // Martin M1
  SDRKIT_SSTV_GEN_MODE_MARTIN_M1 = 2,
  // Martin M2
  SDRKIT_SSTV_GEN_MODE_MARTIN_M2 = 3,
  // Scottie S1
  SDRKIT_SSTV_GEN_MODE_SCOTTIE_S1 = 4,
  // Scottie S2
  SDRKIT_SSTV_GEN_MODE_SCOTTIE_S2 = 5,
  // Scottie DX
  SDRKIT_SSTV_GEN_MODE_SCOTTIE_D_X = 6,
  // PD50
  SDRKIT_SSTV_GEN_MODE_P_D50 = 7,
  // PD90
  SDRKIT_SSTV_GEN_MODE_P_D90 = 8,
  // PD120
  SDRKIT_SSTV_GEN_MODE_P_D120 = 9,
  // PD180
  SDRKIT_SSTV_GEN_MODE_P_D180 = 10,
  // Wraase SC2-180
  SDRKIT_SSTV_GEN_MODE_WRAASE_S_C2_180 = 11,
} sdrkit_sstv_gen_mode;

// The options of the setting "gen_picture" of the stack "sstv".
typedef enum sdrkit_sstv_gen_picture {
  // Test card
  SDRKIT_SSTV_GEN_PICTURE_CARD = 0,
  // Texture
  SDRKIT_SSTV_GEN_PICTURE_TEXTURE = 1,
} sdrkit_sstv_gen_picture;

// The options of the setting "tx_mode" of the stack "sstv".
typedef enum sdrkit_sstv_tx_mode {
  // Robot 36
  SDRKIT_SSTV_TX_MODE_ROBOT_36 = 0,
  // Robot 72
  SDRKIT_SSTV_TX_MODE_ROBOT_72 = 1,
  // Martin M1
  SDRKIT_SSTV_TX_MODE_MARTIN_M1 = 2,
  // Martin M2
  SDRKIT_SSTV_TX_MODE_MARTIN_M2 = 3,
  // Scottie S1
  SDRKIT_SSTV_TX_MODE_SCOTTIE_S1 = 4,
  // Scottie S2
  SDRKIT_SSTV_TX_MODE_SCOTTIE_S2 = 5,
  // Scottie DX
  SDRKIT_SSTV_TX_MODE_SCOTTIE_D_X = 6,
  // PD50
  SDRKIT_SSTV_TX_MODE_P_D50 = 7,
  // PD90
  SDRKIT_SSTV_TX_MODE_P_D90 = 8,
  // PD120
  SDRKIT_SSTV_TX_MODE_P_D120 = 9,
  // PD180
  SDRKIT_SSTV_TX_MODE_P_D180 = 10,
  // Wraase SC2-180
  SDRKIT_SSTV_TX_MODE_WRAASE_S_C2_180 = 11,
} sdrkit_sstv_tx_mode;

// The options of the setting "fit" of the stack "sstv".
typedef enum sdrkit_sstv_fit {
  // Keep the shape, black bars
  SDRKIT_SSTV_FIT_BARS = 0,
  // Keep the shape, crop
  SDRKIT_SSTV_FIT_CROP = 1,
  // Stretch
  SDRKIT_SSTV_FIT_STRETCH = 2,
} sdrkit_sstv_fit;

// The options of the setting "demod" of the stack "ardop".
typedef enum sdrkit_ardop_demod {
  // USB
  SDRKIT_ARDOP_DEMOD_USB = 0,
  // LSB
  SDRKIT_ARDOP_DEMOD_LSB = 1,
  // NFM
  SDRKIT_ARDOP_DEMOD_NFM = 2,
  // AM
  SDRKIT_ARDOP_DEMOD_AM = 3,
} sdrkit_ardop_demod;

// The options of the setting "fec_type" of the stack "ardop".
typedef enum sdrkit_ardop_fec_type {
  // 4FSK.200.50S
  SDRKIT_ARDOP_FEC_TYPE_4FSK_200_50S = 0,
  // 4FSK.500.100S
  SDRKIT_ARDOP_FEC_TYPE_4FSK_500_100S = 1,
  // 4FSK.500.100
  SDRKIT_ARDOP_FEC_TYPE_4FSK_500_100 = 2,
} sdrkit_ardop_fec_type;

// The options of the setting "demod" of the stack "ros".
typedef enum sdrkit_ros_demod {
  // USB
  SDRKIT_ROS_DEMOD_USB = 0,
  // LSB
  SDRKIT_ROS_DEMOD_LSB = 1,
} sdrkit_ros_demod;

// The options of the setting "gen_signal" of the stack "ros".
typedef enum sdrkit_ros_gen_signal {
  // Tone grid
  SDRKIT_ROS_GEN_SIGNAL_GRID = 0,
  // Noise
  SDRKIT_ROS_GEN_SIGNAL_NOISE = 1,
  // FSK
  SDRKIT_ROS_GEN_SIGNAL_FSK = 2,
  // Carrier
  SDRKIT_ROS_GEN_SIGNAL_CARRIER = 3,
  // Sweep
  SDRKIT_ROS_GEN_SIGNAL_SWEEP = 4,
} sdrkit_ros_gen_signal;

// The options of the setting "variant" of the stack "throb".
typedef enum sdrkit_throb_variant {
  // Automatic
  SDRKIT_THROB_VARIANT_AUTO = 0,
  // Throb1
  SDRKIT_THROB_VARIANT_THROB1 = 1,
  // Throb2
  SDRKIT_THROB_VARIANT_THROB2 = 2,
  // Throb4
  SDRKIT_THROB_VARIANT_THROB4 = 3,
  // ThrobX1
  SDRKIT_THROB_VARIANT_THROBX1 = 4,
  // ThrobX2
  SDRKIT_THROB_VARIANT_THROBX2 = 5,
  // ThrobX4
  SDRKIT_THROB_VARIANT_THROBX4 = 6,
} sdrkit_throb_variant;

// The options of the setting "demod" of the stack "throb".
typedef enum sdrkit_throb_demod {
  // USB
  SDRKIT_THROB_DEMOD_USB = 0,
  // LSB
  SDRKIT_THROB_DEMOD_LSB = 1,
  // NFM
  SDRKIT_THROB_DEMOD_NFM = 2,
  // AM
  SDRKIT_THROB_DEMOD_AM = 3,
} sdrkit_throb_demod;

// The options of the setting "gen_variant" of the stack "throb".
typedef enum sdrkit_throb_gen_variant {
  // Throb1
  SDRKIT_THROB_GEN_VARIANT_THROB1 = 0,
  // Throb2
  SDRKIT_THROB_GEN_VARIANT_THROB2 = 1,
  // Throb4
  SDRKIT_THROB_GEN_VARIANT_THROB4 = 2,
  // ThrobX1
  SDRKIT_THROB_GEN_VARIANT_THROBX1 = 3,
  // ThrobX2
  SDRKIT_THROB_GEN_VARIANT_THROBX2 = 4,
  // ThrobX4
  SDRKIT_THROB_GEN_VARIANT_THROBX4 = 5,
} sdrkit_throb_gen_variant;

// The options of the setting "submode" of the stack "qrss".
typedef enum sdrkit_qrss_submode {
  // QRSS
  SDRKIT_QRSS_SUBMODE_QRSS = 0,
  // DFCW
  SDRKIT_QRSS_SUBMODE_DFCW = 1,
} sdrkit_qrss_submode;

// The options of the setting "demod" of the stack "qrss".
typedef enum sdrkit_qrss_demod {
  // USB
  SDRKIT_QRSS_DEMOD_USB = 0,
  // LSB
  SDRKIT_QRSS_DEMOD_LSB = 1,
  // NFM
  SDRKIT_QRSS_DEMOD_NFM = 2,
  // AM
  SDRKIT_QRSS_DEMOD_AM = 3,
} sdrkit_qrss_demod;

// The options of the setting "sideband" of the stack "cp16".
typedef enum sdrkit_cp16_sideband {
  // Automatic
  SDRKIT_CP16_SIDEBAND_AUTO = 0,
  // Upper
  SDRKIT_CP16_SIDEBAND_UPPER = 1,
  // Lower
  SDRKIT_CP16_SIDEBAND_LOWER = 2,
} sdrkit_cp16_sideband;

// The options of the setting "gen_text" of the stack "cp16".
typedef enum sdrkit_cp16_gen_text {
  // Letters and digits
  SDRKIT_CP16_GEN_TEXT_LETTERS = 0,
  // Chinese
  SDRKIT_CP16_GEN_TEXT_CHINESE = 1,
} sdrkit_cp16_gen_text;

// The levels that the front end measures. A level has no value before the first input.
typedef struct sdrkit_hfkbd_front_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The power of the input. Unit: dBFS.
  double level_dbfs;
  // True if `audio_dbfs` holds a value.
  bool has_audio_dbfs;
  // The power of the 200 to 3800 Hz sideband before the AGC. Unit: dBFS.
  double audio_dbfs;
} sdrkit_hfkbd_front_fields;

// The result fields of RTTY.
typedef struct sdrkit_rtty_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The decoded text.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
  // True if `mark_hz` holds a value.
  bool has_mark_hz;
  // The mark tone. It has no value while searching. A frequency in Hz.
  double mark_hz;
  // True if `space_hz` holds a value.
  bool has_space_hz;
  // The space tone. It has no value while searching. A frequency in Hz.
  double space_hz;
  // The speed in use. A frequency in Hz.
  double symbol_rate_hz;
  // The shift in use. A frequency in Hz.
  double shift_hz;
  // The mark is the upper tone.
  bool reverse;
  // Characters decoded.
  int64_t characters;
  // Characters that the framer dropped.
  int64_t framing_errors;
  // How clean the decisions are, 0 to 100. Unit: %.
  double quality_pct;
  // Auto is on.
  bool auto_;
  // Auto has applied a guess that still decodes.
  bool auto_locked;
  // The state of Auto: 0 not asked, 1 waiting for audio or running, 2 applied, 3 nothing found.
  int64_t auto_state;
} sdrkit_rtty_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_hfkbd_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_hfkbd_decoder_fields;

// The result fields of a keyboard sender.
typedef struct sdrkit_hfkbd_keyboard_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a burst is on air: text or the tune carrier.
  bool tx_sending;
  // The characters of the burst on air coded into the signal so far.
  int64_t tx_sent;
  // The characters waiting to be sent.
  int64_t tx_waiting;
} sdrkit_hfkbd_keyboard_tx_fields;

// A row of the stations heard. The row key is the callsign.
typedef struct sdrkit_hfkbd_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign.
  sdrkit_string callsign;
  // Where it was last heard. A frequency in Hz.
  double freq_hz;
  // Its SNR when last heard. Unit: dB.
  double snr_db;
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last time heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_hfkbd_station_row;

// A guess of the last analysis of Auto. The row key is the rank, from 1.
typedef struct sdrkit_rtty_guess_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The lower tone. A frequency in Hz.
  double lower_hz;
  // The shift. A frequency in Hz.
  double shift_hz;
  // The speed. A frequency in Hz.
  double symbol_rate_hz;
  // The mark is the upper tone.
  bool reverse;
  // The text score less 2.5 for each framing error.
  double score;
  // The characters of the trial.
  int64_t characters;
  // The framing errors of the trial.
  int64_t framing_errors;
} sdrkit_rtty_guess_row;

// The options of `modulation` of `sdrkit_psk_fields`.
typedef enum sdrkit_psk_fields_modulation {
  // BPSK31
  SDRKIT_PSK_FIELDS_MODULATION_B_P_S_K31 = 0,
  // BPSK63
  SDRKIT_PSK_FIELDS_MODULATION_B_P_S_K63 = 1,
  // QPSK31
  SDRKIT_PSK_FIELDS_MODULATION_Q_P_S_K31 = 2,
  // QPSK63
  SDRKIT_PSK_FIELDS_MODULATION_Q_P_S_K63 = 3,
  // BPSK125
  SDRKIT_PSK_FIELDS_MODULATION_B_P_S_K125 = 4,
  // BPSK250
  SDRKIT_PSK_FIELDS_MODULATION_B_P_S_K250 = 5,
  // QPSK125
  SDRKIT_PSK_FIELDS_MODULATION_Q_P_S_K125 = 6,
} sdrkit_psk_fields_modulation;

// The result fields of PSK.
typedef struct sdrkit_psk_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The modulation decoded. A value of `sdrkit_psk_fields_modulation`.
  int32_t modulation;
  // The noise floor of the window. Unit: dB.
  double floor_db;
  // True if `focus` holds a value.
  bool has_focus;
  // The id of the signal in focus, if one is.
  int64_t focus;
  // Characters decoded.
  int64_t characters;
  // Characters that the code table does not have.
  int64_t errors;
  // Auto is on.
  bool auto_;
  // Auto has applied a guess that still decodes.
  bool auto_locked;
  // True if `auto_modulation` holds a value.
  bool has_auto_modulation;
  // The modulation that Auto found. It has no value before Auto finds one.
  sdrkit_string auto_modulation;
} sdrkit_psk_fields;

// A PSK signal heard, or one gone whose text stays. The row key is the signal id. The rows of the gone signals stay over a change of frequency: they are the log of the decoded texts.
typedef struct sdrkit_psk_signal_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal id.
  int64_t id;
  // The audio frequency. A frequency in Hz.
  double offset_hz;
  // The correction since it was found. A frequency in Hz.
  double afc_hz;
  // The SNR. Unit: dB.
  double snr_db;
  // How clean the decisions are, 0 to 100. Unit: %.
  double quality_pct;
  // Text is coming.
  bool dcd;
  // The user picked it.
  bool manual;
  // The signal time when it was first heard. Unit: s.
  double first_s;
  // The signal time when it was last heard. Unit: s.
  double last_s;
  // Characters decoded.
  int64_t characters;
  // The decoded text.
  sdrkit_string text;
  // It is no longer heard.
  bool gone;
} sdrkit_psk_signal_row;

// The options of `playing` of `sdrkit_cw_fields`.
typedef enum sdrkit_cw_fields_playing {
  // Nothing
  SDRKIT_CW_FIELDS_PLAYING_NOTHING = 0,
  // Signal
  SDRKIT_CW_FIELDS_PLAYING_SIGNAL = 1,
  // Dial, no signal
  SDRKIT_CW_FIELDS_PLAYING_DIAL = 2,
} sdrkit_cw_fields_playing;

// The result fields of the CW skimmer. The fields of the sender have no value without a signal in focus.
typedef struct sdrkit_cw_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The noise floor of the window. Unit: dB.
  double floor_db;
  // True if `focus` holds a value.
  bool has_focus;
  // The id of the signal in focus, if one is.
  int64_t focus;
  // Characters decoded.
  int64_t characters;
  // Characters that the code table does not have.
  int64_t errors;
  // True if `dot_ms` holds a value.
  bool has_dot_ms;
  // The dot of the signal in focus. Unit: ms.
  double dot_ms;
  // True if `dash_ratio` holds a value.
  bool has_dash_ratio;
  // Its dash as dots.
  double dash_ratio;
  // True if `char_gap` holds a value.
  bool has_char_gap;
  // Its character gap. Unit: dots.
  double char_gap;
  // True if `word_gap` holds a value.
  bool has_word_gap;
  // Its word gap. Unit: dots.
  double word_gap;
  // True if `steadiness_pct` holds a value.
  bool has_steadiness_pct;
  // How steady its keying is. Unit: %.
  double steadiness_pct;
  // True if `wpm` holds a value.
  bool has_wpm;
  // Its speed. Unit: WPM.
  double wpm;
  // The width of the band watched, centred on the dial. A frequency in Hz.
  double span_hz;
  // The signals in the table that are heard now.
  int64_t signals;
  // The signal time now, for the age of a signal's last-heard time. Unit: s.
  double now_s;
  // True if `listened` holds a value.
  bool has_listened;
  // The id of the signal listened to (the row key in the signals table): the focus, else the strongest signal in the table. Absent while there is none, also while the audio plays the dial.
  int64_t listened;
  // What the audio plays: nothing, the signal listened to, or the dial while there is none. A value of `sdrkit_cw_fields_playing`.
  int32_t playing;
  // True if `listened_hz` holds a value.
  bool has_listened_hz;
  // The radio frequency that the audio plays: that of the signal listened to, or the dial. Absent while it plays nothing. A frequency in Hz.
  double listened_hz;
} sdrkit_cw_fields;

// A CW signal heard, or one gone whose text stays. The row key is the signal id.
typedef struct sdrkit_cw_signal_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal id.
  int64_t id;
  // The audio frequency. A frequency in Hz.
  double offset_hz;
  // The speed. Unit: WPM.
  double wpm;
  // The SNR. Unit: dB.
  double snr_db;
  // The key is down.
  bool key_down;
  // The speed is known.
  bool locked;
  // The sender spaces as Farnsworth.
  bool farnsworth;
  // The user picked it.
  bool manual;
  // The signal time when it was first heard. Unit: s.
  double first_s;
  // The signal time when it was last heard. Unit: s.
  double last_s;
  // Characters decoded.
  int64_t characters;
  // The decoded text.
  sdrkit_string text;
  // The confidence of each character, 0 to 9.
  sdrkit_string confidence;
  // The other readings of unclear characters.
  sdrkit_string alternatives;
  // It is no longer heard.
  bool gone;
  // Characters that the code table does not have.
  int64_t errors;
  // The radio frequency of the carrier: the dial plus the offset. A frequency in Hz.
  double freq_hz;
  // The level of the carrier while the key is down, relative to full scale. Unit: dBFS.
  double level_dbfs;
} sdrkit_cw_signal_row;

// The result fields of the keyboard stacks of many tones: MFSK, Olivia, Contestia, DominoEX and THOR.
typedef struct sdrkit_hfkbd_keyboard_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The variant in use.
  sdrkit_string variant;
  // The audio centre now. A frequency in Hz.
  double centre_hz;
  // The tuning error found. A frequency in Hz.
  double offset_hz;
  // The decoder's own SNR. Unit: dB.
  double snr_db;
  // The decoded text.
  sdrkit_string text;
  // Characters decoded.
  int64_t characters;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
  // How clean the decisions are, 0 to 100. Unit: %.
  double quality_pct;
} sdrkit_hfkbd_keyboard_fields;

// The picture being received, or else the newest one. Each field of a picture has no value before the first.
typedef struct sdrkit_hfkbd_picture_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `picture_id` holds a value.
  bool has_picture_id;
  // The number of the picture.
  int64_t picture_id;
  // True if `picture_width` holds a value.
  bool has_picture_width;
  // Its width. Unit: px.
  int64_t picture_width;
  // True if `picture_height` holds a value.
  bool has_picture_height;
  // Its height. Unit: px.
  int64_t picture_height;
  // True if `picture_rows` holds a value.
  bool has_picture_rows;
  // The rows received.
  int64_t picture_rows;
  // True if `picture_progress_pct` holds a value.
  bool has_picture_progress_pct;
  // How much of it is in. Unit: %.
  double picture_progress_pct;
  // True if `picture_complete` holds a value.
  bool has_picture_complete;
  // It is complete.
  bool picture_complete;
  // True if `picture_colour` holds a value.
  bool has_picture_colour;
  // It is in colour.
  bool picture_colour;
  // True if `picture_mode` holds a value.
  bool has_picture_mode;
  // Its mode.
  sdrkit_string picture_mode;
  // The pictures finished.
  int64_t pictures;
} sdrkit_hfkbd_picture_fields;

// The result fields of Hellschreiber.
typedef struct sdrkit_hell_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The variant in use.
  sdrkit_string variant;
  // The audio centre now. A frequency in Hz.
  double centre_hz;
  // The pixel columns received.
  int64_t columns;
  // The SNR. Unit: dB.
  double snr_db;
} sdrkit_hell_fields;

// The options of `interleave` of `sdrkit_mt63_fields`.
typedef enum sdrkit_mt63_fields_interleave {
  // Auto
  SDRKIT_MT63_FIELDS_INTERLEAVE_AUTO = 0,
  // Short
  SDRKIT_MT63_FIELDS_INTERLEAVE_SHORT = 1,
  // Long
  SDRKIT_MT63_FIELDS_INTERLEAVE_LONG = 2,
} sdrkit_mt63_fields_interleave;

// The result fields of MT63.
typedef struct sdrkit_mt63_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The width of the signal in use. A frequency in Hz.
  double bandwidth_hz;
  // The interleave in use. A value of `sdrkit_mt63_fields_interleave`.
  int32_t interleave;
  // The centre of the carriers above the dial. A frequency in Hz.
  double centre_hz;
  // How clean the decisions are, 0 to 100. Unit: %.
  double quality_pct;
  // The noise floor. Unit: dB.
  double floor_db;
  // The times the receiver locked.
  int64_t locks;
  // The correction since the lock. A frequency in Hz.
  double afc_hz;
  // The SNR. Unit: dB.
  double snr_db;
  // Characters decoded.
  int64_t characters;
  // Characters that the code did not resolve.
  int64_t errors;
  // The decoded text.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
} sdrkit_mt63_fields;

// The result fields of FSQ and IFKP. The tones have no value before a signal is heard.
typedef struct sdrkit_fsq_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The speed in use.
  sdrkit_string speed;
  // The width of the tones. A frequency in Hz.
  double band_hz;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `lowest_hz` holds a value.
  bool has_lowest_hz;
  // The lowest tone heard. A frequency in Hz.
  double lowest_hz;
  // True if `highest_hz` holds a value.
  bool has_highest_hz;
  // The highest tone heard. A frequency in Hz.
  double highest_hz;
  // How clean the decisions are, 0 to 100. Unit: %.
  double quality_pct;
  // The SNR. Unit: dB.
  double snr_db;
  // Characters decoded.
  int64_t characters;
  // Symbols that the receiver refused.
  int64_t dropped_symbols;
  // The decoded text.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
} sdrkit_fsq_fields;

// The options of `trigger` of `sdrkit_fsq_message_row`.
typedef enum sdrkit_fsq_message_row_trigger {
  // Text
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_TEXT = 0,
  // Asks for the signal report
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_REPORT = 1,
  // Asks for an acknowledgement
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_ACK = 2,
  // Asks for a file
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_FILE = 3,
  // Relay this message
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_RELAY = 4,
  // Repeat now
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_REPEAT = 5,
  // Repeat in 15 s
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_REPEAT_LATER = 6,
  // Store this text
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_STORE = 7,
  // Asks for the heard list
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_HEARD = 8,
  // Asks for the location
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_LOCATION = 9,
  // Asks for the message of the day
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_MOTD = 10,
  // Asks for the software version
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_VERSION = 11,
  // Picture follows
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_PICTURE = 12,
  // Alert
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_ALERT = 13,
  // Speed up
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_FASTER = 14,
  // Speed down
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_SLOWER = 15,
  // Relayed message
  SDRKIT_FSQ_MESSAGE_ROW_TRIGGER_RELAYED = 16,
} sdrkit_fsq_message_row_trigger;

// A row of the FSQ messages. The row key is the place of the message in the log.
typedef struct sdrkit_fsq_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // True if `from` holds a value.
  bool has_from;
  // The sender, when the line starts with "CALL:".
  sdrkit_string from;
  // The two hex digits equal the CRC-8 of the callsign.
  bool crc_ok;
  // True if `to` holds a value.
  bool has_to;
  // The addressee.
  sdrkit_string to;
  // True if `trigger` holds a value.
  bool has_trigger;
  // The trigger. A value of `sdrkit_fsq_message_row_trigger`.
  int32_t trigger;
  // The text after the trigger.
  sdrkit_string text;
  // Addressed to my call, allcall or cqcqcq.
  bool to_me;
} sdrkit_fsq_message_row;

// The result fields of SSTV.
typedef struct sdrkit_sstv_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `mode` holds a value.
  bool has_mode;
  // The mode of the picture being received or the newest one. It has no value before the first picture.
  sdrkit_string mode;
  // True if `vis` holds a value.
  bool has_vis;
  // Its VIS code, -1 when it was found from the line period.
  int64_t vis;
  // True if `slant_ppm` holds a value.
  bool has_slant_ppm;
  // Its slant: the line period over the nominal one, minus 1. Unit: ppm.
  double slant_ppm;
  // The tuning error. A frequency in Hz.
  double offset_hz;
} sdrkit_sstv_fields;

// The result fields of the SSTV transmitter.
typedef struct sdrkit_sstv_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a picture is being sent.
  bool tx_sending;
  // True if `tx_mode` holds a value.
  bool has_tx_mode;
  // The mode of the picture being sent. No value while nothing is sent.
  sdrkit_string tx_mode;
  // True if `tx_line` holds a value.
  bool has_tx_line;
  // The line being sent, from 1; 0 during the VIS header. No value while nothing is sent.
  int64_t tx_line;
  // True if `tx_lines` holds a value.
  bool has_tx_lines;
  // The lines of the picture being sent. No value while nothing is sent.
  int64_t tx_lines;
  // The time on air of one picture in the transmit mode: the VIS header and the lines. A length of time in seconds.
  double tx_picture_s;
  // True if `tx_next_s` holds a value.
  bool has_tx_next_s;
  // The time until the picture is sent again, while repeat waits in the pause. No value otherwise. A length of time in seconds.
  double tx_next_s;
} sdrkit_sstv_tx_fields;

// The result fields of SITOR.
typedef struct sdrkit_sitor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The decoded text.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
  // The centre of the tones. A frequency in Hz.
  double offset_hz;
  // True if `shift_hz` holds a value.
  bool has_shift_hz;
  // The shift. It has no value before a signal is found. A frequency in Hz.
  double shift_hz;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The speed. It has no value before a signal is found. A frequency in Hz.
  double symbol_rate_hz;
  // The tones are the other way round.
  bool inverted;
  // Words of a good code.
  int64_t good_words;
  // Words of a bad code.
  int64_t bad_words;
  // Characters lost in both copies.
  int64_t mutilated;
  // Characters decoded.
  int64_t characters;
  // The ARQ monitor follows a link.
  bool arq_locked;
  // ARQ blocks read.
  int64_t arq_blocks;
  // ARQ blocks that were new.
  int64_t arq_new_blocks;
  // ARQ blocks sent again.
  int64_t arq_repeats;
  // ARQ slots that did not read.
  int64_t arq_bad;
  // ARQ control signals read.
  int64_t arq_control_signals;
  // True if `arq_cs_offset_bits` holds a value.
  bool has_arq_cs_offset_bits;
  // Where the control signals lie. It has no value before one is found. Unit: bits.
  int64_t arq_cs_offset_bits;
  // ARQ characters decoded.
  int64_t arq_chars;
  // The ARQ text.
  sdrkit_string arq_text;
  // True if `selcall` holds a value.
  bool has_selcall;
  // The selective call of the link, if one was read.
  sdrkit_string selcall;
  // The changes of direction of the link.
  int64_t arq_changes;
} sdrkit_sitor_fields;

// The result fields of ARDOP.
typedef struct sdrkit_ardop_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Leaders heard.
  int64_t leaders;
  // Headers read.
  int64_t good_headers;
  // Leaders whose header did not read.
  int64_t unreadable_leaders;
  // Data frames good.
  int64_t data_ok;
  // Data frames bad.
  int64_t data_bad;
  // Reed-Solomon corrections.
  int64_t rs_corrections;
  // Control frames.
  int64_t control;
  // Other frames.
  int64_t other;
  // Stations heard.
  int64_t stations_heard;
  // Frames heard.
  int64_t frames_ok;
  // True if `text` holds a value.
  bool has_text;
  // The newest text, if a frame held one.
  sdrkit_string text;
} sdrkit_ardop_fields;

// The options of `link_state` of `sdrkit_ardop_tx_fields`.
typedef enum sdrkit_ardop_tx_fields_link_state {
  // Idle
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_IDLE = 0,
  // Connecting
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_CONNECTING = 1,
  // Connected, sending
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_SENDING = 2,
  // Connected, receiving
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_RECEIVING = 3,
  // Disconnecting
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_DISCONNECTING = 4,
  // Closed
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_CLOSED = 5,
  // Failed
  SDRKIT_ARDOP_TX_FIELDS_LINK_STATE_FAILED = 6,
} sdrkit_ardop_tx_fields_link_state;

// The result fields of the ARDOP sender and of its link.
typedef struct sdrkit_ardop_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a burst is on air.
  bool tx_sending;
  // True if `tx_frame` holds a value.
  bool has_tx_frame;
  // The type of the frame on air. No value while nothing is sent.
  sdrkit_string tx_frame;
  // The bursts waiting to be sent.
  int64_t tx_waiting;
  // The state of the link. A value of `sdrkit_ardop_tx_fields_link_state`.
  int32_t link_state;
  // True if `link_peer` holds a value.
  bool has_link_peer;
  // The other station of the link. No value while no link was made.
  sdrkit_string link_peer;
  // True if `link_session` holds a value.
  bool has_link_session;
  // The session id of the link. No value while no link was made.
  int64_t link_session;
  // The data bytes of the stack that the other station acknowledged.
  int64_t link_sent;
  // The data bytes that the other station sent and the stack received.
  int64_t link_received;
  // The data bytes waiting to be sent on the link.
  int64_t link_waiting;
  // The repeats in a row of the frame that waits for its answer.
  int64_t link_retries;
  // Why the link state is what it is; empty when it needs no reason.
  sdrkit_string link_reason;
  // True if `link_text` holds a value.
  bool has_link_text;
  // The text received on the link, the newest 4000 bytes. No value before the first byte.
  sdrkit_string link_text;
  // True if `link_request` holds a value.
  bool has_link_request;
  // The station of the newest connect request to the station callsign that the stack did not answer.
  sdrkit_string link_request;
} sdrkit_ardop_tx_fields;

// A row of the ARDOP frames. The row key is the number of the frame, counted from the start.
typedef struct sdrkit_ardop_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The frame type.
  int64_t type;
  // The name of the type.
  sdrkit_string name;
  // What the frame is.
  sdrkit_string kind;
  // Its modulation.
  sdrkit_string modulation;
  // Its speed. Unit: Bd.
  int64_t baud;
  // The session id.
  int64_t session;
  // True if `from` holds a value.
  bool has_from;
  // The sender, if the frame names one.
  sdrkit_string from;
  // True if `to` holds a value.
  bool has_to;
  // The addressee, if the frame names one.
  sdrkit_string to;
  // True if `locator` holds a value.
  bool has_locator;
  // The locator, if the frame holds one.
  sdrkit_string locator;
  // True if `text` holds a value.
  bool has_text;
  // The text, if the frame holds one.
  sdrkit_string text;
  // True if `quality` holds a value.
  bool has_quality;
  // The quality that the frame reports.
  int64_t quality;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR, if it was measured. Unit: dB.
  double snr_db;
  // True if `leader_ms` holds a value.
  bool has_leader_ms;
  // The length of the leader, if it was measured. Unit: ms.
  int64_t leader_ms;
  // The header read.
  bool header_ok;
  // The body was read.
  bool body_read;
  // The body is good.
  bool body_ok;
  // Reed-Solomon corrections.
  int64_t rs_corrections;
  // The offset of the tones. A frequency in Hz.
  double tone_offset_hz;
  // The SNR of the tones. Unit: dB.
  double tone_snr_db;
} sdrkit_ardop_frame_row;

// A row of the ARDOP stations. The row key is the callsign.
typedef struct sdrkit_ardop_station_row {
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
  // True if `heard` holds a value.
  bool has_heard;
  // The times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last time heard. Seconds since 1970 (UTC).
  double last_heard;
  // The type of its last frame.
  sdrkit_string last_type;
  // True if `called` holds a value.
  bool has_called;
  // The station it called, if it called one.
  sdrkit_string called;
} sdrkit_ardop_station_row;

// The result fields of ROS. The fields of the detection have no value without one.
typedef struct sdrkit_ros_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `sub_mode` holds a value.
  bool has_sub_mode;
  // The sub-mode found, if the rate gives one.
  sdrkit_string sub_mode;
  // True if `name` holds a value.
  bool has_name;
  // What was found.
  sdrkit_string name;
  // True if `why` holds a value.
  bool has_why;
  // Why.
  sdrkit_string why;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `spacing_hz` holds a value.
  bool has_spacing_hz;
  // The spacing of the tone grid. A frequency in Hz.
  double spacing_hz;
  // True if `fit_pct` holds a value.
  bool has_fit_pct;
  // How well the tones fit the grid. Unit: %.
  double fit_pct;
  // True if `lowest_hz` holds a value.
  bool has_lowest_hz;
  // The lowest tone. A frequency in Hz.
  double lowest_hz;
  // True if `highest_hz` holds a value.
  bool has_highest_hz;
  // The highest tone. A frequency in Hz.
  double highest_hz;
  // True if `purity_pct` holds a value.
  bool has_purity_pct;
  // How pure the tones are. Unit: %.
  double purity_pct;
  // True if `distinct` holds a value.
  bool has_distinct;
  // The tone positions used.
  int64_t distinct;
  // True if `score_pct` holds a value.
  bool has_score_pct;
  // The score. Unit: %.
  double score_pct;
  // The signal is identified.
  bool detected;
  // Few symbols were seen.
  bool weak;
  // The tuning error. A frequency in Hz.
  double offset_hz;
  // Identifications.
  int64_t detections;
  // Signals that did not fit.
  int64_t misfits;
} sdrkit_ros_fields;

// A row of the ROS identifications. The row key is the place of the identification in the log.
typedef struct sdrkit_ros_detection_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // True if `sub_mode` holds a value.
  bool has_sub_mode;
  // The sub-mode, if the rate gives one.
  sdrkit_string sub_mode;
  // What was found.
  sdrkit_string name;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // Why.
  sdrkit_string why;
} sdrkit_ros_detection_row;

// The result fields of Throb. The fields of the reading have no value before the first.
typedef struct sdrkit_throb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The decoded text.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
  // True if `variant` holds a value.
  bool has_variant;
  // The variant found.
  sdrkit_string variant;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The audio centre found. A frequency in Hz.
  double centre_hz;
  // True if `score` holds a value.
  bool has_score;
  // The score of the reading.
  double score;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the reading. Unit: dB.
  double snr_db;
  // Characters decoded.
  int64_t characters;
  // A note on the stack.
  sdrkit_string note;
} sdrkit_throb_fields;

// The result fields of QRSS. The fields of the reading have no value before the first.
typedef struct sdrkit_qrss_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of the newest reading.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The tone, if one is found. A frequency in Hz.
  double centre_hz;
  // True if `element_s` holds a value.
  bool has_element_s;
  // The length of an element, while a tone is found. Unit: s.
  double element_s;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the reading, while a tone is found. Unit: dB.
  double snr_db;
  // A tone is found.
  bool locked;
  // The picture rows made.
  int64_t picture_rows;
  // A note on the stack.
  sdrkit_string note;
} sdrkit_qrss_fields;

// The result fields of CP16. The fields of the comb and the clock have no value before they are found.
typedef struct sdrkit_cp16_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The decoded text.
  sdrkit_string text;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The newest callsign of the text, if it has one.
  sdrkit_string callsign;
  // The state of the receiver.
  int64_t state;
  // True if `base_hz` holds a value.
  bool has_base_hz;
  // The lowest carrier. Unit: Hz.
  double base_hz;
  // True if `spacing_hz` holds a value.
  bool has_spacing_hz;
  // The spacing of the carriers. Unit: Hz.
  double spacing_hz;
  // True if `row_ms` holds a value.
  bool has_row_ms;
  // The time of one row. Unit: ms.
  double row_ms;
  // True if `char_s` holds a value.
  bool has_char_s;
  // The seconds of one character. Unit: s.
  double char_s;
  // True if `mirrored` holds a value.
  bool has_mirrored;
  // The signal is on the lower sideband.
  bool mirrored;
  // True if `comb_db` holds a value.
  bool has_comb_db;
  // The score of the comb. Unit: dB.
  double comb_db;
  // The score of the timing, 0 to 1.
  double timing;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR. Unit: dB.
  double snr_db;
  // The picture rows made.
  int64_t picture_rows;
  // Characters decoded.
  int64_t characters;
  // Characters read with a clear margin.
  int64_t sure;
  // Characters read without one.
  int64_t unclear;
  // True if `font_name` holds a value.
  bool has_font_name;
  // The font that the characters are read against. It has no value before a font is loaded.
  sdrkit_string font_name;
  // A note on the stack.
  sdrkit_string note;
} sdrkit_cp16_fields;

// The result fields of WINMOR.
typedef struct sdrkit_winmor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A note on the stack.
  sdrkit_string note;
  // Pilots heard.
  int64_t pilots;
  // True if `error_hz` holds a value.
  bool has_error_hz;
  // The tuning error of the newest pilot, if one was heard. Unit: Hz.
  double error_hz;
} sdrkit_winmor_fields;

// A row of the WINMOR pilots. The row key is the place of the pilot in the log.
typedef struct sdrkit_winmor_pilot_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC. Seconds since 1970 (UTC).
  double time;
  // The tuning error. Unit: Hz.
  double error_hz;
  // The pilot symbols heard.
  int64_t pilot_symbols;
  // The frame-type tones.
  sdrkit_string tones;
} sdrkit_winmor_pilot_row;

// The options of `family` of `sdrkit_hfkbd_spot_event`.
typedef enum sdrkit_hfkbd_spot_event_family {
  // Aircraft
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFKBD_SPOT_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfkbd_spot_event_family;

// The fields of a callsign heard for the first time.
typedef struct sdrkit_hfkbd_spot_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The callsign.
  sdrkit_string callsign;
  // The callsign.
  sdrkit_string source;
  // The SNR. Unit: dB.
  double snr_db;
  // Where it was heard. A frequency in Hz.
  double freq_hz_;
  // The stack that heard it ("RTTY").
  sdrkit_string mode;
  // The family of the source: a message. A value of `sdrkit_hfkbd_spot_event_family`.
  int32_t family;
} sdrkit_hfkbd_spot_event;

// The options of `family` of `sdrkit_hfkbd_text_event`.
typedef enum sdrkit_hfkbd_text_event_family {
  // Aircraft
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFKBD_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfkbd_text_event_family;

// The fields of a finished line of decoded text.
typedef struct sdrkit_hfkbd_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The callsign after "DE", if the line has one.
  sdrkit_string source;
  // The line.
  sdrkit_string text;
  // The SNR. Unit: dB.
  double snr_db;
  // The stack that heard it.
  sdrkit_string mode;
  // The family of the source: a message. A value of `sdrkit_hfkbd_text_event_family`.
  int32_t family;
} sdrkit_hfkbd_text_event;

// The options of `family` of `sdrkit_hfkbd_tx_event`.
typedef enum sdrkit_hfkbd_tx_event_family {
  // Aircraft
  SDRKIT_HFKBD_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFKBD_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFKBD_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFKBD_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFKBD_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFKBD_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFKBD_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFKBD_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFKBD_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFKBD_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFKBD_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFKBD_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFKBD_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfkbd_tx_event_family;

// The fields of a change of a keyboard transmission.
typedef struct sdrkit_hfkbd_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, stopped, the transmitter is off, the format changed, tune carrier, or why a text was refused.
  sdrkit_string reason;
  // The text: as given to `send` for started, queued and refused; as coded for finished.
  sdrkit_string text;
  // The family of the source: a message. A value of `sdrkit_hfkbd_tx_event_family`.
  int32_t family;
} sdrkit_hfkbd_tx_event;

// The fields of a picture received or cut off.
typedef struct sdrkit_mfsk_picture_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the picture.
  int64_t picture_id;
  // Its width. Unit: px.
  int64_t picture_width;
  // Its height. Unit: px.
  int64_t picture_height;
  // The rows received.
  int64_t picture_rows;
  // It is complete.
  bool picture_complete;
  // Its mode.
  sdrkit_string picture_mode;
} sdrkit_mfsk_picture_event;

// The options of `family` of `sdrkit_fsq_message_event`.
typedef enum sdrkit_fsq_message_event_family {
  // Aircraft
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FSQ_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_fsq_message_event_family;

// The options of `trigger` of `sdrkit_fsq_message_event`.
typedef enum sdrkit_fsq_message_event_trigger {
  // Text
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_TEXT = 0,
  // Asks for the signal report
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_REPORT = 1,
  // Asks for an acknowledgement
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_ACK = 2,
  // Asks for a file
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_FILE = 3,
  // Relay this message
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_RELAY = 4,
  // Repeat now
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_REPEAT = 5,
  // Repeat in 15 s
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_REPEAT_LATER = 6,
  // Store this text
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_STORE = 7,
  // Asks for the heard list
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_HEARD = 8,
  // Asks for the location
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_LOCATION = 9,
  // Asks for the message of the day
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_MOTD = 10,
  // Asks for the software version
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_VERSION = 11,
  // Picture follows
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_PICTURE = 12,
  // Alert
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_ALERT = 13,
  // Speed up
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_FASTER = 14,
  // Speed down
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_SLOWER = 15,
  // Relayed message
  SDRKIT_FSQ_MESSAGE_EVENT_TRIGGER_RELAYED = 16,
} sdrkit_fsq_message_event_trigger;

// The fields of an FSQ message.
typedef struct sdrkit_fsq_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender, when the line starts with "CALL:".
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The addressee.
  sdrkit_string target;
  // The text after the trigger.
  sdrkit_string text;
  // The family of the source: a message. A value of `sdrkit_fsq_message_event_family`.
  int32_t family;
  // The two hex digits equal the CRC-8 of the callsign.
  bool crc_ok;
  // True if `trigger` holds a value.
  bool has_trigger;
  // The trigger. A value of `sdrkit_fsq_message_event_trigger`.
  int32_t trigger;
  // Addressed to my call, allcall or cqcqcq.
  bool to_me;
} sdrkit_fsq_message_event;

// The fields of a picture received or cut off.
typedef struct sdrkit_sstv_picture_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the picture.
  int64_t picture_id;
  // Its width. Unit: px.
  int64_t picture_width;
  // Its height. Unit: px.
  int64_t picture_height;
  // The rows received.
  int64_t picture_rows;
  // It is complete.
  bool picture_complete;
  // Its mode.
  sdrkit_string picture_mode;
} sdrkit_sstv_picture_event;

// The options of `family` of `sdrkit_sstv_tx_event`.
typedef enum sdrkit_sstv_tx_event_family {
  // Aircraft
  SDRKIT_SSTV_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SSTV_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SSTV_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SSTV_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SSTV_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SSTV_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SSTV_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SSTV_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SSTV_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SSTV_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SSTV_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SSTV_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SSTV_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_sstv_tx_event_family;

// The fields of a change of the SSTV transmission.
typedef struct sdrkit_sstv_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, stopped, the transmitter is off, or why a picture was refused.
  sdrkit_string reason;
  // The mode of the picture.
  sdrkit_string mode;
  // The family of the source: a message. A value of `sdrkit_sstv_tx_event_family`.
  int32_t family;
} sdrkit_sstv_tx_event;

// The fields of a SITOR-A link event.
typedef struct sdrkit_sitor_arq_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What happened.
  sdrkit_string text;
} sdrkit_sitor_arq_event;

// The options of `family` of `sdrkit_ardop_frame_event`.
typedef enum sdrkit_ardop_frame_event_family {
  // Aircraft
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ARDOP_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_ardop_frame_event_family;

// The fields of an ARDOP frame heard.
typedef struct sdrkit_ardop_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender, if the frame names one.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The addressee, if the frame names one.
  sdrkit_string target;
  // The family of the source: a message. A value of `sdrkit_ardop_frame_event_family`.
  int32_t family;
  // The name of the type.
  sdrkit_string name;
  // The frame type.
  int64_t type;
} sdrkit_ardop_frame_event;

// The options of `state` of `sdrkit_ardop_link_event`.
typedef enum sdrkit_ardop_link_event_state {
  // Idle
  SDRKIT_ARDOP_LINK_EVENT_STATE_IDLE = 0,
  // Connecting
  SDRKIT_ARDOP_LINK_EVENT_STATE_CONNECTING = 1,
  // Connected, sending
  SDRKIT_ARDOP_LINK_EVENT_STATE_SENDING = 2,
  // Connected, receiving
  SDRKIT_ARDOP_LINK_EVENT_STATE_RECEIVING = 3,
  // Disconnecting
  SDRKIT_ARDOP_LINK_EVENT_STATE_DISCONNECTING = 4,
  // Closed
  SDRKIT_ARDOP_LINK_EVENT_STATE_CLOSED = 5,
  // Failed
  SDRKIT_ARDOP_LINK_EVENT_STATE_FAILED = 6,
} sdrkit_ardop_link_event_state;

// The options of `family` of `sdrkit_ardop_link_event`.
typedef enum sdrkit_ardop_link_event_family {
  // Aircraft
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ARDOP_LINK_EVENT_FAMILY_OTHER = 12,
} sdrkit_ardop_link_event_family;

// The fields of a change of the ARDOP link.
typedef struct sdrkit_ardop_link_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The link state after the change. A value of `sdrkit_ardop_link_event_state`.
  int32_t state;
  // The other station.
  sdrkit_string source;
  // What happened: connected, data received, the other station disconnected, a request not answered, or why the link failed.
  sdrkit_string reason;
  // The data received, as text; empty when none came.
  sdrkit_string text;
  // The family of the source: a message. A value of `sdrkit_ardop_link_event_family`.
  int32_t family;
} sdrkit_ardop_link_event;

// The fields of a ROS identification.
typedef struct sdrkit_ros_detection_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `sub_mode` holds a value.
  bool has_sub_mode;
  // The sub-mode, if the rate gives one.
  sdrkit_string sub_mode;
  // What was found.
  sdrkit_string name;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
} sdrkit_ros_detection_event;

// The options of `family` of `sdrkit_throb_text_event`.
typedef enum sdrkit_throb_text_event_family {
  // Aircraft
  SDRKIT_THROB_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_THROB_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_THROB_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_THROB_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_THROB_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_THROB_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_THROB_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_THROB_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_THROB_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_THROB_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_THROB_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_THROB_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_THROB_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_throb_text_event_family;

// The fields of the characters of one Throb reading.
typedef struct sdrkit_throb_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The characters.
  sdrkit_string text;
  // The variant.
  sdrkit_string variant;
  // The audio centre. A frequency in Hz.
  double centre_hz;
  // The family of the source: a message. A value of `sdrkit_throb_text_event_family`.
  int32_t family;
} sdrkit_throb_text_event;

// The options of `family` of `sdrkit_qrss_text_event`.
typedef enum sdrkit_qrss_text_event_family {
  // Aircraft
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_QRSS_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_qrss_text_event_family;

// The fields of a QRSS reading.
typedef struct sdrkit_qrss_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text.
  sdrkit_string text;
  // The length of an element. Unit: s.
  double element_s;
  // The tone. A frequency in Hz.
  double centre_hz;
  // The family of the source: a message. A value of `sdrkit_qrss_text_event_family`.
  int32_t family;
} sdrkit_qrss_text_event;

// The options of `family` of `sdrkit_cp16_char_event`.
typedef enum sdrkit_cp16_char_event_family {
  // Aircraft
  SDRKIT_CP16_CHAR_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CP16_CHAR_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CP16_CHAR_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CP16_CHAR_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CP16_CHAR_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CP16_CHAR_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CP16_CHAR_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CP16_CHAR_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CP16_CHAR_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CP16_CHAR_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CP16_CHAR_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CP16_CHAR_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CP16_CHAR_EVENT_FAMILY_OTHER = 12,
} sdrkit_cp16_char_event_family;

// The fields of a CP16 character read.
typedef struct sdrkit_cp16_char_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The character.
  sdrkit_string text;
  // Its score against the font.
  double score;
  // Its margin over the next character.
  double margin;
  // The margin is clear.
  bool sure;
  // The family of the source: a message. A value of `sdrkit_cp16_char_event_family`.
  int32_t family;
} sdrkit_cp16_char_event;

// The options of `family` of `sdrkit_cp16_tx_event`.
typedef enum sdrkit_cp16_tx_event_family {
  // Aircraft
  SDRKIT_CP16_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CP16_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CP16_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CP16_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CP16_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CP16_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CP16_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CP16_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CP16_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CP16_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CP16_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CP16_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CP16_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_cp16_tx_event_family;

// The fields of a change of the CP16 transmission.
typedef struct sdrkit_cp16_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state of the transmission.
  sdrkit_string state;
  // Why it changed.
  sdrkit_string reason;
  // The text sent.
  sdrkit_string text;
  // The family of the source: a message. A value of `sdrkit_cp16_tx_event_family`.
  int32_t family;
} sdrkit_cp16_tx_event;

// The fields of a WINMOR pilot.
typedef struct sdrkit_winmor_pilot_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What was heard.
  sdrkit_string text;
  // The tuning error. Unit: Hz.
  double error_hz;
  // The pilot symbols heard.
  int64_t pilot_symbols;
  // The frame-type tones.
  sdrkit_string tones;
} sdrkit_winmor_pilot_event;

// The arguments of `remove`.
typedef struct sdrkit_hfkbd_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_hfkbd_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_HFKBD_STATION_ARGS_INIT {sizeof(sdrkit_hfkbd_station_args), {0}}

// The arguments of the keyboard `send`.
typedef struct sdrkit_hfkbd_keyboard_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text to send.
  sdrkit_string text;
} sdrkit_hfkbd_keyboard_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_HFKBD_KEYBOARD_SEND_ARGS_INIT {sizeof(sdrkit_hfkbd_keyboard_send_args), {0}}

// The arguments of the FSQ `send`.
typedef struct sdrkit_fsq_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text to send.
  sdrkit_string text;
  // True if `to` holds a value.
  bool has_to;
  // The addressee.
  sdrkit_string to;
  // True if `trigger` holds a value.
  bool has_trigger;
  // The trigger. The index of an option.
  int32_t trigger;
} sdrkit_fsq_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_FSQ_SEND_ARGS_INIT {sizeof(sdrkit_fsq_send_args), {0}, 0, {0}, 0, 0}

// The arguments of the SSTV `send`.
typedef struct sdrkit_sstv_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The pixels.
  sdrkit_bytes pixels;
  // The width. Unit: px.
  int64_t width;
  // The height. Unit: px.
  int64_t height;
  // The pixel format. The index of an option.
  int32_t pixel_format;
} sdrkit_sstv_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SSTV_SEND_ARGS_INIT {sizeof(sdrkit_sstv_send_args), {0}, 0, 0, 0}

// The arguments of `ping` and `connect`.
typedef struct sdrkit_ardop_call_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The called station.
  sdrkit_string target;
} sdrkit_ardop_call_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ARDOP_CALL_ARGS_INIT {sizeof(sdrkit_ardop_call_args), {0}}

// The arguments of `send`.
typedef struct sdrkit_cp16_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text to send.
  sdrkit_string text;
} sdrkit_cp16_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_CP16_SEND_ARGS_INIT {sizeof(sdrkit_cp16_send_args), {0}}

// Fills `rows` with the changed rows of the table "stations" of the stack "rtty" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rtty_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "auto_guesses" of the stack "rtty" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rtty_auto_guesses_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_rtty_guess_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.RttyGuessRow", "auto_guesses", rows, sizeof(sdrkit_rtty_guess_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfkbd.spot"
// (Callsign heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfkbd_spot_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfkbd_spot_event *event) {
  return sdrkit_typed_event(handle, n, "hfkbd.spot", "hfkbd.SpotEvent", event, sizeof(sdrkit_hfkbd_spot_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfkbd.text"
// (Text line). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfkbd_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfkbd_text_event *event) {
  return sdrkit_typed_event(handle, n, "hfkbd.text", "hfkbd.LineEvent", event, sizeof(sdrkit_hfkbd_text_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfkbd.tx"
// (Keyboard transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfkbd_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfkbd_tx_event *event) {
  return sdrkit_typed_event(handle, n, "hfkbd.tx", "hfkbd.KeyboardTxEvent", event, sizeof(sdrkit_hfkbd_tx_event));
}

// Runs the command "remove" (Remove) of the stack "rtty" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_rtty_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "auto_lock" (Find the settings) of the stack "rtty" on the session of `handle`. Runs one analysis on up to 20 s of the kept audio, once 6 s are kept, and applies its best guess even with Auto off.
static inline int32_t sdrkit_rtty_auto_lock(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "auto_lock", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "rtty" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_rtty_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "rtty" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_rtty_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "rtty" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_rtty_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "rtty" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_rtty_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "rtty" are the sdrkit_rtty_gen_*() functions
// after the settings of its receiver.

// The setting "auto" (Automatic) of the stack "rtty". Finds the speed, the shift and the polarity from the signal. Setting one of them by hand turns it off.
static inline int32_t sdrkit_rtty_get_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto", value);
}
static inline int32_t sdrkit_rtty_set_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto", value);
}

// The setting "baud" (Speed) of the stack "rtty". A value of `sdrkit_rtty_baud`.
static inline int32_t sdrkit_rtty_get_baud(SDRKIT_HANDLE handle, sdrkit_rtty_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rtty_baud)index;
  return code;
}
static inline int32_t sdrkit_rtty_set_baud(SDRKIT_HANDLE handle, sdrkit_rtty_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "baud", (int32_t)value);
}

// The setting "shift" (Shift) of the stack "rtty". A value of `sdrkit_rtty_shift`.
static inline int32_t sdrkit_rtty_get_shift(SDRKIT_HANDLE handle, sdrkit_rtty_shift *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "shift", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rtty_shift)index;
  return code;
}
static inline int32_t sdrkit_rtty_set_shift(SDRKIT_HANDLE handle, sdrkit_rtty_shift value) {
  return sdrkit_typed_setting_set_enum(handle, "shift", (int32_t)value);
}

// The setting "reverse" (Reverse) of the stack "rtty". The mark is the lower audio tone. Off, the mark is the upper tone: the higher radio frequency on USB.
static inline int32_t sdrkit_rtty_get_reverse(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "reverse", value);
}
static inline int32_t sdrkit_rtty_set_reverse(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "reverse", value);
}

// The setting "unshift_on_space" (Unshift on space) of the stack "rtty". A space returns the figures case to letters.
static inline int32_t sdrkit_rtty_get_unshift_on_space(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "unshift_on_space", value);
}
static inline int32_t sdrkit_rtty_set_unshift_on_space(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "unshift_on_space", value);
}

// The setting "demod" (Demodulator) of the stack "rtty". A value of `sdrkit_rtty_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_rtty_get_demod(SDRKIT_HANDLE handle, sdrkit_rtty_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rtty_demod)index;
  return code;
}
static inline int32_t sdrkit_rtty_set_demod(SDRKIT_HANDLE handle, sdrkit_rtty_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "rtty". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_rtty_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_rtty_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_centre_hz" (Audio centre) of the stack "rtty". A frequency in Hz. The middle of the two tones sent: 2210 Hz gives 2125 and 2295 Hz at 170 Hz shift.
static inline int32_t sdrkit_rtty_get_tx_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", value);
}
static inline int32_t sdrkit_rtty_set_tx_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value);
}

// The setting "tx_level" (Level) of the stack "rtty". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_rtty_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_rtty_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "rtty". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_rtty_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_rtty_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "rtty". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_rtty_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_rtty_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_lower_hz" (Lower tone) of the test signal generator of the stack "rtty". A frequency in Hz.
static inline int32_t sdrkit_rtty_gen_get_lower_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lower_hz", value);
}
static inline int32_t sdrkit_rtty_gen_set_lower_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lower_hz", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "psk" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_psk_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_hfkbd_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "signals" of the stack "psk" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_psk_signals_rows(SDRKIT_HANDLE handle,
                                             sdrkit_psk_signal_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.PskSignalRow", "signals", rows, sizeof(sdrkit_psk_signal_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "psk" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_psk_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "psk" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_psk_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "psk" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_psk_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "psk" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_psk_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "psk" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_psk_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "psk" are the sdrkit_psk_gen_*() functions
// after the settings of its receiver.

// The setting "auto" (Automatic) of the stack "psk". Chooses the modulation from the signals. Choosing the modulation by hand turns it off.
static inline int32_t sdrkit_psk_get_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto", value);
}
static inline int32_t sdrkit_psk_set_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto", value);
}

// The setting "modulation" (Modulation) of the stack "psk". A value of `sdrkit_psk_modulation`.
static inline int32_t sdrkit_psk_get_modulation(SDRKIT_HANDLE handle, sdrkit_psk_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_modulation)index;
  return code;
}
static inline int32_t sdrkit_psk_set_modulation(SDRKIT_HANDLE handle, sdrkit_psk_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "modulation", (int32_t)value);
}

// The setting "threshold" (Threshold) of the stack "psk". Unit: dB. How far above the noise floor a signal must be to be decoded.
static inline int32_t sdrkit_psk_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_psk_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// The setting "focus_hz" (Focus) of the stack "psk". A frequency in Hz. The signal nearest this offset is the focus (-1 for none).
static inline int32_t sdrkit_psk_get_focus_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "focus_hz", value);
}
static inline int32_t sdrkit_psk_set_focus_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "focus_hz", value);
}

// The setting "listen" (Listen) of the stack "psk". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_psk_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_psk_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_centre_hz" (Audio frequency) of the stack "psk". A frequency in Hz. The audio frequency of the carrier sent.
static inline int32_t sdrkit_psk_get_tx_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", value);
}
static inline int32_t sdrkit_psk_set_tx_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value);
}

// The setting "tx_level" (Level) of the stack "psk". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_psk_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_psk_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "psk". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_psk_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_psk_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "psk". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_psk_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_psk_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_freq_hz" (Audio frequency) of the test signal generator of the stack "psk". A frequency in Hz.
static inline int32_t sdrkit_psk_gen_get_freq_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_freq_hz", value);
}
static inline int32_t sdrkit_psk_gen_set_freq_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_freq_hz", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "cw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cw_stations_rows(SDRKIT_HANDLE handle,
                                             sdrkit_hfkbd_station_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "signals" of the stack "cw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cw_signals_rows(SDRKIT_HANDLE handle,
                                            sdrkit_cw_signal_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.CwSignalRow", "signals", rows, sizeof(sdrkit_cw_signal_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "cw" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_cw_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "cw" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cw_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cw" are the sdrkit_cw_gen_*() functions
// after the settings of its receiver.

// The setting "threshold" (Threshold) of the stack "cw". Unit: dB. How far above the noise floor a carrier must be to be decoded.
static inline int32_t sdrkit_cw_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_cw_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// The setting "decoder" (Timing decoder) of the stack "cw". A value of `sdrkit_cw_decoder`.
static inline int32_t sdrkit_cw_get_decoder(SDRKIT_HANDLE handle, sdrkit_cw_decoder *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "decoder", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cw_decoder)index;
  return code;
}
static inline int32_t sdrkit_cw_set_decoder(SDRKIT_HANDLE handle, sdrkit_cw_decoder value) {
  return sdrkit_typed_setting_set_enum(handle, "decoder", (int32_t)value);
}

// The setting "wpm" (Speed) of the stack "cw". Unit: WPM. 0 finds the speed by itself. 5 to 50 fixes it.
static inline int32_t sdrkit_cw_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wpm", value);
}
static inline int32_t sdrkit_cw_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wpm", value);
}

// The setting "filter" (Filter width) of the stack "cw". A value of `sdrkit_cw_filter`. The channel filter of each carrier. 50 Hz is the most sensitive and passes keying up to 20 WPM, 100 Hz up to 40 WPM, 250 and 500 Hz up to 60 WPM.
static inline int32_t sdrkit_cw_get_filter(SDRKIT_HANDLE handle, sdrkit_cw_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cw_filter)index;
  return code;
}
static inline int32_t sdrkit_cw_set_filter(SDRKIT_HANDLE handle, sdrkit_cw_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "cw". A value of `sdrkit_cw_listen`. Off is silence. Tone is a clean tone at the pitch keyed as the signal listened to, and silence while there is none, as there is no key to rebuild. Signal is the signal listened to through its filter at the pitch, and the dial through the filter while there is none. The signal listened to is the focus, else the strongest signal in the table.
static inline int32_t sdrkit_cw_get_listen(SDRKIT_HANDLE handle, sdrkit_cw_listen *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "listen", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cw_listen)index;
  return code;
}
static inline int32_t sdrkit_cw_set_listen(SDRKIT_HANDLE handle, sdrkit_cw_listen value) {
  return sdrkit_typed_setting_set_enum(handle, "listen", (int32_t)value);
}

// The setting "pitch" (Pitch) of the stack "cw". A frequency in Hz. The audio frequency of the signal listened to, and of a carrier at the dial.
static inline int32_t sdrkit_cw_get_pitch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pitch", value);
}
static inline int32_t sdrkit_cw_set_pitch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pitch", value);
}

// The setting "focus_hz" (Focus) of the stack "cw". A frequency in Hz. The signal nearest this offset is the focus. A focus where no signal is starts one there, marked as picked.
static inline int32_t sdrkit_cw_get_focus_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "focus_hz", value);
}
static inline int32_t sdrkit_cw_set_focus_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "focus_hz", value);
}

// The setting "forget_s" (Forget after) of the stack "cw". Unit: s. A signal not heard for this long leaves the signals table. After 30 s without a key down it is marked gone.
static inline int32_t sdrkit_cw_get_forget_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "forget_s", value);
}
static inline int32_t sdrkit_cw_set_forget_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "forget_s", value);
}

// The setting "span" (Span) of the stack "cw". A value of `sdrkit_cw_span`. The width of the band that is watched, centred on the dial. A wider span asks the radio for a higher rate.
static inline int32_t sdrkit_cw_get_span(SDRKIT_HANDLE handle, sdrkit_cw_span *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "span", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cw_span)index;
  return code;
}
static inline int32_t sdrkit_cw_set_span(SDRKIT_HANDLE handle, sdrkit_cw_span value) {
  return sdrkit_typed_setting_set_enum(handle, "span", (int32_t)value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "cw". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_cw_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_cw_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_carrier_hz" (Carrier) of the test signal generator of the stack "cw". A frequency in Hz.
static inline int32_t sdrkit_cw_gen_get_carrier_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_hz", value);
}
static inline int32_t sdrkit_cw_gen_set_carrier_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_hz", value);
}

// The setting "gen_wpm" (Speed) of the test signal generator of the stack "cw". Unit: WPM.
static inline int32_t sdrkit_cw_gen_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm", value);
}
static inline int32_t sdrkit_cw_gen_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm", value);
}

// The setting "gen_farnsworth_wpm" (Farnsworth speed) of the test signal generator of the stack "cw". Unit: WPM.
static inline int32_t sdrkit_cw_gen_get_farnsworth_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_farnsworth_wpm", value);
}
static inline int32_t sdrkit_cw_gen_set_farnsworth_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_farnsworth_wpm", value);
}

// The setting "gen_jitter" (Jitter) of the test signal generator of the stack "cw".
static inline int32_t sdrkit_cw_gen_get_jitter(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_jitter", value);
}
static inline int32_t sdrkit_cw_gen_set_jitter(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_jitter", value);
}

// The setting "gen_signals" (Carriers) of the test signal generator of the stack "cw". 0: the default test signal. 1 to 100: carriers spread evenly over the span, each with its own call sign.
static inline int32_t sdrkit_cw_gen_get_signals(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_signals", value);
}
static inline int32_t sdrkit_cw_gen_set_signals(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_signals", value);
}

// The setting "gen_wpm_max" (Top speed) of the test signal generator of the stack "cw". Unit: WPM. With several carriers, the speeds step from the speed up to this (0: all at the speed).
static inline int32_t sdrkit_cw_gen_get_wpm_max(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm_max", value);
}
static inline int32_t sdrkit_cw_gen_set_wpm_max(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm_max", value);
}

// The setting "gen_level_spread_db" (Level spread) of the test signal generator of the stack "cw". Unit: dB. With several carriers, the levels step down evenly over this many dB.
static inline int32_t sdrkit_cw_gen_get_level_spread_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_level_spread_db", value);
}
static inline int32_t sdrkit_cw_gen_set_level_spread_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_level_spread_db", value);
}

// The setting "gen_noise_only" (Noise only) of the test signal generator of the stack "cw". No carrier: only the noise of the test signal settings, as loud as with the carriers.
static inline int32_t sdrkit_cw_gen_get_noise_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_noise_only", value);
}
static inline int32_t sdrkit_cw_gen_set_noise_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_noise_only", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "olivia" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_olivia_stations_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_hfkbd_station_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "olivia" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_olivia_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "olivia" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_olivia_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "olivia" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_olivia_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "olivia" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_olivia_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "olivia" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_olivia_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "olivia" are the sdrkit_olivia_gen_*() functions
// after the settings of its receiver.

// The setting "tones" (Tones) of the stack "olivia". A value of `sdrkit_olivia_tones`.
static inline int32_t sdrkit_olivia_get_tones(SDRKIT_HANDLE handle, sdrkit_olivia_tones *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tones", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_olivia_tones)index;
  return code;
}
static inline int32_t sdrkit_olivia_set_tones(SDRKIT_HANDLE handle, sdrkit_olivia_tones value) {
  return sdrkit_typed_setting_set_enum(handle, "tones", (int32_t)value);
}

// The setting "bandwidth" (Bandwidth) of the stack "olivia". A value of `sdrkit_olivia_bandwidth`.
static inline int32_t sdrkit_olivia_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_olivia_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_olivia_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_olivia_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_olivia_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "centre" (Audio centre) of the stack "olivia". A frequency in Hz.
static inline int32_t sdrkit_olivia_get_centre(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre", value);
}
static inline int32_t sdrkit_olivia_set_centre(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre", value);
}

// The setting "find_signal" (Find signal) of the stack "olivia". The centre follows the signal while nothing is decoding.
static inline int32_t sdrkit_olivia_get_find_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "find_signal", value);
}
static inline int32_t sdrkit_olivia_set_find_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "find_signal", value);
}

// The setting "demod" (Demodulator) of the stack "olivia". A value of `sdrkit_olivia_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_olivia_get_demod(SDRKIT_HANDLE handle, sdrkit_olivia_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_olivia_demod)index;
  return code;
}
static inline int32_t sdrkit_olivia_set_demod(SDRKIT_HANDLE handle, sdrkit_olivia_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "olivia". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_olivia_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_olivia_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_level" (Level) of the stack "olivia". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_olivia_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_olivia_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "olivia". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_olivia_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_olivia_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "olivia". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_olivia_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_olivia_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "contestia" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_contestia_stations_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_hfkbd_station_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "contestia" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_contestia_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "contestia" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_contestia_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "contestia" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_contestia_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "contestia" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_contestia_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "contestia" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_contestia_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "contestia" are the sdrkit_contestia_gen_*() functions
// after the settings of its receiver.

// The setting "tones" (Tones) of the stack "contestia". A value of `sdrkit_contestia_tones`.
static inline int32_t sdrkit_contestia_get_tones(SDRKIT_HANDLE handle, sdrkit_contestia_tones *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tones", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_contestia_tones)index;
  return code;
}
static inline int32_t sdrkit_contestia_set_tones(SDRKIT_HANDLE handle, sdrkit_contestia_tones value) {
  return sdrkit_typed_setting_set_enum(handle, "tones", (int32_t)value);
}

// The setting "bandwidth" (Bandwidth) of the stack "contestia". A value of `sdrkit_contestia_bandwidth`.
static inline int32_t sdrkit_contestia_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_contestia_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_contestia_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_contestia_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_contestia_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "centre" (Audio centre) of the stack "contestia". A frequency in Hz.
static inline int32_t sdrkit_contestia_get_centre(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre", value);
}
static inline int32_t sdrkit_contestia_set_centre(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre", value);
}

// The setting "find_signal" (Find signal) of the stack "contestia". The centre follows the signal while nothing is decoding.
static inline int32_t sdrkit_contestia_get_find_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "find_signal", value);
}
static inline int32_t sdrkit_contestia_set_find_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "find_signal", value);
}

// The setting "demod" (Demodulator) of the stack "contestia". A value of `sdrkit_contestia_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_contestia_get_demod(SDRKIT_HANDLE handle, sdrkit_contestia_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_contestia_demod)index;
  return code;
}
static inline int32_t sdrkit_contestia_set_demod(SDRKIT_HANDLE handle, sdrkit_contestia_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "contestia". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_contestia_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_contestia_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_level" (Level) of the stack "contestia". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_contestia_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_contestia_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "contestia". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_contestia_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_contestia_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "contestia". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_contestia_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_contestia_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "mfsk" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_mfsk_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mfsk.picture"
// (Picture received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mfsk_picture_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mfsk_picture_event *event) {
  return sdrkit_typed_event(handle, n, "mfsk.picture", "hfkbd.PictureEvent", event, sizeof(sdrkit_mfsk_picture_event));
}

// Runs the command "remove" (Remove) of the stack "mfsk" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_mfsk_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "mfsk" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_mfsk_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "mfsk" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_mfsk_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "mfsk" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_mfsk_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "mfsk" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_mfsk_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "mfsk" are the sdrkit_mfsk_gen_*() functions
// after the settings of its receiver.

// The setting "variant" (Variant) of the stack "mfsk". A value of `sdrkit_mfsk_variant`.
static inline int32_t sdrkit_mfsk_get_variant(SDRKIT_HANDLE handle, sdrkit_mfsk_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mfsk_variant)index;
  return code;
}
static inline int32_t sdrkit_mfsk_set_variant(SDRKIT_HANDLE handle, sdrkit_mfsk_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "variant", (int32_t)value);
}

// The setting "centre" (Audio centre) of the stack "mfsk". A frequency in Hz.
static inline int32_t sdrkit_mfsk_get_centre(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre", value);
}
static inline int32_t sdrkit_mfsk_set_centre(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre", value);
}

// The setting "find_signal" (Find signal) of the stack "mfsk". The centre follows the signal while nothing is decoding.
static inline int32_t sdrkit_mfsk_get_find_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "find_signal", value);
}
static inline int32_t sdrkit_mfsk_set_find_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "find_signal", value);
}

// The setting "demod" (Demodulator) of the stack "mfsk". A value of `sdrkit_mfsk_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_mfsk_get_demod(SDRKIT_HANDLE handle, sdrkit_mfsk_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mfsk_demod)index;
  return code;
}
static inline int32_t sdrkit_mfsk_set_demod(SDRKIT_HANDLE handle, sdrkit_mfsk_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "mfsk". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_mfsk_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_mfsk_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_level" (Level) of the stack "mfsk". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_mfsk_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_mfsk_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "mfsk". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_mfsk_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_mfsk_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "mfsk". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_mfsk_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_mfsk_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_picture_width" (Picture width) of the test signal generator of the stack "mfsk". Unit: px. A grey test picture of this width follows 20 characters. 0 sends none.
static inline int32_t sdrkit_mfsk_gen_get_picture_width(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_picture_width", value);
}
static inline int32_t sdrkit_mfsk_gen_set_picture_width(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_picture_width", value);
}

// The setting "gen_picture_height" (Picture height) of the test signal generator of the stack "mfsk". Unit: px.
static inline int32_t sdrkit_mfsk_gen_get_picture_height(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_picture_height", value);
}
static inline int32_t sdrkit_mfsk_gen_set_picture_height(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_picture_height", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "dominoex" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dominoex_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_hfkbd_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "dominoex" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dominoex_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "dominoex" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dominoex_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "dominoex" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_dominoex_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "dominoex" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_dominoex_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "dominoex" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_dominoex_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dominoex" are the sdrkit_dominoex_gen_*() functions
// after the settings of its receiver.

// The setting "variant" (Variant) of the stack "dominoex". A value of `sdrkit_dominoex_variant`.
static inline int32_t sdrkit_dominoex_get_variant(SDRKIT_HANDLE handle, sdrkit_dominoex_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dominoex_variant)index;
  return code;
}
static inline int32_t sdrkit_dominoex_set_variant(SDRKIT_HANDLE handle, sdrkit_dominoex_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "variant", (int32_t)value);
}

// The setting "centre" (Audio centre) of the stack "dominoex". A frequency in Hz.
static inline int32_t sdrkit_dominoex_get_centre(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre", value);
}
static inline int32_t sdrkit_dominoex_set_centre(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre", value);
}

// The setting "find_signal" (Find signal) of the stack "dominoex". The centre follows the signal while nothing is decoding.
static inline int32_t sdrkit_dominoex_get_find_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "find_signal", value);
}
static inline int32_t sdrkit_dominoex_set_find_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "find_signal", value);
}

// The setting "demod" (Demodulator) of the stack "dominoex". A value of `sdrkit_dominoex_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_dominoex_get_demod(SDRKIT_HANDLE handle, sdrkit_dominoex_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dominoex_demod)index;
  return code;
}
static inline int32_t sdrkit_dominoex_set_demod(SDRKIT_HANDLE handle, sdrkit_dominoex_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "dominoex". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_dominoex_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_dominoex_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_level" (Level) of the stack "dominoex". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_dominoex_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_dominoex_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "dominoex". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_dominoex_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_dominoex_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "dominoex". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_dominoex_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_dominoex_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "thor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_thor_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "thor" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_thor_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "thor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_thor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "thor" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_thor_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "thor" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_thor_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "thor" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_thor_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "thor" are the sdrkit_thor_gen_*() functions
// after the settings of its receiver.

// The setting "variant" (Variant) of the stack "thor". A value of `sdrkit_thor_variant`.
static inline int32_t sdrkit_thor_get_variant(SDRKIT_HANDLE handle, sdrkit_thor_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_thor_variant)index;
  return code;
}
static inline int32_t sdrkit_thor_set_variant(SDRKIT_HANDLE handle, sdrkit_thor_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "variant", (int32_t)value);
}

// The setting "centre" (Audio centre) of the stack "thor". A frequency in Hz.
static inline int32_t sdrkit_thor_get_centre(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre", value);
}
static inline int32_t sdrkit_thor_set_centre(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre", value);
}

// The setting "find_signal" (Find signal) of the stack "thor". The centre follows the signal while nothing is decoding.
static inline int32_t sdrkit_thor_get_find_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "find_signal", value);
}
static inline int32_t sdrkit_thor_set_find_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "find_signal", value);
}

// The setting "demod" (Demodulator) of the stack "thor". A value of `sdrkit_thor_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_thor_get_demod(SDRKIT_HANDLE handle, sdrkit_thor_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_thor_demod)index;
  return code;
}
static inline int32_t sdrkit_thor_set_demod(SDRKIT_HANDLE handle, sdrkit_thor_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "thor". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_thor_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_thor_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_level" (Level) of the stack "thor". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_thor_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_thor_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "thor". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_thor_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_thor_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "thor". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_thor_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_thor_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// Runs the command "clear" (Clear) of the stack "hell" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_hell_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "hell" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_hell_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "hell" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_hell_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "hell" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_hell_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hell" are the sdrkit_hell_gen_*() functions
// after the settings of its receiver.

// The setting "variant" (Variant) of the stack "hell". A value of `sdrkit_hell_variant`.
static inline int32_t sdrkit_hell_get_variant(SDRKIT_HANDLE handle, sdrkit_hell_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hell_variant)index;
  return code;
}
static inline int32_t sdrkit_hell_set_variant(SDRKIT_HANDLE handle, sdrkit_hell_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "variant", (int32_t)value);
}

// The setting "centre" (Audio centre) of the stack "hell". A frequency in Hz.
static inline int32_t sdrkit_hell_get_centre(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre", value);
}
static inline int32_t sdrkit_hell_set_centre(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre", value);
}

// The setting "find_signal" (Find signal) of the stack "hell". The centre follows the signal while nothing is decoding.
static inline int32_t sdrkit_hell_get_find_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "find_signal", value);
}
static inline int32_t sdrkit_hell_set_find_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "find_signal", value);
}

// The setting "demod" (Demodulator) of the stack "hell". A value of `sdrkit_hell_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_hell_get_demod(SDRKIT_HANDLE handle, sdrkit_hell_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hell_demod)index;
  return code;
}
static inline int32_t sdrkit_hell_set_demod(SDRKIT_HANDLE handle, sdrkit_hell_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "hell". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_hell_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_hell_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_level" (Level) of the stack "hell". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_hell_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_hell_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "hell". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_hell_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_hell_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "font" (Font file) of the stack "hell". A TrueType font file (or OpenType with TrueType outlines) that the host names. The characters that the Hell glyphs do not have are sent with it, 14 pixels high; ASCII keeps the Hell glyphs. Empty, or a file that cannot be read: the Hell glyphs alone, and ? for the rest.
static inline int32_t sdrkit_hell_get_font(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "font", value);
}
static inline int32_t sdrkit_hell_set_font(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "font", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "hell". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_hell_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_hell_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "mt63" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_mt63_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "mt63" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_mt63_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "mt63" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_mt63_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "mt63" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_mt63_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "mt63" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_mt63_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "mt63" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_mt63_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "mt63" are the sdrkit_mt63_gen_*() functions
// after the settings of its receiver.

// The setting "bandwidth" (Bandwidth) of the stack "mt63". A value of `sdrkit_mt63_bandwidth`.
static inline int32_t sdrkit_mt63_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_mt63_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mt63_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_mt63_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_mt63_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "interleave" (Interleave) of the stack "mt63". A value of `sdrkit_mt63_interleave`.
static inline int32_t sdrkit_mt63_get_interleave(SDRKIT_HANDLE handle, sdrkit_mt63_interleave *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "interleave", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mt63_interleave)index;
  return code;
}
static inline int32_t sdrkit_mt63_set_interleave(SDRKIT_HANDLE handle, sdrkit_mt63_interleave value) {
  return sdrkit_typed_setting_set_enum(handle, "interleave", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "mt63". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_mt63_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_mt63_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_low_hz" (Lowest carrier) of the stack "mt63". A frequency in Hz. The audio frequency of carrier 0; the 64 carriers run up from it, one band width / 64 apart.
static inline int32_t sdrkit_mt63_get_tx_low_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_low_hz", value);
}
static inline int32_t sdrkit_mt63_set_tx_low_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_low_hz", value);
}

// The setting "tx_level" (Level) of the stack "mt63". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_mt63_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_mt63_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "mt63". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_mt63_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_mt63_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "mt63". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_mt63_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_mt63_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_centre_hz" (Audio centre) of the test signal generator of the stack "mt63". A frequency in Hz.
static inline int32_t sdrkit_mt63_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_mt63_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "fsq" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsq_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_hfkbd_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "fsq" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsq_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_fsq_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.FsqMessageRow", "messages", rows, sizeof(sdrkit_fsq_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fsq.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fsq_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fsq_message_event *event) {
  return sdrkit_typed_event(handle, n, "fsq.message", "hfkbd.FsqMessageEvent", event, sizeof(sdrkit_fsq_message_event));
}

// Runs the command "remove" (Remove) of the stack "fsq" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_fsq_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "fsq" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fsq_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a line) of the stack "fsq" on the session of `handle`. Queues one line from the station callsign with its check, to the addressee with the trigger when one is given. Transmit must be on and the station callsign set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_fsq_send(SDRKIT_HANDLE handle, const sdrkit_fsq_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.FsqSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "fsq" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_fsq_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "fsq" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_fsq_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fsq" are the sdrkit_fsq_gen_*() functions
// after the settings of its receiver.

// The setting "speed" (Speed) of the stack "fsq". A value of `sdrkit_fsq_speed`.
static inline int32_t sdrkit_fsq_get_speed(SDRKIT_HANDLE handle, sdrkit_fsq_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsq_speed)index;
  return code;
}
static inline int32_t sdrkit_fsq_set_speed(SDRKIT_HANDLE handle, sdrkit_fsq_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "speed", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "fsq". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_fsq_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_fsq_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_centre_hz" (Audio centre) of the stack "fsq". A frequency in Hz. The middle of the 33 tones sent.
static inline int32_t sdrkit_fsq_get_tx_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", value);
}
static inline int32_t sdrkit_fsq_set_tx_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value);
}

// The setting "tx_level" (Level) of the stack "fsq". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_fsq_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_fsq_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "fsq". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_fsq_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_fsq_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "fsq". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_fsq_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_fsq_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_centre_hz" (Audio centre) of the test signal generator of the stack "fsq". A frequency in Hz.
static inline int32_t sdrkit_fsq_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_fsq_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "ifkp" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ifkp_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Runs the command "remove" (Remove) of the stack "ifkp" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ifkp_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "ifkp" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ifkp_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "ifkp" on the session of `handle`. Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ifkp_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "ifkp" on the session of `handle`. Drops the text waiting and ends the signal with a 10 ms fade.
static inline int32_t sdrkit_ifkp_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "ifkp" on the session of `handle`. Sends an unmodulated carrier at the audio centre for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ifkp_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ifkp" are the sdrkit_ifkp_gen_*() functions
// after the settings of its receiver.

// The setting "speed" (Speed) of the stack "ifkp". A value of `sdrkit_ifkp_speed`.
static inline int32_t sdrkit_ifkp_get_speed(SDRKIT_HANDLE handle, sdrkit_ifkp_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ifkp_speed)index;
  return code;
}
static inline int32_t sdrkit_ifkp_set_speed(SDRKIT_HANDLE handle, sdrkit_ifkp_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "speed", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "ifkp". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_ifkp_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ifkp_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_centre_hz" (Audio centre) of the stack "ifkp". A frequency in Hz. The middle of the 33 tones sent.
static inline int32_t sdrkit_ifkp_get_tx_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_centre_hz", value);
}
static inline int32_t sdrkit_ifkp_set_tx_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_centre_hz", value);
}

// The setting "tx_level" (Level) of the stack "ifkp". Unit: of full scale. The peak magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_ifkp_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_ifkp_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "ifkp". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_ifkp_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_ifkp_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "ifkp". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_ifkp_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_ifkp_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_centre_hz" (Audio centre) of the test signal generator of the stack "ifkp". A frequency in Hz.
static inline int32_t sdrkit_ifkp_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_ifkp_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sstv.picture"
// (Picture received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sstv_picture_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sstv_picture_event *event) {
  return sdrkit_typed_event(handle, n, "sstv.picture", "hfkbd.PictureEvent", event, sizeof(sdrkit_sstv_picture_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sstv.tx"
// (SSTV transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sstv_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sstv_tx_event *event) {
  return sdrkit_typed_event(handle, n, "sstv.tx", "hfkbd.SstvTxEvent", event, sizeof(sdrkit_sstv_tx_event));
}

// Runs the command "clear" (Clear) of the stack "sstv" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_sstv_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a picture) of the stack "sstv" on the session of `handle`. Scales the picture to the transmit mode by the fit setting and sends it: the VIS header, then the lines; again after each pause while repeat is on. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_sstv_send(SDRKIT_HANDLE handle, const sdrkit_sstv_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.SstvSendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "sstv" on the session of `handle`. Ends the picture at once.
static inline int32_t sdrkit_sstv_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sstv" are the sdrkit_sstv_gen_*() functions
// after the settings of its receiver.

// The setting "picture_filter" (Picture filter) of the stack "sstv". Smooths the noise of a weak picture along each scan line and over 3 x 3 pixels, with the noise measured on the syncs and in the picture. A clean picture is left as it is.
static inline int32_t sdrkit_sstv_get_picture_filter(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "picture_filter", value);
}
static inline int32_t sdrkit_sstv_set_picture_filter(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "picture_filter", value);
}

// The setting "demod" (Demodulator) of the stack "sstv". A value of `sdrkit_sstv_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_sstv_get_demod(SDRKIT_HANDLE handle, sdrkit_sstv_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sstv_demod)index;
  return code;
}
static inline int32_t sdrkit_sstv_set_demod(SDRKIT_HANDLE handle, sdrkit_sstv_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "sstv". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_sstv_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_sstv_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "tx_mode" (Transmit mode) of the stack "sstv". A value of `sdrkit_sstv_tx_mode`. The SSTV mode of the pictures sent.
static inline int32_t sdrkit_sstv_get_tx_mode(SDRKIT_HANDLE handle, sdrkit_sstv_tx_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sstv_tx_mode)index;
  return code;
}
static inline int32_t sdrkit_sstv_set_tx_mode(SDRKIT_HANDLE handle, sdrkit_sstv_tx_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_mode", (int32_t)value);
}

// The setting "fit" (Fit) of the stack "sstv". A value of `sdrkit_sstv_fit`. How a picture of another shape than the mode is fitted: whole with black bars, cut to the shape of the mode, or stretched.
static inline int32_t sdrkit_sstv_get_fit(SDRKIT_HANDLE handle, sdrkit_sstv_fit *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fit", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sstv_fit)index;
  return code;
}
static inline int32_t sdrkit_sstv_set_fit(SDRKIT_HANDLE handle, sdrkit_sstv_fit value) {
  return sdrkit_typed_setting_set_enum(handle, "fit", (int32_t)value);
}

// The setting "repeat" (Repeat) of the stack "sstv". Sends the picture again after each pause until stop. Off at the start: a picture is sent once.
static inline int32_t sdrkit_sstv_get_repeat(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "repeat", value);
}
static inline int32_t sdrkit_sstv_set_repeat(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "repeat", value);
}

// The setting "pause_s" (Pause between pictures) of the stack "sstv". A length of time in seconds. The time between the end of a picture and the start of the next one when repeat is on. The transmitter is off in the pause.
static inline int32_t sdrkit_sstv_get_pause_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pause_s", value);
}
static inline int32_t sdrkit_sstv_set_pause_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pause_s", value);
}

// The setting "gen_mode" (Picture mode) of the test signal generator of the stack "sstv". A value of `sdrkit_sstv_gen_mode`. The SSTV mode of the test picture.
static inline int32_t sdrkit_sstv_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_sstv_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sstv_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_sstv_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_sstv_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_vis" (Send the header) of the test signal generator of the stack "sstv". Off: the picture starts without its VIS header, so the stack finds it by the line period.
static inline int32_t sdrkit_sstv_gen_get_vis(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_vis", value);
}
static inline int32_t sdrkit_sstv_gen_set_vis(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_vis", value);
}

// The setting "gen_picture" (Test picture) of the test signal generator of the stack "sstv". A value of `sdrkit_sstv_gen_picture`.
static inline int32_t sdrkit_sstv_gen_get_picture(SDRKIT_HANDLE handle, sdrkit_sstv_gen_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sstv_gen_picture)index;
  return code;
}
static inline int32_t sdrkit_sstv_gen_set_picture(SDRKIT_HANDLE handle, sdrkit_sstv_gen_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_picture", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "sitor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sitor_stations_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hfkbd_station_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sitor.arq"
// (SITOR-A link event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sitor_arq_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sitor_arq_event *event) {
  return sdrkit_typed_event(handle, n, "sitor.arq", "hfkbd.SitorArqEvent", event, sizeof(sdrkit_sitor_arq_event));
}

// Runs the command "remove" (Remove) of the stack "sitor" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_sitor_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "sitor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_sitor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sitor" are the sdrkit_sitor_gen_*() functions
// after the settings of its receiver.

// The setting "listen" (Listen) of the stack "sitor". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_sitor_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_sitor_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "sitor". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_sitor_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_sitor_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_lower_hz" (Lower tone) of the test signal generator of the stack "sitor". A frequency in Hz.
static inline int32_t sdrkit_sitor_gen_get_lower_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lower_hz", value);
}
static inline int32_t sdrkit_sitor_gen_set_lower_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lower_hz", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "ardop" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ardop_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ardop_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.ArdopFrameRow", "frames", rows, sizeof(sdrkit_ardop_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "ardop" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ardop_stations_rows(SDRKIT_HANDLE handle,
                                                sdrkit_ardop_station_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.ArdopStationRow", "stations", rows, sizeof(sdrkit_ardop_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ardop.frame"
// (Frame heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ardop_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ardop_frame_event *event) {
  return sdrkit_typed_event(handle, n, "ardop.frame", "hfkbd.ArdopFrameEvent", event, sizeof(sdrkit_ardop_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ardop.link"
// (ARDOP link). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ardop_link_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ardop_link_event *event) {
  return sdrkit_typed_event(handle, n, "ardop.link", "hfkbd.ArdopLinkEvent", event, sizeof(sdrkit_ardop_link_event));
}

// Runs the command "remove" (Remove) of the stack "ardop" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ardop_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "ardop" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ardop_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "ardop" on the session of `handle`. While a link is up, the text joins the data of the link. Otherwise it is sent once as FEC data frames of the FEC frame type, to no station. Transmit must be on and the station callsign set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ardop_send(SDRKIT_HANDLE handle, const sdrkit_hfkbd_keyboard_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.KeyboardSendArgs", args, NULL, NULL, 0);
}

// Runs the command "send_id" (Send the ID) of the stack "ardop" on the session of `handle`. Sends one ID frame with the station callsign and locator. Transmit must be on and the station callsign set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ardop_send_id(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send_id", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "ping" (Ping) of the stack "ardop" on the session of `handle`. Sends one Ping frame from the station callsign to the called station. Transmit must be on and the station callsign set. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ardop_ping(SDRKIT_HANDLE handle, const sdrkit_ardop_call_args *args) {
  return sdrkit_typed_command(handle, "ping", "hfkbd.ArdopCallArgs", args, NULL, NULL, 0);
}

// Runs the command "connect" (Connect) of the stack "ardop" on the session of `handle`. Calls the station with ConReq500M, every 9 s until it answers, at most 11 times. On the answer the link is up: the text of `send` goes as 4FSK.500.100 data frames, each sent again until the other station acknowledges it. Transmit must be on and the station callsign set. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ardop_connect(SDRKIT_HANDLE handle, const sdrkit_ardop_call_args *args) {
  return sdrkit_typed_command(handle, "connect", "hfkbd.ArdopCallArgs", args, NULL, NULL, 0);
}

// Runs the command "disconnect" (Disconnect) of the stack "ardop" on the session of `handle`. Ends the link once the data waiting is sent: DISC, answered by END. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ardop_disconnect(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "disconnect", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "ardop" on the session of `handle`. Ends the burst on air at once, drops the bursts waiting and closes the link without DISC.
static inline int32_t sdrkit_ardop_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The setting "demod" (Demodulator) of the stack "ardop". A value of `sdrkit_ardop_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_ardop_get_demod(SDRKIT_HANDLE handle, sdrkit_ardop_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ardop_demod)index;
  return code;
}
static inline int32_t sdrkit_ardop_set_demod(SDRKIT_HANDLE handle, sdrkit_ardop_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "ardop". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_ardop_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ardop_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "leader_ms" (Leader) of the stack "ardop". Unit: ms. The two-tone leader before each frame, 100 to 1000 ms in steps of 20. A receiver needs about 100 ms to find it.
static inline int32_t sdrkit_ardop_get_leader_ms(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "leader_ms", value);
}
static inline int32_t sdrkit_ardop_set_leader_ms(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "leader_ms", value);
}

// The setting "fec_type" (FEC frame type) of the stack "ardop". A value of `sdrkit_ardop_fec_type`. The data frames of a text sent with no link: 4FSK.200.50S (16 bytes a frame, 200 Hz), 4FSK.500.100S (32 bytes) or 4FSK.500.100 (64 bytes, 500 Hz).
static inline int32_t sdrkit_ardop_get_fec_type(SDRKIT_HANDLE handle, sdrkit_ardop_fec_type *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fec_type", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ardop_fec_type)index;
  return code;
}
static inline int32_t sdrkit_ardop_set_fec_type(SDRKIT_HANDLE handle, sdrkit_ardop_fec_type value) {
  return sdrkit_typed_setting_set_enum(handle, "fec_type", (int32_t)value);
}

// The setting "auto_answer" (Answer calls) of the stack "ardop". Answers a connect request to the station callsign with ConAck500 and runs the link as the called station, with no key press. Off at the start, so that nothing is sent unattended: a request is then shown and not answered. The engine's transmit switch must be on.
static inline int32_t sdrkit_ardop_get_auto_answer(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_answer", value);
}
static inline int32_t sdrkit_ardop_set_auto_answer(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_answer", value);
}

// Fills `rows` with the changed rows of the table "detections" of the stack "ros" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ros_detections_rows(SDRKIT_HANDLE handle,
                                                sdrkit_ros_detection_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.RosDetectionRow", "detections", rows, sizeof(sdrkit_ros_detection_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ros.detection"
// (ROS identified). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ros_detection_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ros_detection_event *event) {
  return sdrkit_typed_event(handle, n, "ros.detection", "hfkbd.RosDetectionEvent", event, sizeof(sdrkit_ros_detection_event));
}

// Runs the command "clear" (Clear) of the stack "ros" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ros_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ros" are the sdrkit_ros_gen_*() functions
// after the settings of its receiver.

// The setting "demod" (Demodulator) of the stack "ros". A value of `sdrkit_ros_demod`. How the sideband audio is made. USB is usual on HF.
static inline int32_t sdrkit_ros_get_demod(SDRKIT_HANDLE handle, sdrkit_ros_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ros_demod)index;
  return code;
}
static inline int32_t sdrkit_ros_set_demod(SDRKIT_HANDLE handle, sdrkit_ros_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "ros". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_ros_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ros_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_signal" (Test signal) of the test signal generator of the stack "ros". A value of `sdrkit_ros_gen_signal`.
static inline int32_t sdrkit_ros_gen_get_signal(SDRKIT_HANDLE handle, sdrkit_ros_gen_signal *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ros_gen_signal)index;
  return code;
}
static inline int32_t sdrkit_ros_gen_set_signal(SDRKIT_HANDLE handle, sdrkit_ros_gen_signal value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_signal", (int32_t)value);
}

// The setting "gen_baud" (Symbol rate) of the test signal generator of the stack "ros". Unit: Bd.
static inline int32_t sdrkit_ros_gen_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud", value);
}
static inline int32_t sdrkit_ros_gen_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "throb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_throb_stations_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hfkbd_station_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "throb.text"
// (Throb characters). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_throb_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_throb_text_event *event) {
  return sdrkit_typed_event(handle, n, "throb.text", "hfkbd.ThrobTextEvent", event, sizeof(sdrkit_throb_text_event));
}

// Runs the command "remove" (Remove) of the stack "throb" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_throb_remove(SDRKIT_HANDLE handle, const sdrkit_hfkbd_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfkbd.StationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "throb" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_throb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "throb" are the sdrkit_throb_gen_*() functions
// after the settings of its receiver.

// The setting "variant" (Variant) of the stack "throb". A value of `sdrkit_throb_variant`. Automatic finds the variant from the signal. Throb1 also reads Throb2, ThrobX1 also ThrobX2.
static inline int32_t sdrkit_throb_get_variant(SDRKIT_HANDLE handle, sdrkit_throb_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_throb_variant)index;
  return code;
}
static inline int32_t sdrkit_throb_set_variant(SDRKIT_HANDLE handle, sdrkit_throb_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "variant", (int32_t)value);
}

// The setting "demod" (Demodulator) of the stack "throb". A value of `sdrkit_throb_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_throb_get_demod(SDRKIT_HANDLE handle, sdrkit_throb_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_throb_demod)index;
  return code;
}
static inline int32_t sdrkit_throb_set_demod(SDRKIT_HANDLE handle, sdrkit_throb_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "throb". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_throb_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_throb_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "throb". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_throb_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_throb_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_variant" (Test signal variant) of the test signal generator of the stack "throb". A value of `sdrkit_throb_gen_variant`.
static inline int32_t sdrkit_throb_gen_get_variant(SDRKIT_HANDLE handle, sdrkit_throb_gen_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_throb_gen_variant)index;
  return code;
}
static inline int32_t sdrkit_throb_gen_set_variant(SDRKIT_HANDLE handle, sdrkit_throb_gen_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_variant", (int32_t)value);
}

// The setting "gen_centre_hz" (Audio centre) of the test signal generator of the stack "throb". A frequency in Hz.
static inline int32_t sdrkit_throb_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_throb_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "qrss" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qrss_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "qrss.text"
// (QRSS text). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_qrss_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_qrss_text_event *event) {
  return sdrkit_typed_event(handle, n, "qrss.text", "hfkbd.QrssTextEvent", event, sizeof(sdrkit_qrss_text_event));
}

// Runs the command "clear" (Clear) of the stack "qrss" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_qrss_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "qrss" are the sdrkit_qrss_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Keying) of the stack "qrss". A value of `sdrkit_qrss_submode`. QRSS: on-off keying. DFCW: dots and dashes on two frequencies.
static inline int32_t sdrkit_qrss_get_submode(SDRKIT_HANDLE handle, sdrkit_qrss_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qrss_submode)index;
  return code;
}
static inline int32_t sdrkit_qrss_set_submode(SDRKIT_HANDLE handle, sdrkit_qrss_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "auto" (Automatic centre and element length) of the stack "qrss".
static inline int32_t sdrkit_qrss_get_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto", value);
}
static inline int32_t sdrkit_qrss_set_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto", value);
}

// The setting "centre_hz" (Tone) of the stack "qrss". A frequency in Hz. Setting it turns the automatic centre off.
static inline int32_t sdrkit_qrss_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre_hz", value);
}
static inline int32_t sdrkit_qrss_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre_hz", value);
}

// The setting "element_s" (Element length) of the stack "qrss". Unit: s. 0 finds the length from the keying; else 0.5 to 120 s.
static inline int32_t sdrkit_qrss_get_element_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "element_s", value);
}
static inline int32_t sdrkit_qrss_set_element_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "element_s", value);
}

// The setting "span_hz" (Window width) of the stack "qrss". A frequency in Hz.
static inline int32_t sdrkit_qrss_get_span_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "span_hz", value);
}
static inline int32_t sdrkit_qrss_set_span_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "span_hz", value);
}

// The setting "dash_below" (Dash below the dot frequency) of the stack "qrss".
static inline int32_t sdrkit_qrss_get_dash_below(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "dash_below", value);
}
static inline int32_t sdrkit_qrss_set_dash_below(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "dash_below", value);
}

// The setting "demod" (Demodulator) of the stack "qrss". A value of `sdrkit_qrss_demod`. How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m.
static inline int32_t sdrkit_qrss_get_demod(SDRKIT_HANDLE handle, sdrkit_qrss_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qrss_demod)index;
  return code;
}
static inline int32_t sdrkit_qrss_set_demod(SDRKIT_HANDLE handle, sdrkit_qrss_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "listen" (Listen) of the stack "qrss". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_qrss_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_qrss_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_message" (Test text) of the test signal generator of the stack "qrss". The text that the test signal sends. Empty: the stack's own test text.
static inline int32_t sdrkit_qrss_gen_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_message", value);
}
static inline int32_t sdrkit_qrss_gen_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_message", value);
}

// The setting "gen_element_s" (Test signal element length) of the test signal generator of the stack "qrss". Unit: s. 0: 3 s for QRSS, 6 s for DFCW.
static inline int32_t sdrkit_qrss_gen_get_element_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_element_s", value);
}
static inline int32_t sdrkit_qrss_gen_set_element_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_element_s", value);
}

// The setting "gen_shift_hz" (Test signal DFCW shift) of the test signal generator of the stack "qrss". A frequency in Hz. The dash frequency above the dot frequency (negative: below).
static inline int32_t sdrkit_qrss_gen_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", value);
}
static inline int32_t sdrkit_qrss_gen_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "cp16" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cp16_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfkbd_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.StationRow", "stations", rows, sizeof(sdrkit_hfkbd_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cp16.char"
// (CP16 character). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cp16_char_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cp16_char_event *event) {
  return sdrkit_typed_event(handle, n, "cp16.char", "hfkbd.Cp16CharEvent", event, sizeof(sdrkit_cp16_char_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cp16.tx"
// (CP16 transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cp16_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cp16_tx_event *event) {
  return sdrkit_typed_event(handle, n, "cp16.tx", "hfkbd.Cp16TxEvent", event, sizeof(sdrkit_cp16_tx_event));
}

// Runs the command "clear" (Clear) of the stack "cp16" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cp16_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a text) of the stack "cp16" on the session of `handle`. Sends the text with the stack's spacing, lowest carrier, seconds a character and sideband. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_cp16_send(SDRKIT_HANDLE handle, const sdrkit_cp16_send_args *args) {
  return sdrkit_typed_command(handle, "send", "hfkbd.Cp16SendArgs", args, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "cp16" on the session of `handle`. Ends the transmission after the current row.
static inline int32_t sdrkit_cp16_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cp16" are the sdrkit_cp16_gen_*() functions
// after the settings of its receiver.

// The setting "fixed" (Fixed parameters) of the stack "cp16". On: receive with the four parameters below instead of measuring them. Off: they are the transmitter's.
static inline int32_t sdrkit_cp16_get_fixed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "fixed", value);
}
static inline int32_t sdrkit_cp16_set_fixed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "fixed", value);
}

// The setting "base_hz" (Lowest carrier) of the stack "cp16". A frequency in Hz. Every carrier must lie below 3900 Hz.
static inline int32_t sdrkit_cp16_get_base_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "base_hz", value);
}
static inline int32_t sdrkit_cp16_set_base_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "base_hz", value);
}

// The setting "spacing_hz" (Spacing) of the stack "cp16". A frequency in Hz.
static inline int32_t sdrkit_cp16_get_spacing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "spacing_hz", value);
}
static inline int32_t sdrkit_cp16_set_spacing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "spacing_hz", value);
}

// The setting "char_s" (Seconds a character) of the stack "cp16". Unit: s.
static inline int32_t sdrkit_cp16_get_char_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "char_s", value);
}
static inline int32_t sdrkit_cp16_set_char_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "char_s", value);
}

// The setting "sideband" (Sideband) of the stack "cp16". A value of `sdrkit_cp16_sideband`.
static inline int32_t sdrkit_cp16_get_sideband(SDRKIT_HANDLE handle, sdrkit_cp16_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cp16_sideband)index;
  return code;
}
static inline int32_t sdrkit_cp16_set_sideband(SDRKIT_HANDLE handle, sdrkit_cp16_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "sideband", (int32_t)value);
}

// The setting "read" (Read characters) of the stack "cp16".
static inline int32_t sdrkit_cp16_get_read(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "read", value);
}
static inline int32_t sdrkit_cp16_set_read(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "read", value);
}

// The setting "font" (Font file) of the stack "cp16". An HZK16 file or a TrueType font that the host names. Empty: Latin, Cyrillic and Greek from the shared glyphs.
static inline int32_t sdrkit_cp16_get_font(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "font", value);
}
static inline int32_t sdrkit_cp16_set_font(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "font", value);
}

// The setting "listen" (Listen) of the stack "cp16". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_cp16_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_cp16_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_char_s" (Test signal seconds a character) of the test signal generator of the stack "cp16". Unit: s.
static inline int32_t sdrkit_cp16_gen_get_char_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_char_s", value);
}
static inline int32_t sdrkit_cp16_gen_set_char_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_char_s", value);
}

// The setting "gen_spacing_hz" (Test signal spacing) of the test signal generator of the stack "cp16". A frequency in Hz.
static inline int32_t sdrkit_cp16_gen_get_spacing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spacing_hz", value);
}
static inline int32_t sdrkit_cp16_gen_set_spacing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spacing_hz", value);
}

// The setting "gen_lsb" (Test signal on the lower sideband) of the test signal generator of the stack "cp16".
static inline int32_t sdrkit_cp16_gen_get_lsb(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_lsb", value);
}
static inline int32_t sdrkit_cp16_gen_set_lsb(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_lsb", value);
}

// The setting "gen_base_hz" (Test signal lowest carrier) of the test signal generator of the stack "cp16". A frequency in Hz.
static inline int32_t sdrkit_cp16_gen_get_base_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_base_hz", value);
}
static inline int32_t sdrkit_cp16_gen_set_base_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_base_hz", value);
}

// The setting "gen_text" (Test signal text) of the test signal generator of the stack "cp16". A value of `sdrkit_cp16_gen_text`.
static inline int32_t sdrkit_cp16_gen_get_text(SDRKIT_HANDLE handle, sdrkit_cp16_gen_text *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_text", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cp16_gen_text)index;
  return code;
}
static inline int32_t sdrkit_cp16_gen_set_text(SDRKIT_HANDLE handle, sdrkit_cp16_gen_text value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_text", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "pilots" of the stack "winmor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_winmor_pilots_rows(SDRKIT_HANDLE handle,
                                               sdrkit_winmor_pilot_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfkbd.WinmorPilotRow", "pilots", rows, sizeof(sdrkit_winmor_pilot_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "winmor.pilot"
// (Pilot). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_winmor_pilot_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_winmor_pilot_event *event) {
  return sdrkit_typed_event(handle, n, "winmor.pilot", "hfkbd.WinmorPilotEvent", event, sizeof(sdrkit_winmor_pilot_event));
}

// Runs the command "clear" (Clear) of the stack "winmor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_winmor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "winmor" are the sdrkit_winmor_gen_*() functions
// after the settings of its receiver.

// The setting "listen" (Listen) of the stack "winmor". Plays the sideband audio. Decoding is the same either way.
static inline int32_t sdrkit_winmor_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_winmor_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_tones" (Frame-type tones) of the test signal generator of the stack "winmor". Four indices 0 to 7.
static inline int32_t sdrkit_winmor_gen_get_tones(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tones", value);
}
static inline int32_t sdrkit_winmor_gen_set_tones(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tones", value);
}

// The setting "gen_error_hz" (Tuning error) of the test signal generator of the stack "winmor". A frequency in Hz.
static inline int32_t sdrkit_winmor_gen_get_error_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_error_hz", value);
}
static inline int32_t sdrkit_winmor_gen_set_error_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_error_hz", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfkbd_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_rtty_fields_read(SDRKIT_HANDLE handle, sdrkit_rtty_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.RttyFields", fields, sizeof(sdrkit_rtty_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfkbd_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.DecoderFields", fields, sizeof(sdrkit_hfkbd_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfkbd_keyboard_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_keyboard_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.KeyboardTxFields", fields, sizeof(sdrkit_hfkbd_keyboard_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_psk_fields_read(SDRKIT_HANDLE handle, sdrkit_psk_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.PskFields", fields, sizeof(sdrkit_psk_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cw_fields_read(SDRKIT_HANDLE handle, sdrkit_cw_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.CwFields", fields, sizeof(sdrkit_cw_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfkbd_keyboard_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_keyboard_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.KeyboardFields", fields, sizeof(sdrkit_hfkbd_keyboard_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfkbd_picture_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_picture_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.PictureFields", fields, sizeof(sdrkit_hfkbd_picture_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hell_fields_read(SDRKIT_HANDLE handle, sdrkit_hell_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.HellFields", fields, sizeof(sdrkit_hell_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mt63_fields_read(SDRKIT_HANDLE handle, sdrkit_mt63_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.Mt63Fields", fields, sizeof(sdrkit_mt63_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fsq_fields_read(SDRKIT_HANDLE handle, sdrkit_fsq_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FsqFields", fields, sizeof(sdrkit_fsq_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sstv_fields_read(SDRKIT_HANDLE handle, sdrkit_sstv_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.SstvFields", fields, sizeof(sdrkit_sstv_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sstv_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_sstv_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.SstvTxFields", fields, sizeof(sdrkit_sstv_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sitor_fields_read(SDRKIT_HANDLE handle, sdrkit_sitor_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.SitorFields", fields, sizeof(sdrkit_sitor_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ardop_fields_read(SDRKIT_HANDLE handle, sdrkit_ardop_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.ArdopFields", fields, sizeof(sdrkit_ardop_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ardop_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_ardop_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.ArdopTxFields", fields, sizeof(sdrkit_ardop_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ros_fields_read(SDRKIT_HANDLE handle, sdrkit_ros_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.RosFields", fields, sizeof(sdrkit_ros_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_throb_fields_read(SDRKIT_HANDLE handle, sdrkit_throb_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.ThrobFields", fields, sizeof(sdrkit_throb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_qrss_fields_read(SDRKIT_HANDLE handle, sdrkit_qrss_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.QrssFields", fields, sizeof(sdrkit_qrss_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cp16_fields_read(SDRKIT_HANDLE handle, sdrkit_cp16_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.Cp16Fields", fields, sizeof(sdrkit_cp16_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_winmor_fields_read(SDRKIT_HANDLE handle, sdrkit_winmor_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.WinmorFields", fields, sizeof(sdrkit_winmor_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "rtty" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_rtty_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "psk" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_psk_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "cw" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_cw_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "olivia" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_olivia_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "contestia" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_contestia_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "mfsk" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_mfsk_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "dominoex" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_dominoex_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "thor" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_thor_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "hell" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_hell_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "mt63" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_mt63_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "fsq" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_fsq_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "ifkp" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_ifkp_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "sstv" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_sstv_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "sitor" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_sitor_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "ardop" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_ardop_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "ros" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_ros_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "throb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_throb_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "qrss" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_qrss_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "cp16" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_cp16_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

// Fills `fields` with the result fields of the component "front" of the stack "winmor" in the snapshot of
// `handle`. A generic consumer reads them under the keys "front.<key>".
static inline int32_t sdrkit_winmor_front_fields_read(SDRKIT_HANDLE handle, sdrkit_hfkbd_front_fields *fields) {
  return sdrkit_typed_fields(handle, "hfkbd.FrontFields@front", fields, sizeof(sdrkit_hfkbd_front_fields));
}

#endif
