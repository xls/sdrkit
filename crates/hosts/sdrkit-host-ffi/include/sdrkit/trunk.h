/*
 * sdrkit: the typed interface of the stack family "trunk", version 1.6. MIT licence.
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

#ifndef SDRKIT_TRUNK_H
#define SDRKIT_TRUNK_H

#include "../sdrkit.h"
#include "tones.h"

// The version of the family that this header describes.
#define SDRKIT_TRUNK_VERSION_MAJOR 1
#define SDRKIT_TRUNK_VERSION_MINOR 6

// The stack "mpt1327": its id, and the keys of its event types.
#define SDRKIT_STACK_MPT1327 "mpt1327"
#define SDRKIT_EVENT_MPT1327_MESSAGE "mpt1327.message"
#define SDRKIT_EVENT_TRUNK_FOLLOW "trunk.follow"

// The stack "ltr": its id, and the keys of its event types.
#define SDRKIT_STACK_LTR "ltr"
#define SDRKIT_EVENT_LTR_WORD "ltr.word"

// The stack "edacs": its id, and the keys of its event types.
#define SDRKIT_STACK_EDACS "edacs"
#define SDRKIT_EVENT_EDACS_CALL "edacs.call"

// The stack "tones": its id, and the keys of its event types.
#define SDRKIT_STACK_TONES "tones"

// The stack "pager": its id, and the keys of its event types.
#define SDRKIT_STACK_PAGER "pager"
#define SDRKIT_EVENT_POCSAG_PAGE "pocsag.page"
#define SDRKIT_EVENT_FLEX_PAGE "flex.page"
#define SDRKIT_EVENT_PAGER_TX "pager.tx"

// The stack "ermes": its id, and the keys of its event types.
#define SDRKIT_STACK_ERMES "ermes"
#define SDRKIT_EVENT_ERMES_BATCH "ermes.batch"
#define SDRKIT_EVENT_ERMES_MESSAGE "ermes.message"

// The stack "multitone": its id, and the keys of its event types.
#define SDRKIT_STACK_MULTITONE "multitone"
#define SDRKIT_EVENT_MULTITONE_TRANSMISSION "multitone.transmission"
#define SDRKIT_EVENT_MULTITONE_BURST "multitone.burst"

// The stack "motorola_type2": its id, and the keys of its event types.
#define SDRKIT_STACK_MOTOROLA_TYPE2 "motorola_type2"
#define SDRKIT_EVENT_TYPE2_MESSAGE "type2.message"
#define SDRKIT_EVENT_TRUNK_GRANT "trunk.grant"

// The stack "opensky": its id, and the keys of its event types.
#define SDRKIT_STACK_OPENSKY "opensky"
#define SDRKIT_EVENT_OPENSKY_VERDICT "opensky.verdict"

// The stack "gsc": its id, and the keys of its event types.
#define SDRKIT_STACK_GSC "gsc"
#define SDRKIT_EVENT_GSC_PAGE "gsc.page"

// The stack "scada": its id, and the keys of its event types.
#define SDRKIT_STACK_SCADA "scada"
#define SDRKIT_EVENT_SCADA_MESSAGE "scada.message"
#define SDRKIT_EVENT_SCADA_DEVICE "scada.device"

// The options of the setting "framing" of the stack "mpt1327".
typedef enum sdrkit_mpt1327_framing {
  // Standard sync word
  SDRKIT_MPT1327_FRAMING_STANDARD = 0,
  // Ignore sync word (French networks)
  SDRKIT_MPT1327_FRAMING_TOLERANT = 1,
  // Automatic
  SDRKIT_MPT1327_FRAMING_AUTO = 2,
} sdrkit_mpt1327_framing;

// The options of the setting "demod" of the stack "tones".
typedef enum sdrkit_tones_demod {
  // auto (USB on HF, FM above 30 MHz)
  SDRKIT_TONES_DEMOD_AUTO = 0,
  // FM
  SDRKIT_TONES_DEMOD_FM = 1,
  // USB
  SDRKIT_TONES_DEMOD_USB = 2,
  // LSB
  SDRKIT_TONES_DEMOD_LSB = 3,
  // AM
  SDRKIT_TONES_DEMOD_AM = 4,
} sdrkit_tones_demod;

// The options of the setting "gate" of the stack "tones".
typedef enum sdrkit_tones_gate {
  // always
  SDRKIT_TONES_GATE_ALWAYS = 0,
  // on any tone or code
  SDRKIT_TONES_GATE_ANY = 1,
  // only on the chosen tone or code
  SDRKIT_TONES_GATE_CHOSEN = 2,
} sdrkit_tones_gate;

// The options of the setting "gate_kind" of the stack "tones".
typedef enum sdrkit_tones_gate_kind {
  // CTCSS
  SDRKIT_TONES_GATE_KIND_CTCSS = 0,
  // DCS
  SDRKIT_TONES_GATE_KIND_DCS = 1,
} sdrkit_tones_gate_kind;

// The options of the setting "gate_tone" of the stack "tones".
typedef enum sdrkit_tones_gate_tone {
  // 67.0 Hz
  SDRKIT_TONES_GATE_TONE_67_0 = 0,
  // 69.3 Hz
  SDRKIT_TONES_GATE_TONE_69_3 = 1,
  // 71.9 Hz
  SDRKIT_TONES_GATE_TONE_71_9 = 2,
  // 74.4 Hz
  SDRKIT_TONES_GATE_TONE_74_4 = 3,
  // 77.0 Hz
  SDRKIT_TONES_GATE_TONE_77_0 = 4,
  // 79.7 Hz
  SDRKIT_TONES_GATE_TONE_79_7 = 5,
  // 82.5 Hz
  SDRKIT_TONES_GATE_TONE_82_5 = 6,
  // 85.4 Hz
  SDRKIT_TONES_GATE_TONE_85_4 = 7,
  // 88.5 Hz
  SDRKIT_TONES_GATE_TONE_88_5 = 8,
  // 91.5 Hz
  SDRKIT_TONES_GATE_TONE_91_5 = 9,
  // 94.8 Hz
  SDRKIT_TONES_GATE_TONE_94_8 = 10,
  // 97.4 Hz
  SDRKIT_TONES_GATE_TONE_97_4 = 11,
  // 100.0 Hz
  SDRKIT_TONES_GATE_TONE_100_0 = 12,
  // 103.5 Hz
  SDRKIT_TONES_GATE_TONE_103_5 = 13,
  // 107.2 Hz
  SDRKIT_TONES_GATE_TONE_107_2 = 14,
  // 110.9 Hz
  SDRKIT_TONES_GATE_TONE_110_9 = 15,
  // 114.8 Hz
  SDRKIT_TONES_GATE_TONE_114_8 = 16,
  // 118.8 Hz
  SDRKIT_TONES_GATE_TONE_118_8 = 17,
  // 123.0 Hz
  SDRKIT_TONES_GATE_TONE_123_0 = 18,
  // 127.3 Hz
  SDRKIT_TONES_GATE_TONE_127_3 = 19,
  // 131.8 Hz
  SDRKIT_TONES_GATE_TONE_131_8 = 20,
  // 136.5 Hz
  SDRKIT_TONES_GATE_TONE_136_5 = 21,
  // 141.3 Hz
  SDRKIT_TONES_GATE_TONE_141_3 = 22,
  // 146.2 Hz
  SDRKIT_TONES_GATE_TONE_146_2 = 23,
  // 151.4 Hz
  SDRKIT_TONES_GATE_TONE_151_4 = 24,
  // 156.7 Hz
  SDRKIT_TONES_GATE_TONE_156_7 = 25,
  // 159.8 Hz
  SDRKIT_TONES_GATE_TONE_159_8 = 26,
  // 162.2 Hz
  SDRKIT_TONES_GATE_TONE_162_2 = 27,
  // 165.5 Hz
  SDRKIT_TONES_GATE_TONE_165_5 = 28,
  // 167.9 Hz
  SDRKIT_TONES_GATE_TONE_167_9 = 29,
  // 171.3 Hz
  SDRKIT_TONES_GATE_TONE_171_3 = 30,
  // 173.8 Hz
  SDRKIT_TONES_GATE_TONE_173_8 = 31,
  // 177.3 Hz
  SDRKIT_TONES_GATE_TONE_177_3 = 32,
  // 179.9 Hz
  SDRKIT_TONES_GATE_TONE_179_9 = 33,
  // 183.5 Hz
  SDRKIT_TONES_GATE_TONE_183_5 = 34,
  // 186.2 Hz
  SDRKIT_TONES_GATE_TONE_186_2 = 35,
  // 189.9 Hz
  SDRKIT_TONES_GATE_TONE_189_9 = 36,
  // 192.8 Hz
  SDRKIT_TONES_GATE_TONE_192_8 = 37,
  // 196.6 Hz
  SDRKIT_TONES_GATE_TONE_196_6 = 38,
  // 199.5 Hz
  SDRKIT_TONES_GATE_TONE_199_5 = 39,
  // 203.5 Hz
  SDRKIT_TONES_GATE_TONE_203_5 = 40,
  // 206.5 Hz
  SDRKIT_TONES_GATE_TONE_206_5 = 41,
  // 210.7 Hz
  SDRKIT_TONES_GATE_TONE_210_7 = 42,
  // 218.1 Hz
  SDRKIT_TONES_GATE_TONE_218_1 = 43,
  // 225.7 Hz
  SDRKIT_TONES_GATE_TONE_225_7 = 44,
  // 229.1 Hz
  SDRKIT_TONES_GATE_TONE_229_1 = 45,
  // 233.6 Hz
  SDRKIT_TONES_GATE_TONE_233_6 = 46,
  // 241.8 Hz
  SDRKIT_TONES_GATE_TONE_241_8 = 47,
  // 250.3 Hz
  SDRKIT_TONES_GATE_TONE_250_3 = 48,
  // 254.1 Hz
  SDRKIT_TONES_GATE_TONE_254_1 = 49,
} sdrkit_tones_gate_tone;

// The options of the setting "gate_code" of the stack "tones".
typedef enum sdrkit_tones_gate_code {
  // D023
  SDRKIT_TONES_GATE_CODE_D023 = 0,
  // D025
  SDRKIT_TONES_GATE_CODE_D025 = 1,
  // D026
  SDRKIT_TONES_GATE_CODE_D026 = 2,
  // D031
  SDRKIT_TONES_GATE_CODE_D031 = 3,
  // D032
  SDRKIT_TONES_GATE_CODE_D032 = 4,
  // D043
  SDRKIT_TONES_GATE_CODE_D043 = 5,
  // D047
  SDRKIT_TONES_GATE_CODE_D047 = 6,
  // D051
  SDRKIT_TONES_GATE_CODE_D051 = 7,
  // D054
  SDRKIT_TONES_GATE_CODE_D054 = 8,
  // D065
  SDRKIT_TONES_GATE_CODE_D065 = 9,
  // D071
  SDRKIT_TONES_GATE_CODE_D071 = 10,
  // D072
  SDRKIT_TONES_GATE_CODE_D072 = 11,
  // D073
  SDRKIT_TONES_GATE_CODE_D073 = 12,
  // D074
  SDRKIT_TONES_GATE_CODE_D074 = 13,
  // D114
  SDRKIT_TONES_GATE_CODE_D114 = 14,
  // D115
  SDRKIT_TONES_GATE_CODE_D115 = 15,
  // D116
  SDRKIT_TONES_GATE_CODE_D116 = 16,
  // D125
  SDRKIT_TONES_GATE_CODE_D125 = 17,
  // D131
  SDRKIT_TONES_GATE_CODE_D131 = 18,
  // D132
  SDRKIT_TONES_GATE_CODE_D132 = 19,
  // D134
  SDRKIT_TONES_GATE_CODE_D134 = 20,
  // D143
  SDRKIT_TONES_GATE_CODE_D143 = 21,
  // D152
  SDRKIT_TONES_GATE_CODE_D152 = 22,
  // D155
  SDRKIT_TONES_GATE_CODE_D155 = 23,
  // D156
  SDRKIT_TONES_GATE_CODE_D156 = 24,
  // D162
  SDRKIT_TONES_GATE_CODE_D162 = 25,
  // D165
  SDRKIT_TONES_GATE_CODE_D165 = 26,
  // D172
  SDRKIT_TONES_GATE_CODE_D172 = 27,
  // D174
  SDRKIT_TONES_GATE_CODE_D174 = 28,
  // D205
  SDRKIT_TONES_GATE_CODE_D205 = 29,
  // D223
  SDRKIT_TONES_GATE_CODE_D223 = 30,
  // D226
  SDRKIT_TONES_GATE_CODE_D226 = 31,
  // D243
  SDRKIT_TONES_GATE_CODE_D243 = 32,
  // D244
  SDRKIT_TONES_GATE_CODE_D244 = 33,
  // D245
  SDRKIT_TONES_GATE_CODE_D245 = 34,
  // D251
  SDRKIT_TONES_GATE_CODE_D251 = 35,
  // D261
  SDRKIT_TONES_GATE_CODE_D261 = 36,
  // D263
  SDRKIT_TONES_GATE_CODE_D263 = 37,
  // D265
  SDRKIT_TONES_GATE_CODE_D265 = 38,
  // D271
  SDRKIT_TONES_GATE_CODE_D271 = 39,
  // D306
  SDRKIT_TONES_GATE_CODE_D306 = 40,
  // D311
  SDRKIT_TONES_GATE_CODE_D311 = 41,
  // D315
  SDRKIT_TONES_GATE_CODE_D315 = 42,
  // D331
  SDRKIT_TONES_GATE_CODE_D331 = 43,
  // D343
  SDRKIT_TONES_GATE_CODE_D343 = 44,
  // D346
  SDRKIT_TONES_GATE_CODE_D346 = 45,
  // D351
  SDRKIT_TONES_GATE_CODE_D351 = 46,
  // D364
  SDRKIT_TONES_GATE_CODE_D364 = 47,
  // D365
  SDRKIT_TONES_GATE_CODE_D365 = 48,
  // D371
  SDRKIT_TONES_GATE_CODE_D371 = 49,
  // D411
  SDRKIT_TONES_GATE_CODE_D411 = 50,
  // D412
  SDRKIT_TONES_GATE_CODE_D412 = 51,
  // D413
  SDRKIT_TONES_GATE_CODE_D413 = 52,
  // D423
  SDRKIT_TONES_GATE_CODE_D423 = 53,
  // D431
  SDRKIT_TONES_GATE_CODE_D431 = 54,
  // D432
  SDRKIT_TONES_GATE_CODE_D432 = 55,
  // D445
  SDRKIT_TONES_GATE_CODE_D445 = 56,
  // D464
  SDRKIT_TONES_GATE_CODE_D464 = 57,
  // D465
  SDRKIT_TONES_GATE_CODE_D465 = 58,
  // D466
  SDRKIT_TONES_GATE_CODE_D466 = 59,
  // D503
  SDRKIT_TONES_GATE_CODE_D503 = 60,
  // D506
  SDRKIT_TONES_GATE_CODE_D506 = 61,
  // D516
  SDRKIT_TONES_GATE_CODE_D516 = 62,
  // D532
  SDRKIT_TONES_GATE_CODE_D532 = 63,
  // D546
  SDRKIT_TONES_GATE_CODE_D546 = 64,
  // D565
  SDRKIT_TONES_GATE_CODE_D565 = 65,
  // D606
  SDRKIT_TONES_GATE_CODE_D606 = 66,
  // D612
  SDRKIT_TONES_GATE_CODE_D612 = 67,
  // D624
  SDRKIT_TONES_GATE_CODE_D624 = 68,
  // D627
  SDRKIT_TONES_GATE_CODE_D627 = 69,
  // D631
  SDRKIT_TONES_GATE_CODE_D631 = 70,
  // D632
  SDRKIT_TONES_GATE_CODE_D632 = 71,
  // D654
  SDRKIT_TONES_GATE_CODE_D654 = 72,
  // D662
  SDRKIT_TONES_GATE_CODE_D662 = 73,
  // D664
  SDRKIT_TONES_GATE_CODE_D664 = 74,
  // D703
  SDRKIT_TONES_GATE_CODE_D703 = 75,
  // D712
  SDRKIT_TONES_GATE_CODE_D712 = 76,
  // D723
  SDRKIT_TONES_GATE_CODE_D723 = 77,
  // D731
  SDRKIT_TONES_GATE_CODE_D731 = 78,
  // D732
  SDRKIT_TONES_GATE_CODE_D732 = 79,
  // D734
  SDRKIT_TONES_GATE_CODE_D734 = 80,
  // D743
  SDRKIT_TONES_GATE_CODE_D743 = 81,
  // D754
  SDRKIT_TONES_GATE_CODE_D754 = 82,
} sdrkit_tones_gate_code;

// The options of the setting "channel" of the stack "pager".
typedef enum sdrkit_pager_channel {
  // 25 kHz
  SDRKIT_PAGER_CHANNEL_25K = 0,
  // 12.5 kHz
  SDRKIT_PAGER_CHANNEL_12_5K = 1,
} sdrkit_pager_channel;

// The options of the setting "read" of the stack "pager".
typedef enum sdrkit_pager_read {
  // Auto
  SDRKIT_PAGER_READ_AUTO = 0,
  // By function code
  SDRKIT_PAGER_READ_FUNCTION = 1,
  // Force alphanumeric
  SDRKIT_PAGER_READ_ALPHA = 2,
  // Force numeric
  SDRKIT_PAGER_READ_NUMERIC = 3,
} sdrkit_pager_read;

// The options of the setting "charset" of the stack "pager".
typedef enum sdrkit_pager_charset {
  // ASCII
  SDRKIT_PAGER_CHARSET_ASCII = 0,
  // German (ä ö ü ß)
  SDRKIT_PAGER_CHARSET_GERMAN = 1,
} sdrkit_pager_charset;

// The options of the setting "protocol" of the stack "pager".
typedef enum sdrkit_pager_protocol {
  // POCSAG
  SDRKIT_PAGER_PROTOCOL_POCSAG = 0,
  // FLEX
  SDRKIT_PAGER_PROTOCOL_FLEX = 1,
} sdrkit_pager_protocol;

// The options of the setting "pocsag_rate" of the stack "pager".
typedef enum sdrkit_pager_pocsag_rate {
  // 512 bit/s
  SDRKIT_PAGER_POCSAG_RATE_512_BIT_S = 0,
  // 1200 bit/s
  SDRKIT_PAGER_POCSAG_RATE_1200_BIT_S = 1,
  // 2400 bit/s
  SDRKIT_PAGER_POCSAG_RATE_2400_BIT_S = 2,
} sdrkit_pager_pocsag_rate;

// The options of the setting "polarity" of the stack "pager".
typedef enum sdrkit_pager_polarity {
  // Normal (1 is the lower frequency)
  SDRKIT_PAGER_POLARITY_NORMAL = 0,
  // Inverted (1 is the upper frequency)
  SDRKIT_PAGER_POLARITY_INVERTED = 1,
} sdrkit_pager_polarity;

// The options of the setting "gen_shape" of the stack "opensky".
typedef enum sdrkit_opensky_gen_shape {
  // OpenSky: 9600 baud, 4 levels, 4800 Hz, 2 of 4 slots
  SDRKIT_OPENSKY_GEN_SHAPE_OPENSKY = 0,
  // 4800 baud, 4 levels, 1944 Hz, continuous
  SDRKIT_OPENSKY_GEN_SHAPE_P25 = 1,
  // 9600 baud, 2 levels, 2500 Hz, continuous
  SDRKIT_OPENSKY_GEN_SHAPE_EDACS = 2,
  // 3600 baud, 2 levels, 1200 Hz, continuous
  SDRKIT_OPENSKY_GEN_SHAPE_3600_BAUD_2_LEVELS_1200_HZ_CONTINUOUS = 3,
} sdrkit_opensky_gen_shape;

// The options of the setting "gen_pages" of the stack "gsc".
typedef enum sdrkit_gsc_gen_pages {
  // Tone, numeric, alphanumeric and voice pages
  SDRKIT_GSC_GEN_PAGES_ALL = 0,
  // A tone page
  SDRKIT_GSC_GEN_PAGES_TONE = 1,
  // A numeric page
  SDRKIT_GSC_GEN_PAGES_NUMERIC = 2,
  // An alphanumeric page
  SDRKIT_GSC_GEN_PAGES_ALPHA = 3,
  // A voice page
  SDRKIT_GSC_GEN_PAGES_VOICE = 4,
} sdrkit_gsc_gen_pages;

// The options of the setting "gen_link" of the stack "scada".
typedef enum sdrkit_scada_gen_link {
  // Bell 202 AFSK 1200 Bd
  SDRKIT_SCADA_GEN_LINK_BELL202 = 0,
  // V.23 AFSK 1200 Bd
  SDRKIT_SCADA_GEN_LINK_V23 = 1,
  // 2-FSK 4800 bit/s
  SDRKIT_SCADA_GEN_LINK_FSK2_4800 = 2,
  // 2-FSK 9600 bit/s
  SDRKIT_SCADA_GEN_LINK_FSK2_9600 = 3,
  // 2-FSK 19200 bit/s
  SDRKIT_SCADA_GEN_LINK_FSK2_19200 = 4,
  // 4-FSK 4800 bit/s
  SDRKIT_SCADA_GEN_LINK_FSK4_4800 = 5,
  // 4-FSK 9600 bit/s
  SDRKIT_SCADA_GEN_LINK_FSK4_9600 = 6,
  // 4-FSK 19200 bit/s
  SDRKIT_SCADA_GEN_LINK_FSK4_19200 = 7,
} sdrkit_scada_gen_link;

// The options of the setting "gen_framing" of the stack "scada".
typedef enum sdrkit_scada_gen_framing {
  // 8E1
  SDRKIT_SCADA_GEN_FRAMING_8E1 = 0,
  // 8O1
  SDRKIT_SCADA_GEN_FRAMING_8O1 = 1,
  // 8N1
  SDRKIT_SCADA_GEN_FRAMING_8N1 = 2,
  // 8N2
  SDRKIT_SCADA_GEN_FRAMING_8N2 = 3,
  // HDLC NRZ
  SDRKIT_SCADA_GEN_FRAMING_HDLC_NRZ = 4,
  // HDLC NRZI
  SDRKIT_SCADA_GEN_FRAMING_HDLC_NRZI = 5,
} sdrkit_scada_gen_framing;

// The options of the setting "gen_traffic" of the stack "scada".
typedef enum sdrkit_scada_gen_traffic {
  // Modbus RTU
  SDRKIT_SCADA_GEN_TRAFFIC_MODBUS = 0,
  // Modbus ASCII
  SDRKIT_SCADA_GEN_TRAFFIC_MODBUS_ASCII = 1,
  // DNP3
  SDRKIT_SCADA_GEN_TRAFFIC_DNP3 = 2,
} sdrkit_scada_gen_traffic;

// The result fields of MPT 1327.
typedef struct sdrkit_mpt1327_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
  bool locked;
  // Codewords good.
  int64_t codewords_good;
  // Codewords repaired.
  int64_t codewords_repaired;
  // Codewords bad.
  int64_t codewords_bad;
  // System codewords.
  int64_t ccscs;
  // The messages read.
  int64_t messages;
  // True if `system_identity` holds a value.
  bool has_system_identity;
  // The system identity of the control channel.
  int64_t system_identity;
  // Sync check off.
  bool sync_check_off;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
} sdrkit_mpt1327_fields;

// The options of `type` of `sdrkit_mpt1327_message_row`.
typedef enum sdrkit_mpt1327_message_row_type {
  // GTC: go to channel
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_G_T_C = 0,
  // ALH: aloha
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H = 1,
  // ALHS: aloha, except RQD
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H_S = 2,
  // ALHD: aloha, except RQS
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H_D = 3,
  // ALHE: aloha, emergency only
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H_E = 4,
  // ALHR: aloha, registration or emergency
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H_R = 5,
  // ALHX: aloha, except RQR
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H_X = 6,
  // ALHF: aloha, fall-back
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_L_H_F = 7,
  // ACK: acknowledgement
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K = 8,
  // ACKI: intermediate acknowledgement
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_I = 9,
  // ACKQ: call queued
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_Q = 10,
  // ACKX: message rejected
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_X = 11,
  // ACKV: called unit unavailable
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_V = 12,
  // ACKE: emergency call acknowledged
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_E = 13,
  // ACKT: try on the given address
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_T = 14,
  // ACKB: call-back
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_C_K_B = 15,
  // AHY: availability check
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_H_Y = 16,
  // AHYX: cancel alert
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_H_Y_X = 17,
  // AHYP: presence check
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_H_Y_P = 18,
  // AHYQ: status message
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_H_Y_Q = 19,
  // AHYC: short data invitation
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_A_H_Y_C = 20,
  // MARK: control channel marker
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_M_A_R_K = 21,
  // MAINT: call maintenance
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_M_A_I_N_T = 22,
  // CLEAR: clear down
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_C_L_E_A_R = 23,
  // MOVE: move to control channel
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_M_O_V_E = 24,
  // BCAST: broadcast
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_B_C_A_S_T = 25,
  // HEAD: short data header
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_H_E_A_D = 26,
  // SAM: single address message
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_S_A_M = 27,
  // STDDATA: standard data
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_S_T_D_D_A_T_A = 28,
  // STDACK: standard data acknowledgement
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_S_T_D_A_C_K = 29,
  // Spare
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_SPARE = 30,
  // Reserved
  SDRKIT_MPT1327_MESSAGE_ROW_TYPE_R_S_V_D = 31,
} sdrkit_mpt1327_message_row_type;

// An MPT 1327 message. The row key is the sequence number of the message.
typedef struct sdrkit_mpt1327_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a message, s. Unit: s.
  double time_s;
  // The type of a message. A value of `sdrkit_mpt1327_message_row_type`.
  int32_t type;
  // The calling address.
  sdrkit_string caller;
  // The called address.
  sdrkit_string called;
  // True if `channel` holds a value.
  bool has_channel;
  // A channel number.
  int64_t channel;
  // What a message means, for people.
  sdrkit_string meaning;
} sdrkit_mpt1327_message_row;

// The options of `use` of `sdrkit_mpt1327_channel_row`.
typedef enum sdrkit_mpt1327_channel_row_use {
  // Control channel
  SDRKIT_MPT1327_CHANNEL_ROW_USE_CONTROL = 0,
  // Traffic (GTC)
  SDRKIT_MPT1327_CHANNEL_ROW_USE_TRAFFIC = 1,
} sdrkit_mpt1327_channel_row_use;

// An MPT 1327 channel. The row key is the channel number, five digits.
typedef struct sdrkit_mpt1327_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A channel number.
  int64_t channel;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency of a channel. A frequency in Hz.
  double freq_hz;
  // What a channel is used for. A value of `sdrkit_mpt1327_channel_row_use`.
  int32_t use;
  // The messages that named a channel.
  int64_t seen;
  // True if `last_called` holds a value.
  bool has_last_called;
  // The last called address on a traffic channel.
  sdrkit_string last_called;
  // True if `last_caller` holds a value.
  bool has_last_caller;
  // The last calling address on a traffic channel.
  sdrkit_string last_caller;
} sdrkit_mpt1327_channel_row;

// The result fields of LTR.
typedef struct sdrkit_ltr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
  bool locked;
  // The good words.
  int64_t words_good;
  // Bad words.
  int64_t words_bad;
  // Syncs.
  int64_t syncs;
  // True if `area` holds a value.
  bool has_area;
  // The AREA bit.
  int64_t area;
  // Repeaters seen.
  int64_t repeaters;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // Data swing. A frequency in Hz.
  double swing_hz;
  // Words heard.
  bool data_valid;
} sdrkit_ltr_fields;

// The options of `kind` of `sdrkit_ltr_word_row`.
typedef enum sdrkit_ltr_word_row_kind {
  // Call
  SDRKIT_LTR_WORD_ROW_KIND_CALL = 0,
  // Idle
  SDRKIT_LTR_WORD_ROW_KIND_IDLE = 1,
  // Callsign ID
  SDRKIT_LTR_WORD_ROW_KIND_CALLSIGN_ID = 2,
  // Busy
  SDRKIT_LTR_WORD_ROW_KIND_BUSY = 3,
  // Token
  SDRKIT_LTR_WORD_ROW_KIND_TOKEN = 4,
} sdrkit_ltr_word_row_kind;

// A good LTR word. The row key is the sequence number of the word.
typedef struct sdrkit_ltr_word_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The talkgroup, "{AREA}-{HOME}-{ID}".
  sdrkit_string talkgroup;
  // The AREA bit.
  int64_t area;
  // The GO-TO repeater.
  int64_t goto_;
  // The HOME repeater.
  int64_t home;
  // The ID.
  int64_t id;
  // The FREE repeater.
  int64_t free;
  // The 7 check bits as received.
  int64_t check;
  // What the ID says. A value of `sdrkit_ltr_word_row_kind`.
  int32_t kind;
  // The signal time of a word, s. Unit: s.
  double time_s;
} sdrkit_ltr_word_row;

// How often a repeater was named in each field. The row key is the repeater.
typedef struct sdrkit_ltr_repeater_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A repeater number.
  int64_t repeater;
  // Times a repeater was named as go-to.
  int64_t as_goto;
  // Times named as home.
  int64_t as_home;
  // Times named as free.
  int64_t as_free;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_ltr_repeater_row;

// The options of `polarity` of `sdrkit_edacs_fields`.
typedef enum sdrkit_edacs_fields_polarity {
  // Normal
  SDRKIT_EDACS_FIELDS_POLARITY_NORMAL = 0,
  // Inverted
  SDRKIT_EDACS_FIELDS_POLARITY_INVERTED = 1,
} sdrkit_edacs_fields_polarity;

// The options of `following` of `sdrkit_edacs_fields`.
typedef enum sdrkit_edacs_fields_following {
  // Idle
  SDRKIT_EDACS_FIELDS_FOLLOWING_IDLE = 0,
  // Listening
  SDRKIT_EDACS_FIELDS_FOLLOWING_LISTENING = 1,
  // Waiting for the carrier
  SDRKIT_EDACS_FIELDS_FOLLOWING_WAITING = 2,
  // No voice channel table
  SDRKIT_EDACS_FIELDS_FOLLOWING_NO_TABLE = 3,
  // The capture's frequency is not known
  SDRKIT_EDACS_FIELDS_FOLLOWING_NO_CENTRE = 4,
  // A call is outside the captured span
  SDRKIT_EDACS_FIELDS_FOLLOWING_OUTSIDE_CAPTURE = 5,
} sdrkit_edacs_fields_following;

// The result fields of EDACS.
typedef struct sdrkit_edacs_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
  bool locked;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The data polarity. A value of `sdrkit_edacs_fields_polarity`.
  int32_t polarity;
  // True if `site` holds a value.
  bool has_site;
  // The site.
  int64_t site;
  // True if `area` holds a value.
  bool has_area;
  // The area.
  int64_t area;
  // True if `control_lcn` holds a value.
  bool has_control_lcn;
  // The control channel's LCN.
  int64_t control_lcn;
  // True if `highest_lcn` holds a value.
  bool has_highest_lcn;
  // The highest LCN seen in a message.
  int64_t highest_lcn;
  // Words.
  int64_t words;
  // Words of a known type.
  int64_t words_known;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // Data swing. A frequency in Hz.
  double swing_hz;
  // True if `following_lcn` holds a value.
  bool has_following_lcn;
  // The LCN followed now.
  int64_t following_lcn;
  // What the follower does now. A value of `sdrkit_edacs_fields_following`.
  int32_t following;
} sdrkit_edacs_fields;

// The options of `urgency` of `sdrkit_edacs_call_row`.
typedef enum sdrkit_edacs_call_row_urgency {
  // None
  SDRKIT_EDACS_CALL_ROW_URGENCY_NONE = 0,
  // Notice
  SDRKIT_EDACS_CALL_ROW_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_EDACS_CALL_ROW_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_EDACS_CALL_ROW_URGENCY_EMERGENCY = 3,
} sdrkit_edacs_call_row_urgency;

// The options of `end` of `sdrkit_edacs_call_row`.
typedef enum sdrkit_edacs_call_row_end {
  // On air
  SDRKIT_EDACS_CALL_ROW_END_ACTIVE = 0,
  // Terminator
  SDRKIT_EDACS_CALL_ROW_END_TERMINATOR = 1,
  // Hang time
  SDRKIT_EDACS_CALL_ROW_END_HANG_TIME = 2,
  // Replaced
  SDRKIT_EDACS_CALL_ROW_END_REPLACED = 3,
  // Closed
  SDRKIT_EDACS_CALL_ROW_END_CLOSED = 4,
  // Single
  SDRKIT_EDACS_CALL_ROW_END_SINGLE = 5,
} sdrkit_edacs_call_row_end;

// An EDACS call. `source` is the calling unit, `target` the group or the called unit, `channel` the LCN.
typedef struct sdrkit_edacs_call_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // A frequency. A frequency in Hz.
  double freq_hz;
  // A group call.
  bool group;
  // The start of a call, signal time. Unit: s.
  double start_s;
  // The length of a call. A length of time in seconds.
  double duration;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_edacs_call_row_urgency`.
  int32_t urgency;
  // A digital call (not decoded).
  bool digital;
  // A call that no message announced.
  bool carrier_only;
  // Where a call is in its life. A value of `sdrkit_edacs_call_row_end`.
  int32_t end;
  // The call is followed now.
  bool followed;
} sdrkit_edacs_call_row;

// An EDACS talkgroup. The row key is the group id.
typedef struct sdrkit_edacs_talkgroup_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A group.
  int64_t gid;
  // Calls.
  int64_t calls;
  // True if `last_unit` holds a value.
  bool has_last_unit;
  // Last unit.
  int64_t last_unit;
  // Last LCN.
  int64_t last_lcn;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // A frequency. A frequency in Hz.
  double freq_hz;
  // On air.
  bool active;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_edacs_talkgroup_row;

// An EDACS channel. The row key is the LCN.
typedef struct sdrkit_edacs_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A voice channel number.
  int64_t lcn;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // A frequency. A frequency in Hz.
  double freq_hz;
  // Level. Unit: dB.
  double level_db;
  // Floor. Unit: dB.
  double floor_db;
  // In the capture.
  bool in_capture;
  // Carrier.
  bool carrier;
  // Control channel.
  bool control;
} sdrkit_edacs_channel_row;

// An EDACS frame. The row key is the sequence number of the frame.
typedef struct sdrkit_edacs_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Message A.
  int64_t a;
  // Message B.
  int64_t b;
  // Copy errors of A.
  int64_t err_a;
  // Copy errors of B.
  int64_t err_b;
  // Time. Unit: s.
  double time_s;
  // Read as.
  sdrkit_string text;
} sdrkit_edacs_frame_row;

// A message type and how often it was heard. The row key is the type.
typedef struct sdrkit_edacs_word_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Word.
  int64_t word;
  // Count.
  int64_t count;
} sdrkit_edacs_word_row;

// The options of `demod` of `sdrkit_tones_fields`.
typedef enum sdrkit_tones_fields_demod {
  // auto (USB on HF, FM above 30 MHz)
  SDRKIT_TONES_FIELDS_DEMOD_AUTO = 0,
  // FM
  SDRKIT_TONES_FIELDS_DEMOD_FM = 1,
  // USB
  SDRKIT_TONES_FIELDS_DEMOD_USB = 2,
  // LSB
  SDRKIT_TONES_FIELDS_DEMOD_LSB = 3,
  // AM
  SDRKIT_TONES_FIELDS_DEMOD_AM = 4,
} sdrkit_tones_fields_demod;

// The result fields of the tone decoders.
typedef struct sdrkit_tones_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The audio level. Unit: dB.
  double audio_db;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the carrier. No value for audio input. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the tuned frequency. No value for audio input. A frequency in Hz.
  double offset_hz;
  // True if `carrier_recent` holds a value.
  bool has_carrier_recent;
  // A carrier was seen in the last 5 s. No value for audio input.
  bool carrier_recent;
  // True if `ctcss_hz` holds a value.
  bool has_ctcss_hz;
  // The CTCSS tone that is locked now. A frequency in Hz.
  double ctcss_hz;
  // True if `dcs` holds a value.
  bool has_dcs;
  // The DCS code that is locked now.
  sdrkit_string dcs;
  // True if `dcs_alias` holds a value.
  bool has_dcs_alias;
  // The other reading of the waveform of the DCS code.
  sdrkit_string dcs_alias;
  // True if `strongest_hz` holds a value.
  bool has_strongest_hz;
  // The strongest tone of the audio spectrum. A frequency in Hz.
  double strongest_hz;
  // The DTMF keys of the string that goes on now.
  sdrkit_string dtmf;
  // The listening gate is open.
  bool gate_open;
  // The sample rate of the input. A frequency in Hz.
  double input_rate_hz;
  // The input is audio.
  bool audio_input;
  // The demodulation in use. A value of `sdrkit_tones_fields_demod`.
  int32_t demod;
  // Every decision of the decoders.
  int64_t detections;
  // The CTCSS tones heard.
  int64_t ctcss_count;
  // The DCS codes heard.
  int64_t dcs_count;
  // The DTMF strings heard.
  int64_t dtmf_count;
  // The sequential calls heard.
  int64_t selcall_count;
  // The SELCAL calls heard.
  int64_t selcal_count;
  // The two-tone pages heard.
  int64_t two_tone_count;
} sdrkit_tones_fields;

// The options of `kind` of `sdrkit_tones_heard_row`.
typedef enum sdrkit_tones_heard_row_kind {
  // CTCSS
  SDRKIT_TONES_HEARD_ROW_KIND_CTCSS = 0,
  // DCS
  SDRKIT_TONES_HEARD_ROW_KIND_DCS = 1,
  // DTMF
  SDRKIT_TONES_HEARD_ROW_KIND_DTMF = 2,
  // Selcall
  SDRKIT_TONES_HEARD_ROW_KIND_SELCALL = 3,
  // SELCAL
  SDRKIT_TONES_HEARD_ROW_KIND_SELCAL = 4,
  // 2-tone
  SDRKIT_TONES_HEARD_ROW_KIND_TWO_TONE = 5,
} sdrkit_tones_heard_row_kind;

// A row of the list of tones heard. The row key is the sequence number of the row.
typedef struct sdrkit_tones_heard_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the start. Unit: s.
  double start_s;
  // What the row holds. A value of `sdrkit_tones_heard_row_kind`.
  int32_t kind;
  // What was heard ("123.0 Hz", "D023N", "159#", "12345", "AM-CP", "682.5 + 1000.0 Hz").
  sdrkit_string text;
  // The length so far. A length of time in seconds.
  double length;
  // The details.
  sdrkit_string detail;
  // The level, relative to a full-scale sine. Unit: dB.
  double level_db;
  // It still goes on.
  bool active;
} sdrkit_tones_heard_row;

// The result fields of the pager stack.
typedef struct sdrkit_pager_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A decoder receives a transmission.
  bool receiving;
  // A carrier is present.
  bool carrier;
  // A decoder holds sync.
  bool locked;
  // The protocol and speed of the last sync ("POCSAG 1200").
  sdrkit_string last_speed;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the tuned frequency at the last sync. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the carrier, while a carrier is present. Unit: dB.
  double snr_db;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // The pages read.
  int64_t pages;
  // The code words valid as received or repaired.
  int64_t codewords_good;
  // The code words repaired.
  int64_t codewords_repaired;
  // The code words lost.
  int64_t codewords_lost;
  // The pages that the acceptance rules refused (address ranges, vector values, message checks).
  int64_t pages_refused;
} sdrkit_pager_fields;

// The result fields of the POCSAG sender.
typedef struct sdrkit_pager_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is on air.
  bool tx_sending;
  // The pages of the transmission on air.
  int64_t tx_sent;
  // The pages waiting for the next transmission.
  int64_t tx_waiting;
} sdrkit_pager_tx_fields;

// The options of `type` of `sdrkit_pager_page_row`.
typedef enum sdrkit_pager_page_row_type {
  // Tone
  SDRKIT_PAGER_PAGE_ROW_TYPE_TONE = 0,
  // Numeric
  SDRKIT_PAGER_PAGE_ROW_TYPE_NUMERIC = 1,
  // Text
  SDRKIT_PAGER_PAGE_ROW_TYPE_ALPHA = 2,
  // Secure
  SDRKIT_PAGER_PAGE_ROW_TYPE_SECURE = 3,
  // Binary
  SDRKIT_PAGER_PAGE_ROW_TYPE_BINARY = 4,
  // Instruction
  SDRKIT_PAGER_PAGE_ROW_TYPE_INSTRUCTION = 5,
  // Sequential call
  SDRKIT_PAGER_PAGE_ROW_TYPE_SELCALL = 6,
} sdrkit_pager_page_row_type;

// A row of the page list. The row key is the sequence number of the page.
typedef struct sdrkit_pager_page_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the page. Unit: s.
  double time_s;
  // The protocol and speed ("POCSAG 1200", "FLEX 3200/4", "Selcall ZVEI1").
  sdrkit_string protocol;
  // The RIC or capcode (0 for a sequential call).
  int64_t address;
  // True if `function` holds a value.
  bool has_function;
  // The POCSAG function code.
  int64_t function;
  // What the page carries. A value of `sdrkit_pager_page_row_type`.
  int32_t type;
  // The message.
  sdrkit_string message;
  // A word was lost or the end is not sure.
  bool damaged;
  // The bits that the repairs changed.
  int64_t bits_repaired;
  // The copies voted into the page.
  int64_t copies;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency of the carrier, when it is known. A frequency in Hz.
  double freq_hz;
} sdrkit_pager_page_row;

// A row of the statistics of each speed. The row key is the speed.
typedef struct sdrkit_pager_speed_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol and speed.
  sdrkit_string speed;
  // The code words valid as received.
  int64_t good;
  // The code words repaired.
  int64_t repaired;
  // The code words lost.
  int64_t lost;
  // The pages read.
  int64_t pages;
  // The batches (POCSAG) or frames (FLEX).
  int64_t transmissions;
  // The pages that the acceptance rules refused.
  int64_t refused;
} sdrkit_pager_speed_row;

// The options of `protocol` of `sdrkit_pager_heard_row`.
typedef enum sdrkit_pager_heard_row_protocol {
  // POCSAG
  SDRKIT_PAGER_HEARD_ROW_PROTOCOL_POCSAG = 0,
  // FLEX
  SDRKIT_PAGER_HEARD_ROW_PROTOCOL_FLEX = 1,
} sdrkit_pager_heard_row_protocol;

// A row of the addresses heard. The row key is the protocol and the address ("pocsag/1234567").
typedef struct sdrkit_pager_heard_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The RIC or capcode.
  int64_t address;
  // The paging protocol. A value of `sdrkit_pager_heard_row_protocol`.
  int32_t protocol;
  // The pages of the address.
  int64_t pages;
  // The protocol and speed of the last page.
  sdrkit_string speed;
  // True if `last_text` holds a value.
  bool has_last_text;
  // The last text (a tone-only page does not erase it).
  sdrkit_string last_text;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_pager_heard_row;

// The counts and the carrier of the ERMES receiver.
typedef struct sdrkit_ermes_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The good batches.
  int64_t batches;
  // The batches dropped.
  int64_t bad_batches;
  // The initial addresses.
  int64_t addresses;
  // The messages.
  int64_t messages;
  // The code words repaired.
  int64_t words_repaired;
  // The times that the preamble and the sync word were found.
  int64_t syncs;
  // True if `network` holds a value.
  bool has_network;
  // The network of the last good batch.
  sdrkit_string network;
  // True if `carrier` holds a value.
  bool has_carrier;
  // A carrier is present.
  bool carrier;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the carrier. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_ermes_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_trunk_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_trunk_decoder_fields;

// A row of the ERMES table: the initial addresses of a batch, or a message. The row key is the number of the row.
typedef struct sdrkit_ermes_entry_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row. Unit: s.
  double time_s;
  // What the row is: the addresses of a batch, or a message.
  sdrkit_string kind;
  // The batch letter.
  sdrkit_string batch;
  // True if `address` holds a value.
  bool has_address;
  // The local address of a message.
  int64_t address;
  // True if `category` holds a value.
  bool has_category;
  // The category of a message.
  sdrkit_string category;
  // The addresses, or the text of the message.
  sdrkit_string text;
} sdrkit_ermes_entry_row;

// The result fields of the Multitone detector.
typedef struct sdrkit_multitone_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Carrier.
  bool carrier;
  // The transmissions measured.
  int64_t transmissions;
  // Bursts.
  int64_t bursts;
  // Valid BCH words.
  int64_t valid_words;
  // Other words.
  int64_t other_words;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
} sdrkit_multitone_fields;

// The options of `kind` of `sdrkit_multitone_transmission_row`.
typedef enum sdrkit_multitone_transmission_row_kind {
  // FFSK data
  SDRKIT_MULTITONE_TRANSMISSION_ROW_KIND_FFSK_DATA = 0,
  // FSK data
  SDRKIT_MULTITONE_TRANSMISSION_ROW_KIND_FSK_DATA = 1,
  // Tone sequence
  SDRKIT_MULTITONE_TRANSMISSION_ROW_KIND_TONE_SEQUENCE = 2,
  // Repeating multi-tone audio
  SDRKIT_MULTITONE_TRANSMISSION_ROW_KIND_REPEATING = 3,
  // Multi-tone audio
  SDRKIT_MULTITONE_TRANSMISSION_ROW_KIND_MULTI_TONE = 4,
  // Other
  SDRKIT_MULTITONE_TRANSMISSION_ROW_KIND_OTHER = 5,
} sdrkit_multitone_transmission_row_kind;

// A transmission. The row key is its number.
typedef struct sdrkit_multitone_transmission_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Time. Unit: s.
  double time_s;
  // Duration. A length of time in seconds.
  double duration;
  // SNR. Unit: dB.
  double snr_db;
  // Deviation. A frequency in Hz.
  double deviation_hz;
  // Kind. A value of `sdrkit_multitone_transmission_row_kind`.
  int32_t kind;
  // True if `period_ms` holds a value.
  bool has_period_ms;
  // Period. Unit: ms.
  double period_ms;
  // True if `burst` holds a value.
  bool has_burst;
  // Burst.
  int64_t burst;
} sdrkit_multitone_transmission_row;

// A tone of a transmission. The row key is the transmission, `/` and the place of the tone.
typedef struct sdrkit_multitone_tone_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The transmission a row belongs to: a row key of [`TX_TABLE`].
  sdrkit_string transmission;
  // A frequency. A frequency in Hz.
  double hz;
  // Length. A length of time in seconds.
  double seconds;
  // Start in the transmission. A length of time in seconds.
  double start;
} sdrkit_multitone_tone_row;

// A spectral line of a transmission. The row key is the transmission, `/` and the place of the line.
typedef struct sdrkit_multitone_line_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The transmission a row belongs to: a row key of [`TX_TABLE`].
  sdrkit_string transmission;
  // A frequency. A frequency in Hz.
  double hz;
  // Above the median. Unit: dB.
  double db;
} sdrkit_multitone_line_row;

// A tone heard and how often. The row key is the tone.
typedef struct sdrkit_multitone_heard_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A frequency. A frequency in Hz.
  double hz;
  // Count.
  int64_t count;
  // Length. A length of time in seconds.
  double seconds;
  // Last. Unit: s.
  double last_s;
} sdrkit_multitone_heard_row;

// The options of `kind` of `sdrkit_multitone_burst_row`.
typedef enum sdrkit_multitone_burst_row_kind {
  // FFSK
  SDRKIT_MULTITONE_BURST_ROW_KIND_FFSK = 0,
  // FSK
  SDRKIT_MULTITONE_BURST_ROW_KIND_FSK = 1,
} sdrkit_multitone_burst_row_kind;

// An FSK burst. The row key is its number, eight digits.
typedef struct sdrkit_multitone_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Burst.
  int64_t burst;
  // Kind. A value of `sdrkit_multitone_burst_row_kind`.
  int32_t kind;
  // Baud.
  double baud;
  // Bits.
  int64_t bits;
  // Valid BCH words.
  int64_t bch_valid;
  // Words.
  int64_t bch_total;
  // Verdict.
  sdrkit_string verdict;
} sdrkit_multitone_burst_row;

// The result fields of Motorola Type II.
typedef struct sdrkit_motorola_type2_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
  bool locked;
  // The state: 0 no input, 1 listening, 2 a good word in the last 5 s.
  int64_t rx_state;
  // Good words.
  int64_t good_words;
  // Bad words.
  int64_t bad_words;
  // Information bits repaired.
  int64_t bits_repaired;
  // Messages.
  int64_t messages;
  // Messages understood.
  int64_t messages_known;
  // True if `system` holds a value.
  bool has_system;
  // A system ID.
  sdrkit_string system;
  // True if `site_id` holds a value.
  bool has_site_id;
  // The site ID.
  int64_t site_id;
  // True if `features` holds a value.
  bool has_features;
  // Feature bits, as text.
  sdrkit_string features;
  // True if `control_channel_hz` holds a value.
  bool has_control_channel_hz;
  // A control channel frequency. A frequency in Hz.
  double control_channel_hz;
  // True if `system_type` holds a value.
  bool has_system_type;
  // Type II or Type I.
  sdrkit_string system_type;
  // SmartZone or SmartNet.
  sdrkit_string network;
  // Inverted polarity.
  bool inverted;
  // The data swing. Unit: Hz.
  double swing_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `plan` holds a value.
  bool has_plan;
  // The plan in use.
  sdrkit_string plan;
  // A note.
  sdrkit_string note;
} sdrkit_motorola_type2_fields;

// A Type II message. The row key is the sequence number of the message.
typedef struct sdrkit_motorola_type2_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time. Unit: s.
  double time_s;
  // An address.
  sdrkit_string address;
  // A group word.
  bool group;
  // A command.
  sdrkit_string command;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_motorola_type2_message_row;

// A Type II talkgroup. The row key is the talkgroup.
typedef struct sdrkit_motorola_type2_talkgroup_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A talkgroup.
  sdrkit_string talkgroup;
  // Grants counted.
  int64_t calls;
  // True if `last_source` holds a value.
  bool has_last_source;
  // The last calling unit.
  int64_t last_source;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // A frequency. A frequency in Hz.
  double freq_hz;
  // The last time. Unit: s.
  double last_s;
  // Active.
  bool active;
  // Digital.
  bool digital;
  // Encrypted.
  bool encrypted;
  // Patched.
  bool patched;
} sdrkit_motorola_type2_talkgroup_row;

// A Type II site. The row key is the system and the site.
typedef struct sdrkit_motorola_type2_site_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A system ID.
  sdrkit_string system;
  // A site.
  int64_t site;
  // A band code.
  int64_t band;
  // Feature bits, as text.
  sdrkit_string features;
  // True if `control_channel_hz` holds a value.
  bool has_control_channel_hz;
  // A control channel frequency. A frequency in Hz.
  double control_channel_hz;
  // An adjacent site.
  bool adjacent;
} sdrkit_motorola_type2_site_row;

// A member of a patch. The row key is the talkgroup and the member.
typedef struct sdrkit_motorola_type2_patch_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A talkgroup.
  sdrkit_string talkgroup;
  // A patch member.
  sdrkit_string member;
  // The last time. Unit: s.
  double last_s;
} sdrkit_motorola_type2_patch_row;

// A Type II call. The row key is the talkgroup.
typedef struct sdrkit_motorola_type2_call_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A talkgroup.
  sdrkit_string talkgroup;
  // The source of a call.
  int64_t source_unit;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // A frequency. A frequency in Hz.
  double freq_hz;
  // Digital.
  bool digital;
  // Emergency.
  bool emergency;
  // Encrypted.
  bool encrypted;
  // The start of a call. Unit: s.
  double start_s;
  // The length of a call. Unit: s.
  double duration_s;
  // The state of a call.
  sdrkit_string state;
  // True if `note` holds a value.
  bool has_note;
  // A note.
  sdrkit_string note;
} sdrkit_motorola_type2_call_row;

// The options of `verdict` of `sdrkit_opensky_fields`.
typedef enum sdrkit_opensky_fields_verdict {
  // Consistent with OpenSky
  SDRKIT_OPENSKY_FIELDS_VERDICT_CONSISTENT = 0,
  // Not the OpenSky pattern
  SDRKIT_OPENSKY_FIELDS_VERDICT_OTHER = 1,
} sdrkit_opensky_fields_verdict;

// The result fields of the OpenSky identifier.
typedef struct sdrkit_opensky_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The state: 0 no input, 1 analysing, 2 a result.
  int64_t rx_state;
  // Blocks analysed.
  int64_t blocks;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // Symbol rate found, Hz. A frequency in Hz.
  double symbol_rate_hz;
  // True if `line_db` holds a value.
  bool has_line_db;
  // The strength of the symbol rate line. Unit: dB.
  double line_db;
  // True if `levels` holds a value.
  bool has_levels;
  // The levels.
  int64_t levels;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The deviation. Unit: Hz.
  double deviation_hz;
  // True if `burst_ms` holds a value.
  bool has_burst_ms;
  // The burst period. Unit: ms.
  double burst_ms;
  // True if `duty` holds a value.
  bool has_duty;
  // The duty.
  double duty;
  // True if `width_hz` holds a value.
  bool has_width_hz;
  // The occupied width. Unit: Hz.
  double width_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `verdict` holds a value.
  bool has_verdict;
  // The verdict. A value of `sdrkit_opensky_fields_verdict`.
  int32_t verdict;
  // True if `text` holds a value.
  bool has_text;
  // The text of a decoded message.
  sdrkit_string text;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_opensky_fields;

// The options of `verdict` of `sdrkit_opensky_block_row`.
typedef enum sdrkit_opensky_block_row_verdict {
  // Consistent with OpenSky
  SDRKIT_OPENSKY_BLOCK_ROW_VERDICT_CONSISTENT = 0,
  // Not the OpenSky pattern
  SDRKIT_OPENSKY_BLOCK_ROW_VERDICT_OTHER = 1,
} sdrkit_opensky_block_row_verdict;

// The measures of one block. The row key is the sequence number of the block.
typedef struct sdrkit_opensky_block_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // Symbol rate found, Hz. A frequency in Hz.
  double symbol_rate_hz;
  // The levels.
  int64_t levels;
  // The occupied width. Unit: Hz.
  double width_hz;
  // The burst period. Unit: ms.
  double burst_ms;
  // The verdict. A value of `sdrkit_opensky_block_row_verdict`.
  int32_t verdict;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_opensky_block_row;

// The counts and the measurements of the GSC receiver.
typedef struct sdrkit_gsc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The pages of every kind.
  int64_t pages;
  // The tone-only pages.
  int64_t tone_pages;
  // The numeric pages.
  int64_t numeric_pages;
  // The alphanumeric pages.
  int64_t alpha_pages;
  // The tone-and-voice pages.
  int64_t voice_pages;
  // The preamble words seen (at most one a second).
  int64_t preambles;
  // The receiver holds block lock.
  bool locked;
  // True if `preamble` holds a value.
  bool has_preamble;
  // The preamble index remembered (0 to 9).
  int64_t preamble;
  // True if `call` holds a value.
  bool has_call;
  // How the call is sent: "single call" or "batch" (known after the start code).
  sdrkit_string call;
  // The spectrum is inverted (from the last start code).
  bool inverted;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The deviation measured from the levels. A frequency in Hz.
  double deviation_hz;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the sender's bit clock. Unit: ppm.
  double clock_ppm;
  // True if `quality_db` holds a value.
  bool has_quality_db;
  // The quality of the last block read while locked: the mean of the soft bits squared over their variance. Unit: dB.
  double quality_db;
  // True if `carrier` holds a value.
  bool has_carrier;
  // A carrier is present.
  bool carrier;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the carrier. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_gsc_fields;

// A row of the GSC table: a page or the end of a voice page. The row key is the number of the row.
typedef struct sdrkit_gsc_page_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row. Unit: s.
  double time_s;
  // What the row is: "Tone page", "Numeric page", "Alpha page", "Voice page" or "Voice end".
  sdrkit_string kind;
  // The pager code: six digits, or N and five digits when no preamble was seen.
  sdrkit_string code;
  // The address function, 1 to 4.
  int64_t function;
  // Which words of the address are inverted.
  sdrkit_string words;
  // True if `preamble` holds a value.
  bool has_preamble;
  // The preamble index (0 to 9).
  int64_t preamble;
  // How the call was sent: "single call", "batch" or "no preamble seen".
  sdrkit_string call;
  // The text of a data page, or "tone and voice".
  sdrkit_string text;
  // True if `blocks` holds a value.
  bool has_blocks;
  // The data blocks of a data page.
  int64_t blocks;
  // True if `note` holds a value.
  bool has_note;
  // A note: more blocks were announced, the voice is not decoded, how a voice page ended.
  sdrkit_string note;
  // The bits corrected in the two address words.
  int64_t bits_corrected;
  // True if `word` holds a value.
  bool has_word;
  // The activation word of a voice page, or the word that ended it.
  int64_t word;
  // True if `voice_s` holds a value.
  bool has_voice_s;
  // The length of the voice. Unit: s.
  double voice_s;
} sdrkit_gsc_page_row;

// The counts, the link and the measurements of the SCADA receiver.
typedef struct sdrkit_scada_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A chain of the modem holds lock.
  bool locked;
  // True if `link` holds a value.
  bool has_link;
  // The link of the last good frame: the modem, the framing, the tones or deviation and the carrier offset.
  sdrkit_string link;
  // The messages kept.
  int64_t messages;
  // The Modbus messages (RTU and ASCII).
  int64_t modbus;
  // The DNP3 messages.
  int64_t dnp3;
  // The HDLC frames that hold neither protocol.
  int64_t unknown;
  // The frames whose CRC, LRC or block CRC failed.
  int64_t crc_failures;
  // The DNP3 Secure Authentication messages (shown raw).
  int64_t encrypted;
  // The locked bursts that gave no frame (vendor radio framings), kept as raw bits.
  int64_t not_recognised;
  // The devices heard.
  int64_t devices;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset measured on the link of the last good frame. A frequency in Hz.
  double offset_hz;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The deviation measured on that link (the FM deviation on audio FSK, the outer one on 4-FSK). A frequency in Hz.
  double deviation_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the channel. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms, and what is shown raw.
  sdrkit_string note;
} sdrkit_scada_fields;

// A row of the SCADA messages. The row key is the number of the row.
typedef struct sdrkit_scada_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The message number.
  int64_t id;
  // The signal time. Unit: s.
  double time_s;
  // True if `wall_clock_utc` holds a value.
  bool has_wall_clock_utc;
  // The wall-clock time, when the host gives a clock. Seconds since 1970 (UTC).
  double wall_clock_utc;
  // The link: the modem and the framing ("Bell 202 AFSK 1200 Bd, 8E1").
  sdrkit_string link;
  // The protocol: "Modbus RTU", "Modbus ASCII", "DNP3" or "unknown".
  sdrkit_string protocol;
  // The sender ("master", "slave 17", "outstation 1").
  sdrkit_string from;
  // The receiver.
  sdrkit_string to;
  // The function.
  sdrkit_string function;
  // The summary: addresses, quantities, values, objects, indications.
  sdrkit_string summary;
  // The check: "good", "failed", or empty when there is none.
  sdrkit_string crc;
  // True if `direction` holds a value.
  bool has_direction;
  // The direction: "request", "reply" or "unsolicited".
  sdrkit_string direction;
  // True if `pair_id` holds a value.
  bool has_pair_id;
  // The id of the paired message.
  int64_t pair_id;
  // True if `response_ms` holds a value.
  bool has_response_ms;
  // The time from the request to this reply. Unit: ms.
  double response_ms;
  // DNP3 Secure Authentication: shown raw.
  bool encrypted;
  // The bytes as hex, or the bits of a burst not recognised.
  sdrkit_string raw_hex;
  // -20 log10 of the eye error of the symbols. Unit: dB.
  double quality_db;
} sdrkit_scada_message_row;

// A row of the SCADA devices. The row key is the protocol and the address ("modbus/17", "dnp3/1").
typedef struct sdrkit_scada_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // "Modbus" or "DNP3".
  sdrkit_string protocol;
  // The address.
  int64_t address;
  // The role: "slave", "master" or "outstation".
  sdrkit_string role;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // When it was first heard, wall clock. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard, wall clock. Seconds since 1970 (UTC).
  double last_heard;
  // When it was first heard, signal time. Unit: s.
  double first_s;
  // When it was last heard, signal time. Unit: s.
  double last_s;
  // True if `last_poll_s` holds a value.
  bool has_last_poll_s;
  // The last poll, signal time. Unit: s.
  double last_poll_s;
  // The polls sent or received.
  int64_t polls;
  // The replies sent or received.
  int64_t replies;
  // The polls that had no reply.
  int64_t unanswered;
  // The exception replies.
  int64_t exceptions;
  // The unsolicited responses.
  int64_t unsolicited;
  // The frames whose check failed.
  int64_t crc_failures;
  // True if `mean_response_ms` holds a value.
  bool has_mean_response_ms;
  // The mean response time. Unit: ms.
  double mean_response_ms;
  // True if `last_iin` holds a value.
  bool has_last_iin;
  // The last internal indications of a DNP3 outstation.
  sdrkit_string last_iin;
  // The addresses it talked with.
  sdrkit_string peers;
  // The registers or points seen so far ("40108 = 555", "AI 0 = 1000").
  sdrkit_string points;
} sdrkit_scada_device_row;

// The reply of `export_csv`.
typedef struct sdrkit_scada_csv_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The messages as CSV text: the header line, then one line a message.
  sdrkit_string csv;
} sdrkit_scada_csv_reply;

// The items that each decoder component of the stack "ermes" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ermes_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `modem` holds a value.
  bool has_modem;
  // The items that the component "modem" dropped.
  int64_t modem;
} sdrkit_ermes_dropped;

// The items that each decoder component of the stack "gsc" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_gsc_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `modem` holds a value.
  bool has_modem;
  // The items that the component "modem" dropped.
  int64_t modem;
} sdrkit_gsc_dropped;

// The items that each decoder component of the stack "scada" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_scada_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `modem` holds a value.
  bool has_modem;
  // The items that the component "modem" dropped.
  int64_t modem;
} sdrkit_scada_dropped;

// The options of `type` of `sdrkit_mpt1327_message_event`.
typedef enum sdrkit_mpt1327_message_event_type {
  // GTC: go to channel
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_G_T_C = 0,
  // ALH: aloha
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H = 1,
  // ALHS: aloha, except RQD
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H_S = 2,
  // ALHD: aloha, except RQS
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H_D = 3,
  // ALHE: aloha, emergency only
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H_E = 4,
  // ALHR: aloha, registration or emergency
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H_R = 5,
  // ALHX: aloha, except RQR
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H_X = 6,
  // ALHF: aloha, fall-back
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_L_H_F = 7,
  // ACK: acknowledgement
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K = 8,
  // ACKI: intermediate acknowledgement
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_I = 9,
  // ACKQ: call queued
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_Q = 10,
  // ACKX: message rejected
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_X = 11,
  // ACKV: called unit unavailable
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_V = 12,
  // ACKE: emergency call acknowledged
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_E = 13,
  // ACKT: try on the given address
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_T = 14,
  // ACKB: call-back
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_C_K_B = 15,
  // AHY: availability check
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_H_Y = 16,
  // AHYX: cancel alert
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_H_Y_X = 17,
  // AHYP: presence check
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_H_Y_P = 18,
  // AHYQ: status message
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_H_Y_Q = 19,
  // AHYC: short data invitation
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_A_H_Y_C = 20,
  // MARK: control channel marker
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_M_A_R_K = 21,
  // MAINT: call maintenance
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_M_A_I_N_T = 22,
  // CLEAR: clear down
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_C_L_E_A_R = 23,
  // MOVE: move to control channel
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_M_O_V_E = 24,
  // BCAST: broadcast
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_B_C_A_S_T = 25,
  // HEAD: short data header
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_H_E_A_D = 26,
  // SAM: single address message
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_S_A_M = 27,
  // STDDATA: standard data
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_S_T_D_D_A_T_A = 28,
  // STDACK: standard data acknowledgement
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_S_T_D_A_C_K = 29,
  // Spare
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_SPARE = 30,
  // Reserved
  SDRKIT_MPT1327_MESSAGE_EVENT_TYPE_R_S_V_D = 31,
} sdrkit_mpt1327_message_event_type;

// The options of `family` of `sdrkit_mpt1327_message_event`.
typedef enum sdrkit_mpt1327_message_event_family {
  // Aircraft
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MPT1327_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_mpt1327_message_event_family;

// An address codeword: `source` is the caller, `target` the called address, `channel` the channel named, `text` the meaning.
typedef struct sdrkit_mpt1327_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The type of a message. A value of `sdrkit_mpt1327_message_event_type`.
  int32_t type;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_mpt1327_message_event_family`.
  int32_t family;
} sdrkit_mpt1327_message_event;

// The options of `action` of `sdrkit_trunk_follow_event`.
typedef enum sdrkit_trunk_follow_event_action {
  // Follow
  SDRKIT_TRUNK_FOLLOW_EVENT_ACTION_FOLLOW = 0,
  // Skipped
  SDRKIT_TRUNK_FOLLOW_EVENT_ACTION_SKIPPED = 1,
  // Release
  SDRKIT_TRUNK_FOLLOW_EVENT_ACTION_RELEASE = 2,
} sdrkit_trunk_follow_event_action;

// The options of `reason` of `sdrkit_trunk_follow_event`.
typedef enum sdrkit_trunk_follow_event_reason {
  // Encrypted
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_ENCRYPTED = 0,
  // No frequency for the channel
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_NO_FREQUENCY = 1,
  // Not in the followed groups
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_NOT_LISTED = 2,
  // Busy with another call
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_BUSY = 3,
  // The call ended
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_ENDED = 4,
  // A better call came
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_REPLACED = 5,
  // Following turned off
  SDRKIT_TRUNK_FOLLOW_EVENT_REASON_OFF = 6,
} sdrkit_trunk_follow_event_reason;

// The options of `urgency` of `sdrkit_trunk_follow_event`.
typedef enum sdrkit_trunk_follow_event_urgency {
  // None
  SDRKIT_TRUNK_FOLLOW_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_TRUNK_FOLLOW_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_TRUNK_FOLLOW_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_TRUNK_FOLLOW_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_trunk_follow_event_urgency;

// The options of `family` of `sdrkit_trunk_follow_event`.
typedef enum sdrkit_trunk_follow_event_family {
  // Aircraft
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TRUNK_FOLLOW_EVENT_FAMILY_OTHER = 12,
} sdrkit_trunk_follow_event_family;

// A follow decision. `source` is the caller, `target` the talkgroup or unit, `channel` the channel number.
typedef struct sdrkit_trunk_follow_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The action of a follow decision. A value of `sdrkit_trunk_follow_event_action`.
  int32_t action;
  // True if `reason` holds a value.
  bool has_reason;
  // The reason of a skip or a release. A value of `sdrkit_trunk_follow_event_reason`.
  int32_t reason;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_trunk_follow_event_urgency`.
  int32_t urgency;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_trunk_follow_event_family`.
  int32_t family;
} sdrkit_trunk_follow_event;

// The options of `kind` of `sdrkit_ltr_word_event`.
typedef enum sdrkit_ltr_word_event_kind {
  // Call
  SDRKIT_LTR_WORD_EVENT_KIND_CALL = 0,
  // Idle
  SDRKIT_LTR_WORD_EVENT_KIND_IDLE = 1,
  // Callsign ID
  SDRKIT_LTR_WORD_EVENT_KIND_CALLSIGN_ID = 2,
  // Busy
  SDRKIT_LTR_WORD_EVENT_KIND_BUSY = 3,
  // Token
  SDRKIT_LTR_WORD_EVENT_KIND_TOKEN = 4,
} sdrkit_ltr_word_event_kind;

// The options of `family` of `sdrkit_ltr_word_event`.
typedef enum sdrkit_ltr_word_event_family {
  // Aircraft
  SDRKIT_LTR_WORD_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_LTR_WORD_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_LTR_WORD_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_LTR_WORD_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_LTR_WORD_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_LTR_WORD_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_LTR_WORD_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_LTR_WORD_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_LTR_WORD_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_LTR_WORD_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_LTR_WORD_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_LTR_WORD_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_LTR_WORD_EVENT_FAMILY_OTHER = 12,
} sdrkit_ltr_word_event_family;

// A good LTR word: `target` is the talkgroup.
typedef struct sdrkit_ltr_word_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // The AREA bit.
  int64_t area;
  // The GO-TO repeater.
  int64_t goto_;
  // The HOME repeater.
  int64_t home;
  // The ID.
  int64_t id;
  // The FREE repeater.
  int64_t free;
  // What the ID says. A value of `sdrkit_ltr_word_event_kind`.
  int32_t kind;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ltr_word_event_family`.
  int32_t family;
} sdrkit_ltr_word_event;

// The options of `end` of `sdrkit_edacs_call_event`.
typedef enum sdrkit_edacs_call_event_end {
  // On air
  SDRKIT_EDACS_CALL_EVENT_END_ACTIVE = 0,
  // Terminator
  SDRKIT_EDACS_CALL_EVENT_END_TERMINATOR = 1,
  // Hang time
  SDRKIT_EDACS_CALL_EVENT_END_HANG_TIME = 2,
  // Replaced
  SDRKIT_EDACS_CALL_EVENT_END_REPLACED = 3,
  // Closed
  SDRKIT_EDACS_CALL_EVENT_END_CLOSED = 4,
  // Single
  SDRKIT_EDACS_CALL_EVENT_END_SINGLE = 5,
} sdrkit_edacs_call_event_end;

// The options of `urgency` of `sdrkit_edacs_call_event`.
typedef enum sdrkit_edacs_call_event_urgency {
  // None
  SDRKIT_EDACS_CALL_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_EDACS_CALL_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_EDACS_CALL_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_EDACS_CALL_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_edacs_call_event_urgency;

// The options of `family` of `sdrkit_edacs_call_event`.
typedef enum sdrkit_edacs_call_event_family {
  // Aircraft
  SDRKIT_EDACS_CALL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_EDACS_CALL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_EDACS_CALL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_EDACS_CALL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_EDACS_CALL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_EDACS_CALL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_EDACS_CALL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_EDACS_CALL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_EDACS_CALL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_EDACS_CALL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_EDACS_CALL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_EDACS_CALL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_EDACS_CALL_EVENT_FAMILY_OTHER = 12,
} sdrkit_edacs_call_event_family;

// An EDACS call that started or ended.
typedef struct sdrkit_edacs_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Where a call is in its life. A value of `sdrkit_edacs_call_event_end`.
  int32_t end;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // A group call.
  bool group;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_edacs_call_event_urgency`.
  int32_t urgency;
  // A call that no message announced.
  bool carrier_only;
  // The length of a call. A length of time in seconds.
  double duration;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_edacs_call_event_family`.
  int32_t family;
} sdrkit_edacs_call_event;

// The options of `type` of `sdrkit_pager_pocsag_page_event`.
typedef enum sdrkit_pager_pocsag_page_event_type {
  // Tone
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_TONE = 0,
  // Numeric
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_NUMERIC = 1,
  // Text
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_ALPHA = 2,
  // Secure
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_SECURE = 3,
  // Binary
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_BINARY = 4,
  // Instruction
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_INSTRUCTION = 5,
  // Sequential call
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_TYPE_SELCALL = 6,
} sdrkit_pager_pocsag_page_event_type;

// The options of `family` of `sdrkit_pager_pocsag_page_event`.
typedef enum sdrkit_pager_pocsag_page_event_family {
  // Aircraft
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PAGER_POCSAG_PAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_pager_pocsag_page_event_family;

// A POCSAG page: `source` is the RIC, `text` the message.
typedef struct sdrkit_pager_pocsag_page_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The RIC.
  sdrkit_string source;
  // The message.
  sdrkit_string text;
  // True if `function` holds a value.
  bool has_function;
  // The function code.
  int64_t function;
  // True if `baud` holds a value.
  bool has_baud;
  // The bit rate. Unit: bit/s.
  int64_t baud;
  // What the page carries. A value of `sdrkit_pager_pocsag_page_event_type`.
  int32_t type;
  // A word was lost or the end is not sure.
  bool damaged;
  // The kind of thing that the event is about. A value of `sdrkit_pager_pocsag_page_event_family`.
  int32_t family;
} sdrkit_pager_pocsag_page_event;

// The options of `type` of `sdrkit_pager_flex_page_event`.
typedef enum sdrkit_pager_flex_page_event_type {
  // Tone
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_TONE = 0,
  // Numeric
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_NUMERIC = 1,
  // Text
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_ALPHA = 2,
  // Secure
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_SECURE = 3,
  // Binary
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_BINARY = 4,
  // Instruction
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_INSTRUCTION = 5,
  // Sequential call
  SDRKIT_PAGER_FLEX_PAGE_EVENT_TYPE_SELCALL = 6,
} sdrkit_pager_flex_page_event_type;

// The options of `family` of `sdrkit_pager_flex_page_event`.
typedef enum sdrkit_pager_flex_page_event_family {
  // Aircraft
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PAGER_FLEX_PAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_pager_flex_page_event_family;

// A FLEX page: `source` is the capcode, `text` the message.
typedef struct sdrkit_pager_flex_page_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The capcode.
  sdrkit_string source;
  // The message.
  sdrkit_string text;
  // The FLEX speed ("FLEX 3200/4").
  sdrkit_string speed;
  // What the page carries. A value of `sdrkit_pager_flex_page_event_type`.
  int32_t type;
  // A word was lost or the end is not sure.
  bool damaged;
  // The kind of thing that the event is about. A value of `sdrkit_pager_flex_page_event_family`.
  int32_t family;
} sdrkit_pager_flex_page_event;

// The options of `family` of `sdrkit_pager_tx_event`.
typedef enum sdrkit_pager_tx_event_family {
  // Aircraft
  SDRKIT_PAGER_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PAGER_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PAGER_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PAGER_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PAGER_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PAGER_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PAGER_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PAGER_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PAGER_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PAGER_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PAGER_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PAGER_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PAGER_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_pager_tx_event_family;

// The fields of a change of a POCSAG transmission.
typedef struct sdrkit_pager_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a page was refused.
  sdrkit_string reason;
  // The addresses of the pages, separated by commas.
  sdrkit_string target;
  // The text of the page for started, queued and refused; empty for finished.
  sdrkit_string text;
  // The family of the source: a pager. A value of `sdrkit_pager_tx_event_family`.
  int32_t family;
} sdrkit_pager_tx_event;

// The options of `family` of `sdrkit_ermes_batch_event`.
typedef enum sdrkit_ermes_batch_event_family {
  // Aircraft
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ERMES_BATCH_EVENT_FAMILY_OTHER = 12,
} sdrkit_ermes_batch_event_family;

// A good ERMES batch.
typedef struct sdrkit_ermes_batch_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The batch letter.
  sdrkit_string batch;
  // The network.
  sdrkit_string network;
  // The initial addresses, decimal.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_ermes_batch_event_family`.
  int32_t family;
} sdrkit_ermes_batch_event;

// The options of `family` of `sdrkit_ermes_message_event`.
typedef enum sdrkit_ermes_message_event_family {
  // Aircraft
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ERMES_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_ermes_message_event_family;

// An ERMES message.
typedef struct sdrkit_ermes_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `target` holds a value.
  bool has_target;
  // The local address.
  sdrkit_string target;
  // True if `category` holds a value.
  bool has_category;
  // The category.
  sdrkit_string category;
  // The text of the message.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_ermes_message_event_family`.
  int32_t family;
} sdrkit_ermes_message_event;

// The options of `kind` of `sdrkit_multitone_transmission_event`.
typedef enum sdrkit_multitone_transmission_event_kind {
  // FFSK data
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_KIND_FFSK_DATA = 0,
  // FSK data
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_KIND_FSK_DATA = 1,
  // Tone sequence
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_KIND_TONE_SEQUENCE = 2,
  // Repeating multi-tone audio
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_KIND_REPEATING = 3,
  // Multi-tone audio
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_KIND_MULTI_TONE = 4,
  // Other
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_KIND_OTHER = 5,
} sdrkit_multitone_transmission_event_kind;

// The options of `family` of `sdrkit_multitone_transmission_event`.
typedef enum sdrkit_multitone_transmission_event_family {
  // Aircraft
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MULTITONE_TRANSMISSION_EVENT_FAMILY_OTHER = 12,
} sdrkit_multitone_transmission_event_family;

// A finished transmission.
typedef struct sdrkit_multitone_transmission_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Kind. A value of `sdrkit_multitone_transmission_event_kind`.
  int32_t kind;
  // Duration. A length of time in seconds.
  double duration;
  // SNR. Unit: dB.
  double snr_db;
  // Deviation. A frequency in Hz.
  double deviation_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_multitone_transmission_event_family`.
  int32_t family;
} sdrkit_multitone_transmission_event;

// The options of `kind` of `sdrkit_multitone_burst_event`.
typedef enum sdrkit_multitone_burst_event_kind {
  // FFSK
  SDRKIT_MULTITONE_BURST_EVENT_KIND_FFSK = 0,
  // FSK
  SDRKIT_MULTITONE_BURST_EVENT_KIND_FSK = 1,
} sdrkit_multitone_burst_event_kind;

// The options of `family` of `sdrkit_multitone_burst_event`.
typedef enum sdrkit_multitone_burst_event_family {
  // Aircraft
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MULTITONE_BURST_EVENT_FAMILY_OTHER = 12,
} sdrkit_multitone_burst_event_family;

// A finished FSK burst.
typedef struct sdrkit_multitone_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Kind. A value of `sdrkit_multitone_burst_event_kind`.
  int32_t kind;
  // Baud.
  double baud;
  // Bits.
  int64_t bits;
  // Valid BCH words.
  int64_t bch_valid;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_multitone_burst_event_family`.
  int32_t family;
} sdrkit_multitone_burst_event;

// The options of `family` of `sdrkit_motorola_type2_message_event`.
typedef enum sdrkit_motorola_type2_message_event_family {
  // Aircraft
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MOTOROLA_TYPE2_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_motorola_type2_message_event_family;

// A Type II message.
typedef struct sdrkit_motorola_type2_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // An address.
  sdrkit_string address;
  // A group word.
  bool group;
  // A command.
  sdrkit_string command;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_motorola_type2_message_event_family`.
  int32_t family;
} sdrkit_motorola_type2_message_event;

// The options of `urgency` of `sdrkit_trunk_grant_event`.
typedef enum sdrkit_trunk_grant_event_urgency {
  // None
  SDRKIT_TRUNK_GRANT_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_TRUNK_GRANT_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_TRUNK_GRANT_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_TRUNK_GRANT_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_trunk_grant_event_urgency;

// The options of `family` of `sdrkit_trunk_grant_event`.
typedef enum sdrkit_trunk_grant_event_family {
  // Aircraft
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TRUNK_GRANT_EVENT_FAMILY_OTHER = 12,
} sdrkit_trunk_grant_event_family;

// A voice grant: `target` is the talkgroup, `source` the caller, `channel` the channel number.
typedef struct sdrkit_trunk_grant_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // A group word.
  bool group;
  // Emergency.
  bool emergency;
  // Encrypted.
  bool encrypted;
  // Digital.
  bool digital;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_trunk_grant_event_urgency`.
  int32_t urgency;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_trunk_grant_event_family`.
  int32_t family;
} sdrkit_trunk_grant_event;

// The options of `verdict` of `sdrkit_opensky_verdict_event`.
typedef enum sdrkit_opensky_verdict_event_verdict {
  // Consistent with OpenSky
  SDRKIT_OPENSKY_VERDICT_EVENT_VERDICT_CONSISTENT = 0,
  // Not the OpenSky pattern
  SDRKIT_OPENSKY_VERDICT_EVENT_VERDICT_OTHER = 1,
} sdrkit_opensky_verdict_event_verdict;

// The options of `family` of `sdrkit_opensky_verdict_event`.
typedef enum sdrkit_opensky_verdict_event_family {
  // Aircraft
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_OPENSKY_VERDICT_EVENT_FAMILY_OTHER = 12,
} sdrkit_opensky_verdict_event_family;

// The verdict of one block.
typedef struct sdrkit_opensky_verdict_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The verdict. A value of `sdrkit_opensky_verdict_event_verdict`.
  int32_t verdict;
  // Symbol rate found, Hz. A frequency in Hz.
  double symbol_rate_hz;
  // The levels.
  int64_t levels;
  // The deviation. Unit: Hz.
  double deviation_hz;
  // The burst period. Unit: ms.
  double burst_ms;
  // The occupied width. Unit: Hz.
  double width_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_opensky_verdict_event_family`.
  int32_t family;
} sdrkit_opensky_verdict_event;

// The options of `family` of `sdrkit_gsc_page_event`.
typedef enum sdrkit_gsc_page_event_family {
  // Aircraft
  SDRKIT_GSC_PAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_GSC_PAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_GSC_PAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_GSC_PAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_GSC_PAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_GSC_PAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_GSC_PAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_GSC_PAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_GSC_PAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_GSC_PAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_GSC_PAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_GSC_PAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_GSC_PAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_gsc_page_event_family;

// A GSC page: `target` is the pager code.
typedef struct sdrkit_gsc_page_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The pager code.
  sdrkit_string target;
  // The kind of page.
  sdrkit_string kind;
  // The address function, 1 to 4.
  int64_t function;
  // The text of a data page.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_gsc_page_event_family`.
  int32_t family;
} sdrkit_gsc_page_event;

// The options of `family` of `sdrkit_scada_message_event`.
typedef enum sdrkit_scada_message_event_family {
  // Aircraft
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SCADA_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_scada_message_event_family;

// A SCADA message: `source` is the sender, `target` the receiver, `text` the summary.
typedef struct sdrkit_scada_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender.
  sdrkit_string source;
  // The receiver.
  sdrkit_string target;
  // The protocol.
  sdrkit_string protocol;
  // The function.
  sdrkit_string function;
  // The summary.
  sdrkit_string text;
  // The check: "good", "failed", or empty.
  sdrkit_string crc;
  // The kind of thing that the event is about. A value of `sdrkit_scada_message_event_family`.
  int32_t family;
} sdrkit_scada_message_event;

// The options of `family` of `sdrkit_scada_device_event`.
typedef enum sdrkit_scada_device_event_family {
  // Aircraft
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SCADA_DEVICE_EVENT_FAMILY_OTHER = 12,
} sdrkit_scada_device_event_family;

// A new SCADA device.
typedef struct sdrkit_scada_device_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // "Modbus" or "DNP3".
  sdrkit_string protocol;
  // The address.
  int64_t address;
  // The role.
  sdrkit_string role;
  // The kind of thing that the event is about. A value of `sdrkit_scada_device_event_family`.
  int32_t family;
} sdrkit_scada_device_event;

// The arguments of the pager `send`.
typedef struct sdrkit_pager_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The address (RIC).
  int64_t address;
  // True if `function` holds a value.
  bool has_function;
  // The function bits.
  int64_t function;
  // True if `page_type` holds a value.
  bool has_page_type;
  // What the page carries. The index of an option.
  int32_t page_type;
  // True if `text` holds a value.
  bool has_text;
  // The message.
  sdrkit_string text;
} sdrkit_pager_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_PAGER_SEND_ARGS_INIT {sizeof(sdrkit_pager_send_args), 0, 0, 0, 0, 0, 0, {0}}

// Fills `rows` with the changed rows of the table "messages" of the stack "mpt1327" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_mpt1327_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mpt1327_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.Mpt1327MessageRow", "messages", rows, sizeof(sdrkit_mpt1327_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "mpt1327" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_mpt1327_channels_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mpt1327_channel_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.Mpt1327ChannelRow", "channels", rows, sizeof(sdrkit_mpt1327_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mpt1327.message"
// (MPT 1327 message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpt1327_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpt1327_message_event *event) {
  return sdrkit_typed_event(handle, n, "mpt1327.message", "trunk.Mpt1327MessageEvent", event, sizeof(sdrkit_mpt1327_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "trunk.follow"
// (Follow decision). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_trunk_follow_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_trunk_follow_event *event) {
  return sdrkit_typed_event(handle, n, "trunk.follow", "trunk.FollowEvent", event, sizeof(sdrkit_trunk_follow_event));
}

// The setting "framing" (Framing) of the stack "mpt1327". A value of `sdrkit_mpt1327_framing`. How codewords are found: by the standard SYNC word, by their check bits alone (networks with another sync word), or automatically.
static inline int32_t sdrkit_mpt1327_get_framing(SDRKIT_HANDLE handle, sdrkit_mpt1327_framing *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "framing", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mpt1327_framing)index;
  return code;
}
static inline int32_t sdrkit_mpt1327_set_framing(SDRKIT_HANDLE handle, sdrkit_mpt1327_framing value) {
  return sdrkit_typed_setting_set_enum(handle, "framing", (int32_t)value);
}

// The setting "control_channel" (Control channel number) of the stack "mpt1327". The control channel's own number, for the frequency column (-1: the first channel announced as a control channel).
static inline int32_t sdrkit_mpt1327_get_control_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "control_channel", value);
}
static inline int32_t sdrkit_mpt1327_set_control_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "control_channel", value);
}

// The setting "spacing_khz" (Channel spacing) of the stack "mpt1327". Unit: kHz. The spacing between channel numbers, for the frequency column.
static inline int32_t sdrkit_mpt1327_get_spacing_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "spacing_khz", value);
}
static inline int32_t sdrkit_mpt1327_set_spacing_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "spacing_khz", value);
}

// The setting "follow" (Follow calls) of the stack "mpt1327". Ask the radio to listen to the voice channel of each call.
static inline int32_t sdrkit_mpt1327_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_mpt1327_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "follow_groups" (Follow groups) of the stack "mpt1327". Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused.
static inline int32_t sdrkit_mpt1327_get_follow_groups(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "follow_groups", value);
}
static inline int32_t sdrkit_mpt1327_set_follow_groups(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "follow_groups", value);
}

// The setting "follow_max_s" (Longest follow) of the stack "mpt1327". A length of time in seconds. A call followed longer than this ends (a stuck carrier).
static inline int32_t sdrkit_mpt1327_get_follow_max_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "follow_max_s", value);
}
static inline int32_t sdrkit_mpt1327_set_follow_max_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "follow_max_s", value);
}

// Fills `rows` with the changed rows of the table "words" of the stack "ltr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `talkgroup` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_ltr_words_rows(SDRKIT_HANDLE handle,
                                           sdrkit_ltr_word_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.LtrWordRow", "words", rows, sizeof(sdrkit_ltr_word_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "repeaters" of the stack "ltr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ltr_repeaters_rows(SDRKIT_HANDLE handle,
                                               sdrkit_ltr_repeater_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.LtrRepeaterRow", "repeaters", rows, sizeof(sdrkit_ltr_repeater_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ltr.word"
// (LTR word). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ltr_word_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ltr_word_event *event) {
  return sdrkit_typed_event(handle, n, "ltr.word", "trunk.LtrWordEvent", event, sizeof(sdrkit_ltr_word_event));
}

// Runs the command "clear" (Clear) of the stack "ltr" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ltr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "follow" (Follow calls) of the stack "ltr". Ask the radio to listen to the voice channel of each call.
static inline int32_t sdrkit_ltr_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_ltr_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "follow_groups" (Follow groups) of the stack "ltr". Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused.
static inline int32_t sdrkit_ltr_get_follow_groups(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "follow_groups", value);
}
static inline int32_t sdrkit_ltr_set_follow_groups(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "follow_groups", value);
}

// The setting "follow_max_s" (Longest follow) of the stack "ltr". A length of time in seconds. A call followed longer than this ends (a stuck carrier).
static inline int32_t sdrkit_ltr_get_follow_max_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "follow_max_s", value);
}
static inline int32_t sdrkit_ltr_set_follow_max_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "follow_max_s", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "edacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `source` names a row.
static inline int32_t sdrkit_edacs_calls_rows(SDRKIT_HANDLE handle,
                                             sdrkit_edacs_call_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.EdacsCallRow", "calls", rows, sizeof(sdrkit_edacs_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "edacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_edacs_talkgroups_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_edacs_talkgroup_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.EdacsTalkgroupRow", "talkgroups", rows, sizeof(sdrkit_edacs_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "edacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_edacs_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_edacs_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.EdacsChannelRow", "channels", rows, sizeof(sdrkit_edacs_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "edacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_edacs_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_edacs_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.EdacsFrameRow", "frames", rows, sizeof(sdrkit_edacs_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "words" of the stack "edacs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_edacs_words_rows(SDRKIT_HANDLE handle,
                                             sdrkit_edacs_word_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.EdacsWordRow", "words", rows, sizeof(sdrkit_edacs_word_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "edacs.call"
// (EDACS call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_edacs_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_edacs_call_event *event) {
  return sdrkit_typed_event(handle, n, "edacs.call", "trunk.EdacsCallEvent", event, sizeof(sdrkit_edacs_call_event));
}

// Runs the command "clear" (Clear) of the stack "edacs" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_edacs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "extended" (Extended Addressing) of the stack "edacs". Extended Addressing instead of Standard / Networked.
static inline int32_t sdrkit_edacs_get_extended(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "extended", value);
}
static inline int32_t sdrkit_edacs_set_extended(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "extended", value);
}

// The setting "esk" (ESK) of the stack "edacs". Remove the ESK byte A0 some systems XOR into both messages.
static inline int32_t sdrkit_edacs_get_esk(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "esk", value);
}
static inline int32_t sdrkit_edacs_set_esk(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "esk", value);
}

// The setting "lcn_table" (Voice channel table) of the stack "edacs". One line per channel, MHz: "frequency" (LCN = line number) or "LCN frequency". LCN 1 to 64.
static inline int32_t sdrkit_edacs_get_lcn_table(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "lcn_table", value);
}
static inline int32_t sdrkit_edacs_set_lcn_table(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "lcn_table", value);
}

// The setting "follow" (Follow calls) of the stack "edacs". Ask the radio to listen to the voice channel of each call.
static inline int32_t sdrkit_edacs_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_edacs_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "follow_carriers" (Also follow carriers) of the stack "edacs". Also follow carriers that no message announced.
static inline int32_t sdrkit_edacs_get_follow_carriers(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_carriers", value);
}
static inline int32_t sdrkit_edacs_set_follow_carriers(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_carriers", value);
}

// The setting "follow_groups" (Follow groups) of the stack "edacs". Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused.
static inline int32_t sdrkit_edacs_get_follow_groups(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "follow_groups", value);
}
static inline int32_t sdrkit_edacs_set_follow_groups(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "follow_groups", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "tones" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `start_s`.
static inline int32_t sdrkit_tones_events_rows(SDRKIT_HANDLE handle,
                                              sdrkit_tones_heard_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.TonesHeardRow", "events", rows, sizeof(sdrkit_tones_heard_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "tones" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_tones_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "demod" (Demodulation) of the stack "tones". A value of `sdrkit_tones_demod`.
static inline int32_t sdrkit_tones_get_demod(SDRKIT_HANDLE handle, sdrkit_tones_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_tones_demod)index;
  return code;
}
static inline int32_t sdrkit_tones_set_demod(SDRKIT_HANDLE handle, sdrkit_tones_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "audio_input" (Audio input) of the stack "tones". The input is audio (a sound card or an audio recording): real samples, 1.0 is 5 kHz of deviation. The source sets it.
static inline int32_t sdrkit_tones_get_audio_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_input", value);
}
static inline int32_t sdrkit_tones_set_audio_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_input", value);
}

// The setting "gate" (Gate) of the stack "tones". A value of `sdrkit_tones_gate`.
static inline int32_t sdrkit_tones_get_gate(SDRKIT_HANDLE handle, sdrkit_tones_gate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_tones_gate)index;
  return code;
}
static inline int32_t sdrkit_tones_set_gate(SDRKIT_HANDLE handle, sdrkit_tones_gate value) {
  return sdrkit_typed_setting_set_enum(handle, "gate", (int32_t)value);
}

// The setting "gate_kind" (Gate on) of the stack "tones". A value of `sdrkit_tones_gate_kind`.
static inline int32_t sdrkit_tones_get_gate_kind(SDRKIT_HANDLE handle, sdrkit_tones_gate_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gate_kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_tones_gate_kind)index;
  return code;
}
static inline int32_t sdrkit_tones_set_gate_kind(SDRKIT_HANDLE handle, sdrkit_tones_gate_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "gate_kind", (int32_t)value);
}

// The setting "gate_tone" (Tone) of the stack "tones". A value of `sdrkit_tones_gate_tone`. The CTCSS tone that opens the gate.
static inline int32_t sdrkit_tones_get_gate_tone(SDRKIT_HANDLE handle, sdrkit_tones_gate_tone *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gate_tone", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_tones_gate_tone)index;
  return code;
}
static inline int32_t sdrkit_tones_set_gate_tone(SDRKIT_HANDLE handle, sdrkit_tones_gate_tone value) {
  return sdrkit_typed_setting_set_enum(handle, "gate_tone", (int32_t)value);
}

// The setting "gate_code" (Code) of the stack "tones". A value of `sdrkit_tones_gate_code`. The DCS code that opens the gate.
static inline int32_t sdrkit_tones_get_gate_code(SDRKIT_HANDLE handle, sdrkit_tones_gate_code *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gate_code", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_tones_gate_code)index;
  return code;
}
static inline int32_t sdrkit_tones_set_gate_code(SDRKIT_HANDLE handle, sdrkit_tones_gate_code value) {
  return sdrkit_typed_setting_set_enum(handle, "gate_code", (int32_t)value);
}

// The setting "gate_inverted" (Code inverted) of the stack "tones".
static inline int32_t sdrkit_tones_get_gate_inverted(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gate_inverted", value);
}
static inline int32_t sdrkit_tones_set_gate_inverted(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gate_inverted", value);
}

// The setting "listen" (Listen) of the stack "tones".
static inline int32_t sdrkit_tones_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_tones_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// Fills `rows` with the changed rows of the table "pages" of the stack "pager" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// Addressee: `address`. Text: `message`. Time: `time_s`.
static inline int32_t sdrkit_pager_pages_rows(SDRKIT_HANDLE handle,
                                             sdrkit_pager_page_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.PagerPageRow", "pages", rows, sizeof(sdrkit_pager_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "speeds" of the stack "pager" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pager_speeds_rows(SDRKIT_HANDLE handle,
                                              sdrkit_pager_speed_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.PagerSpeedRow", "speeds", rows, sizeof(sdrkit_pager_speed_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "pager" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// Time: `last_heard`.
static inline int32_t sdrkit_pager_heard_rows(SDRKIT_HANDLE handle,
                                             sdrkit_pager_heard_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.PagerHeardRow", "heard", rows, sizeof(sdrkit_pager_heard_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "pocsag.page"
// (POCSAG page). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_pager_pocsag_page_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_pager_pocsag_page_event *event) {
  return sdrkit_typed_event(handle, n, "pocsag.page", "trunk.PocsagPageEvent", event, sizeof(sdrkit_pager_pocsag_page_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "flex.page"
// (FLEX page). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_pager_flex_page_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_pager_flex_page_event *event) {
  return sdrkit_typed_event(handle, n, "flex.page", "trunk.FlexPageEvent", event, sizeof(sdrkit_pager_flex_page_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "pager.tx"
// (POCSAG transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_pager_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_pager_tx_event *event) {
  return sdrkit_typed_event(handle, n, "pager.tx", "trunk.PagerTxEvent", event, sizeof(sdrkit_pager_tx_event));
}

// Runs the command "clear" (Clear) of the stack "pager" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_pager_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a page) of the stack "pager" on the session of `handle`. Checks the page and queues it. A transmission starts when none is on air; pages sent during a transmission go in the next one. POCSAG only: with the setting protocol on FLEX it is refused. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_pager_send(SDRKIT_HANDLE handle, const sdrkit_pager_send_args *args) {
  return sdrkit_typed_command(handle, "send", "trunk.PagerSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "pager" on the session of `handle`. Drops the pages waiting and ends the transmission after the current bit with a 1 ms fall.
static inline int32_t sdrkit_pager_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The setting "pocsag" (POCSAG) of the stack "pager". Look for POCSAG syncs.
static inline int32_t sdrkit_pager_get_pocsag(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "pocsag", value);
}
static inline int32_t sdrkit_pager_set_pocsag(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "pocsag", value);
}

// The setting "flex" (FLEX) of the stack "pager". Look for FLEX syncs.
static inline int32_t sdrkit_pager_get_flex(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "flex", value);
}
static inline int32_t sdrkit_pager_set_flex(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "flex", value);
}

// The setting "pocsag_512" (POCSAG 512) of the stack "pager".
static inline int32_t sdrkit_pager_get_pocsag_512(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "pocsag_512", value);
}
static inline int32_t sdrkit_pager_set_pocsag_512(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "pocsag_512", value);
}

// The setting "pocsag_1200" (POCSAG 1200) of the stack "pager".
static inline int32_t sdrkit_pager_get_pocsag_1200(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "pocsag_1200", value);
}
static inline int32_t sdrkit_pager_set_pocsag_1200(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "pocsag_1200", value);
}

// The setting "pocsag_2400" (POCSAG 2400) of the stack "pager". Turn off 2400 when only a slower speed is on air, to avoid false decodes.
static inline int32_t sdrkit_pager_get_pocsag_2400(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "pocsag_2400", value);
}
static inline int32_t sdrkit_pager_set_pocsag_2400(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "pocsag_2400", value);
}

// The setting "selcall" (Sequential calls) of the stack "pager". Also decode the sequential tone calls sent on the same channel.
static inline int32_t sdrkit_pager_get_selcall(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "selcall", value);
}
static inline int32_t sdrkit_pager_set_selcall(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "selcall", value);
}

// The setting "channel" (Channel) of the stack "pager". A value of `sdrkit_pager_channel`. 12.5 kHz cuts a neighbour better but needs the signal on the channel centre.
static inline int32_t sdrkit_pager_get_channel(SDRKIT_HANDLE handle, sdrkit_pager_channel *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pager_channel)index;
  return code;
}
static inline int32_t sdrkit_pager_set_channel(SDRKIT_HANDLE handle, sdrkit_pager_channel value) {
  return sdrkit_typed_setting_set_enum(handle, "channel", (int32_t)value);
}

// The setting "narrow_decoder" (Narrow second decoder) of the stack "pager". On a 25 kHz channel, a second POCSAG and FLEX decoder behind a 7 kHz filter reads weak pages the first misses.
static inline int32_t sdrkit_pager_get_narrow_decoder(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "narrow_decoder", value);
}
static inline int32_t sdrkit_pager_set_narrow_decoder(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "narrow_decoder", value);
}

// The setting "combine" (Combine copies) of the stack "pager". Paging networks repeat messages: vote the copies word by word.
static inline int32_t sdrkit_pager_get_combine(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "combine", value);
}
static inline int32_t sdrkit_pager_set_combine(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "combine", value);
}

// The setting "read" (Read POCSAG pages) of the stack "pager". A value of `sdrkit_pager_read`.
static inline int32_t sdrkit_pager_get_read(SDRKIT_HANDLE handle, sdrkit_pager_read *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "read", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pager_read)index;
  return code;
}
static inline int32_t sdrkit_pager_set_read(SDRKIT_HANDLE handle, sdrkit_pager_read value) {
  return sdrkit_typed_setting_set_enum(handle, "read", (int32_t)value);
}

// The setting "read_by_address" (Read by address) of the stack "pager". Rules for single addresses: "address:alpha" or "address:numeric", separated by commas or spaces.
static inline int32_t sdrkit_pager_get_read_by_address(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "read_by_address", value);
}
static inline int32_t sdrkit_pager_set_read_by_address(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "read_by_address", value);
}

// The setting "charset" (Character set) of the stack "pager". A value of `sdrkit_pager_charset`.
static inline int32_t sdrkit_pager_get_charset(SDRKIT_HANDLE handle, sdrkit_pager_charset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "charset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pager_charset)index;
  return code;
}
static inline int32_t sdrkit_pager_set_charset(SDRKIT_HANDLE handle, sdrkit_pager_charset value) {
  return sdrkit_typed_setting_set_enum(handle, "charset", (int32_t)value);
}

// The setting "wide_input" (One carrier) of the stack "pager". The input is one carrier at 192 ksps (a channel of a bank span).
static inline int32_t sdrkit_pager_get_wide_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide_input", value);
}
static inline int32_t sdrkit_pager_set_wide_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide_input", value);
}

// The setting "protocol" (Protocol) of the stack "pager". A value of `sdrkit_pager_protocol`. The protocol that send uses. Only POCSAG is sent: FLEX is a licensed paging service, and send refuses it.
static inline int32_t sdrkit_pager_get_protocol(SDRKIT_HANDLE handle, sdrkit_pager_protocol *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "protocol", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pager_protocol)index;
  return code;
}
static inline int32_t sdrkit_pager_set_protocol(SDRKIT_HANDLE handle, sdrkit_pager_protocol value) {
  return sdrkit_typed_setting_set_enum(handle, "protocol", (int32_t)value);
}

// The setting "pocsag_rate" (Speed) of the stack "pager". A value of `sdrkit_pager_pocsag_rate`. The POCSAG bit rate sent: 512, 1200 or 2400 bit/s.
static inline int32_t sdrkit_pager_get_pocsag_rate(SDRKIT_HANDLE handle, sdrkit_pager_pocsag_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pocsag_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pager_pocsag_rate)index;
  return code;
}
static inline int32_t sdrkit_pager_set_pocsag_rate(SDRKIT_HANDLE handle, sdrkit_pager_pocsag_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "pocsag_rate", (int32_t)value);
}

// The setting "deviation" (Deviation) of the stack "pager". A frequency in Hz. The frequency of a steady bit from the carrier: 4500 Hz in ITU-R M.584-2.
static inline int32_t sdrkit_pager_get_deviation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "deviation", value);
}
static inline int32_t sdrkit_pager_set_deviation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "deviation", value);
}

// The setting "polarity" (Polarity) of the stack "pager". A value of `sdrkit_pager_polarity`. Normal sends a 1 as the lower frequency; some transmitters invert it.
static inline int32_t sdrkit_pager_get_polarity(SDRKIT_HANDLE handle, sdrkit_pager_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pager_polarity)index;
  return code;
}
static inline int32_t sdrkit_pager_set_polarity(SDRKIT_HANDLE handle, sdrkit_pager_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "polarity", (int32_t)value);
}

// The setting "tx_level" (Level) of the stack "pager". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_pager_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_pager_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "ermes" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// Addressee: `address`. Text: `text`. Time: `time_s`.
static inline int32_t sdrkit_ermes_rows(SDRKIT_HANDLE handle,
                                       sdrkit_ermes_entry_row *rows,
                                       uint32_t capacity,
                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.ErmesEntryRow", "rows", rows, sizeof(sdrkit_ermes_entry_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ermes.batch"
// (ERMES batch). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ermes_batch_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ermes_batch_event *event) {
  return sdrkit_typed_event(handle, n, "ermes.batch", "trunk.ErmesBatchEvent", event, sizeof(sdrkit_ermes_batch_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ermes.message"
// (ERMES message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ermes_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ermes_message_event *event) {
  return sdrkit_typed_event(handle, n, "ermes.message", "trunk.ErmesMessageEvent", event, sizeof(sdrkit_ermes_message_event));
}

// Runs the command "clear" (Clear) of the stack "ermes" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ermes_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ermes" are the sdrkit_ermes_gen_*() functions
// after the settings of its receiver.

// The setting "gen_last_batch" (Test signal last batch) of the test signal generator of the stack "ermes".
static inline int32_t sdrkit_ermes_gen_get_last_batch(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_last_batch", value);
}
static inline int32_t sdrkit_ermes_gen_set_last_batch(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_last_batch", value);
}

// Fills `rows` with the changed rows of the table "transmissions" of the stack "multitone" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the calls of the stack.
// Time: `time_s`.
static inline int32_t sdrkit_multitone_transmissions_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_multitone_transmission_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MultitoneTransmissionRow", "transmissions", rows, sizeof(sdrkit_multitone_transmission_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tx_tones" of the stack "multitone" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_multitone_tx_tones_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_multitone_tone_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MultitoneToneRow", "tx_tones", rows, sizeof(sdrkit_multitone_tone_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tx_lines" of the stack "multitone" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_multitone_tx_lines_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_multitone_line_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MultitoneLineRow", "tx_lines", rows, sizeof(sdrkit_multitone_line_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tones_heard" of the stack "multitone" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_multitone_tones_heard_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_multitone_heard_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MultitoneHeardRow", "tones_heard", rows, sizeof(sdrkit_multitone_heard_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "multitone" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_multitone_bursts_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_multitone_burst_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MultitoneBurstRow", "bursts", rows, sizeof(sdrkit_multitone_burst_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "multitone.transmission"
// (Multitone transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_multitone_transmission_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_multitone_transmission_event *event) {
  return sdrkit_typed_event(handle, n, "multitone.transmission", "trunk.MultitoneTransmissionEvent", event, sizeof(sdrkit_multitone_transmission_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "multitone.burst"
// (Multitone FSK burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_multitone_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_multitone_burst_event *event) {
  return sdrkit_typed_event(handle, n, "multitone.burst", "trunk.MultitoneBurstEvent", event, sizeof(sdrkit_multitone_burst_event));
}

// Runs the command "clear" (Clear) of the stack "multitone" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_multitone_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "motorola_type2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `address` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_motorola_type2_messages_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_motorola_type2_message_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MotorolaType2MessageRow", "messages", rows, sizeof(sdrkit_motorola_type2_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "talkgroups" of the stack "motorola_type2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `talkgroup` names a row.
static inline int32_t sdrkit_motorola_type2_talkgroups_rows(SDRKIT_HANDLE handle,
                                                           sdrkit_motorola_type2_talkgroup_row *rows,
                                                           uint32_t capacity,
                                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MotorolaType2TalkgroupRow", "talkgroups", rows, sizeof(sdrkit_motorola_type2_talkgroup_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "motorola_type2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_motorola_type2_sites_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_motorola_type2_site_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MotorolaType2SiteRow", "sites", rows, sizeof(sdrkit_motorola_type2_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "patches" of the stack "motorola_type2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `talkgroup` names a row.
static inline int32_t sdrkit_motorola_type2_patches_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_motorola_type2_patch_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MotorolaType2PatchRow", "patches", rows, sizeof(sdrkit_motorola_type2_patch_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "motorola_type2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `talkgroup` names a row.
static inline int32_t sdrkit_motorola_type2_calls_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_motorola_type2_call_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.MotorolaType2CallRow", "calls", rows, sizeof(sdrkit_motorola_type2_call_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "type2.message"
// (Type II message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_motorola_type2_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_motorola_type2_message_event *event) {
  return sdrkit_typed_event(handle, n, "type2.message", "trunk.MotorolaType2MessageEvent", event, sizeof(sdrkit_motorola_type2_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "trunk.grant"
// (Voice grant). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_trunk_grant_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_trunk_grant_event *event) {
  return sdrkit_typed_event(handle, n, "trunk.grant", "trunk.GrantEvent", event, sizeof(sdrkit_trunk_grant_event));
}

// Runs the command "clear" (Clear) of the stack "motorola_type2" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_motorola_type2_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "motorola_type2" are the sdrkit_motorola_type2_gen_*() functions
// after the settings of its receiver.

// The setting "plan" (Channel plan) of the stack "motorola_type2". The path of a plan file in the host's storage (docs/schema/plans.json): one numbered band for each row of channel numbers. Empty for no plan: grants are listed with no frequency and never followed.
static inline int32_t sdrkit_motorola_type2_get_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "plan", value);
}
static inline int32_t sdrkit_motorola_type2_set_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "plan", value);
}

// The setting "follow" (Follow calls) of the stack "motorola_type2". Ask the radio to listen to the voice channel of each call.
static inline int32_t sdrkit_motorola_type2_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_motorola_type2_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "follow_private" (Also follow unit-to-unit calls) of the stack "motorola_type2". Hand private calls to the follower too. Telephone interconnect calls are never followed.
static inline int32_t sdrkit_motorola_type2_get_follow_private(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_private", value);
}
static inline int32_t sdrkit_motorola_type2_set_follow_private(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_private", value);
}

// The setting "follow_groups" (Follow groups) of the stack "motorola_type2". Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused.
static inline int32_t sdrkit_motorola_type2_get_follow_groups(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "follow_groups", value);
}
static inline int32_t sdrkit_motorola_type2_set_follow_groups(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "follow_groups", value);
}

// The setting "follow_max_s" (Longest follow) of the stack "motorola_type2". A length of time in seconds. A call followed longer than this ends (a stuck carrier).
static inline int32_t sdrkit_motorola_type2_get_follow_max_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "follow_max_s", value);
}
static inline int32_t sdrkit_motorola_type2_set_follow_max_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "follow_max_s", value);
}

// The setting "gen_deviation_hz" (Test signal deviation) of the test signal generator of the stack "motorola_type2". A frequency in Hz.
static inline int32_t sdrkit_motorola_type2_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_motorola_type2_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// The setting "gen_invert" (Test signal inverted) of the test signal generator of the stack "motorola_type2".
static inline int32_t sdrkit_motorola_type2_gen_get_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert", value);
}
static inline int32_t sdrkit_motorola_type2_gen_set_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert", value);
}

// The setting "gen_word_errors" (Test signal bad words) of the test signal generator of the stack "motorola_type2".
static inline int32_t sdrkit_motorola_type2_gen_get_word_errors(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_word_errors", value);
}
static inline int32_t sdrkit_motorola_type2_gen_set_word_errors(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_word_errors", value);
}

// The setting "gen_idle_inside" (Test signal idle inside a message) of the test signal generator of the stack "motorola_type2".
static inline int32_t sdrkit_motorola_type2_gen_get_idle_inside(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_idle_inside", value);
}
static inline int32_t sdrkit_motorola_type2_gen_set_idle_inside(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_idle_inside", value);
}

// Fills `rows` with the changed rows of the table "blocks" of the stack "opensky" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_opensky_blocks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_opensky_block_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.OpenskyBlockRow", "blocks", rows, sizeof(sdrkit_opensky_block_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "opensky.verdict"
// (OpenSky verdict). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_opensky_verdict_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_opensky_verdict_event *event) {
  return sdrkit_typed_event(handle, n, "opensky.verdict", "trunk.OpenskyVerdictEvent", event, sizeof(sdrkit_opensky_verdict_event));
}

// Runs the command "clear" (Clear) of the stack "opensky" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_opensky_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "opensky" are the sdrkit_opensky_gen_*() functions
// after the settings of its receiver.

// The setting "gen_shape" (Test signal shape) of the test signal generator of the stack "opensky". A value of `sdrkit_opensky_gen_shape`.
static inline int32_t sdrkit_opensky_gen_get_shape(SDRKIT_HANDLE handle, sdrkit_opensky_gen_shape *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_shape", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_opensky_gen_shape)index;
  return code;
}
static inline int32_t sdrkit_opensky_gen_set_shape(SDRKIT_HANDLE handle, sdrkit_opensky_gen_shape value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_shape", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "pages" of the stack "gsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `call` names a row.
// Addressee: `code`. Text: `text`. Time: `time_s`.
static inline int32_t sdrkit_gsc_pages_rows(SDRKIT_HANDLE handle,
                                           sdrkit_gsc_page_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.GscPageRow", "pages", rows, sizeof(sdrkit_gsc_page_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "gsc.page"
// (GSC page). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_gsc_page_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_gsc_page_event *event) {
  return sdrkit_typed_event(handle, n, "gsc.page", "trunk.GscPageEvent", event, sizeof(sdrkit_gsc_page_event));
}

// Runs the command "clear" (Clear) of the stack "gsc" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_gsc_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gsc" are the sdrkit_gsc_gen_*() functions
// after the settings of its receiver.

// The setting "gen_pages" (Test signal pages) of the test signal generator of the stack "gsc". A value of `sdrkit_gsc_gen_pages`.
static inline int32_t sdrkit_gsc_gen_get_pages(SDRKIT_HANDLE handle, sdrkit_gsc_gen_pages *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_pages", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gsc_gen_pages)index;
  return code;
}
static inline int32_t sdrkit_gsc_gen_set_pages(SDRKIT_HANDLE handle, sdrkit_gsc_gen_pages value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_pages", (int32_t)value);
}

// The setting "gen_batch" (Test signal batch) of the test signal generator of the stack "gsc".
static inline int32_t sdrkit_gsc_gen_get_batch(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_batch", value);
}
static inline int32_t sdrkit_gsc_gen_set_batch(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_batch", value);
}

// The setting "gen_no_preamble" (Test signal without preamble) of the test signal generator of the stack "gsc".
static inline int32_t sdrkit_gsc_gen_get_no_preamble(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_no_preamble", value);
}
static inline int32_t sdrkit_gsc_gen_set_no_preamble(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_no_preamble", value);
}

// The setting "gen_deviation_hz" (Test signal deviation) of the test signal generator of the stack "gsc". Unit: Hz.
static inline int32_t sdrkit_gsc_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_gsc_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "scada" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `from` names a row.
// Time: `wall_clock_utc`.
static inline int32_t sdrkit_scada_messages_rows(SDRKIT_HANDLE handle,
                                                sdrkit_scada_message_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.ScadaMessageRow", "messages", rows, sizeof(sdrkit_scada_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "scada" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_scada_devices_rows(SDRKIT_HANDLE handle,
                                               sdrkit_scada_device_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "trunk.ScadaDeviceRow", "devices", rows, sizeof(sdrkit_scada_device_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "scada.message"
// (SCADA message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_scada_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_scada_message_event *event) {
  return sdrkit_typed_event(handle, n, "scada.message", "trunk.ScadaMessageEvent", event, sizeof(sdrkit_scada_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "scada.device"
// (SCADA device). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_scada_device_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_scada_device_event *event) {
  return sdrkit_typed_event(handle, n, "scada.device", "trunk.ScadaDeviceEvent", event, sizeof(sdrkit_scada_device_event));
}

// Runs the command "clear" (Clear) of the stack "scada" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_scada_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "export_csv" (Export the messages as CSV) of the stack "scada" on the session of `handle`. Replies with the messages as CSV text: id, time, wall clock, link, protocol, from, to, function, summary, CRC, direction, pair, response time, encrypted, raw hex.
static inline int32_t sdrkit_scada_export_csv(SDRKIT_HANDLE handle, sdrkit_scada_csv_reply *reply) {
  return sdrkit_typed_command(handle, "export_csv", NULL, NULL, "trunk.ScadaCsvReply", reply, sizeof(sdrkit_scada_csv_reply));
}

// The settings of the test signal generator of the stack "scada" are the sdrkit_scada_gen_*() functions
// after the settings of its receiver.

// The setting "gen_link" (Test signal link) of the test signal generator of the stack "scada". A value of `sdrkit_scada_gen_link`.
static inline int32_t sdrkit_scada_gen_get_link(SDRKIT_HANDLE handle, sdrkit_scada_gen_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_scada_gen_link)index;
  return code;
}
static inline int32_t sdrkit_scada_gen_set_link(SDRKIT_HANDLE handle, sdrkit_scada_gen_link value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_link", (int32_t)value);
}

// The setting "gen_framing" (Test signal framing) of the test signal generator of the stack "scada". A value of `sdrkit_scada_gen_framing`.
static inline int32_t sdrkit_scada_gen_get_framing(SDRKIT_HANDLE handle, sdrkit_scada_gen_framing *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_framing", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_scada_gen_framing)index;
  return code;
}
static inline int32_t sdrkit_scada_gen_set_framing(SDRKIT_HANDLE handle, sdrkit_scada_gen_framing value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_framing", (int32_t)value);
}

// The setting "gen_traffic" (Test signal traffic) of the test signal generator of the stack "scada". A value of `sdrkit_scada_gen_traffic`.
static inline int32_t sdrkit_scada_gen_get_traffic(SDRKIT_HANDLE handle, sdrkit_scada_gen_traffic *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_traffic", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_scada_gen_traffic)index;
  return code;
}
static inline int32_t sdrkit_scada_gen_set_traffic(SDRKIT_HANDLE handle, sdrkit_scada_gen_traffic value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_traffic", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mpt1327_fields_read(SDRKIT_HANDLE handle, sdrkit_mpt1327_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.Mpt1327Fields", fields, sizeof(sdrkit_mpt1327_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ltr_fields_read(SDRKIT_HANDLE handle, sdrkit_ltr_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.LtrFields", fields, sizeof(sdrkit_ltr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_edacs_fields_read(SDRKIT_HANDLE handle, sdrkit_edacs_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.EdacsFields", fields, sizeof(sdrkit_edacs_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tones_fields_read(SDRKIT_HANDLE handle, sdrkit_tones_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.TonesFields", fields, sizeof(sdrkit_tones_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_pager_fields_read(SDRKIT_HANDLE handle, sdrkit_pager_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.PagerFields", fields, sizeof(sdrkit_pager_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_pager_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_pager_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.PagerTxFields", fields, sizeof(sdrkit_pager_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ermes_fields_read(SDRKIT_HANDLE handle, sdrkit_ermes_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.ErmesFields", fields, sizeof(sdrkit_ermes_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_trunk_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_trunk_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.DecoderFields", fields, sizeof(sdrkit_trunk_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_multitone_fields_read(SDRKIT_HANDLE handle, sdrkit_multitone_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.MultitoneFields", fields, sizeof(sdrkit_multitone_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_motorola_type2_fields_read(SDRKIT_HANDLE handle, sdrkit_motorola_type2_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.MotorolaType2Fields", fields, sizeof(sdrkit_motorola_type2_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_opensky_fields_read(SDRKIT_HANDLE handle, sdrkit_opensky_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.OpenskyFields", fields, sizeof(sdrkit_opensky_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gsc_fields_read(SDRKIT_HANDLE handle, sdrkit_gsc_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.GscFields", fields, sizeof(sdrkit_gsc_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_scada_fields_read(SDRKIT_HANDLE handle, sdrkit_scada_fields *fields) {
  return sdrkit_typed_fields(handle, "trunk.ScadaFields", fields, sizeof(sdrkit_scada_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ermes" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ermes_dropped_read(SDRKIT_HANDLE handle, sdrkit_ermes_dropped *dropped) {
  return sdrkit_typed_fields(handle, "trunk.ermes.dropped", dropped, sizeof(sdrkit_ermes_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "gsc" in the
// snapshot of `handle`.
static inline int32_t sdrkit_gsc_dropped_read(SDRKIT_HANDLE handle, sdrkit_gsc_dropped *dropped) {
  return sdrkit_typed_fields(handle, "trunk.gsc.dropped", dropped, sizeof(sdrkit_gsc_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "scada" in the
// snapshot of `handle`.
static inline int32_t sdrkit_scada_dropped_read(SDRKIT_HANDLE handle, sdrkit_scada_dropped *dropped) {
  return sdrkit_typed_fields(handle, "trunk.scada.dropped", dropped, sizeof(sdrkit_scada_dropped));
}

#endif
