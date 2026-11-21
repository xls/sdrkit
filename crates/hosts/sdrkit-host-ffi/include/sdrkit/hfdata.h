/*
 * sdrkit: the typed interface of the stack family "hfdata", version 1.4. MIT licence.
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

#ifndef SDRKIT_HFDATA_H
#define SDRKIT_HFDATA_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_HFDATA_VERSION_MAJOR 1
#define SDRKIT_HFDATA_VERSION_MINOR 4

// The stack "crowd36": its id, and the keys of its event types.
#define SDRKIT_STACK_CROWD36 "crowd36"
#define SDRKIT_EVENT_TEXT "text"
#define SDRKIT_EVENT_TONES_FOUND "tones.found"
#define SDRKIT_EVENT_TONES_LOST "tones.lost"

// The stack "xpa": its id, and the keys of its event types.
#define SDRKIT_STACK_XPA "xpa"

// The stack "fsk200": its id, and the keys of its event types.
#define SDRKIT_STACK_FSK200 "fsk200"
#define SDRKIT_EVENT_F06_BLOCK "f06.block"

// The stack "gwfsk": its id, and the keys of its event types.
#define SDRKIT_STACK_GWFSK "gwfsk"
#define SDRKIT_EVENT_GW_PACKET "gw.packet"

// The stack "fsk_modems": its id, and the keys of its event types.
#define SDRKIT_STACK_FSK_MODEMS "fsk_modems"
#define SDRKIT_EVENT_MODEM_MESSAGE "modem.message"

// The stack "uvb76": its id, and the keys of its event types.
#define SDRKIT_STACK_UVB76 "uvb76"
#define SDRKIT_EVENT_UVB76_EVENT "uvb76.event"

// The stack "vlf_msk": its id, and the keys of its event types.
#define SDRKIT_STACK_VLF_MSK "vlf_msk"
#define SDRKIT_EVENT_VLF_TRANSMITTER "vlf.transmitter"

// The stack "efr": its id, and the keys of its event types.
#define SDRKIT_STACK_EFR "efr"
#define SDRKIT_EVENT_FSK_LOCK "fsk.lock"
#define SDRKIT_EVENT_FSK_LOST "fsk.lost"
#define SDRKIT_EVENT_EFR_TELEGRAM "efr.telegram"

// The stack "gtor": its id, and the keys of its event types.
#define SDRKIT_STACK_GTOR "gtor"
#define SDRKIT_EVENT_GTOR_FRAME "gtor.frame"

// The stack "arqe": its id, and the keys of its event types.
#define SDRKIT_STACK_ARQE "arqe"

// The stack "autospec": its id, and the keys of its event types.
#define SDRKIT_STACK_AUTOSPEC "autospec"

// The stack "piccolo": its id, and the keys of its event types.
#define SDRKIT_STACK_PICCOLO "piccolo"
#define SDRKIT_EVENT_PICCOLO_PAIRS "piccolo.pairs"

// The stack "coquelet": its id, and the keys of its event types.
#define SDRKIT_STACK_COQUELET "coquelet"
#define SDRKIT_EVENT_COQUELET_PAIRS "coquelet.pairs"

// The stack "ale3g": its id, and the keys of its event types.
#define SDRKIT_STACK_ALE3G "ale3g"
#define SDRKIT_EVENT_ALE3G_PDU "ale3g.pdu"
#define SDRKIT_EVENT_ALE3G_DATAGRAM "ale3g.datagram"

// The stack "s4481": its id, and the keys of its event types.
#define SDRKIT_STACK_S4481 "s4481"
#define SDRKIT_EVENT_S4481_EVENT "s4481.event"

// The stack "link11": its id, and the keys of its event types.
#define SDRKIT_STACK_LINK11 "link11"
#define SDRKIT_EVENT_LINK11_EVENT "link11.event"
#define SDRKIT_EVENT_LINK11_MESSAGE "link11.message"

// The stack "cis_hf": its id, and the keys of its event types.
#define SDRKIT_STACK_CIS_HF "cis_hf"
#define SDRKIT_EVENT_CIS_MESSAGE "cis.message"
#define SDRKIT_EVENT_CIS_IDENT "cis.ident"

// The stack "hfid": its id, and the keys of its event types.
#define SDRKIT_STACK_HFID "hfid"

// The stack "ale": its id, and the keys of its event types.
#define SDRKIT_STACK_ALE "ale"
#define SDRKIT_EVENT_ALE_CALL "ale.call"
#define SDRKIT_EVENT_ALE_TX "ale.tx"
#define SDRKIT_EVENT_ALE_LINK "ale.link"

// The stack "hfradar": its id, and the keys of its event types.
#define SDRKIT_STACK_HFRADAR "hfradar"
#define SDRKIT_EVENT_HFRADAR_RADAR_HEARD "hfradar.radar_heard"

// The stack "pactor2": its id, and the keys of its event types.
#define SDRKIT_STACK_PACTOR2 "pactor2"
#define SDRKIT_EVENT_PACTORHF_ANALYSIS "pactorhf.analysis"

// The stack "pactor3": its id, and the keys of its event types.
#define SDRKIT_STACK_PACTOR3 "pactor3"

// The stack "pactor4": its id, and the keys of its event types.
#define SDRKIT_STACK_PACTOR4 "pactor4"

// The stack "pactor1": its id, and the keys of its event types.
#define SDRKIT_STACK_PACTOR1 "pactor1"
#define SDRKIT_EVENT_PACTOR_FRAME "pactor.frame"

// The stack "hfbeacons": its id, and the keys of its event types.
#define SDRKIT_STACK_HFBEACONS "hfbeacons"
#define SDRKIT_EVENT_HFB_DETECTION "hfb.detection"
#define SDRKIT_EVENT_HFB_TX "hfb.tx"

// The stack "wavebuoy": its id, and the keys of its event types.
#define SDRKIT_STACK_WAVEBUOY "wavebuoy"
#define SDRKIT_EVENT_WAVEBUOY_VECTOR "wavebuoy.vector"

// The stack "ms110": its id, and the keys of its event types.
#define SDRKIT_STACK_MS110 "ms110"
#define SDRKIT_EVENT_MS110_EVENT "ms110.event"
#define SDRKIT_EVENT_MS110_MESSAGE "ms110.message"

// The stack "s4285": its id, and the keys of its event types.
#define SDRKIT_STACK_S4285 "s4285"
#define SDRKIT_EVENT_S4285_EVENT "s4285.event"
#define SDRKIT_EVENT_S4285_MESSAGE "s4285.message"

// The stack "s4539": its id, and the keys of its event types.
#define SDRKIT_STACK_S4539 "s4539"
#define SDRKIT_EVENT_S4539_EVENT "s4539.event"
#define SDRKIT_EVENT_S4539_MESSAGE "s4539.message"

// The stack "rpr": its id, and the keys of its event types.
#define SDRKIT_STACK_RPR "rpr"
#define SDRKIT_EVENT_RPR_BURST "rpr.burst"

// The options of the setting "sideband" of the stack "crowd36".
typedef enum sdrkit_crowd36_sideband {
  // Upper
  SDRKIT_CROWD36_SIDEBAND_UPPER = 0,
  // Lower
  SDRKIT_CROWD36_SIDEBAND_LOWER = 1,
} sdrkit_crowd36_sideband;

// The options of the setting "speed" of the stack "crowd36".
typedef enum sdrkit_crowd36_speed {
  // 40 Bd
  SDRKIT_CROWD36_SPEED_40_BD = 0,
  // 20 Bd
  SDRKIT_CROWD36_SPEED_20_BD = 1,
  // 10 Bd
  SDRKIT_CROWD36_SPEED_10_BD = 2,
} sdrkit_crowd36_speed;

// The options of the setting "submode" of the stack "xpa".
typedef enum sdrkit_xpa_submode {
  // XPA
  SDRKIT_XPA_SUBMODE_XPA = 0,
  // XPA2
  SDRKIT_XPA_SUBMODE_XPA2 = 1,
} sdrkit_xpa_submode;

// The options of the setting "speed" of the stack "xpa".
typedef enum sdrkit_xpa_speed {
  // Find it
  SDRKIT_XPA_SPEED_AUTO = 0,
  // 10 Bd
  SDRKIT_XPA_SPEED_10_BD = 1,
  // 20 Bd
  SDRKIT_XPA_SPEED_20_BD = 2,
} sdrkit_xpa_speed;

// The options of the setting "sideband" of the stack "xpa".
typedef enum sdrkit_xpa_sideband {
  // Find it
  SDRKIT_XPA_SIDEBAND_AUTO = 0,
  // Upper
  SDRKIT_XPA_SIDEBAND_UPPER = 1,
  // Lower
  SDRKIT_XPA_SIDEBAND_LOWER = 2,
} sdrkit_xpa_sideband;

// The options of the setting "numbering" of the stack "xpa".
typedef enum sdrkit_xpa_numbering {
  // Find it
  SDRKIT_XPA_NUMBERING_AUTO = 0,
  // -3 tones
  SDRKIT_XPA_NUMBERING_MINUS3 = 1,
  // -2 tones
  SDRKIT_XPA_NUMBERING_MINUS2 = 2,
  // -1 tone
  SDRKIT_XPA_NUMBERING_MINUS1 = 3,
  // As found
  SDRKIT_XPA_NUMBERING_NONE = 4,
  // +1 tone
  SDRKIT_XPA_NUMBERING_PLUS1 = 5,
  // +2 tones
  SDRKIT_XPA_NUMBERING_PLUS2 = 6,
  // +3 tones
  SDRKIT_XPA_NUMBERING_PLUS3 = 7,
} sdrkit_xpa_numbering;

// The options of the setting "gen_baud" of the stack "xpa".
typedef enum sdrkit_xpa_gen_baud {
  // 10 Bd
  SDRKIT_XPA_GEN_BAUD_10_BD = 0,
  // 20 Bd
  SDRKIT_XPA_GEN_BAUD_20_BD = 1,
} sdrkit_xpa_gen_baud;

// The options of the setting "submode" of the stack "fsk200".
typedef enum sdrkit_fsk200_submode {
  // FSK200/500 (F01)
  SDRKIT_FSK200_SUBMODE_F01 = 0,
  // FSK200/1000 (F06)
  SDRKIT_FSK200_SUBMODE_F06 = 1,
} sdrkit_fsk200_submode;

// The options of the setting "polarity" of the stack "fsk200".
typedef enum sdrkit_fsk200_polarity {
  // Find it
  SDRKIT_FSK200_POLARITY_AUTO = 0,
  // Low tone is mark or 0
  SDRKIT_FSK200_POLARITY_NORMAL = 1,
  // High tone is mark or 0
  SDRKIT_FSK200_POLARITY_REVERSED = 2,
} sdrkit_fsk200_polarity;

// The options of the setting "text_case" of the stack "fsk200".
typedef enum sdrkit_fsk200_text_case {
  // Figures
  SDRKIT_FSK200_TEXT_CASE_FIGURES = 0,
  // Follow the shifts
  SDRKIT_FSK200_TEXT_CASE_ITA2 = 1,
} sdrkit_fsk200_text_case;

// The options of the setting "gen_kind" of the stack "fsk200".
typedef enum sdrkit_fsk200_gen_kind {
  // F06 figure blocks
  SDRKIT_FSK200_GEN_KIND_F06 = 0,
  // ASCII blocks
  SDRKIT_FSK200_GEN_KIND_ASCII = 1,
  // F06a file
  SDRKIT_FSK200_GEN_KIND_F06A = 2,
} sdrkit_fsk200_gen_kind;

// The options of the setting "polarity" of the stack "gwfsk".
typedef enum sdrkit_gwfsk_polarity {
  // Find it
  SDRKIT_GWFSK_POLARITY_AUTO = 0,
  // Low tone is 1
  SDRKIT_GWFSK_POLARITY_NORMAL = 1,
  // High tone is 1
  SDRKIT_GWFSK_POLARITY_REVERSED = 2,
} sdrkit_gwfsk_polarity;

// The options of the setting "submode" of the stack "fsk_modems".
typedef enum sdrkit_fsk_modems_submode {
  // Find it
  SDRKIT_FSK_MODEMS_SUBMODE_AUTO = 0,
  // Bell 103 originate
  SDRKIT_FSK_MODEMS_SUBMODE_BELL103_ORIG = 1,
  // Bell 103 answer
  SDRKIT_FSK_MODEMS_SUBMODE_BELL103_ANS = 2,
  // V.21 channel 1
  SDRKIT_FSK_MODEMS_SUBMODE_V21_CH1 = 3,
  // V.21 channel 2
  SDRKIT_FSK_MODEMS_SUBMODE_V21_CH2 = 4,
  // V.23 mode 2
  SDRKIT_FSK_MODEMS_SUBMODE_V23_MODE2 = 5,
  // Bell 202
  SDRKIT_FSK_MODEMS_SUBMODE_BELL202 = 6,
} sdrkit_fsk_modems_submode;

// The options of the setting "sideband" of the stack "fsk_modems".
typedef enum sdrkit_fsk_modems_sideband {
  // The stronger
  SDRKIT_FSK_MODEMS_SIDEBAND_AUTO = 0,
  // Upper
  SDRKIT_FSK_MODEMS_SIDEBAND_UPPER = 1,
  // Lower
  SDRKIT_FSK_MODEMS_SIDEBAND_LOWER = 2,
} sdrkit_fsk_modems_sideband;

// The options of the setting "gen_standard" of the stack "fsk_modems".
typedef enum sdrkit_fsk_modems_gen_standard {
  // Bell 103 originate
  SDRKIT_FSK_MODEMS_GEN_STANDARD_BELL103_ORIG = 0,
  // Bell 103 answer
  SDRKIT_FSK_MODEMS_GEN_STANDARD_BELL103_ANS = 1,
  // V.21 channel 1
  SDRKIT_FSK_MODEMS_GEN_STANDARD_V21_CH1 = 2,
  // V.21 channel 2
  SDRKIT_FSK_MODEMS_GEN_STANDARD_V21_CH2 = 3,
  // V.23 mode 2
  SDRKIT_FSK_MODEMS_GEN_STANDARD_V23_MODE2 = 4,
  // Bell 202
  SDRKIT_FSK_MODEMS_GEN_STANDARD_BELL202 = 5,
} sdrkit_fsk_modems_gen_standard;

// The options of the setting "polarity" of the stack "efr".
typedef enum sdrkit_efr_polarity {
  // Automatic
  SDRKIT_EFR_POLARITY_AUTO = 0,
  // Normal
  SDRKIT_EFR_POLARITY_NORMAL = 1,
  // Inverted
  SDRKIT_EFR_POLARITY_INVERTED = 2,
} sdrkit_efr_polarity;

// The options of the setting "baud" of the stack "gtor".
typedef enum sdrkit_gtor_baud {
  // Automatic
  SDRKIT_GTOR_BAUD_AUTO = 0,
  // 100 Bd
  SDRKIT_GTOR_BAUD_100_BD = 1,
  // 200 Bd
  SDRKIT_GTOR_BAUD_200_BD = 2,
  // 300 Bd
  SDRKIT_GTOR_BAUD_300_BD = 3,
} sdrkit_gtor_baud;

// The options of the setting "gen_baud" of the stack "gtor".
typedef enum sdrkit_gtor_gen_baud {
  // Automatic
  SDRKIT_GTOR_GEN_BAUD_AUTO = 0,
  // 100 Bd
  SDRKIT_GTOR_GEN_BAUD_100_BD = 1,
  // 200 Bd
  SDRKIT_GTOR_GEN_BAUD_200_BD = 2,
  // 300 Bd
  SDRKIT_GTOR_GEN_BAUD_300_BD = 3,
} sdrkit_gtor_gen_baud;

// The options of the setting "submode" of the stack "arqe".
typedef enum sdrkit_arqe_submode {
  // ARQ-E
  SDRKIT_ARQE_SUBMODE_ARQE = 0,
  // ARQ-N
  SDRKIT_ARQE_SUBMODE_ARQN = 1,
} sdrkit_arqe_submode;

// The options of the setting "polarity" of the stack "arqe".
typedef enum sdrkit_arqe_polarity {
  // Automatic
  SDRKIT_ARQE_POLARITY_AUTO = 0,
  // Normal
  SDRKIT_ARQE_POLARITY_NORMAL = 1,
  // Inverted
  SDRKIT_ARQE_POLARITY_INVERTED = 2,
} sdrkit_arqe_polarity;

// The options of the setting "cycle" of the stack "arqe".
typedef enum sdrkit_arqe_cycle {
  // Automatic
  SDRKIT_ARQE_CYCLE_AUTO = 0,
  // No inversion
  SDRKIT_ARQE_CYCLE_NONE = 1,
  // Every 4th word
  SDRKIT_ARQE_CYCLE_EVERY_4TH_WORD = 2,
  // Every 8th word
  SDRKIT_ARQE_CYCLE_EVERY_8TH_WORD = 3,
} sdrkit_arqe_cycle;

// The options of the setting "alphabet" of the stack "arqe".
typedef enum sdrkit_arqe_alphabet {
  // US
  SDRKIT_ARQE_ALPHABET_US = 0,
  // ITU
  SDRKIT_ARQE_ALPHABET_ITU = 1,
} sdrkit_arqe_alphabet;

// The options of the setting "word_code" of the stack "arqe".
typedef enum sdrkit_arqe_word_code {
  // Identification and parity
  SDRKIT_ARQE_WORD_CODE_PARITY = 0,
  // ITU-R F.342 constant ratio
  SDRKIT_ARQE_WORD_CODE_F342 = 1,
} sdrkit_arqe_word_code;

// The options of the setting "channels" of the stack "arqe".
typedef enum sdrkit_arqe_channels {
  // Automatic
  SDRKIT_ARQE_CHANNELS_AUTO = 0,
  // One channel
  SDRKIT_ARQE_CHANNELS_ONE_CHANNEL = 1,
  // Two channels
  SDRKIT_ARQE_CHANNELS_TWO_CHANNELS = 2,
  // Four channels
  SDRKIT_ARQE_CHANNELS_FOUR_CHANNELS = 3,
} sdrkit_arqe_channels;

// The options of the setting "channel" of the stack "arqe".
typedef enum sdrkit_arqe_channel {
  // A
  SDRKIT_ARQE_CHANNEL_A = 0,
  // B
  SDRKIT_ARQE_CHANNEL_B = 1,
  // C
  SDRKIT_ARQE_CHANNEL_C = 2,
  // D
  SDRKIT_ARQE_CHANNEL_D = 3,
} sdrkit_arqe_channel;

// The options of the setting "gen_cycle" of the stack "arqe".
typedef enum sdrkit_arqe_gen_cycle {
  // Automatic
  SDRKIT_ARQE_GEN_CYCLE_AUTO = 0,
  // No inversion
  SDRKIT_ARQE_GEN_CYCLE_NONE = 1,
  // Every 4th word
  SDRKIT_ARQE_GEN_CYCLE_EVERY_4TH_WORD = 2,
  // Every 8th word
  SDRKIT_ARQE_GEN_CYCLE_EVERY_8TH_WORD = 3,
} sdrkit_arqe_gen_cycle;

// The options of the setting "gen_channels" of the stack "arqe".
typedef enum sdrkit_arqe_gen_channels {
  // Automatic
  SDRKIT_ARQE_GEN_CHANNELS_AUTO = 0,
  // One channel
  SDRKIT_ARQE_GEN_CHANNELS_ONE_CHANNEL = 1,
  // Two channels
  SDRKIT_ARQE_GEN_CHANNELS_TWO_CHANNELS = 2,
  // Four channels
  SDRKIT_ARQE_GEN_CHANNELS_FOUR_CHANNELS = 3,
} sdrkit_arqe_gen_channels;

// The options of the setting "gen_word_code" of the stack "arqe".
typedef enum sdrkit_arqe_gen_word_code {
  // Identification and parity
  SDRKIT_ARQE_GEN_WORD_CODE_PARITY = 0,
  // ITU-R F.342 constant ratio
  SDRKIT_ARQE_GEN_WORD_CODE_F342 = 1,
} sdrkit_arqe_gen_word_code;

// The options of the setting "submode" of the stack "autospec".
typedef enum sdrkit_autospec_submode {
  // AUTOSPEC
  SDRKIT_AUTOSPEC_SUBMODE_AUTOSPEC = 0,
  // SPREAD
  SDRKIT_AUTOSPEC_SUBMODE_SPREAD = 1,
} sdrkit_autospec_submode;

// The options of the setting "polarity" of the stack "autospec".
typedef enum sdrkit_autospec_polarity {
  // Automatic
  SDRKIT_AUTOSPEC_POLARITY_AUTO = 0,
  // Normal
  SDRKIT_AUTOSPEC_POLARITY_NORMAL = 1,
  // Inverted
  SDRKIT_AUTOSPEC_POLARITY_INVERTED = 2,
} sdrkit_autospec_polarity;

// The options of the setting "depth" of the stack "autospec".
typedef enum sdrkit_autospec_depth {
  // Automatic
  SDRKIT_AUTOSPEC_DEPTH_AUTO = 0,
  // 11
  SDRKIT_AUTOSPEC_DEPTH_11 = 1,
  // 21
  SDRKIT_AUTOSPEC_DEPTH_21 = 2,
  // 51
  SDRKIT_AUTOSPEC_DEPTH_51 = 3,
} sdrkit_autospec_depth;

// The options of the setting "alphabet" of the stack "autospec".
typedef enum sdrkit_autospec_alphabet {
  // US
  SDRKIT_AUTOSPEC_ALPHABET_US = 0,
  // ITU
  SDRKIT_AUTOSPEC_ALPHABET_ITU = 1,
} sdrkit_autospec_alphabet;

// The options of the setting "gen_depth" of the stack "autospec".
typedef enum sdrkit_autospec_gen_depth {
  // Automatic
  SDRKIT_AUTOSPEC_GEN_DEPTH_AUTO = 0,
  // 11
  SDRKIT_AUTOSPEC_GEN_DEPTH_11 = 1,
  // 21
  SDRKIT_AUTOSPEC_GEN_DEPTH_21 = 2,
  // 51
  SDRKIT_AUTOSPEC_GEN_DEPTH_51 = 3,
} sdrkit_autospec_gen_depth;

// The options of the setting "alphabet" of the stack "piccolo".
typedef enum sdrkit_piccolo_alphabet {
  // US
  SDRKIT_PICCOLO_ALPHABET_US = 0,
  // ITU
  SDRKIT_PICCOLO_ALPHABET_ITU = 1,
} sdrkit_piccolo_alphabet;

// The options of the setting "tone_ms" of the stack "coquelet".
typedef enum sdrkit_coquelet_tone_ms {
  // Automatic
  SDRKIT_COQUELET_TONE_MS_AUTO = 0,
  // 37.5 ms
  SDRKIT_COQUELET_TONE_MS_37_5 = 1,
  // 50 ms
  SDRKIT_COQUELET_TONE_MS_50_MS = 2,
  // 75 ms
  SDRKIT_COQUELET_TONE_MS_75_MS = 3,
} sdrkit_coquelet_tone_ms;

// The options of the setting "alphabet" of the stack "coquelet".
typedef enum sdrkit_coquelet_alphabet {
  // US
  SDRKIT_COQUELET_ALPHABET_US = 0,
  // ITU
  SDRKIT_COQUELET_ALPHABET_ITU = 1,
} sdrkit_coquelet_alphabet;

// The options of the setting "gen_tone_ms" of the stack "coquelet".
typedef enum sdrkit_coquelet_gen_tone_ms {
  // Automatic
  SDRKIT_COQUELET_GEN_TONE_MS_AUTO = 0,
  // 37.5 ms
  SDRKIT_COQUELET_GEN_TONE_MS_37_5 = 1,
  // 50 ms
  SDRKIT_COQUELET_GEN_TONE_MS_50_MS = 2,
  // 75 ms
  SDRKIT_COQUELET_GEN_TONE_MS_75_MS = 3,
} sdrkit_coquelet_gen_tone_ms;

// The options of the setting "gen_set" of the stack "ale3g".
typedef enum sdrkit_ale3g_gen_set {
  // A full exchange
  SDRKIT_ALE3G_GEN_SET_EXCHANGE = 0,
  // Link setup (BW0) only
  SDRKIT_ALE3G_GEN_SET_BW0 = 1,
} sdrkit_ale3g_gen_set;

// The options of the setting "channel_mode" of the stack "s4481".
typedef enum sdrkit_s4481_channel_mode {
  // Automatic
  SDRKIT_S4481_CHANNEL_MODE_AUTO = 0,
  // Single (+-425 Hz)
  SDRKIT_S4481_CHANNEL_MODE_SINGLE = 1,
  // Multi (+-42.5 Hz)
  SDRKIT_S4481_CHANNEL_MODE_MULTI = 2,
} sdrkit_s4481_channel_mode;

// The options of the setting "baud" of the stack "s4481".
typedef enum sdrkit_s4481_baud {
  // Automatic
  SDRKIT_S4481_BAUD_AUTO = 0,
  // 45.45 Bd
  SDRKIT_S4481_BAUD_45_45 = 1,
  // 50 Bd
  SDRKIT_S4481_BAUD_50_BD = 2,
  // 75 Bd
  SDRKIT_S4481_BAUD_75_BD = 3,
  // 100 Bd
  SDRKIT_S4481_BAUD_100_BD = 4,
} sdrkit_s4481_baud;

// The options of the setting "alphabet" of the stack "s4481".
typedef enum sdrkit_s4481_alphabet {
  // US
  SDRKIT_S4481_ALPHABET_US = 0,
  // ITU
  SDRKIT_S4481_ALPHABET_ITU = 1,
} sdrkit_s4481_alphabet;

// The options of the setting "gen_baud" of the stack "s4481".
typedef enum sdrkit_s4481_gen_baud {
  // Automatic
  SDRKIT_S4481_GEN_BAUD_AUTO = 0,
  // 45.45 Bd
  SDRKIT_S4481_GEN_BAUD_45_45 = 1,
  // 50 Bd
  SDRKIT_S4481_GEN_BAUD_50_BD = 2,
  // 75 Bd
  SDRKIT_S4481_GEN_BAUD_75_BD = 3,
  // 100 Bd
  SDRKIT_S4481_GEN_BAUD_100_BD = 4,
} sdrkit_s4481_gen_baud;

// The options of the setting "frame_rate" of the stack "link11".
typedef enum sdrkit_link11_frame_rate {
  // Automatic
  SDRKIT_LINK11_FRAME_RATE_AUTO = 0,
  // Fast (13.33 ms)
  SDRKIT_LINK11_FRAME_RATE_FAST = 1,
  // Slow (22 ms)
  SDRKIT_LINK11_FRAME_RATE_SLOW = 2,
} sdrkit_link11_frame_rate;

// The options of the setting "gen_rate" of the stack "link11".
typedef enum sdrkit_link11_gen_rate {
  // Automatic
  SDRKIT_LINK11_GEN_RATE_AUTO = 0,
  // Fast (13.33 ms)
  SDRKIT_LINK11_GEN_RATE_FAST = 1,
  // Slow (22 ms)
  SDRKIT_LINK11_GEN_RATE_SLOW = 2,
} sdrkit_link11_gen_rate;

// The options of the setting "submode" of the stack "cis_hf".
typedef enum sdrkit_cis_hf_submode {
  // CIS-36-50 / CIS-50-50 (BEE)
  SDRKIT_CIS_HF_SUBMODE_BEE = 0,
  // CIS-14
  SDRKIT_CIS_HF_SUBMODE_CIS14 = 1,
  // CIS-12
  SDRKIT_CIS_HF_SUBMODE_CIS12 = 2,
  // CIS-36 (identification)
  SDRKIT_CIS_HF_SUBMODE_CIS36 = 3,
  // CIS OFDM (identification)
  SDRKIT_CIS_HF_SUBMODE_OFDM_HDR = 4,
  // CIS-128 (identification)
  SDRKIT_CIS_HF_SUBMODE_CIS128 = 5,
  // CIS-3000 (identification)
  SDRKIT_CIS_HF_SUBMODE_CIS3000 = 6,
} sdrkit_cis_hf_submode;

// The options of the setting "gen_variant" of the stack "cis_hf".
typedef enum sdrkit_cis_hf_gen_variant {
  // CIS-36-50
  SDRKIT_CIS_HF_GEN_VARIANT_36_50 = 0,
  // CIS-50-50
  SDRKIT_CIS_HF_GEN_VARIANT_50_100 = 1,
} sdrkit_cis_hf_gen_variant;

// The options of the setting "gen_order" of the stack "cis_hf".
typedef enum sdrkit_cis_hf_gen_order {
  // DBPSK
  SDRKIT_CIS_HF_GEN_ORDER_DBPSK = 0,
  // DQPSK
  SDRKIT_CIS_HF_GEN_ORDER_DQPSK = 1,
} sdrkit_cis_hf_gen_order;

// The options of the setting "gen_rate" of the stack "cis_hf".
typedef enum sdrkit_cis_hf_gen_rate {
  // 10 Bd
  SDRKIT_CIS_HF_GEN_RATE_10_BD = 0,
  // 20 Bd
  SDRKIT_CIS_HF_GEN_RATE_20_BD = 1,
  // 40 Bd
  SDRKIT_CIS_HF_GEN_RATE_40_BD = 2,
} sdrkit_cis_hf_gen_rate;

// The options of the setting "gen_ofdm" of the stack "cis_hf".
typedef enum sdrkit_cis_hf_gen_ofdm {
  // CIS-45 (40 Bd)
  SDRKIT_CIS_HF_GEN_OFDM_CIS45_40 = 0,
  // CIS-45 (33.3 Bd)
  SDRKIT_CIS_HF_GEN_OFDM_CIS45_33 = 1,
  // CIS-60
  SDRKIT_CIS_HF_GEN_OFDM_CIS60 = 2,
  // CIS-93
  SDRKIT_CIS_HF_GEN_OFDM_CIS93 = 3,
} sdrkit_cis_hf_gen_ofdm;

// The options of the setting "gen_calls" of the stack "hfid".
typedef enum sdrkit_hfid_gen_calls {
  // The test calls
  SDRKIT_HFID_GEN_CALLS_TEST_CALLS = 0,
  // The sounding only
  SDRKIT_HFID_GEN_CALLS_SOUNDING = 1,
} sdrkit_hfid_gen_calls;

// The options of the setting "demod" of the stack "ale".
typedef enum sdrkit_ale_demod {
  // Auto
  SDRKIT_ALE_DEMOD_AUTO = 0,
  // USB
  SDRKIT_ALE_DEMOD_USB = 1,
  // LSB
  SDRKIT_ALE_DEMOD_LSB = 2,
  // NFM
  SDRKIT_ALE_DEMOD_NFM = 3,
  // AM
  SDRKIT_ALE_DEMOD_AM = 4,
} sdrkit_ale_demod;

// The options of the setting "sounding" of the stack "ale".
typedef enum sdrkit_ale_sounding {
  // TWS, no answer
  SDRKIT_ALE_SOUNDING_TWS = 0,
  // TIS, answer welcome
  SDRKIT_ALE_SOUNDING_TIS = 1,
} sdrkit_ale_sounding;

// The options of the setting "gen_calls" of the stack "ale".
typedef enum sdrkit_ale_gen_calls {
  // The test calls
  SDRKIT_ALE_GEN_CALLS_TEST_CALLS = 0,
  // The sounding only
  SDRKIT_ALE_GEN_CALLS_SOUNDING = 1,
} sdrkit_ale_gen_calls;

// The options of the setting "gen_signal" of the stack "hfradar".
typedef enum sdrkit_hfradar_gen_signal {
  // Radars
  SDRKIT_HFRADAR_GEN_SIGNAL_RADARS = 0,
  // Nothing
  SDRKIT_HFRADAR_GEN_SIGNAL_NOTHING = 1,
  // Radars with an ID
  SDRKIT_HFRADAR_GEN_SIGNAL_RADARS_WITH_ID = 2,
  // Junk
  SDRKIT_HFRADAR_GEN_SIGNAL_JUNK = 3,
} sdrkit_hfradar_gen_signal;

// The options of the setting "gen_constellation" of the stack "pactor4".
typedef enum sdrkit_pactor4_gen_constellation {
  // 2 points
  SDRKIT_PACTOR4_GEN_CONSTELLATION_2_POINTS = 0,
  // 4 points
  SDRKIT_PACTOR4_GEN_CONSTELLATION_4_POINTS = 1,
  // 8 points
  SDRKIT_PACTOR4_GEN_CONSTELLATION_8_POINTS = 2,
  // 16 points
  SDRKIT_PACTOR4_GEN_CONSTELLATION_16_POINTS = 3,
} sdrkit_pactor4_gen_constellation;

// The options of the setting "demod" of the stack "pactor1".
typedef enum sdrkit_pactor1_demod {
  // Auto
  SDRKIT_PACTOR1_DEMOD_AUTO = 0,
  // USB
  SDRKIT_PACTOR1_DEMOD_USB = 1,
  // LSB
  SDRKIT_PACTOR1_DEMOD_LSB = 2,
  // NFM
  SDRKIT_PACTOR1_DEMOD_NFM = 3,
  // AM
  SDRKIT_PACTOR1_DEMOD_AM = 4,
} sdrkit_pactor1_demod;

// The options of the setting "gen_script" of the stack "pactor1".
typedef enum sdrkit_pactor1_gen_script {
  // ARQ
  SDRKIT_PACTOR1_GEN_SCRIPT_ARQ = 0,
  // Unproto (FEC)
  SDRKIT_PACTOR1_GEN_SCRIPT_FEC = 1,
} sdrkit_pactor1_gen_script;

// The options of the setting "gen_signal" of the stack "hfbeacons".
typedef enum sdrkit_hfbeacons_gen_signal {
  // Beacons
  SDRKIT_HFBEACONS_GEN_SIGNAL_BEACONS = 0,
  // Nothing
  SDRKIT_HFBEACONS_GEN_SIGNAL_NOTHING = 1,
} sdrkit_hfbeacons_gen_signal;

// The options of the setting "format" of the stack "wavebuoy".
typedef enum sdrkit_wavebuoy_format {
  // automatic (from the bandwidth)
  SDRKIT_WAVEBUOY_FORMAT_AUTO = 0,
  // 2FSK
  SDRKIT_WAVEBUOY_FORMAT_2FSK = 1,
  // 4MFSK
  SDRKIT_WAVEBUOY_FORMAT_4MFSK = 2,
} sdrkit_wavebuoy_format;

// The options of the setting "gen_signal" of the stack "wavebuoy".
typedef enum sdrkit_wavebuoy_gen_signal {
  // 2FSK
  SDRKIT_WAVEBUOY_GEN_SIGNAL_2FSK = 0,
  // 4FSK
  SDRKIT_WAVEBUOY_GEN_SIGNAL_4FSK = 1,
  // Nothing
  SDRKIT_WAVEBUOY_GEN_SIGNAL_NOTHING = 2,
  // Carrier
  SDRKIT_WAVEBUOY_GEN_SIGNAL_CARRIER = 3,
} sdrkit_wavebuoy_gen_signal;

// The options of the setting "gen_bps" of the stack "ms110".
typedef enum sdrkit_ms110_gen_bps {
  // 75 bit/s
  SDRKIT_MS110_GEN_BPS_75_BIT_S = 0,
  // 150 bit/s
  SDRKIT_MS110_GEN_BPS_150_BIT_S = 1,
  // 300 bit/s
  SDRKIT_MS110_GEN_BPS_300_BIT_S = 2,
  // 600 bit/s
  SDRKIT_MS110_GEN_BPS_600_BIT_S = 3,
  // 1200 bit/s
  SDRKIT_MS110_GEN_BPS_1200_BIT_S = 4,
  // 2400 bit/s
  SDRKIT_MS110_GEN_BPS_2400_BIT_S = 5,
  // 4800 bit/s
  SDRKIT_MS110_GEN_BPS_4800_BIT_S = 6,
} sdrkit_ms110_gen_bps;

// The options of the setting "gen_interleaver" of the stack "ms110".
typedef enum sdrkit_ms110_gen_interleaver {
  // Short (0.6 s)
  SDRKIT_MS110_GEN_INTERLEAVER_SHORT = 0,
  // Long (4.8 s)
  SDRKIT_MS110_GEN_INTERLEAVER_LONG = 1,
} sdrkit_ms110_gen_interleaver;

// The options of the setting "profile" of the stack "s4285".
typedef enum sdrkit_s4285_profile {
  // Automatic (coded, by the start of message)
  SDRKIT_S4285_PROFILE_AUTO = 0,
  // 2400 bit/s coded, short
  SDRKIT_S4285_PROFILE_2400_SHORT = 1,
  // 1200 bit/s coded, short
  SDRKIT_S4285_PROFILE_1200_SHORT = 2,
  // 600 bit/s coded, short
  SDRKIT_S4285_PROFILE_600_SHORT = 3,
  // 300 bit/s coded, short
  SDRKIT_S4285_PROFILE_300_SHORT = 4,
  // 150 bit/s coded, short
  SDRKIT_S4285_PROFILE_150_SHORT = 5,
  // 75 bit/s coded, short
  SDRKIT_S4285_PROFILE_75_SHORT = 6,
  // 3600 bit/s uncoded
  SDRKIT_S4285_PROFILE_3600_UNCODED = 7,
  // 2400 bit/s uncoded
  SDRKIT_S4285_PROFILE_2400_UNCODED = 8,
  // 1200 bit/s uncoded
  SDRKIT_S4285_PROFILE_1200_UNCODED = 9,
  // 2400 bit/s coded, long
  SDRKIT_S4285_PROFILE_2400_LONG = 10,
  // 1200 bit/s coded, long
  SDRKIT_S4285_PROFILE_1200_LONG = 11,
  // 600 bit/s coded, long
  SDRKIT_S4285_PROFILE_600_LONG = 12,
  // 300 bit/s coded, long
  SDRKIT_S4285_PROFILE_300_LONG = 13,
  // 150 bit/s coded, long
  SDRKIT_S4285_PROFILE_150_LONG = 14,
  // 75 bit/s coded, long
  SDRKIT_S4285_PROFILE_75_LONG = 15,
} sdrkit_s4285_profile;

// The options of the setting "scrambler_start" of the stack "s4285".
typedef enum sdrkit_s4285_scrambler_start {
  // All ones
  SDRKIT_S4285_SCRAMBLER_START_ALL_ONES = 0,
  // The value 1
  SDRKIT_S4285_SCRAMBLER_START_ONE = 1,
} sdrkit_s4285_scrambler_start;

// The options of the setting "gen_profile" of the stack "s4285".
typedef enum sdrkit_s4285_gen_profile {
  // Automatic (coded, by the start of message)
  SDRKIT_S4285_GEN_PROFILE_AUTO = 0,
  // 2400 bit/s coded, short
  SDRKIT_S4285_GEN_PROFILE_2400_SHORT = 1,
  // 1200 bit/s coded, short
  SDRKIT_S4285_GEN_PROFILE_1200_SHORT = 2,
  // 600 bit/s coded, short
  SDRKIT_S4285_GEN_PROFILE_600_SHORT = 3,
  // 300 bit/s coded, short
  SDRKIT_S4285_GEN_PROFILE_300_SHORT = 4,
  // 150 bit/s coded, short
  SDRKIT_S4285_GEN_PROFILE_150_SHORT = 5,
  // 75 bit/s coded, short
  SDRKIT_S4285_GEN_PROFILE_75_SHORT = 6,
  // 3600 bit/s uncoded
  SDRKIT_S4285_GEN_PROFILE_3600_UNCODED = 7,
  // 2400 bit/s uncoded
  SDRKIT_S4285_GEN_PROFILE_2400_UNCODED = 8,
  // 1200 bit/s uncoded
  SDRKIT_S4285_GEN_PROFILE_1200_UNCODED = 9,
  // 2400 bit/s coded, long
  SDRKIT_S4285_GEN_PROFILE_2400_LONG = 10,
  // 1200 bit/s coded, long
  SDRKIT_S4285_GEN_PROFILE_1200_LONG = 11,
  // 600 bit/s coded, long
  SDRKIT_S4285_GEN_PROFILE_600_LONG = 12,
  // 300 bit/s coded, long
  SDRKIT_S4285_GEN_PROFILE_300_LONG = 13,
  // 150 bit/s coded, long
  SDRKIT_S4285_GEN_PROFILE_150_LONG = 14,
  // 75 bit/s coded, long
  SDRKIT_S4285_GEN_PROFILE_75_LONG = 15,
} sdrkit_s4285_gen_profile;

// The options of the setting "gen_bps" of the stack "s4539".
typedef enum sdrkit_s4539_gen_bps {
  // 3200 bit/s (QPSK)
  SDRKIT_S4539_GEN_BPS_3200_BIT_S = 0,
  // 4800 bit/s (8-PSK)
  SDRKIT_S4539_GEN_BPS_4800_BIT_S = 1,
  // 6400 bit/s (16-QAM)
  SDRKIT_S4539_GEN_BPS_6400_BIT_S = 2,
  // 8000 bit/s (32-QAM)
  SDRKIT_S4539_GEN_BPS_8000_BIT_S = 3,
  // 9600 bit/s (64-QAM)
  SDRKIT_S4539_GEN_BPS_9600_BIT_S = 4,
  // 12800 bit/s (64-QAM, uncoded)
  SDRKIT_S4539_GEN_BPS_12800_BIT_S = 5,
} sdrkit_s4539_gen_bps;

// The options of the setting "gen_frames" of the stack "s4539".
typedef enum sdrkit_s4539_gen_frames {
  // Ultra short (1 frame)
  SDRKIT_S4539_GEN_FRAMES_ULTRA_SHORT = 0,
  // Very short (3 frames)
  SDRKIT_S4539_GEN_FRAMES_VERY_SHORT = 1,
  // Short (9 frames)
  SDRKIT_S4539_GEN_FRAMES_SHORT = 2,
  // Medium (18 frames)
  SDRKIT_S4539_GEN_FRAMES_MEDIUM = 3,
  // Long (36 frames)
  SDRKIT_S4539_GEN_FRAMES_LONG = 4,
  // Very long (72 frames)
  SDRKIT_S4539_GEN_FRAMES_VERY_LONG = 5,
} sdrkit_s4539_gen_frames;

// The options of the setting "sideband" of the stack "rpr".
typedef enum sdrkit_rpr_sideband {
  // Upper
  SDRKIT_RPR_SIDEBAND_UPPER = 0,
  // Lower
  SDRKIT_RPR_SIDEBAND_LOWER = 1,
} sdrkit_rpr_sideband;

// The options of the setting "gen_speed" of the stack "rpr".
typedef enum sdrkit_rpr_gen_speed {
  // Level 1 (D-BPSK)
  SDRKIT_RPR_GEN_SPEED_LEVEL1 = 0,
  // Level 2 (D-QPSK)
  SDRKIT_RPR_GEN_SPEED_LEVEL2 = 1,
} sdrkit_rpr_gen_speed;

// The result fields of the CROWD36 stack.
typedef struct sdrkit_crowd36_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The text read, the last characters.
  sdrkit_string text;
  // The search state: 0 searching, 1 tones found, 2 decoding.
  int64_t state;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the tones from their nominal place. Absent while the tones are not found. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR in 3 kHz of the last fit. Absent while the tones are not found. Unit: dB.
  double snr_db;
  // True if `sync_tones` holds a value.
  bool has_sync_tones;
  // The sync tones of the last sync, after the correction. Absent before a sync.
  sdrkit_string sync_tones;
  // Characters read.
  int64_t characters;
  // Text lines read.
  int64_t good;
  // Tones read outside the character tables.
  int64_t bad;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
  // Text lines refused by the check of their form.
  int64_t refused;
} sdrkit_crowd36_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_hfdata_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_hfdata_decoder_fields;

// A row of a number-sender stack: one thing read. The row key is the count of rows before it, as six digits.
typedef struct sdrkit_hfdata_sender_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time it was read. Seconds since 1970 (UTC).
  double time;
  // What it is: start, message start, groups, block sync, break, end, sync, text.
  sdrkit_string kind;
  // The signal and speed it was read as.
  sdrkit_string id;
  // What was read.
  sdrkit_string text;
  // True if `snr` holds a value.
  bool has_snr;
  // The SNR in 3 kHz of the last fit of the tones. Absent without a fit. Unit: dB.
  double snr;
} sdrkit_hfdata_sender_row;

// The result fields of the XPA stack.
typedef struct sdrkit_xpa_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The groups read, the last characters.
  sdrkit_string text;
  // The search state: 0 searching, 1 tones found, 2 decoding.
  int64_t state;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the tones from their nominal place. Absent while the tones are not found. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR in 3 kHz of the last fit. Absent while the tones are not found. Unit: dB.
  double snr_db;
  // True if `speed` holds a value.
  bool has_speed;
  // The speed of the reading that won. Absent before a reading wins. Unit: Bd.
  double speed;
  // True if `sideband` holds a value.
  bool has_sideband;
  // The sideband of the reading that won: "upper" or "lower (tones reversed)".
  sdrkit_string sideband;
  // True if `numbering` holds a value.
  bool has_numbering;
  // The numbering of the reading that won: "as found" or "moved by +N tones".
  sdrkit_string numbering;
  // Groups read.
  int64_t groups;
  // Lines of groups read.
  int64_t good;
  // Tones read without a meaning.
  int64_t bad;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_xpa_fields;

// The result fields of the FSK200 stack. A field that the submode in use does not count has no value.
typedef struct sdrkit_fsk200_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The text and blocks read, the last characters.
  sdrkit_string text;
  // The search state: 0 searching, 1 tones found, 2 decoding.
  int64_t state;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The centre of the tone pair above the dial. Absent while the tones are not found. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR in 3 kHz of the last fit. Absent while the tones are not found. Unit: dB.
  double snr_db;
  // True if `tones` holds a value.
  bool has_tones;
  // The tones from the dial and their sense. Absent while the tones are not found.
  sdrkit_string tones;
  // True if `traffic` holds a value.
  bool has_traffic;
  // The traffic kind of f06: F06 figure groups, ASCII or F06a file.
  sdrkit_string traffic;
  // True if `characters` holds a value.
  bool has_characters;
  // Characters read (f01).
  int64_t characters;
  // True if `framing_errors` holds a value.
  bool has_framing_errors;
  // Characters with a stop unit of space (f01).
  int64_t framing_errors;
  // True if `blocks_good` holds a value.
  bool has_blocks_good;
  // Blocks whose CRC holds, and dividers (f06).
  int64_t blocks_good;
  // True if `blocks_corrected` holds a value.
  bool has_blocks_corrected;
  // Blocks corrected by one bit (f06).
  int64_t blocks_corrected;
  // True if `crc_errors` holds a value.
  bool has_crc_errors;
  // Blocks with a CRC error (f06).
  int64_t crc_errors;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_fsk200_fields;

// The result fields of the GW FSK stack.
typedef struct sdrkit_gwfsk_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The state: 0 nothing heard, 1 bursts heard, 2 bursts read.
  int64_t state;
  // True if `tones` holds a value.
  bool has_tones;
  // The tones of the last burst whose tones were found.
  sdrkit_string tones;
  // True if `last_station` holds a value.
  bool has_last_station;
  // The code of the last shore station heard.
  sdrkit_string last_station;
  // Bursts analysed.
  int64_t bursts;
  // Bursts with tones that were not understood.
  int64_t not_understood;
  // Packets read.
  int64_t packets;
  // Channel markers read.
  int64_t markers;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_gwfsk_fields;

// The result fields of the telephone FSK modem stack.
typedef struct sdrkit_fsk_modems_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The state: 0 nothing heard, 1 a burst is open, 2 messages read.
  int64_t state;
  // True if `standard` holds a value.
  bool has_standard;
  // The standard of the last message.
  sdrkit_string standard;
  // True if `tones` holds a value.
  bool has_tones;
  // The tones, the sideband and the dial error of the last message.
  sdrkit_string tones;
  // True if `last_message` holds a value.
  bool has_last_message;
  // The bytes, framing errors and length of the last message.
  sdrkit_string last_message;
  // Messages read.
  int64_t messages;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_fsk_modems_fields;

// The result fields of the UVB-76 buzzer monitor. The measures of the train are absent while no train is locked.
typedef struct sdrkit_uvb76_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the stack does not do and what no public document confirms.
  sdrkit_string note;
  // The state: 0 no carrier, 1 carrier, 2 buzz train locked.
  int64_t state;
  // A carrier is in the channel.
  bool carrier;
  // Buzzes heard.
  int64_t buzzes;
  // True if `rate_per_min` holds a value.
  bool has_rate_per_min;
  // The buzz rate, 60 over the median period. Unit: /min.
  double rate_per_min;
  // True if `buzz_s` holds a value.
  bool has_buzz_s;
  // The median buzz length. A length of time in seconds.
  double buzz_s;
  // True if `period_s` holds a value.
  bool has_period_s;
  // The median period. A length of time in seconds.
  double period_s;
  // True if `spread_ms` holds a value.
  bool has_spread_ms;
  // The median distance of the periods from their median. Unit: ms.
  double spread_ms;
  // Train breaks logged.
  int64_t breaks;
  // Long transmissions logged.
  int64_t long_transmissions;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_uvb76_fields;

// The result fields of the VLF MSK monitor. The measures are absent while no transmitter is on.
typedef struct sdrkit_vlf_msk_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the stack does not do and what no public document confirms.
  sdrkit_string note;
  // The state: 0 off air or too weak, 1 carrier only, 2 MSK.
  int64_t state;
  // The class: "MSK 200 bit/s", "MSK about N bit/s", "carrier only" or "off air or too weak".
  sdrkit_string signal_class;
  // True if `carrier_hz` holds a value.
  bool has_carrier_hz;
  // The carrier from the tuned frequency. A frequency in Hz.
  double carrier_hz;
  // True if `bit_rate` holds a value.
  bool has_bit_rate;
  // The bit rate. Unit: bit/s.
  double bit_rate;
  // True if `line_spacing_hz` holds a value.
  bool has_line_spacing_hz;
  // The spacing of the two lines of the squared signal. A frequency in Hz.
  double line_spacing_hz;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level over the noise. Unit: dB.
  double level_db;
  // True if `hint` holds a value.
  bool has_hint;
  // The listed station near the carrier: "likely NAME".
  sdrkit_string hint;
  // Transmitters that came on.
  int64_t transmitters;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_vlf_msk_fields;

// The result fields of a teleprinter stack.
typedef struct sdrkit_hfdata_teleprinter_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The text received, the last characters.
  sdrkit_string text;
  // The search state: 0 searching, 1 signal found, 2 decoding.
  int64_t state;
  // True if `baud` holds a value.
  bool has_baud;
  // The speed of the signal. Absent while the front has no lock. Unit: Bd.
  double baud;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The centre of the signal above the dial. Absent while the front has no lock. Unit: Hz.
  double centre_hz;
  // True if `shift_hz` holds a value.
  bool has_shift_hz;
  // The shift between the tones. Absent while the front has no lock or the signal has no shift. Unit: Hz.
  double shift_hz;
  // What was found beside the speed (cycle, depth, tone length). Empty while not decoding.
  sdrkit_string detail;
  // Words, characters or frames read.
  int64_t good;
  // Words, characters or frames that failed their check.
  int64_t bad;
  // The level of the channel, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_hfdata_teleprinter_fields;

// A telegram of EFR. The row key is the number of the telegram.
typedef struct sdrkit_efr_telegram_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time a telegram ended. Seconds since 1970 (UTC).
  double time;
  // The checksum: "good" or "bad".
  sdrkit_string checksum;
  // The message number (user byte 1).
  int64_t message;
  // Address 1 (user byte 2).
  int64_t address1;
  // Address 2 (user byte 3).
  int64_t address2;
  // The number of payload bytes.
  int64_t payload_bytes;
  // The payload as hex.
  sdrkit_string payload;
} sdrkit_efr_telegram_row;

// The result fields of 3G-ALE.
typedef struct sdrkit_ale3g_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The parts that no public document confirms.
  sdrkit_string note;
  // The text received, the last lines.
  sdrkit_string text;
  // Bursts read.
  int64_t bursts_total;
  // PDUs with a good check.
  int64_t pdus_good;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal-to-noise ratio of the last burst. Absent without a measure. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The frequency error of the last burst. Absent without a measure. A frequency in Hz.
  double offset_hz;
} sdrkit_ale3g_fields;

// A PDU of 3G-ALE. The row key is the number of the PDU.
typedef struct sdrkit_ale3g_pdu_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the PDU. Seconds since 1970 (UTC).
  double utc;
  // The burst (BW0 to BW4).
  sdrkit_string burst;
  // The PDU type.
  sdrkit_string type;
  // The check of the PDU passed.
  bool crc_good;
  // The PDU in words.
  sdrkit_string words;
  // The signal-to-noise ratio of the burst. Unit: dB.
  double snr_db;
  // The frequency error of the burst. A frequency in Hz.
  double offset_hz;
  // The data bytes of a packet or datagram.
  sdrkit_string data;
} sdrkit_ale3g_pdu_row;

// The result fields of STANAG 4481.
typedef struct sdrkit_s4481_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The text received, the last characters.
  sdrkit_string text;
  // What is received: one channel, sub-carriers, or nothing.
  sdrkit_string reception;
} sdrkit_s4481_fields;

// A channel of STANAG 4481: the single channel or a sub-carrier. The row key is the number of the channel.
typedef struct sdrkit_s4481_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of the channel (0: the single channel).
  int64_t channel;
  // The mark tone. Unit: Hz.
  double mark_hz;
  // The space tone. Unit: Hz.
  double space_hz;
  // The channel carries a signal.
  bool on;
  // True if `speed_bd` holds a value.
  bool has_speed_bd;
  // The speed of the channel. Absent until it is found. Unit: Bd.
  double speed_bd;
  // "start-stop" or "sync".
  sdrkit_string framing;
  // Characters read.
  int64_t characters;
  // Characters with a framing error.
  int64_t framing_errors;
  // Bits received.
  int64_t bits;
  // The signal-to-noise ratio of the channel. Unit: dB.
  double snr_db;
  // The last text of the channel.
  sdrkit_string last_text;
} sdrkit_s4481_channel_row;

// The result fields of Link 11.
typedef struct sdrkit_link11_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The parts that no public document confirms.
  sdrkit_string note;
  // The text received, the last lines.
  sdrkit_string text;
  // Messages since the last Clear.
  int64_t messages_total;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The Doppler offset of the last message. Absent without a measure. A frequency in Hz.
  double offset_hz;
} sdrkit_link11_fields;

// A message of Link 11. The row key is the number of the message.
typedef struct sdrkit_link11_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The start of the message, seconds. Unit: s.
  double start_s;
  // The frame rate: "fast" or "slow".
  sdrkit_string rate;
  // Preamble frames.
  int64_t preamble_frames;
  // Data frames.
  int64_t data_frames;
  // The duration, seconds. Unit: s.
  double duration_s;
  // The Doppler offset. Unit: Hz.
  double doppler_hz;
  // The constellation offset, degrees. Unit: deg.
  double offset_deg;
  // The signal-to-noise ratio of the message. Unit: dB.
  double snr_db;
} sdrkit_link11_message_row;

// The result fields of the CIS stack. A field that the submode in use does not measure has no value.
typedef struct sdrkit_cis_hf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What no public document confirms.
  sdrkit_string note;
  // The text received, the last characters.
  sdrkit_string text;
  // The search state: 0 searching, 1 signal found, 2 decoding (or identified).
  int64_t state;
  // What was found, in one line.
  sdrkit_string found;
  // The name of the signal identified, or empty.
  sdrkit_string identified;
  // Characters, words or frames read.
  int64_t good;
  // Characters, words or frames that failed their check.
  int64_t bad;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The frequency error. Absent without a measure. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal-to-noise ratio. Absent without a measure. Unit: dB.
  double snr_db;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The centre of the signal above the dial (2-FSK and CIS-3000). Unit: Hz.
  double centre_hz;
  // True if `shift_hz` holds a value.
  bool has_shift_hz;
  // The shift between the tones (2-FSK). Unit: Hz.
  double shift_hz;
  // True if `eye` holds a value.
  bool has_eye;
  // The eye opening of the 2-FSK demodulators.
  double eye;
  // True if `frames` holds a value.
  bool has_frames;
  // Frames read (CIS-14).
  int64_t frames;
  // True if `state_bits` holds a value.
  bool has_state_bits;
  // The state bits of the last frames (CIS-14), raw.
  sdrkit_string state_bits;
  // True if `pilot_hz` holds a value.
  bool has_pilot_hz;
  // The pilot (CIS-12). Unit: Hz.
  double pilot_hz;
  // True if `carriers` holds a value.
  bool has_carriers;
  // Carriers seen (CIS-12, OFDM).
  int64_t carriers;
  // True if `symbol_rate` holds a value.
  bool has_symbol_rate;
  // The symbol rate. Unit: Bd.
  double symbol_rate;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The modulation found.
  sdrkit_string modulation;
  // True if `m2` holds a value.
  bool has_m2;
  // The moment of the 2nd power.
  double m2;
  // True if `m4` holds a value.
  bool has_m4;
  // The moment of the 4th power.
  double m4;
  // True if `m8` holds a value.
  bool has_m8;
  // The moment of the 8th power.
  double m8;
  // True if `tones` holds a value.
  bool has_tones;
  // Tones seen (CIS-36).
  int64_t tones;
  // True if `spacing_hz` holds a value.
  bool has_spacing_hz;
  // The spacing of the tones or carriers. Unit: Hz.
  double spacing_hz;
  // True if `low_hz` holds a value.
  bool has_low_hz;
  // The lowest tone or band edge. Unit: Hz.
  double low_hz;
  // True if `high_hz` holds a value.
  bool has_high_hz;
  // The highest tone or band edge. Unit: Hz.
  double high_hz;
  // True if `span_hz` holds a value.
  bool has_span_hz;
  // The span of the tones (CIS-36). Unit: Hz.
  double span_hz;
  // True if `double_gaps` holds a value.
  bool has_double_gaps;
  // Double gaps between tones (CIS-36).
  int64_t double_gaps;
  // True if `useful_ms` holds a value.
  bool has_useful_ms;
  // The useful part of an OFDM symbol. Unit: ms.
  double useful_ms;
  // True if `guard_ms` holds a value.
  bool has_guard_ms;
  // The guard of an OFDM symbol. Unit: ms.
  double guard_ms;
  // True if `centre_off` holds a value.
  bool has_centre_off;
  // The centre carrier of the OFDM signal is off.
  bool centre_off;
  // True if `pilot_3300` holds a value.
  bool has_pilot_3300;
  // A pilot at 3300 Hz is there.
  bool pilot_3300;
  // True if `line2_db` holds a value.
  bool has_line2_db;
  // The spectral line of the 2nd power. Unit: dB.
  double line2_db;
  // True if `line4_db` holds a value.
  bool has_line4_db;
  // The spectral line of the 4th power. Unit: dB.
  double line4_db;
  // True if `line8_db` holds a value.
  bool has_line8_db;
  // The spectral line of the 8th power. Unit: dB.
  double line8_db;
  // True if `rate_line_db` holds a value.
  bool has_rate_line_db;
  // The spectral line at the symbol rate. Unit: dB.
  double rate_line_db;
} sdrkit_cis_hf_fields;

// A `bee` message of CIS-36-50. The row key is the number of the message.
typedef struct sdrkit_cis_hf_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time the message ended. Seconds since 1970 (UTC).
  double time;
  // The speed of the message. Unit: Bd.
  double speed;
  // The start word.
  sdrkit_string start_word;
  // The session key.
  sdrkit_string session_key;
  // Characters read.
  int64_t characters;
  // Characters that failed the 3:4 check.
  int64_t failing;
  // What ended the message.
  sdrkit_string ended_by;
} sdrkit_cis_hf_message_row;

// A carrier of CIS-12. The row key is the number of the carrier.
typedef struct sdrkit_cis_hf_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The carrier number (1 to 12).
  int64_t number;
  // The carrier frequency. Unit: Hz.
  double freq_hz;
  // The carrier is on.
  bool on;
} sdrkit_cis_hf_carrier_row;

// The options of `best` of `sdrkit_hfid_fields`.
typedef enum sdrkit_hfid_fields_best {
  // 2400 Bd serial-tone PSK family
  SDRKIT_HFID_FIELDS_BEST_SERIAL_2400 = 0,
  // STANAG 4285
  SDRKIT_HFID_FIELDS_BEST_STANAG_4285 = 1,
  // MIL-STD-188-110A serial tone
  SDRKIT_HFID_FIELDS_BEST_MIL_110A = 2,
  // MIL-STD-188-110B / STANAG 4539 high rate
  SDRKIT_HFID_FIELDS_BEST_MIL_110B = 3,
  // STANAG 4481 PSK (unverified match)
  SDRKIT_HFID_FIELDS_BEST_STANAG_4481 = 4,
  // 3G-ALE burst (8-PSK, 2400 Bd)
  SDRKIT_HFID_FIELDS_BEST_ALE_3G = 5,
  // Link 11 CLEW (parallel tones)
  SDRKIT_HFID_FIELDS_BEST_LINK_11 = 6,
  // 2G-ALE (8-FSK, 125 Bd)
  SDRKIT_HFID_FIELDS_BEST_ALE_2G = 7,
  // CIS-36 (36-tone MFSK)
  SDRKIT_HFID_FIELDS_BEST_CIS_36 = 8,
  // Multi-tone MFSK (CIS-12 / Chinese-style, unverified)
  SDRKIT_HFID_FIELDS_BEST_MULTI_TONE = 9,
  // PACTOR-I / 200 Bd FSK (200 Hz shift)
  SDRKIT_HFID_FIELDS_BEST_PACTOR1 = 10,
  // PACTOR-II-like (100 Bd DPSK, narrow)
  SDRKIT_HFID_FIELDS_BEST_PACTOR2 = 11,
  // Buzzer (UVB-76 style: carrier with a periodic buzz)
  SDRKIT_HFID_FIELDS_BEST_BUZZER = 12,
  // AM voice / numbers-station style carrier
  SDRKIT_HFID_FIELDS_BEST_CARRIER = 13,
  // Unclassified signal
  SDRKIT_HFID_FIELDS_BEST_UNCLASSIFIED = 14,
} sdrkit_hfid_fields_best;

// The result fields of the HF signal identifier: the measures of the last analysis.
typedef struct sdrkit_hfid_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A signal was found by the last analysis.
  bool signal;
  // True if `best` holds a value.
  bool has_best;
  // The best candidate. A value of `sdrkit_hfid_fields_best`.
  int32_t best;
  // True if `best_confidence_pct` holds a value.
  bool has_best_confidence_pct;
  // The confidence of the best candidate, 0 to 95 %. Unit: %.
  double best_confidence_pct;
  // The analyses run.
  int64_t analyses;
  // True if `band_low` holds a value.
  bool has_band_low;
  // The lower edge of the occupied band. A frequency in Hz.
  double band_low;
  // True if `band_high` holds a value.
  bool has_band_high;
  // The upper edge of the occupied band. A frequency in Hz.
  double band_high;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal-to-noise ratio in the band. Absent without a signal. Unit: dB.
  double snr_db;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // The noise floor. Unit: dBFS.
  double noise_dbfs;
  // True if `carrier_line` holds a value.
  bool has_carrier_line;
  // The steady carrier line. A frequency in Hz.
  double carrier_line;
  // True if `carrier_line_db` holds a value.
  bool has_carrier_line_db;
  // The carrier line over its surroundings. Unit: dB.
  double carrier_line_db;
  // True if `tone_605_db` holds a value.
  bool has_tone_605_db;
  // The 605 Hz tone over its surroundings. Unit: dB.
  double tone_605_db;
  // A tone at 2915 Hz.
  bool tone_2915;
  // True if `comb_spacing` holds a value.
  bool has_comb_spacing;
  // The tone comb spacing. A frequency in Hz.
  double comb_spacing;
  // True if `comb_regularity_pct` holds a value.
  bool has_comb_regularity_pct;
  // The regularity of the comb, 0 to 100 %. Unit: %.
  double comb_regularity_pct;
  // True if `comb_tones` holds a value.
  bool has_comb_tones;
  // The tones of the comb.
  int64_t comb_tones;
  // True if `shift` holds a value.
  bool has_shift;
  // The two-tone shift. A frequency in Hz.
  double shift;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate at the clock line. A frequency in Hz.
  double symbol_rate_hz;
  // True if `clock_line_db` holds a value.
  bool has_clock_line_db;
  // The clock line over its surroundings. Unit: dB.
  double clock_line_db;
  // True if `envelope_modulation` holds a value.
  bool has_envelope_modulation;
  // The slow envelope modulation. A frequency in Hz.
  double envelope_modulation;
  // True if `envelope_modulation_db` holds a value.
  bool has_envelope_modulation_db;
  // The envelope modulation line over its surroundings. Unit: dB.
  double envelope_modulation_db;
  // True if `bursts` holds a value.
  bool has_bursts;
  // The bursts in the analysed audio (0: continuous).
  int64_t bursts;
  // True if `duty_pct` holds a value.
  bool has_duty_pct;
  // The part of the time with a burst on. Unit: %.
  double duty_pct;
  // True if `order` holds a value.
  bool has_order;
  // The constellation order at the clock: 2, 4, 8, or 99 for higher or unclear.
  int64_t order;
  // True if `repeat_symbols` holds a value.
  bool has_repeat_symbols;
  // The repetition period in symbols.
  int64_t repeat_symbols;
  // True if `repeat_period` holds a value.
  bool has_repeat_period;
  // The repetition period. A length of time in seconds.
  double repeat_period;
  // True if `repeat_correlation_pct` holds a value.
  bool has_repeat_correlation_pct;
  // The correlation at the repetition period, 0 to 100 %. Unit: %.
  double repeat_correlation_pct;
  // The measured parameters as lines of text.
  sdrkit_string measured;
  // The level of the audio, dBFS. Unit: dBFS.
  double level_dbfs;
  // The audio rate of the analysis. A frequency in Hz.
  double audio_rate;
} sdrkit_hfid_fields;

// The options of `waveform` of `sdrkit_hfid_candidate_row`.
typedef enum sdrkit_hfid_candidate_row_waveform {
  // 2400 Bd serial-tone PSK family
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_SERIAL_2400 = 0,
  // STANAG 4285
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_STANAG_4285 = 1,
  // MIL-STD-188-110A serial tone
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_MIL_110A = 2,
  // MIL-STD-188-110B / STANAG 4539 high rate
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_MIL_110B = 3,
  // STANAG 4481 PSK (unverified match)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_STANAG_4481 = 4,
  // 3G-ALE burst (8-PSK, 2400 Bd)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_ALE_3G = 5,
  // Link 11 CLEW (parallel tones)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_LINK_11 = 6,
  // 2G-ALE (8-FSK, 125 Bd)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_ALE_2G = 7,
  // CIS-36 (36-tone MFSK)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_CIS_36 = 8,
  // Multi-tone MFSK (CIS-12 / Chinese-style, unverified)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_MULTI_TONE = 9,
  // PACTOR-I / 200 Bd FSK (200 Hz shift)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_PACTOR1 = 10,
  // PACTOR-II-like (100 Bd DPSK, narrow)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_PACTOR2 = 11,
  // Buzzer (UVB-76 style: carrier with a periodic buzz)
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_BUZZER = 12,
  // AM voice / numbers-station style carrier
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_CARRIER = 13,
  // Unclassified signal
  SDRKIT_HFID_CANDIDATE_ROW_WAVEFORM_UNCLASSIFIED = 14,
} sdrkit_hfid_candidate_row_waveform;

// A candidate of the last analysis of the identifier. The row key is the place, "0" first.
typedef struct sdrkit_hfid_candidate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The waveform. A value of `sdrkit_hfid_candidate_row_waveform`.
  int32_t waveform;
  // The confidence of the estimate, 0 to 95 %. Unit: %.
  double confidence_pct;
  // What the score is based on.
  sdrkit_string basis;
  // What the waveform is.
  sdrkit_string note;
  // A public source.
  sdrkit_string link;
} sdrkit_hfid_candidate_row;

// The options of `clock` of `sdrkit_ale_fields`.
typedef enum sdrkit_ale_fields_clock {
  // System clock (UTC)
  SDRKIT_ALE_FIELDS_CLOCK_SYSTEM = 0,
  // The recording's start time
  SDRKIT_ALE_FIELDS_CLOCK_RECORDING = 1,
  // Stream time (no clock)
  SDRKIT_ALE_FIELDS_CLOCK_STREAM = 2,
} sdrkit_ale_fields_clock;

// The result fields of ALE.
typedef struct sdrkit_ale_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // In word sync (the lock state "Sync").
  bool in_sync;
  // A word was decoded in the last 60 s.
  bool words_recent;
  // Words decoded.
  int64_t words_ok;
  // Words missed.
  int64_t words_missed;
  // Calls in the event log.
  int64_t calls;
  // Stations heard.
  int64_t stations;
  // The frequency correction. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal-to-noise ratio of the last call. Absent before a call. Unit: dB.
  double snr_db;
  // True if `ber` holds a value.
  bool has_ber;
  // The bit error rate of the last call. Absent before a call.
  double ber;
  // The level of the audio, dBFS. Unit: dBFS.
  double level_dbfs;
  // Where the times come from. A value of `sdrkit_ale_fields_clock`.
  int32_t clock;
} sdrkit_ale_fields;

// The options of `tx_kind` of `sdrkit_ale_tx_fields`.
typedef enum sdrkit_ale_tx_fields_tx_kind {
  // Call
  SDRKIT_ALE_TX_FIELDS_TX_KIND_CALL = 0,
  // Sounding
  SDRKIT_ALE_TX_FIELDS_TX_KIND_SOUNDING = 1,
  // Response
  SDRKIT_ALE_TX_FIELDS_TX_KIND_RESPONSE = 2,
  // Acknowledgement
  SDRKIT_ALE_TX_FIELDS_TX_KIND_ACK = 3,
  // AMD message
  SDRKIT_ALE_TX_FIELDS_TX_KIND_AMD = 4,
} sdrkit_ale_tx_fields_tx_kind;

// The options of `tx_link` of `sdrkit_ale_tx_fields`.
typedef enum sdrkit_ale_tx_fields_tx_link {
  // Idle
  SDRKIT_ALE_TX_FIELDS_TX_LINK_IDLE = 0,
  // Calling
  SDRKIT_ALE_TX_FIELDS_TX_LINK_CALLING = 1,
  // Responding
  SDRKIT_ALE_TX_FIELDS_TX_LINK_RESPONDING = 2,
  // Linked
  SDRKIT_ALE_TX_FIELDS_TX_LINK_LINKED = 3,
  // Timed out
  SDRKIT_ALE_TX_FIELDS_TX_LINK_TIMED_OUT = 4,
  // Not answered
  SDRKIT_ALE_TX_FIELDS_TX_LINK_NOT_ANSWERED = 5,
} sdrkit_ale_tx_fields_tx_link;

// The result fields of the ALE transmitter. The values of a transmission have no value while nothing is sent.
typedef struct sdrkit_ale_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a call or a sounding is being sent.
  bool tx_sending;
  // True if `tx_kind` holds a value.
  bool has_tx_kind;
  // What is being sent. A value of `sdrkit_ale_tx_fields_tx_kind`.
  int32_t tx_kind;
  // True if `tx_to` holds a value.
  bool has_tx_to;
  // The called address of a call.
  sdrkit_string tx_to;
  // True if `tx_word` holds a value.
  bool has_tx_word;
  // The word being sent, from 1.
  int64_t tx_word;
  // True if `tx_words` holds a value.
  bool has_tx_words;
  // The words of the transmission.
  int64_t tx_words;
  // What the transmitter does not do.
  sdrkit_string tx_note;
  // The state of the three-way handshake. A value of `sdrkit_ale_tx_fields_tx_link`.
  int32_t tx_link;
  // True if `tx_link_station` holds a value.
  bool has_tx_link_station;
  // The other station of the handshake. Absent while idle.
  sdrkit_string tx_link_station;
  // Why the link state is what it is; empty when it needs no reason.
  sdrkit_string tx_link_reason;
} sdrkit_ale_tx_fields;

// The options of `type` of `sdrkit_ale_event_row`.
typedef enum sdrkit_ale_event_row_type {
  // Sounding
  SDRKIT_ALE_EVENT_ROW_TYPE_SOUNDING = 0,
  // Individual
  SDRKIT_ALE_EVENT_ROW_TYPE_INDIVIDUAL = 1,
  // Net
  SDRKIT_ALE_EVENT_ROW_TYPE_NET = 2,
  // Group
  SDRKIT_ALE_EVENT_ROW_TYPE_GROUP = 3,
  // AllCall
  SDRKIT_ALE_EVENT_ROW_TYPE_ALL_CALL = 4,
  // AnyCall
  SDRKIT_ALE_EVENT_ROW_TYPE_ANY_CALL = 5,
  // Response
  SDRKIT_ALE_EVENT_ROW_TYPE_RESPONSE = 6,
  // Ack
  SDRKIT_ALE_EVENT_ROW_TYPE_ACK = 7,
  // Net response
  SDRKIT_ALE_EVENT_ROW_TYPE_NET_RESPONSE = 8,
} sdrkit_ale_event_row_type;

// A call in the ALE event log. The row key is the sequence number of the call.
typedef struct sdrkit_ale_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the first word. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the first word, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // True if `dial` holds a value.
  bool has_dial;
  // The dial it was heard on, when known. A frequency in Hz.
  double dial;
  // True if `from` holds a value.
  bool has_from;
  // The caller, when known.
  sdrkit_string from;
  // The TO addresses then the THRU members.
  sdrkit_string to;
  // The type of the call. A value of `sdrkit_ale_event_row_type`.
  int32_t type;
  // TIS (else TWS).
  bool tis;
  // The AMD text.
  sdrkit_string amd;
  // The commands.
  sdrkit_string commands;
  // The bit error rate of the call.
  double ber;
  // The mean word SNR in 3 kHz. Unit: dB.
  double snr_db;
  // The frequency correction at the end of the call. A frequency in Hz.
  double offset_hz;
  // Words in the transmission.
  int64_t words;
} sdrkit_ale_event_row;

// The options of `last_type` of `sdrkit_ale_station_row`.
typedef enum sdrkit_ale_station_row_last_type {
  // Sounding
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_SOUNDING = 0,
  // Individual
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_INDIVIDUAL = 1,
  // Net
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_NET = 2,
  // Group
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_GROUP = 3,
  // AllCall
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_ALL_CALL = 4,
  // AnyCall
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_ANY_CALL = 5,
  // Response
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_RESPONSE = 6,
  // Ack
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_ACK = 7,
  // Net response
  SDRKIT_ALE_STATION_ROW_LAST_TYPE_NET_RESPONSE = 8,
} sdrkit_ale_station_row_last_type;

// A caller of ALE. The row key is the address.
typedef struct sdrkit_ale_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address.
  sdrkit_string address;
  // True if `dial` holds a value.
  bool has_dial;
  // The dial of the last call, when known. A frequency in Hz.
  double dial;
  // The SNR of the last call. Unit: dB.
  double snr_db;
  // The bit error rate of the last call.
  double ber;
  // The type of the last call. A value of `sdrkit_ale_station_row_last_type`.
  int32_t last_type;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_ale_station_row;

// The options of `type` of `sdrkit_ale_word_row`.
typedef enum sdrkit_ale_word_row_type {
  // DATA
  SDRKIT_ALE_WORD_ROW_TYPE_DATA = 0,
  // THRU
  SDRKIT_ALE_WORD_ROW_TYPE_THRU = 1,
  // TO
  SDRKIT_ALE_WORD_ROW_TYPE_TO = 2,
  // TWS
  SDRKIT_ALE_WORD_ROW_TYPE_TWS = 3,
  // FROM
  SDRKIT_ALE_WORD_ROW_TYPE_FROM = 4,
  // TIS
  SDRKIT_ALE_WORD_ROW_TYPE_TIS = 5,
  // CMD
  SDRKIT_ALE_WORD_ROW_TYPE_CMD = 6,
  // REP
  SDRKIT_ALE_WORD_ROW_TYPE_REP = 7,
} sdrkit_ale_word_row_type;

// A decoded ALE word. The row key is the sequence number of the word.
typedef struct sdrkit_ale_word_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of its first symbol. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // The word type. A value of `sdrkit_ale_word_row_type`.
  int32_t type;
  // The three characters of the word (non-printing ones as '.').
  sdrkit_string text;
  // Vote errors.
  int64_t votes;
  // Golay corrections.
  int64_t golay;
} sdrkit_ale_word_row;

// A net address learned from the answers. The row key is the address.
typedef struct sdrkit_ale_net_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address.
  sdrkit_string address;
} sdrkit_ale_net_row;

// The link quality of a station on a channel. The row key is the address and the dial.
typedef struct sdrkit_ale_lqa_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address.
  sdrkit_string address;
  // True if `dial` holds a value.
  bool has_dial;
  // The dial, when known. A frequency in Hz.
  double dial;
  // The mean vote errors a word of its last call, 0 to 30.
  int64_t lqa_ber;
  // The SINAD of its last call, 0 to 30 dB. Unit: dB.
  int64_t sinad_db;
  // The multipath figure of its last call (not measured: 0).
  int64_t multipath;
  // True if `reported_ber` holds a value.
  bool has_reported_ber;
  // The BER that the station reported of this station's signal, in its last call to the own address with an LQA report.
  int64_t reported_ber;
  // True if `reported_sinad_db` holds a value.
  bool has_reported_sinad_db;
  // The SINAD that the station reported of this station's signal, dB. Unit: dB.
  int64_t reported_sinad_db;
  // True if `reported_multipath` holds a value.
  bool has_reported_multipath;
  // The multipath that the station reported.
  int64_t reported_multipath;
  // True if `heard` holds a value.
  bool has_heard;
  // Calls heard from it on this channel.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When last heard on this channel. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_ale_lqa_row;

// An AMD text heard. The row key is the sequence number of the text.
typedef struct sdrkit_ale_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the call's first word. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the call's first word, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // True if `dial` holds a value.
  bool has_dial;
  // The dial it was heard on, when known. A frequency in Hz.
  double dial;
  // True if `from` holds a value.
  bool has_from;
  // The caller, when known.
  sdrkit_string from;
  // The called addresses.
  sdrkit_string to;
  // The text.
  sdrkit_string text;
} sdrkit_ale_message_row;

// The options of `clock` of `sdrkit_hfradar_fields`.
typedef enum sdrkit_hfradar_fields_clock {
  // System clock (UTC)
  SDRKIT_HFRADAR_FIELDS_CLOCK_SYSTEM = 0,
  // The recording's start time
  SDRKIT_HFRADAR_FIELDS_CLOCK_RECORDING = 1,
  // Stream time (no clock)
  SDRKIT_HFRADAR_FIELDS_CLOCK_STREAM = 2,
} sdrkit_hfradar_fields_clock;

// The result fields of the HF ocean radar stack.
typedef struct sdrkit_hfradar_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `band` holds a value.
  bool has_band;
  // The radar band the radio is in, if it is in one.
  sdrkit_string band;
  // Radars shown.
  int64_t radar_count;
  // Sweeps counted.
  int64_t sweeps_counted;
  // Tracks rejected.
  int64_t tracks_rejected;
  // Broadband bursts.
  int64_t broadband_bursts;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The best SNR of the radars heard. Absent without a radar. Unit: dB.
  double snr_db;
  // True if `scan_stops` holds a value.
  bool has_scan_stops;
  // The stops of the scan, while it steps.
  int64_t scan_stops;
  // The time of one waterfall row. A length of time in seconds.
  double waterfall_row;
  // The signal time of the newest waterfall row. A length of time in seconds.
  double waterfall_newest;
  // Where the times come from. A value of `sdrkit_hfradar_fields_clock`.
  int32_t clock;
} sdrkit_hfradar_fields;

// The options of `direction` of `sdrkit_hfradar_radar_row`.
typedef enum sdrkit_hfradar_radar_row_direction {
  // rising
  SDRKIT_HFRADAR_RADAR_ROW_DIRECTION_RISING = 0,
  // falling
  SDRKIT_HFRADAR_RADAR_ROW_DIRECTION_FALLING = 1,
} sdrkit_hfradar_radar_row_direction;

// A radar. The row key is the radar number.
typedef struct sdrkit_hfradar_radar_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The radar number.
  int64_t number;
  // The centre of the sweep. A frequency in Hz.
  double centre;
  // The centre against the tuned frequency. A frequency in Hz.
  double offset_hz;
  // True if `band` holds a value.
  bool has_band;
  // The radar band, if the radar is in one.
  sdrkit_string band;
  // The width of the sweep. A frequency in Hz.
  double width;
  // The sweep rate. Unit: Hz/s.
  double rate_hz_s;
  // The direction of the sweep. A value of `sdrkit_hfradar_radar_row_direction`.
  int32_t direction;
  // True if `repeat` holds a value.
  bool has_repeat;
  // The repetition period of the sweeps, when measured. A length of time in seconds.
  double repeat;
  // The signal-to-noise ratio. Unit: dB.
  double snr_db;
  // The level, dBFS. Unit: dBFS.
  double level_dbfs;
  // Sweeps counted.
  int64_t sweeps;
  // The signal time when first heard. A length of time in seconds.
  double first;
  // The signal time when last heard. A length of time in seconds.
  double last;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The wall time when first heard, when the clock is known. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The wall time when last heard, in whole seconds, when the clock is known. Seconds since 1970 (UTC).
  double last_heard;
  // True if `id` holds a value.
  bool has_id;
  // The keyed ID, when read.
  sdrkit_string id;
  // True if `id_wpm` holds a value.
  bool has_id_wpm;
  // The speed of the keyed ID. Unit: wpm.
  double id_wpm;
  // Times the ID was seen.
  int64_t id_seen;
  // What the radar may be.
  sdrkit_string hint;
} sdrkit_hfradar_radar_row;

// A radar heard. The row key is the sequence number.
typedef struct sdrkit_hfradar_heard_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The wall time, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // The radar number.
  int64_t number;
  // The centre of the sweep. A frequency in Hz.
  double frequency;
  // True if `band` holds a value.
  bool has_band;
  // The radar band, if the radar is in one.
  sdrkit_string band;
  // The signal-to-noise ratio. Unit: dB.
  double snr_db;
} sdrkit_hfradar_heard_row;

// An oceanographic radar band. The row key is the band name.
typedef struct sdrkit_hfradar_band_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The band name ("12 - 14 MHz").
  sdrkit_string name;
  // The lower edge of the working range. A frequency in Hz.
  double low;
  // The upper edge of the working range. A frequency in Hz.
  double high;
  // The middle of the band: where `band.tune` tunes the radio. A frequency in Hz.
  double middle;
  // The range class of the radars of the band.
  sdrkit_string range_class;
} sdrkit_hfradar_band_row;

// The options of `clock` of `sdrkit_hfdata_pactorhf_fields`.
typedef enum sdrkit_hfdata_pactorhf_fields_clock {
  // System clock (UTC)
  SDRKIT_HFDATA_PACTORHF_FIELDS_CLOCK_SYSTEM = 0,
  // The recording's start time
  SDRKIT_HFDATA_PACTORHF_FIELDS_CLOCK_RECORDING = 1,
  // Stream time (no clock)
  SDRKIT_HFDATA_PACTORHF_FIELDS_CLOCK_STREAM = 2,
} sdrkit_hfdata_pactorhf_fields_clock;

// The result fields of the PACTOR-II, -III and -IV stacks: the measures of the last analysis.
typedef struct sdrkit_hfdata_pactorhf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A signal was found by the last analysis.
  bool signal;
  // The variant found: 2, 3 or 4 (0 for nothing).
  int64_t variant;
  // The signal on the channel is not the variant that the stack receives.
  bool other_variant;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate. Absent without a signal. A frequency in Hz.
  double symbol_rate_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The clock error. Absent without a signal. Unit: ppm.
  double clock_ppm;
  // True if `centre` holds a value.
  bool has_centre;
  // The middle of the outer carriers. A frequency in Hz.
  double centre;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The centre against its nominal place. A frequency in Hz.
  double offset_hz;
  // True if `spacing` holds a value.
  bool has_spacing;
  // The carrier spacing (0 for one carrier). A frequency in Hz.
  double spacing;
  // True if `bandwidth` holds a value.
  bool has_bandwidth;
  // The occupied bandwidth. A frequency in Hz.
  double bandwidth;
  // True if `clock_line_db` holds a value.
  bool has_clock_line_db;
  // The clock line of the symbol clock. Unit: dB.
  double clock_line_db;
  // The number of carriers.
  int64_t carrier_count;
  // The PACTOR-III speed level (0 when unknown or another variant).
  int64_t speed_level;
  // The PACTOR-IV constellation estimate: 2, 4, 8, 16 (QAM), 0 unknown.
  int64_t constellation;
  // The number of symbols demodulated on each carrier.
  int64_t symbol_count;
  // True if `stretch_start` holds a value.
  bool has_stretch_start;
  // The start of the demodulated stretch, into the analysed audio. A length of time in seconds.
  double stretch_start;
  // True if `stretch_length` holds a value.
  bool has_stretch_length;
  // The length of the demodulated stretch. A length of time in seconds.
  double stretch_length;
  // A repeating frame structure was found.
  bool frame_found;
  // True if `frame_symbols` holds a value.
  bool has_frame_symbols;
  // The frame period in symbols.
  int64_t frame_symbols;
  // True if `frame_period` holds a value.
  bool has_frame_period;
  // The frame period. A length of time in seconds.
  double frame_period;
  // True if `frame_copies` holds a value.
  bool has_frame_copies;
  // The number of frame copies in the buffer.
  int64_t frame_copies;
  // True if `frame_agreement_pct` holds a value.
  bool has_frame_agreement_pct;
  // The mean agreement of neighbouring copies. Unit: %.
  double frame_agreement_pct;
  // True if `constant_start` holds a value.
  bool has_constant_start;
  // The first symbol of the constant part.
  int64_t constant_start;
  // True if `constant_symbols` holds a value.
  bool has_constant_symbols;
  // The length of the constant part, in symbols.
  int64_t constant_symbols;
  // The constant part as hexadecimal, 2 bits per symbol.
  sdrkit_string constant_hex;
  // The analysis note (PACTOR-IV).
  sdrkit_string note;
  // The newest phase steps of the first carrier, one digit per symbol.
  sdrkit_string symbols_1;
  // The newest phase steps of the second carrier.
  sdrkit_string symbols_2;
  // The newest phase steps of the third carrier.
  sdrkit_string symbols_3;
  // The analyses run.
  int64_t analyses;
  // The analyses that found a signal.
  int64_t analyses_with_signal;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The audio level of the last second. Unit: dBFS.
  double level_dbfs;
  // Where the times come from. A value of `sdrkit_hfdata_pactorhf_fields_clock`.
  int32_t clock;
} sdrkit_hfdata_pactorhf_fields;

// The options of `modulation` of `sdrkit_hfdata_pactorhf_carrier_row`.
typedef enum sdrkit_hfdata_pactorhf_carrier_row_modulation {
  // D-BPSK
  SDRKIT_HFDATA_PACTORHF_CARRIER_ROW_MODULATION_D_BPSK = 0,
  // D-QPSK
  SDRKIT_HFDATA_PACTORHF_CARRIER_ROW_MODULATION_D_QPSK = 1,
  // D-8PSK
  SDRKIT_HFDATA_PACTORHF_CARRIER_ROW_MODULATION_D_8PSK = 2,
  // D-16PSK
  SDRKIT_HFDATA_PACTORHF_CARRIER_ROW_MODULATION_D_16PSK = 3,
} sdrkit_hfdata_pactorhf_carrier_row_modulation;

// A carrier of the last analysis. The row key is the carrier's number, "00" first.
typedef struct sdrkit_hfdata_pactorhf_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The carrier frequency. A frequency in Hz.
  double frequency;
  // The clock line of the carrier. Unit: dB.
  double line_db;
  // Phase steps per symbol.
  int64_t steps;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The modulation, when the phase steps are 2, 4, 8 or 16. A value of `sdrkit_hfdata_pactorhf_carrier_row_modulation`.
  int32_t modulation;
  // The concentration of the phase steps, 0 to 100 %. Unit: %.
  double concentration_pct;
} sdrkit_hfdata_pactorhf_carrier_row;

// An event of the PACTOR-II, -III and -IV stacks. The row key is the sequence number.
typedef struct sdrkit_hfdata_pactorhf_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // What happened.
  sdrkit_string text;
} sdrkit_hfdata_pactorhf_event_row;

// The options of `clock` of `sdrkit_pactor1_fields`.
typedef enum sdrkit_pactor1_fields_clock {
  // System clock (UTC)
  SDRKIT_PACTOR1_FIELDS_CLOCK_SYSTEM = 0,
  // The recording's start time
  SDRKIT_PACTOR1_FIELDS_CLOCK_RECORDING = 1,
  // Stream time (no clock)
  SDRKIT_PACTOR1_FIELDS_CLOCK_STREAM = 2,
} sdrkit_pactor1_fields_clock;

// The result fields of PACTOR-I.
typedef struct sdrkit_pactor1_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A tone pair is locked or a frame was read in the last 30 s (the lock state "Signal").
  bool signal;
  // A frame was read in the last 30 s (the lock state "Packets").
  bool packets_recent;
  // Good packets.
  int64_t packets_ok;
  // Good packets at 200 baud.
  int64_t packets_fast;
  // Good packets at 100 baud.
  int64_t packets_slow;
  // Packets whose CRC failed.
  int64_t packets_bad;
  // Control signals.
  int64_t control_signals;
  // Repeated packets.
  int64_t repeats;
  // Packets read by memory ARQ.
  int64_t memory_arq_saves;
  // Changeovers (CS3).
  int64_t changeovers;
  // Connect packets.
  int64_t connects;
  // The link is in unproto (FEC) mode, else ARQ.
  bool fec_mode;
  // The Huffman table holds all 95 printable characters.
  bool huffman_complete;
  // Entries in the Huffman table.
  int64_t huffman_entries;
  // True if `pair_low` holds a value.
  bool has_pair_low;
  // The locked lower tone. Absent without a lock. A frequency in Hz.
  double pair_low;
  // True if `pair_high` holds a value.
  bool has_pair_high;
  // The locked upper tone. Absent without a lock. A frequency in Hz.
  double pair_high;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The centre of the pair against 1500 Hz. Absent without a lock. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the tone pair. Absent without a lock. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The audio level of the last second. Unit: dBFS.
  double level_dbfs;
  // The text of both sides.
  sdrkit_string text;
  // Where the times come from. A value of `sdrkit_pactor1_fields_clock`.
  int32_t clock;
} sdrkit_pactor1_fields;

// The options of `kind` of `sdrkit_pactor1_frame_row`.
typedef enum sdrkit_pactor1_frame_row_kind {
  // data
  SDRKIT_PACTOR1_FRAME_ROW_KIND_DATA = 0,
  // connect
  SDRKIT_PACTOR1_FRAME_ROW_KIND_CONNECT = 1,
  // QRT
  SDRKIT_PACTOR1_FRAME_ROW_KIND_QRT = 2,
  // CS1
  SDRKIT_PACTOR1_FRAME_ROW_KIND_CS1 = 3,
  // CS2
  SDRKIT_PACTOR1_FRAME_ROW_KIND_CS2 = 4,
  // CS3
  SDRKIT_PACTOR1_FRAME_ROW_KIND_CS3 = 5,
  // CS4
  SDRKIT_PACTOR1_FRAME_ROW_KIND_CS4 = 6,
  // bad packet
  SDRKIT_PACTOR1_FRAME_ROW_KIND_BAD_PACKET = 7,
} sdrkit_pactor1_frame_row_kind;

// A frame of PACTOR-I. The row key is the frame's number in the log.
typedef struct sdrkit_pactor1_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame's first bit. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of the frame, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // The kind of frame. A value of `sdrkit_pactor1_frame_row_kind`.
  int32_t kind;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `header` holds a value.
  bool has_header;
  // The header byte of a packet, 0x55 or 0xAA.
  int64_t header;
  // True if `number` holds a value.
  bool has_number;
  // The packet number 0 to 3.
  int64_t number;
  // Huffman text.
  bool huffman;
  // The break-in flag.
  bool break_in;
  // The QRT flag.
  bool qrt;
  // The CRC passed.
  bool crc_ok;
  // Read by memory ARQ.
  bool memory_arq;
  // A repeat of the packet before.
  bool repeat;
  // A repeat with no control signal since the packet before.
  bool fec_repeat;
  // The tones were swapped.
  bool inverted;
  // The sending side, 1 or 2.
  int64_t side;
  // The text of the frame.
  sdrkit_string text;
  // The data bytes in hexadecimal.
  sdrkit_string data_hex;
  // True if `call` holds a value.
  bool has_call;
  // The called station of a connect packet: the row key of its row of the stations table.
  sdrkit_string call;
  // The SNR of the frame. Unit: dB.
  double snr_db;
  // The lower tone of the frame. A frequency in Hz.
  double pair_low;
} sdrkit_pactor1_frame_row;

// The options of `seen_as` of `sdrkit_pactor1_station_row`.
typedef enum sdrkit_pactor1_station_row_seen_as {
  // called in a connect packet
  SDRKIT_PACTOR1_STATION_ROW_SEEN_AS_CONNECT = 0,
  // named in text
  SDRKIT_PACTOR1_STATION_ROW_SEEN_AS_TEXT = 1,
} sdrkit_pactor1_station_row_seen_as;

// A station of PACTOR-I. The row key is the callsign.
typedef struct sdrkit_pactor1_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign.
  sdrkit_string call;
  // How the station was seen. A value of `sdrkit_pactor1_station_row_seen_as`.
  int32_t seen_as;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_pactor1_station_row;

// The options of `clock` of `sdrkit_hfbeacons_fields`.
typedef enum sdrkit_hfbeacons_fields_clock {
  // System clock (UTC)
  SDRKIT_HFBEACONS_FIELDS_CLOCK_SYSTEM = 0,
  // The recording's start time
  SDRKIT_HFBEACONS_FIELDS_CLOCK_RECORDING = 1,
  // Stream time (no clock)
  SDRKIT_HFBEACONS_FIELDS_CLOCK_STREAM = 2,
} sdrkit_hfbeacons_fields_clock;

// The result fields of the HF beacon monitor.
typedef struct sdrkit_hfbeacons_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The low edge of the span shown (centre - 0.4 x rate). A frequency in Hz.
  double span_low;
  // The high edge of the span shown (centre + 0.4 x rate). A frequency in Hz.
  double span_high;
  // Beacons of the list in the span.
  int64_t in_span;
  // Beacons heard now.
  int64_t heard_now;
  // Channels watched.
  int64_t channels;
  // Frames processed.
  int64_t frames;
  // Keying runs started.
  int64_t runs_started;
  // Where the times come from. A value of `sdrkit_hfbeacons_fields_clock`.
  int32_t clock;
  // The clock is good enough for the time slots of the beacons.
  bool clock_valid;
  // Beacons in the list.
  int64_t list_count;
  // The station position is known.
  bool station_known;
  // True if `best_band` holds a value.
  bool has_best_band;
  // The band to try now, when the model names one.
  sdrkit_string best_band;
  // True if `best_band_stations` holds a value.
  bool has_best_band_stations;
  // The stations heard on that band.
  int64_t best_band_stations;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
} sdrkit_hfbeacons_fields;

// The result fields of the own beacon. The values of a sending have no value while nothing is sent.
typedef struct sdrkit_hfbeacons_hfb_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the beacon runs: it sends now or waits for its next sending.
  bool tx_beacon_on;
  // Whether a sending is on air.
  bool tx_sending;
  // True if `tx_progress_pct` holds a value.
  bool has_tx_progress_pct;
  // The share of the sending on air that is sent. Unit: %.
  double tx_progress_pct;
  // True if `tx_next_s` holds a value.
  bool has_tx_next_s;
  // The time to the next sending, while the beacon waits. A length of time in seconds.
  double tx_next_s;
  // The sendings since `start`.
  int64_t tx_sendings;
  // The time on air of one sending with the present settings. A length of time in seconds.
  double tx_sending_s;
  // What the beacon is: a proposal of the specification, not taken from a standard.
  sdrkit_string tx_note;
  // The share of the interval that one sending is on air. Unit: %.
  double tx_duty_pct;
} sdrkit_hfbeacons_hfb_tx_fields;

// The options of `kind` of `sdrkit_hfbeacons_beacon_row`.
typedef enum sdrkit_hfbeacons_beacon_row_kind {
  // NCDXF/IARU
  SDRKIT_HFBEACONS_BEACON_ROW_KIND_NCDXF = 0,
  // Single letter
  SDRKIT_HFBEACONS_BEACON_ROW_KIND_LETTER = 1,
  // Long Dashes
  SDRKIT_HFBEACONS_BEACON_ROW_KIND_DASHES = 2,
  // Carrier
  SDRKIT_HFBEACONS_BEACON_ROW_KIND_CARRIER = 3,
} sdrkit_hfbeacons_beacon_row_kind;

// A beacon of the list. The row key is the place in the list, "0000" first.
typedef struct sdrkit_hfbeacons_beacon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The beacon's name.
  sdrkit_string name;
  // The frequency of the beacon. A frequency in Hz.
  double frequency;
  // The keying pattern. A value of `sdrkit_hfbeacons_beacon_row_kind`.
  int32_t kind;
  // The entry of the list is uncertain.
  bool uncertain;
  // The beacon is inside the capture.
  bool in_capture;
  // The beacon is heard now.
  bool heard;
  // The confidence of the identity.
  int64_t confidence;
  // True if `last` holds a value.
  bool has_last;
  // The signal time when last heard. A length of time in seconds.
  double last;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC when last heard, in whole seconds. Seconds since 1970 (UTC).
  double last_heard;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR when last heard. Unit: dB.
  double snr_db;
  // True if `best_1h_db` holds a value.
  bool has_best_1h_db;
  // The best SNR of the last hour. Unit: dB.
  double best_1h_db;
  // True if `best_24h_db` holds a value.
  bool has_best_24h_db;
  // The best SNR of the last 24 hours. Unit: dB.
  double best_24h_db;
  // Power steps heard.
  int64_t steps;
  // The SNR of each power step.
  sdrkit_string step_snrs;
  // True if `distance_km` holds a value.
  bool has_distance_km;
  // The distance to the station. Unit: km.
  double distance_km;
  // True if `bearing_deg` holds a value.
  bool has_bearing_deg;
  // The bearing from the station. Unit: deg.
  double bearing_deg;
  // The newest 48 history points in dB, "-" for listened and not heard.
  sdrkit_string trend;
  // History points in 24 hours.
  int64_t listens;
  // History points heard.
  int64_t heard_count;
  // A note on the beacon.
  sdrkit_string note;
  // The band of the beacon.
  sdrkit_string band;
  // True if `position` holds a value.
  bool has_position;
  // The position of the beacon.
  sdrkit_position position;
} sdrkit_hfbeacons_beacon_row;

// A band of the beacon list. The row key is the place, "00" first.
typedef struct sdrkit_hfbeacons_band_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The band.
  sdrkit_string band;
  // Beacons of the band in the list.
  int64_t in_list;
  // Beacons of the band in the capture.
  int64_t in_capture;
  // Beacons of the band heard now.
  int64_t heard_now;
  // True if `farthest_km` holds a value.
  bool has_farthest_km;
  // The farthest beacon heard. Unit: km.
  double farthest_km;
} sdrkit_hfbeacons_band_row;

// The options of `state` of `sdrkit_hfbeacons_propagation_row`.
typedef enum sdrkit_hfbeacons_propagation_row_state {
  // open
  SDRKIT_HFBEACONS_PROPAGATION_ROW_STATE_OPEN = 0,
  // marginal
  SDRKIT_HFBEACONS_PROPAGATION_ROW_STATE_MARGINAL = 1,
  // closed
  SDRKIT_HFBEACONS_PROPAGATION_ROW_STATE_CLOSED = 2,
} sdrkit_hfbeacons_propagation_row_state;

// The propagation of a band. The row key is the place, "00" first.
typedef struct sdrkit_hfbeacons_propagation_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The band.
  sdrkit_string band;
  // The state of the band. A value of `sdrkit_hfbeacons_propagation_row_state`.
  int32_t state;
  // Stations heard.
  int64_t stations;
  // True if `best_snr_db` holds a value.
  bool has_best_snr_db;
  // The best SNR. Unit: dB.
  double best_snr_db;
  // True if `farthest_km` holds a value.
  bool has_farthest_km;
  // The farthest station heard. Unit: km.
  double farthest_km;
  // The score of the band.
  double score;
} sdrkit_hfbeacons_propagation_row;

// The options of `format` of `sdrkit_wavebuoy_fields`.
typedef enum sdrkit_wavebuoy_fields_format {
  // carrier / narrow tone
  SDRKIT_WAVEBUOY_FIELDS_FORMAT_CARRIER = 0,
  // 2FSK
  SDRKIT_WAVEBUOY_FIELDS_FORMAT_2FSK = 1,
  // 4MFSK
  SDRKIT_WAVEBUOY_FIELDS_FORMAT_4MFSK = 2,
} sdrkit_wavebuoy_fields_format;

// The result fields of the wave buoy stack.
typedef struct sdrkit_wavebuoy_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The link format. A value of `sdrkit_wavebuoy_fields_format`.
  int32_t format;
  // The format is forced by the `format` setting.
  bool format_forced;
  // The carrier finder sees a signal.
  bool signal_found;
  // The data is valid: 2FSK with the HXV vectors locked, or 4FSK with the symbols locked.
  bool data_valid;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The centre of the signal about the tuned frequency. A frequency in Hz.
  double offset_hz;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The absolute frequency of the signal centre. A frequency in Hz.
  double frequency;
  // True if `channel` holds a value.
  bool has_channel;
  // The listed buoy channel within 6 kHz of the signal.
  sdrkit_string channel;
  // True if `peak_db` holds a value.
  bool has_peak_db;
  // The peak of the spectrum over the floor. Unit: dB.
  double peak_db;
  // True if `width` holds a value.
  bool has_width;
  // The width holding 90 % of the signal power. A frequency in Hz.
  double width;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR in 100 Hz. Unit: dB.
  double snr_db;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The clock error. Unit: ppm.
  double clock_ppm;
  // True if `tone_step` holds a value.
  bool has_tone_step;
  // The measured distance between the tones. A frequency in Hz.
  double tone_step;
  // True if `symbol_error_steps` holds a value.
  bool has_symbol_error_steps;
  // The RMS distance of the symbol means to the nearest tone, in tone steps. Unit: steps.
  double symbol_error_steps;
  // Symbols demodulated since the last restart.
  int64_t symbol_count;
  // The symbols are locked.
  bool locked;
  // The newest 320 bits (higher tone = 1; 4FSK: the tone number as two bits).
  sdrkit_string bits;
  // The HXV vectors are locked.
  bool hxv_locked;
  // The tones are the other way round.
  bool hxv_inverted;
  // Vectors accepted.
  int64_t vector_count;
  // Vectors accepted with no bit repaired.
  int64_t clean_count;
  // Vectors accepted with 1 or 2 bits repaired.
  int64_t repaired_count;
  // Vectors lost.
  int64_t lost_count;
  // Blocks with a system word read.
  int64_t block_count;
  // True if `last_block` holds a value.
  bool has_last_block;
  // The system word number of the last block.
  int64_t last_block;
  // The last vector is a WR-SG vector (two vertical samples, west AAA).
  bool wrsg;
  // True if `heave_cm` holds a value.
  bool has_heave_cm;
  // The heave of the last vector. Unit: cm.
  int64_t heave_cm;
  // True if `north_cm` holds a value.
  bool has_north_cm;
  // The north displacement of the last vector. Unit: cm.
  int64_t north_cm;
  // True if `west_cm` holds a value.
  bool has_west_cm;
  // The west displacement of the last vector. Unit: cm.
  int64_t west_cm;
  // System file words received, of 16.
  int64_t system_word_count;
  // Spectrum points received, of 64.
  int64_t spectrum_point_count;
  // True if `tn` holds a value.
  bool has_tn;
  // The buoy type number Tn (system word 0).
  int64_t tn;
  // True if `gps_ok` holds a value.
  bool has_gps_ok;
  // The GPS module works (system word 0).
  bool gps_ok;
  // True if `gps_fix` holds a value.
  bool has_gps_fix;
  // The GPS has a fix (system word 0).
  bool gps_fix;
  // True if `hrms_m` holds a value.
  bool has_hrms_m;
  // The RMS wave height (system word 1). Unit: m.
  double hrms_m;
  // True if `hs_m` holds a value.
  bool has_hs_m;
  // The significant wave height, 4 Hrms (system word 1). Unit: m.
  double hs_m;
  // True if `fz` holds a value.
  bool has_fz;
  // The zero-crossing frequency (system word 2). A frequency in Hz.
  double fz;
  // True if `tz_s` holds a value.
  bool has_tz_s;
  // The zero-crossing period, 1 / fz (system word 2). Unit: s.
  double tz_s;
  // True if `psd_max_m2_hz` holds a value.
  bool has_psd_max_m2_hz;
  // The largest spectral density (system word 3). Unit: m^2/Hz.
  double psd_max_m2_hz;
  // True if `tr_c` holds a value.
  bool has_tr_c;
  // The buoy temperature Tr (system word 4). Unit: °C.
  double tr_c;
  // True if `tw_c` holds a value.
  bool has_tw_c;
  // The water temperature Tw (system word 5). Unit: °C.
  double tw_c;
  // True if `life_left_weeks` holds a value.
  bool has_life_left_weeks;
  // The operational life left (system word 6). Unit: weeks.
  int64_t life_left_weeks;
  // True if `battery_status` holds a value.
  bool has_battery_status;
  // The battery status, 0 to 7 (system word 6).
  int64_t battery_status;
  // True if `av0_m_s2` holds a value.
  bool has_av0_m_s2;
  // The vertical accelerometer offset (system word 7). Unit: m/s^2.
  double av0_m_s2;
  // True if `ax0_m_s2` holds a value.
  bool has_ax0_m_s2;
  // The x accelerometer offset (system word 8). Unit: m/s^2.
  double ax0_m_s2;
  // True if `ay0_m_s2` holds a value.
  bool has_ay0_m_s2;
  // The y accelerometer offset (system word 9). Unit: m/s^2.
  double ay0_m_s2;
  // True if `position` holds a value.
  bool has_position;
  // The buoy position (system words 10 to 13).
  sdrkit_position position;
  // True if `orientation_deg` holds a value.
  bool has_orientation_deg;
  // The orientation (system word 14). Unit: °.
  double orientation_deg;
  // True if `inclination_deg` holds a value.
  bool has_inclination_deg;
  // The inclination (system word 15). Unit: °.
  double inclination_deg;
  // True if `period_bits` holds a value.
  bool has_period_bits;
  // The repetition period of the bit stream. Unit: bits.
  int64_t period_bits;
  // True if `period_z` holds a value.
  bool has_period_z;
  // How far the period stands out of random bits (z score).
  double period_z;
  // True if `sync` holds a value.
  bool has_sync;
  // The constant part of a period, in hex.
  sdrkit_string sync;
  // True if `sync_start_bit` holds a value.
  bool has_sync_start_bit;
  // The first bit of the constant part in the period. Unit: bit.
  int64_t sync_start_bit;
  // True if `crc_attempt` holds a value.
  bool has_crc_attempt;
  // The CRC or checksum that fits the frames (an attempt, never a verified Datawell CRC).
  sdrkit_string crc_attempt;
  // Frames the attempt passed in the last analysis.
  int64_t crc_passes;
  // Frames the CRC attempt accepts.
  int64_t frames_ok;
  // Frames the CRC attempt refuses.
  int64_t frames_bad;
} sdrkit_wavebuoy_fields;

// An entry of the directional spectrum received. The row key is the entry number, "00" first.
typedef struct sdrkit_wavebuoy_spectrum_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The spectrum entry number, 0 to 63.
  int64_t entry;
  // The frequency of the entry. A frequency in Hz.
  double frequency;
  // True if `psd_m2_hz` holds a value.
  bool has_psd_m2_hz;
  // The spectral density, once PSDmax is known. Unit: m^2/Hz.
  double psd_m2_hz;
  // The mean wave direction. Unit: °.
  double direction_deg;
  // The directional spread. Unit: °.
  double spread_deg;
  // The second-order Fourier coefficient m2 (no unit).
  double m2;
  // The second-order Fourier coefficient n2 (no unit).
  double n2;
  // The check factor K (no unit).
  double k;
  // The density relative to PSDmax (no unit).
  double relative_psd;
} sdrkit_wavebuoy_spectrum_row;

// A system file word received. The row key is the word number, "00" first.
typedef struct sdrkit_wavebuoy_system_word_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The system file word number, 0 to 15.
  int64_t word;
  // The 12-bit value of the word.
  int64_t value;
} sdrkit_wavebuoy_system_word_row;

// A frame cut at the sync. The row key is the frame's number in the log.
typedef struct sdrkit_wavebuoy_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. A length of time in seconds.
  double start;
  // The bits of the frame in hex.
  sdrkit_string hex;
  // The CRC attempt accepts the frame.
  bool accepted;
} sdrkit_wavebuoy_frame_row;

// The result fields of a serial-tone modem.
typedef struct sdrkit_hfdata_serial_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The parts of the stack that no public document confirms.
  sdrkit_string note;
  // The text received, the last characters.
  sdrkit_string text;
  // The rate and interleaver being received.
  sdrkit_string waveform;
  // Messages ended by their end-of-message pattern.
  int64_t messages_total;
  // Interleaver blocks decoded.
  int64_t blocks_total;
  // D_PDUs with a good header CRC.
  int64_t dpdus_good;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal-to-noise ratio. Absent without a measure. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The frequency error. Absent without a measure. A frequency in Hz.
  double offset_hz;
} sdrkit_hfdata_serial_fields;

// A STANAG 5066 D_PDU found in the bits of a modem. The row key is the number of the D_PDU.
typedef struct sdrkit_hfdata_dpdu_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the D_PDU. Seconds since 1970 (UTC).
  double utc;
  // The D_PDU type.
  sdrkit_string type;
  // The EOT byte.
  int64_t eot;
  // The destination address (hex).
  sdrkit_string destination;
  // The source address (hex).
  sdrkit_string source;
  // Header bytes.
  int64_t header_bytes;
  // Payload bytes.
  int64_t payload_bytes;
  // The data CRC: good, bad or none.
  sdrkit_string data_crc;
  // The first 32 payload bytes (hex).
  sdrkit_string payload;
} sdrkit_hfdata_dpdu_row;

// The options of `state` of `sdrkit_rpr_fields`.
typedef enum sdrkit_rpr_fields_state {
  // No signal
  SDRKIT_RPR_FIELDS_STATE_NOTHING = 0,
  // Comb found
  SDRKIT_RPR_FIELDS_STATE_COMB = 1,
  // Bursts demodulated
  SDRKIT_RPR_FIELDS_STATE_BURSTS = 2,
} sdrkit_rpr_fields_state;

// The options of `modulation` of `sdrkit_rpr_fields`.
typedef enum sdrkit_rpr_fields_modulation {
  // D-BPSK
  SDRKIT_RPR_FIELDS_MODULATION_D_BPSK = 0,
  // D-QPSK
  SDRKIT_RPR_FIELDS_MODULATION_D_QPSK = 1,
} sdrkit_rpr_fields_modulation;

// The options of `clock` of `sdrkit_rpr_fields`.
typedef enum sdrkit_rpr_fields_clock {
  // System clock (UTC)
  SDRKIT_RPR_FIELDS_CLOCK_SYSTEM = 0,
  // The recording's start time
  SDRKIT_RPR_FIELDS_CLOCK_RECORDING = 1,
  // Stream time (no clock)
  SDRKIT_RPR_FIELDS_CLOCK_STREAM = 2,
} sdrkit_rpr_fields_clock;

// The result fields of the Robust Packet stack: the comb of the last analysis and the last burst.
typedef struct sdrkit_rpr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the last analysis found: nothing, the comb, or bursts. A value of `sdrkit_rpr_fields_state`.
  int32_t state;
  // True if `comb_centre` holds a value.
  bool has_comb_centre;
  // The centre of the comb in the audio. Absent without a comb. A frequency in Hz.
  double comb_centre;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The comb centre against the audio centre setting. A frequency in Hz.
  double offset_hz;
  // True if `comb_db` holds a value.
  bool has_comb_db;
  // The mean power inside the comb over the mean power beside it. Unit: dB.
  double comb_db;
  // The carriers seen: those of the last burst with a clear step pattern, else the comb cells that stand out.
  int64_t carrier_count;
  // True if `spacing` holds a value.
  bool has_spacing;
  // The carrier spacing measured on the last burst. A frequency in Hz.
  double spacing;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate of the last burst, at the peak of its clock line. A frequency in Hz.
  double symbol_rate_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The clock error of the last burst of more than 150 symbols. Unit: ppm.
  double clock_ppm;
  // The speed level of the last burst: 1 (D-BPSK) or 2 (D-QPSK); 0 before a burst.
  int64_t speed_level;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The modulation of the last burst. A value of `sdrkit_rpr_fields_modulation`.
  int32_t modulation;
  // True if `net_rate` holds a value.
  bool has_net_rate;
  // The net rate of the speed level before the AX.25 overhead: 200 or 600 bit/s. Unit: bit/s.
  int64_t net_rate;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last burst, from the phase error of its steps. Unit: dB.
  double snr_db;
  // True if `best_snr_db` holds a value.
  bool has_best_snr_db;
  // The best SNR of a burst since the last Clear. Unit: dB.
  double best_snr_db;
  // True if `burst_length` holds a value.
  bool has_burst_length;
  // The length of the last burst. A length of time in seconds.
  double burst_length;
  // The newest step indices of carrier 0, one digit per symbol.
  sdrkit_string symbols_1;
  // The newest step indices of carrier 1.
  sdrkit_string symbols_2;
  // The newest step indices of carrier 2.
  sdrkit_string symbols_3;
  // What the stack does not do.
  sdrkit_string note;
  // The analyses run.
  int64_t analyses;
  // The analyses that found the comb.
  int64_t detections;
  // The bursts demodulated since the last Clear.
  int64_t bursts;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The audio level of the last second. Unit: dBFS.
  double level_dbfs;
  // Where the times come from. A value of `sdrkit_rpr_fields_clock`.
  int32_t clock;
} sdrkit_rpr_fields;

// A carrier of the last burst. The row key is the carrier number, "0" first.
typedef struct sdrkit_rpr_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The carrier frequency in the audio: its place in the comb and its measured turn per symbol. A frequency in Hz.
  double frequency;
  // The concentration of its phase steps for the order of the burst, 0 to 100 %. Unit: %.
  double concentration_pct;
} sdrkit_rpr_carrier_row;

// The options of `modulation` of `sdrkit_rpr_burst_row`.
typedef enum sdrkit_rpr_burst_row_modulation {
  // D-BPSK
  SDRKIT_RPR_BURST_ROW_MODULATION_D_BPSK = 0,
  // D-QPSK
  SDRKIT_RPR_BURST_ROW_MODULATION_D_QPSK = 1,
} sdrkit_rpr_burst_row_modulation;

// A burst demodulated. The row key is the sequence number.
typedef struct sdrkit_rpr_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of its start. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC of its start, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // Its length. A length of time in seconds.
  double length;
  // The speed level: 1 or 2.
  int64_t speed_level;
  // The modulation. A value of `sdrkit_rpr_burst_row_modulation`.
  int32_t modulation;
  // The net rate of the speed level: 200 or 600 bit/s. Unit: bit/s.
  int64_t net_rate;
  // The carriers with a clear step pattern.
  int64_t carriers;
  // The symbols demodulated on each carrier.
  int64_t symbols;
  // The SNR from the phase error of its steps. Unit: dB.
  double snr_db;
  // The comb centre against the audio centre setting. A frequency in Hz.
  double offset;
  // The step indices of the first 24 symbol times as hexadecimal: carrier 0 to 7 in turn, 1 bit (D-BPSK) or 2 bits (D-QPSK) each, most significant first. The bits are still coded and scrambled.
  sdrkit_string first_steps;
} sdrkit_rpr_burst_row;

// An event of the Robust Packet stack. The row key is the sequence number.
typedef struct sdrkit_rpr_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double start;
  // True if `time` holds a value.
  bool has_time;
  // The UTC, when the clock is known. Seconds since 1970 (UTC).
  double time;
  // What happened.
  sdrkit_string text;
} sdrkit_rpr_event_row;

// New text: a line, 64 characters, or what came before a pause.
typedef struct sdrkit_hfdata_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text.
  sdrkit_string text;
} sdrkit_hfdata_text_event;

// The tones of a sender were found.
typedef struct sdrkit_hfdata_tones_found_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The offset of the tones from their nominal place. A frequency in Hz.
  double offset;
  // The SNR in 3 kHz. Unit: dB.
  double snr;
} sdrkit_hfdata_tones_found_event;

// The tones of a sender are gone.
typedef struct sdrkit_hfdata_tones_lost_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The offset of the tones when they were lost. A frequency in Hz.
  double offset;
  // Characters or groups read in the lock.
  int64_t read;
} sdrkit_hfdata_tones_lost_event;

// A block of f06 was read.
typedef struct sdrkit_fsk200_f06_block_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the row.
  sdrkit_string text;
  // True if `number` holds a value.
  bool has_number;
  // The block number. Absent for a divider.
  int64_t number;
  // Divider, Header, Block or File.
  sdrkit_string kind;
  // The CRC result.
  sdrkit_string crc;
} sdrkit_fsk200_f06_block_event;

// A burst of GW FSK was read.
typedef struct sdrkit_gwfsk_gw_packet_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the row.
  sdrkit_string text;
  // Channel marker, Packet, Position or ACK.
  sdrkit_string kind;
  // True if `packet_type` holds a value.
  bool has_packet_type;
  // The type of a packet.
  int64_t packet_type;
  // True if `subtype` holds a value.
  bool has_subtype;
  // The subtype of a packet.
  int64_t subtype;
} sdrkit_gwfsk_gw_packet_event;

// A burst of a telephone FSK modem was read.
typedef struct sdrkit_fsk_modems_modem_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the row.
  sdrkit_string text;
  // The standard.
  sdrkit_string standard;
  // The sideband: upper or lower.
  sdrkit_string sideband;
  // The mark and space tones.
  sdrkit_string tones;
  // The bytes, as hex.
  sdrkit_string bytes;
  // Framing errors.
  int64_t framing_errors;
  // The length of the burst. A length of time in seconds.
  double duration;
} sdrkit_fsk_modems_modem_message_event;

// A buzz train, a break or a long transmission.
typedef struct sdrkit_uvb76_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the row.
  sdrkit_string text;
  // Buzz train, Train break or Long transmission.
  sdrkit_string kind;
  // The value: the rate, the time since the last buzz or the length.
  sdrkit_string value;
} sdrkit_uvb76_event;

// A transmitter came on, went off or changed.
typedef struct sdrkit_vlf_msk_vlf_transmitter_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the row.
  sdrkit_string text;
  // on, off or changed.
  sdrkit_string kind;
  // True if `rate` holds a value.
  bool has_rate;
  // The bit rate. Unit: bit/s.
  double rate;
  // True if `carrier` holds a value.
  bool has_carrier;
  // The carrier from the tuned frequency. A frequency in Hz.
  double carrier;
  // True if `level` holds a value.
  bool has_level;
  // The level over the noise. Unit: dB.
  double level;
  // True if `hint` holds a value.
  bool has_hint;
  // The listed station near the carrier.
  sdrkit_string hint;
} sdrkit_vlf_msk_vlf_transmitter_event;

// A lock of the words, characters or frames.
typedef struct sdrkit_hfdata_fsk_lock_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The speed of the signal. Unit: Bd.
  double baud;
  // The shift between the tones. Unit: Hz.
  double shift_hz;
  // The centre of the signal above the dial. Unit: Hz.
  double centre_hz;
  // What was found beside the speed.
  sdrkit_string detail;
} sdrkit_hfdata_fsk_lock_event;

// The sync of the words, characters or frames was lost.
typedef struct sdrkit_hfdata_fsk_lost_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // Why sync was lost.
  sdrkit_string reason;
} sdrkit_hfdata_fsk_lost_event;

// A telegram of EFR.
typedef struct sdrkit_efr_telegram_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The checksum: "good" or "bad".
  sdrkit_string checksum;
  // The message number (user byte 1).
  int64_t message;
  // Address 1 (user byte 2).
  int64_t address1;
  // Address 2 (user byte 3).
  int64_t address2;
  // The number of payload bytes.
  int64_t payload_bytes;
  // The payload as hex.
  sdrkit_string payload;
} sdrkit_efr_telegram_event;

// A frame of G-TOR.
typedef struct sdrkit_gtor_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The speed of the frame. Unit: Bd.
  double baud;
  // The command of a frame: data, changeover, disconnect, connect.
  sdrkit_string command;
  // The block number, 0 to 3.
  int64_t block;
  // "plain" or "Golay".
  sdrkit_string form;
  // The bits were inverted.
  bool inverted;
  // The compression: none, Huffman, swapped Huffman.
  sdrkit_string compression;
} sdrkit_gtor_frame_event;

// The tone pairs of Piccolo were found.
typedef struct sdrkit_piccolo_pairs_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_piccolo_pairs_event;

// The tone pairs of Coquelet were found.
typedef struct sdrkit_coquelet_pairs_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_coquelet_pairs_event;

// A PDU of 3G-ALE.
typedef struct sdrkit_ale3g_pdu_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The burst (BW0 to BW4).
  sdrkit_string burst;
  // The PDU type.
  sdrkit_string type;
} sdrkit_ale3g_pdu_event;

// A complete datagram of 3G-ALE.
typedef struct sdrkit_ale3g_datagram_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_ale3g_datagram_event;

// An event of STANAG 4481.
typedef struct sdrkit_s4481_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_s4481_event;

// An event of Link 11.
typedef struct sdrkit_link11_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_link11_event;

// A message of Link 11.
typedef struct sdrkit_link11_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the message.
  sdrkit_string text;
  // Data frames.
  int64_t data_frames;
  // The quarter turns of every data frame (15 digits a frame).
  sdrkit_string steps;
} sdrkit_link11_message_event;

// A `bee` message of CIS-36-50.
typedef struct sdrkit_cis_hf_cis_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The start word.
  sdrkit_string start_word;
  // The session key.
  sdrkit_string session_key;
  // Characters read.
  int64_t characters;
  // Characters that failed the 3:4 check.
  int64_t failing;
} sdrkit_cis_hf_cis_message_event;

// A signal was identified.
typedef struct sdrkit_cis_hf_cis_ident_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The name of the signal identified.
  sdrkit_string identified;
} sdrkit_cis_hf_cis_ident_event;

// The options of `type` of `sdrkit_ale_call_event`.
typedef enum sdrkit_ale_call_event_type {
  // Sounding
  SDRKIT_ALE_CALL_EVENT_TYPE_SOUNDING = 0,
  // Individual
  SDRKIT_ALE_CALL_EVENT_TYPE_INDIVIDUAL = 1,
  // Net
  SDRKIT_ALE_CALL_EVENT_TYPE_NET = 2,
  // Group
  SDRKIT_ALE_CALL_EVENT_TYPE_GROUP = 3,
  // AllCall
  SDRKIT_ALE_CALL_EVENT_TYPE_ALL_CALL = 4,
  // AnyCall
  SDRKIT_ALE_CALL_EVENT_TYPE_ANY_CALL = 5,
  // Response
  SDRKIT_ALE_CALL_EVENT_TYPE_RESPONSE = 6,
  // Ack
  SDRKIT_ALE_CALL_EVENT_TYPE_ACK = 7,
  // Net response
  SDRKIT_ALE_CALL_EVENT_TYPE_NET_RESPONSE = 8,
} sdrkit_ale_call_event_type;

// The options of `family` of `sdrkit_ale_call_event`.
typedef enum sdrkit_ale_call_event_family {
  // Aircraft
  SDRKIT_ALE_CALL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ALE_CALL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ALE_CALL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ALE_CALL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ALE_CALL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ALE_CALL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ALE_CALL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ALE_CALL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ALE_CALL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ALE_CALL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ALE_CALL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ALE_CALL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ALE_CALL_EVENT_FAMILY_OTHER = 12,
} sdrkit_ale_call_event_family;

// An ALE call: `source` is the caller, `target` the called addresses and `text` the call for people.
typedef struct sdrkit_ale_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The type of the call. A value of `sdrkit_ale_call_event_type`.
  int32_t type;
  // The caller.
  sdrkit_string source;
  // The called addresses.
  sdrkit_string target;
  // The call for people.
  sdrkit_string text;
  // TIS (else TWS).
  bool tis;
  // The AMD text.
  sdrkit_string amd;
  // The commands.
  sdrkit_string commands;
  // The bit error rate of the call.
  double ber;
  // The mean word SNR in 3 kHz. Unit: dB.
  double snr_db;
  // The frequency correction at the end of the call. A frequency in Hz.
  double offset_hz;
  // Words in the transmission.
  int64_t words;
  // What the event is about: a message. A value of `sdrkit_ale_call_event_family`.
  int32_t family;
} sdrkit_ale_call_event;

// The options of `kind` of `sdrkit_ale_tx_event`.
typedef enum sdrkit_ale_tx_event_kind {
  // Call
  SDRKIT_ALE_TX_EVENT_KIND_CALL = 0,
  // Sounding
  SDRKIT_ALE_TX_EVENT_KIND_SOUNDING = 1,
  // Response
  SDRKIT_ALE_TX_EVENT_KIND_RESPONSE = 2,
  // Acknowledgement
  SDRKIT_ALE_TX_EVENT_KIND_ACK = 3,
  // AMD message
  SDRKIT_ALE_TX_EVENT_KIND_AMD = 4,
} sdrkit_ale_tx_event_kind;

// The options of `family` of `sdrkit_ale_tx_event`.
typedef enum sdrkit_ale_tx_event_family {
  // Aircraft
  SDRKIT_ALE_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ALE_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ALE_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ALE_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ALE_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ALE_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ALE_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ALE_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ALE_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ALE_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ALE_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ALE_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ALE_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_ale_tx_event_family;

// The fields of a change of the ALE transmission.
typedef struct sdrkit_ale_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused.
  sdrkit_string reason;
  // What is sent. A value of `sdrkit_ale_tx_event_kind`.
  int32_t kind;
  // The called address of a call; empty for a sounding.
  sdrkit_string to;
  // The family of the source: a message. A value of `sdrkit_ale_tx_event_family`.
  int32_t family;
} sdrkit_ale_tx_event;

// The options of `state` of `sdrkit_ale_link_event`.
typedef enum sdrkit_ale_link_event_state {
  // Idle
  SDRKIT_ALE_LINK_EVENT_STATE_IDLE = 0,
  // Calling
  SDRKIT_ALE_LINK_EVENT_STATE_CALLING = 1,
  // Responding
  SDRKIT_ALE_LINK_EVENT_STATE_RESPONDING = 2,
  // Linked
  SDRKIT_ALE_LINK_EVENT_STATE_LINKED = 3,
  // Timed out
  SDRKIT_ALE_LINK_EVENT_STATE_TIMED_OUT = 4,
  // Not answered
  SDRKIT_ALE_LINK_EVENT_STATE_NOT_ANSWERED = 5,
} sdrkit_ale_link_event_state;

// The options of `family` of `sdrkit_ale_link_event`.
typedef enum sdrkit_ale_link_event_family {
  // Aircraft
  SDRKIT_ALE_LINK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ALE_LINK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ALE_LINK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ALE_LINK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ALE_LINK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ALE_LINK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ALE_LINK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ALE_LINK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ALE_LINK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ALE_LINK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ALE_LINK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ALE_LINK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ALE_LINK_EVENT_FAMILY_OTHER = 12,
} sdrkit_ale_link_event_family;

// The fields of a step of the three-way handshake.
typedef struct sdrkit_ale_link_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The link state after the step. A value of `sdrkit_ale_link_event_state`.
  int32_t state;
  // The other station.
  sdrkit_string source;
  // The step: the call sent, the call heard, the response heard, the acknowledgement sent or heard, or why there was no answer.
  sdrkit_string reason;
  // The family of the source: a message. A value of `sdrkit_ale_link_event_family`.
  int32_t family;
} sdrkit_ale_link_event;

// The options of `direction` of `sdrkit_hfradar_radar_heard_event`.
typedef enum sdrkit_hfradar_radar_heard_event_direction {
  // rising
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_DIRECTION_RISING = 0,
  // falling
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_DIRECTION_FALLING = 1,
} sdrkit_hfradar_radar_heard_event_direction;

// The options of `family` of `sdrkit_hfradar_radar_heard_event`.
typedef enum sdrkit_hfradar_radar_heard_event_family {
  // Aircraft
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFRADAR_RADAR_HEARD_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfradar_radar_heard_event_family;

// A radar heard first, or again after a minute's pause. `source` is the keyed ID when it was read.
typedef struct sdrkit_hfradar_radar_heard_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The radar number.
  int64_t radar;
  // True if `band` holds a value.
  bool has_band;
  // The radar band, if the radar is in one.
  sdrkit_string band;
  // The width of the sweep. A frequency in Hz.
  double width;
  // The direction of the sweep. A value of `sdrkit_hfradar_radar_heard_event_direction`.
  int32_t direction;
  // True if `source` holds a value.
  bool has_source;
  // The keyed ID, when read.
  sdrkit_string source;
  // What the event is about. A value of `sdrkit_hfradar_radar_heard_event_family`.
  int32_t family;
} sdrkit_hfradar_radar_heard_event;

// The options of `family` of `sdrkit_hfdata_pactorhf_analysis_event`.
typedef enum sdrkit_hfdata_pactorhf_analysis_event_family {
  // Aircraft
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFDATA_PACTORHF_ANALYSIS_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfdata_pactorhf_analysis_event_family;

// A new variant or carrier count: `text` describes the signal.
typedef struct sdrkit_hfdata_pactorhf_analysis_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The variant found.
  int64_t variant;
  // The signal in words.
  sdrkit_string text;
  // What the event is about. A value of `sdrkit_hfdata_pactorhf_analysis_event_family`.
  int32_t family;
} sdrkit_hfdata_pactorhf_analysis_event;

// The options of `kind` of `sdrkit_pactor1_pactor_frame_event`.
typedef enum sdrkit_pactor1_pactor_frame_event_kind {
  // data
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_DATA = 0,
  // connect
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_CONNECT = 1,
  // QRT
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_QRT = 2,
  // CS1
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_CS1 = 3,
  // CS2
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_CS2 = 4,
  // CS3
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_CS3 = 5,
  // CS4
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_CS4 = 6,
  // bad packet
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_KIND_BAD_PACKET = 7,
} sdrkit_pactor1_pactor_frame_event_kind;

// The options of `family` of `sdrkit_pactor1_pactor_frame_event`.
typedef enum sdrkit_pactor1_pactor_frame_event_family {
  // Aircraft
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PACTOR1_PACTOR_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_pactor1_pactor_frame_event_family;

// A frame of PACTOR-I.
typedef struct sdrkit_pactor1_pactor_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The kind of frame. A value of `sdrkit_pactor1_pactor_frame_event_kind`.
  int32_t kind;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `header` holds a value.
  bool has_header;
  // The header byte of a packet.
  int64_t header;
  // True if `number` holds a value.
  bool has_number;
  // The packet number 0 to 3.
  int64_t number;
  // True if `source` holds a value.
  bool has_source;
  // The sender; not read.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The called station of a connect packet.
  sdrkit_string target;
  // The text of the frame.
  sdrkit_string text;
  // The sending side, 1 or 2.
  int64_t side;
  // The tones were swapped.
  bool inverted;
  // A repeat of the packet before.
  bool repeat;
  // The SNR of the frame. Unit: dB.
  double snr_db;
  // What the event is about: a message. A value of `sdrkit_pactor1_pactor_frame_event_family`.
  int32_t family;
} sdrkit_pactor1_pactor_frame_event;

// The options of `family` of `sdrkit_hfbeacons_hfb_detection_event`.
typedef enum sdrkit_hfbeacons_hfb_detection_event_family {
  // Aircraft
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFBEACONS_HFB_DETECTION_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfbeacons_hfb_detection_event_family;

// A beacon heard with its identity confirmed: `source` is the beacon's name, `text` the note.
typedef struct sdrkit_hfbeacons_hfb_detection_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The beacon's name.
  sdrkit_string source;
  // The SNR. Unit: dB.
  double snr_db;
  // The confidence of the identity.
  int64_t confidence;
  // The note.
  sdrkit_string text;
  // True if `position` holds a value.
  bool has_position;
  // The position of the beacon, when known.
  sdrkit_position position;
  // What the event is about. A value of `sdrkit_hfbeacons_hfb_detection_event_family`.
  int32_t family;
} sdrkit_hfbeacons_hfb_detection_event;

// The options of `family` of `sdrkit_hfbeacons_hfb_tx_event`.
typedef enum sdrkit_hfbeacons_hfb_tx_event_family {
  // Aircraft
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HFBEACONS_HFB_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_hfbeacons_hfb_tx_event_family;

// The fields of a change of the own beacon.
typedef struct sdrkit_hfbeacons_hfb_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started (the first sending), sending (a later one), sent (a sending ended), finished (the beacon ended) or refused.
  sdrkit_string state;
  // Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused.
  sdrkit_string reason;
  // The text keyed.
  sdrkit_string text;
  // The family of the source: a message. A value of `sdrkit_hfbeacons_hfb_tx_event_family`.
  int32_t family;
} sdrkit_hfbeacons_hfb_tx_event;

// The options of `family` of `sdrkit_wavebuoy_vector_event`.
typedef enum sdrkit_wavebuoy_vector_event_family {
  // Aircraft
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_WAVEBUOY_VECTOR_EVENT_FAMILY_OTHER = 12,
} sdrkit_wavebuoy_vector_event_family;

// An accepted HXV vector.
typedef struct sdrkit_wavebuoy_vector_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The heave of the vector. Unit: cm.
  int64_t heave_cm;
  // The north displacement of the vector. Unit: cm.
  int64_t north_cm;
  // The west displacement of the vector. Unit: cm.
  int64_t west_cm;
  // The cyclic data word of the vector.
  int64_t cyclic;
  // What the event is about: a sensor. A value of `sdrkit_wavebuoy_vector_event_family`.
  int32_t family;
} sdrkit_wavebuoy_vector_event;

// A status event of MIL-STD-188-110: sync, end of message, loss.
typedef struct sdrkit_ms110_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_ms110_event;

// A message of MIL-STD-188-110 with its text.
typedef struct sdrkit_ms110_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the message.
  sdrkit_string text;
  // The bytes of the message.
  int64_t bytes;
} sdrkit_ms110_message_event;

// A status event of STANAG 4285: sync, end of message, loss.
typedef struct sdrkit_s4285_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_s4285_event;

// A message of STANAG 4285 with its text.
typedef struct sdrkit_s4285_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the message.
  sdrkit_string text;
  // The bytes of the message.
  int64_t bytes;
} sdrkit_s4285_message_event;

// A status event of STANAG 4539: sync, end of message, loss.
typedef struct sdrkit_s4539_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
} sdrkit_s4539_event;

// A message of STANAG 4539 with its text.
typedef struct sdrkit_s4539_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the message.
  sdrkit_string text;
  // The bytes of the message.
  int64_t bytes;
} sdrkit_s4539_message_event;

// The options of `modulation` of `sdrkit_rpr_burst_event`.
typedef enum sdrkit_rpr_burst_event_modulation {
  // D-BPSK
  SDRKIT_RPR_BURST_EVENT_MODULATION_D_BPSK = 0,
  // D-QPSK
  SDRKIT_RPR_BURST_EVENT_MODULATION_D_QPSK = 1,
} sdrkit_rpr_burst_event_modulation;

// The options of `family` of `sdrkit_rpr_burst_event`.
typedef enum sdrkit_rpr_burst_event_family {
  // Aircraft
  SDRKIT_RPR_BURST_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RPR_BURST_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RPR_BURST_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RPR_BURST_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RPR_BURST_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RPR_BURST_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RPR_BURST_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RPR_BURST_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RPR_BURST_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RPR_BURST_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RPR_BURST_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RPR_BURST_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RPR_BURST_EVENT_FAMILY_OTHER = 12,
} sdrkit_rpr_burst_event_family;

// A burst demodulated, with its whole stream of phase steps.
typedef struct sdrkit_rpr_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The speed level: 1 or 2.
  int64_t speed_level;
  // The modulation. A value of `sdrkit_rpr_burst_event_modulation`.
  int32_t modulation;
  // The carriers with a clear step pattern.
  int64_t carriers;
  // The symbols demodulated on each carrier.
  int64_t symbols;
  // The length of the burst. A length of time in seconds.
  double length;
  // The SNR from the phase error of its steps. Unit: dB.
  double snr_db;
  // The comb centre against the audio centre setting. A frequency in Hz.
  double offset_hz;
  // The step indices, one byte each (0 or 1 for D-BPSK, 0 to 3 for D-QPSK; the step is index x 360 / order degrees after the constant turn of the carrier): for each symbol time after the first, carrier 0 to 7 in turn.
  sdrkit_bytes steps;
  // The burst in words.
  sdrkit_string text;
  // What the event is about. A value of `sdrkit_rpr_burst_event_family`.
  int32_t family;
} sdrkit_rpr_burst_event;

// The arguments of `remove` of an ALE station.
typedef struct sdrkit_ale_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_ale_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ALE_STATION_ARGS_INIT {sizeof(sdrkit_ale_station_args), {0}}

// The arguments of the ALE `call`.
typedef struct sdrkit_ale_call_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The called address.
  sdrkit_string to;
} sdrkit_ale_call_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ALE_CALL_ARGS_INIT {sizeof(sdrkit_ale_call_args), {0}}

// The arguments of the ALE `amd`.
typedef struct sdrkit_ale_amd_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The called address.
  sdrkit_string to;
  // The text.
  sdrkit_string text;
} sdrkit_ale_amd_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ALE_AMD_ARGS_INIT {sizeof(sdrkit_ale_amd_args), {0}, {0}}

// The arguments of `band.tune` of the `hfradar` stack.
typedef struct sdrkit_hfradar_band_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the band ("12 - 14 MHz").
  sdrkit_string row;
} sdrkit_hfradar_band_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_HFRADAR_BAND_ARGS_INIT {sizeof(sdrkit_hfradar_band_args), {0}}

// The arguments of `remove` of a PACTOR-I station.
typedef struct sdrkit_pactor1_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_pactor1_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_PACTOR1_STATION_ARGS_INIT {sizeof(sdrkit_pactor1_station_args), {0}}

// Fills `rows` with the changed rows of the table "rows" of the stack "crowd36" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_crowd36_rows(SDRKIT_HANDLE handle,
                                         sdrkit_hfdata_sender_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "text"
// (Text). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdata_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdata_text_event *event) {
  return sdrkit_typed_event(handle, n, "text", "hfdata.TextEvent", event, sizeof(sdrkit_hfdata_text_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.found"
// (Tones found). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdata_tones_found_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdata_tones_found_event *event) {
  return sdrkit_typed_event(handle, n, "tones.found", "hfdata.TonesFoundEvent", event, sizeof(sdrkit_hfdata_tones_found_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.lost"
// (Tones lost). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdata_tones_lost_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdata_tones_lost_event *event) {
  return sdrkit_typed_event(handle, n, "tones.lost", "hfdata.TonesLostEvent", event, sizeof(sdrkit_hfdata_tones_lost_event));
}

// Runs the command "clear" (Clear) of the stack "crowd36" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_crowd36_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "crowd36" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_crowd36_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "crowd36" are the sdrkit_crowd36_gen_*() functions
// after the settings of its receiver.

// The setting "sync_high_tone" (Sync high tone) of the stack "crowd36". Unit: tone. The tone number the higher sync tone is taken as; it sets the numbering of every tone. It must match the station.
static inline int32_t sdrkit_crowd36_get_sync_high_tone(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sync_high_tone", value);
}
static inline int32_t sdrkit_crowd36_set_sync_high_tone(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sync_high_tone", value);
}

// The setting "sideband" (Sideband) of the stack "crowd36". A value of `sdrkit_crowd36_sideband`. lower mirrors the numbering; the sync cannot tell the sidebands apart.
static inline int32_t sdrkit_crowd36_get_sideband(SDRKIT_HANDLE handle, sdrkit_crowd36_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_crowd36_sideband)index;
  return code;
}
static inline int32_t sdrkit_crowd36_set_sideband(SDRKIT_HANDLE handle, sdrkit_crowd36_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "sideband", (int32_t)value);
}

// The setting "speed" (Speed) of the stack "crowd36". A value of `sdrkit_crowd36_speed`. The stack and the test signal use this speed.
static inline int32_t sdrkit_crowd36_get_speed(SDRKIT_HANDLE handle, sdrkit_crowd36_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_crowd36_speed)index;
  return code;
}
static inline int32_t sdrkit_crowd36_set_speed(SDRKIT_HANDLE handle, sdrkit_crowd36_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "speed", (int32_t)value);
}

// The setting "search_hz" (Search width) of the stack "crowd36". A frequency in Hz. The tones are searched up to this far from their nominal place (and within the channel).
static inline int32_t sdrkit_crowd36_get_search_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_hz", value);
}
static inline int32_t sdrkit_crowd36_set_search_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_hz", value);
}

// The setting "gen_groups" (Groups) of the test signal generator of the stack "crowd36". Random five-letter groups from the seed.
static inline int32_t sdrkit_crowd36_gen_get_groups(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_groups", value);
}
static inline int32_t sdrkit_crowd36_gen_set_groups(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_groups", value);
}

// The setting "gen_sync_high" (Sync high tone) of the test signal generator of the stack "crowd36". Unit: tone.
static inline int32_t sdrkit_crowd36_gen_get_sync_high(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_sync_high", value);
}
static inline int32_t sdrkit_crowd36_gen_set_sync_high(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_sync_high", value);
}

// The setting "gen_sync_low" (Sync low tone) of the test signal generator of the stack "crowd36". Unit: tone.
static inline int32_t sdrkit_crowd36_gen_get_sync_low(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_sync_low", value);
}
static inline int32_t sdrkit_crowd36_gen_set_sync_low(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_sync_low", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "xpa" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_xpa_rows(SDRKIT_HANDLE handle,
                                     sdrkit_hfdata_sender_row *rows,
                                     uint32_t capacity,
                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "xpa" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_xpa_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "xpa" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_xpa_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "xpa" are the sdrkit_xpa_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Signal) of the stack "xpa". A value of `sdrkit_xpa_submode`.
static inline int32_t sdrkit_xpa_get_submode(SDRKIT_HANDLE handle, sdrkit_xpa_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_xpa_submode)index;
  return code;
}
static inline int32_t sdrkit_xpa_set_submode(SDRKIT_HANDLE handle, sdrkit_xpa_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "speed" (Speed) of the stack "xpa". A value of `sdrkit_xpa_speed`. XPA only: Find it reads 10 and 20 Bd side by side; a value reads one.
static inline int32_t sdrkit_xpa_get_speed(SDRKIT_HANDLE handle, sdrkit_xpa_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_xpa_speed)index;
  return code;
}
static inline int32_t sdrkit_xpa_set_speed(SDRKIT_HANDLE handle, sdrkit_xpa_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "speed", (int32_t)value);
}

// The setting "sideband" (Sideband) of the stack "xpa". A value of `sdrkit_xpa_sideband`. Find it reads both sidebands side by side; a value halves the readings.
static inline int32_t sdrkit_xpa_get_sideband(SDRKIT_HANDLE handle, sdrkit_xpa_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_xpa_sideband)index;
  return code;
}
static inline int32_t sdrkit_xpa_set_sideband(SDRKIT_HANDLE handle, sdrkit_xpa_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "sideband", (int32_t)value);
}

// The setting "numbering" (Tone numbering correction) of the stack "xpa". A value of `sdrkit_xpa_numbering`. Find it reads the corrections -3 to +3 tones side by side; a value reads one.
static inline int32_t sdrkit_xpa_get_numbering(SDRKIT_HANDLE handle, sdrkit_xpa_numbering *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "numbering", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_xpa_numbering)index;
  return code;
}
static inline int32_t sdrkit_xpa_set_numbering(SDRKIT_HANDLE handle, sdrkit_xpa_numbering value) {
  return sdrkit_typed_setting_set_enum(handle, "numbering", (int32_t)value);
}

// The setting "search_hz" (Search width) of the stack "xpa". A frequency in Hz. The tones are searched up to this far from their nominal place (and within the channel).
static inline int32_t sdrkit_xpa_get_search_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_hz", value);
}
static inline int32_t sdrkit_xpa_set_search_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_hz", value);
}

// The setting "gen_groups" (Groups) of the test signal generator of the stack "xpa". Random five-figure groups from the seed; 0 takes 30 for XPA and 24 for XPA2.
static inline int32_t sdrkit_xpa_gen_get_groups(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_groups", value);
}
static inline int32_t sdrkit_xpa_gen_set_groups(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_groups", value);
}

// The setting "gen_baud" (XPA speed) of the test signal generator of the stack "xpa". A value of `sdrkit_xpa_gen_baud`.
static inline int32_t sdrkit_xpa_gen_get_baud(SDRKIT_HANDLE handle, sdrkit_xpa_gen_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_xpa_gen_baud)index;
  return code;
}
static inline int32_t sdrkit_xpa_gen_set_baud(SDRKIT_HANDLE handle, sdrkit_xpa_gen_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_baud", (int32_t)value);
}

// The setting "gen_lower" (Lower sideband) of the test signal generator of the stack "xpa".
static inline int32_t sdrkit_xpa_gen_get_lower(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_lower", value);
}
static inline int32_t sdrkit_xpa_gen_set_lower(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_lower", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "fsk200" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsk200_rows(SDRKIT_HANDLE handle,
                                        sdrkit_hfdata_sender_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "f06.block"
// (Block). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fsk200_f06_block_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fsk200_f06_block_event *event) {
  return sdrkit_typed_event(handle, n, "f06.block", "hfdata.F06BlockEvent", event, sizeof(sdrkit_fsk200_f06_block_event));
}

// Runs the command "clear" (Clear) of the stack "fsk200" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fsk200_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "fsk200" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_fsk200_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fsk200" are the sdrkit_fsk200_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Signal) of the stack "fsk200". A value of `sdrkit_fsk200_submode`.
static inline int32_t sdrkit_fsk200_get_submode(SDRKIT_HANDLE handle, sdrkit_fsk200_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk200_submode)index;
  return code;
}
static inline int32_t sdrkit_fsk200_set_submode(SDRKIT_HANDLE handle, sdrkit_fsk200_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "polarity" (Tone sense) of the stack "fsk200". A value of `sdrkit_fsk200_polarity`. Find it reads both senses side by side; f06 tests the sync word and its complement.
static inline int32_t sdrkit_fsk200_get_polarity(SDRKIT_HANDLE handle, sdrkit_fsk200_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk200_polarity)index;
  return code;
}
static inline int32_t sdrkit_fsk200_set_polarity(SDRKIT_HANDLE handle, sdrkit_fsk200_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "polarity", (int32_t)value);
}

// The setting "text_case" (Case) of the stack "fsk200". A value of `sdrkit_fsk200_text_case`. f01: the traffic is figure groups; Follow the shifts reads other ITA2 traffic.
static inline int32_t sdrkit_fsk200_get_text_case(SDRKIT_HANDLE handle, sdrkit_fsk200_text_case *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "text_case", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk200_text_case)index;
  return code;
}
static inline int32_t sdrkit_fsk200_set_text_case(SDRKIT_HANDLE handle, sdrkit_fsk200_text_case value) {
  return sdrkit_typed_setting_set_enum(handle, "text_case", (int32_t)value);
}

// The setting "sync_errors" (Sync word errors accepted) of the stack "fsk200". Unit: bits. f06: above 1 a block is kept only when its CRC holds.
static inline int32_t sdrkit_fsk200_get_sync_errors(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sync_errors", value);
}
static inline int32_t sdrkit_fsk200_set_sync_errors(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sync_errors", value);
}

// The setting "correct_one_bit" (Correct one bit by the CRC) of the stack "fsk200". f06: a block whose CRC fails is tried with each bit flipped, weakest first.
static inline int32_t sdrkit_fsk200_get_correct_one_bit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "correct_one_bit", value);
}
static inline int32_t sdrkit_fsk200_set_correct_one_bit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "correct_one_bit", value);
}

// The setting "gen_groups" (Groups) of the test signal generator of the stack "fsk200". f01: random five-figure groups from the seed, three to a block.
static inline int32_t sdrkit_fsk200_gen_get_groups(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_groups", value);
}
static inline int32_t sdrkit_fsk200_gen_set_groups(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_groups", value);
}

// The setting "gen_blocks" (Blocks) of the test signal generator of the stack "fsk200". f06: figure blocks of random numbers from the seed.
static inline int32_t sdrkit_fsk200_gen_get_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_blocks", value);
}
static inline int32_t sdrkit_fsk200_gen_set_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_blocks", value);
}

// The setting "gen_kind" (Traffic) of the test signal generator of the stack "fsk200". A value of `sdrkit_fsk200_gen_kind`.
static inline int32_t sdrkit_fsk200_gen_get_kind(SDRKIT_HANDLE handle, sdrkit_fsk200_gen_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk200_gen_kind)index;
  return code;
}
static inline int32_t sdrkit_fsk200_gen_set_kind(SDRKIT_HANDLE handle, sdrkit_fsk200_gen_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_kind", (int32_t)value);
}

// The setting "gen_text" (Text) of the test signal generator of the stack "fsk200".
static inline int32_t sdrkit_fsk200_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_fsk200_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// The setting "gen_name" (File name) of the test signal generator of the stack "fsk200".
static inline int32_t sdrkit_fsk200_gen_get_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_name", value);
}
static inline int32_t sdrkit_fsk200_gen_set_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_name", value);
}

// The setting "gen_bit_errors" (Bits flipped) of the test signal generator of the stack "fsk200". f06: block:bit pairs separated by commas, the bit counted from 0 at the first bit of the block.
static inline int32_t sdrkit_fsk200_gen_get_bit_errors(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_bit_errors", value);
}
static inline int32_t sdrkit_fsk200_gen_set_bit_errors(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_bit_errors", value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "fsk200". A frequency in Hz.
static inline int32_t sdrkit_fsk200_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_fsk200_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// The setting "gen_reversed" (Reversed sense) of the test signal generator of the stack "fsk200".
static inline int32_t sdrkit_fsk200_gen_get_reversed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_reversed", value);
}
static inline int32_t sdrkit_fsk200_gen_set_reversed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_reversed", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "gwfsk" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gwfsk_rows(SDRKIT_HANDLE handle,
                                       sdrkit_hfdata_sender_row *rows,
                                       uint32_t capacity,
                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "gw.packet"
// (Packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_gwfsk_gw_packet_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_gwfsk_gw_packet_event *event) {
  return sdrkit_typed_event(handle, n, "gw.packet", "hfdata.GwPacketEvent", event, sizeof(sdrkit_gwfsk_gw_packet_event));
}

// Runs the command "clear" (Clear) of the stack "gwfsk" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_gwfsk_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "gwfsk" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_gwfsk_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gwfsk" are the sdrkit_gwfsk_gen_*() functions
// after the settings of its receiver.

// The setting "polarity" (Bit sense) of the stack "gwfsk". A value of `sdrkit_gwfsk_polarity`. Find it tries both readings of each burst.
static inline int32_t sdrkit_gwfsk_get_polarity(SDRKIT_HANDLE handle, sdrkit_gwfsk_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gwfsk_polarity)index;
  return code;
}
static inline int32_t sdrkit_gwfsk_set_polarity(SDRKIT_HANDLE handle, sdrkit_gwfsk_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "polarity", (int32_t)value);
}

// The setting "show_unknown" (Show bursts not understood) of the stack "gwfsk".
static inline int32_t sdrkit_gwfsk_get_show_unknown(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "show_unknown", value);
}
static inline int32_t sdrkit_gwfsk_set_show_unknown(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "show_unknown", value);
}

// The setting "stations" (Shore station names) of the stack "gwfsk". code=name pairs separated by semicolons (hex codes), from the host's data file; a code not listed shows Unknown.
static inline int32_t sdrkit_gwfsk_get_stations(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "stations", value);
}
static inline int32_t sdrkit_gwfsk_set_stations(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "stations", value);
}

// The setting "gen_position" (Position report) of the test signal generator of the stack "gwfsk". The position report, a marker, a connection opening and an ACK instead of the default bursts.
static inline int32_t sdrkit_gwfsk_gen_get_position(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_position", value);
}
static inline int32_t sdrkit_gwfsk_gen_set_position(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_position", value);
}

// The setting "gen_gap_s" (Gap) of the test signal generator of the stack "gwfsk". A length of time in seconds.
static inline int32_t sdrkit_gwfsk_gen_get_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_s", value);
}
static inline int32_t sdrkit_gwfsk_gen_set_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "gwfsk". A frequency in Hz.
static inline int32_t sdrkit_gwfsk_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_gwfsk_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// The setting "gen_reversed" (Reversed sense) of the test signal generator of the stack "gwfsk".
static inline int32_t sdrkit_gwfsk_gen_get_reversed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_reversed", value);
}
static inline int32_t sdrkit_gwfsk_gen_set_reversed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_reversed", value);
}

// The setting "gen_content" (Bursts) of the test signal generator of the stack "gwfsk". Items separated by semicolons: marker HH (station code), text T/S C CHARS, mmsi T/S C DIGITS, packet T/S C HEX (six bytes), ack, frame200 HEX (14 bytes at 200 Bd). T/S is type/subtype, C the count bit.
static inline int32_t sdrkit_gwfsk_gen_get_content(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_content", value);
}
static inline int32_t sdrkit_gwfsk_gen_set_content(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_content", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "fsk_modems" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsk_modems_rows(SDRKIT_HANDLE handle,
                                            sdrkit_hfdata_sender_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "modem.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fsk_modems_modem_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fsk_modems_modem_message_event *event) {
  return sdrkit_typed_event(handle, n, "modem.message", "hfdata.ModemMessageEvent", event, sizeof(sdrkit_fsk_modems_modem_message_event));
}

// Runs the command "clear" (Clear) of the stack "fsk_modems" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fsk_modems_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "fsk_modems" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_fsk_modems_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fsk_modems" are the sdrkit_fsk_modems_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Standard) of the stack "fsk_modems". A value of `sdrkit_fsk_modems_submode`. Find it tries the six standards and names the one that fits.
static inline int32_t sdrkit_fsk_modems_get_submode(SDRKIT_HANDLE handle, sdrkit_fsk_modems_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk_modems_submode)index;
  return code;
}
static inline int32_t sdrkit_fsk_modems_set_submode(SDRKIT_HANDLE handle, sdrkit_fsk_modems_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "dial_search_hz" (Dial error searched) of the stack "fsk_modems". A frequency in Hz. The dial error is tried over plus and minus this, in 10 Hz steps, then refined in 2 Hz steps.
static inline int32_t sdrkit_fsk_modems_get_dial_search_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_search_hz", value);
}
static inline int32_t sdrkit_fsk_modems_set_dial_search_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_search_hz", value);
}

// The setting "sideband" (Sideband) of the stack "fsk_modems". A value of `sdrkit_fsk_modems_sideband`.
static inline int32_t sdrkit_fsk_modems_get_sideband(SDRKIT_HANDLE handle, sdrkit_fsk_modems_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk_modems_sideband)index;
  return code;
}
static inline int32_t sdrkit_fsk_modems_set_sideband(SDRKIT_HANDLE handle, sdrkit_fsk_modems_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "sideband", (int32_t)value);
}

// The setting "gen_standard" (Standard) of the test signal generator of the stack "fsk_modems". A value of `sdrkit_fsk_modems_gen_standard`.
static inline int32_t sdrkit_fsk_modems_gen_get_standard(SDRKIT_HANDLE handle, sdrkit_fsk_modems_gen_standard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_standard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fsk_modems_gen_standard)index;
  return code;
}
static inline int32_t sdrkit_fsk_modems_gen_set_standard(SDRKIT_HANDLE handle, sdrkit_fsk_modems_gen_standard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_standard", (int32_t)value);
}

// The setting "gen_count" (Bytes) of the test signal generator of the stack "fsk_modems". Random printable bytes from the seed.
static inline int32_t sdrkit_fsk_modems_gen_get_count(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_count", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_count(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_count", value);
}

// The setting "gen_stop_bits" (Stop bits) of the test signal generator of the stack "fsk_modems".
static inline int32_t sdrkit_fsk_modems_gen_get_stop_bits(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_stop_bits", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_stop_bits(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_stop_bits", value);
}

// The setting "gen_gap_bits" (Idle bits between bytes) of the test signal generator of the stack "fsk_modems".
static inline int32_t sdrkit_fsk_modems_gen_get_gap_bits(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_gap_bits", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_gap_bits(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_gap_bits", value);
}

// The setting "gen_idle_s" (Idle mark) of the test signal generator of the stack "fsk_modems". A length of time in seconds.
static inline int32_t sdrkit_fsk_modems_gen_get_idle_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_idle_s", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_idle_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_idle_s", value);
}

// The setting "gen_lead_s" (Silence before) of the test signal generator of the stack "fsk_modems". A length of time in seconds.
static inline int32_t sdrkit_fsk_modems_gen_get_lead_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lead_s", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_lead_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lead_s", value);
}

// The setting "gen_tail_s" (Silence after) of the test signal generator of the stack "fsk_modems". A length of time in seconds.
static inline int32_t sdrkit_fsk_modems_gen_get_tail_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tail_s", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_tail_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tail_s", value);
}

// The setting "gen_lower" (Lower sideband) of the test signal generator of the stack "fsk_modems".
static inline int32_t sdrkit_fsk_modems_gen_get_lower(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_lower", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_lower(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_lower", value);
}

// The setting "gen_baud_scale" (Speed factor) of the test signal generator of the stack "fsk_modems". The speed of the test signal times this (a wrong speed for a test of false decodes).
static inline int32_t sdrkit_fsk_modems_gen_get_baud_scale(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud_scale", value);
}
static inline int32_t sdrkit_fsk_modems_gen_set_baud_scale(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud_scale", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "uvb76" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_uvb76_rows(SDRKIT_HANDLE handle,
                                       sdrkit_hfdata_sender_row *rows,
                                       uint32_t capacity,
                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "uvb76.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_uvb76_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_uvb76_event *event) {
  return sdrkit_typed_event(handle, n, "uvb76.event", "hfdata.Uvb76Event", event, sizeof(sdrkit_uvb76_event));
}

// Runs the command "clear" (Clear) of the stack "uvb76" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_uvb76_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "uvb76" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_uvb76_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "uvb76" are the sdrkit_uvb76_gen_*() functions
// after the settings of its receiver.

// The setting "min_buzz_s" (Shortest buzz) of the stack "uvb76". A length of time in seconds. A shorter rise of the modulation is a click and is ignored.
static inline int32_t sdrkit_uvb76_get_min_buzz_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_buzz_s", value);
}
static inline int32_t sdrkit_uvb76_set_min_buzz_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_buzz_s", value);
}

// The setting "long_tx_s" (Longest buzz) of the stack "uvb76". A length of time in seconds. Longer modulation is a long transmission (a voice message is possible).
static inline int32_t sdrkit_uvb76_get_long_tx_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "long_tx_s", value);
}
static inline int32_t sdrkit_uvb76_set_long_tx_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "long_tx_s", value);
}

// The setting "carrier_share" (Carrier line share) of the stack "uvb76". The share of the channel power in the strongest five bins that shows a carrier; without a carrier nothing is timed.
static inline int32_t sdrkit_uvb76_get_carrier_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "carrier_share", value);
}
static inline int32_t sdrkit_uvb76_set_carrier_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "carrier_share", value);
}

// The setting "gen_period_s" (Buzz period) of the test signal generator of the stack "uvb76". A length of time in seconds.
static inline int32_t sdrkit_uvb76_gen_get_period_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_period_s", value);
}
static inline int32_t sdrkit_uvb76_gen_set_period_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_period_s", value);
}

// The setting "gen_buzz_s" (Buzz length) of the test signal generator of the stack "uvb76". A length of time in seconds.
static inline int32_t sdrkit_uvb76_gen_get_buzz_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_buzz_s", value);
}
static inline int32_t sdrkit_uvb76_gen_set_buzz_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_buzz_s", value);
}

// The setting "gen_depth" (Modulation depth) of the test signal generator of the stack "uvb76".
static inline int32_t sdrkit_uvb76_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_uvb76_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// The setting "gen_break_at_s" (Pause at) of the test signal generator of the stack "uvb76". A length of time in seconds. 0: no pause.
static inline int32_t sdrkit_uvb76_gen_get_break_at_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_break_at_s", value);
}
static inline int32_t sdrkit_uvb76_gen_set_break_at_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_break_at_s", value);
}

// The setting "gen_break_s" (Pause) of the test signal generator of the stack "uvb76". A length of time in seconds.
static inline int32_t sdrkit_uvb76_gen_get_break_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_break_s", value);
}
static inline int32_t sdrkit_uvb76_gen_set_break_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_break_s", value);
}

// The setting "gen_voice_at_s" (Voice at) of the test signal generator of the stack "uvb76". A length of time in seconds. 0: no long transmission.
static inline int32_t sdrkit_uvb76_gen_get_voice_at_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_voice_at_s", value);
}
static inline int32_t sdrkit_uvb76_gen_set_voice_at_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_voice_at_s", value);
}

// The setting "gen_voice_s" (Voice length) of the test signal generator of the stack "uvb76". A length of time in seconds.
static inline int32_t sdrkit_uvb76_gen_get_voice_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_voice_s", value);
}
static inline int32_t sdrkit_uvb76_gen_set_voice_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_voice_s", value);
}

// Fills `rows` with the changed rows of the table "rows" of the stack "vlf_msk" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_vlf_msk_rows(SDRKIT_HANDLE handle,
                                         sdrkit_hfdata_sender_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.SenderRow", "rows", rows, sizeof(sdrkit_hfdata_sender_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "vlf.transmitter"
// (Transmitter). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_vlf_msk_vlf_transmitter_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_vlf_msk_vlf_transmitter_event *event) {
  return sdrkit_typed_event(handle, n, "vlf.transmitter", "hfdata.VlfTransmitterEvent", event, sizeof(sdrkit_vlf_msk_vlf_transmitter_event));
}

// Runs the command "clear" (Clear) of the stack "vlf_msk" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_vlf_msk_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "vlf_msk" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_vlf_msk_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "vlf_msk" are the sdrkit_vlf_msk_gen_*() functions
// after the settings of its receiver.

// The setting "report_s" (Report time) of the stack "vlf_msk". A length of time in seconds. A state must hold for two reports before it is taken.
static inline int32_t sdrkit_vlf_msk_get_report_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "report_s", value);
}
static inline int32_t sdrkit_vlf_msk_set_report_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "report_s", value);
}

// The setting "rates" (Standard rates) of the stack "vlf_msk". Unit: bit/s. A measured rate within the tolerance of one of these is shown as it.
static inline int32_t sdrkit_vlf_msk_get_rates(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "rates", value);
}
static inline int32_t sdrkit_vlf_msk_set_rates(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "rates", value);
}

// The setting "rate_tolerance" (Rate tolerance) of the stack "vlf_msk". Unit: %.
static inline int32_t sdrkit_vlf_msk_get_rate_tolerance(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "rate_tolerance", value);
}
static inline int32_t sdrkit_vlf_msk_set_rate_tolerance(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "rate_tolerance", value);
}

// The setting "hint_hz" (Station hint window) of the stack "vlf_msk". A frequency in Hz.
static inline int32_t sdrkit_vlf_msk_get_hint_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hint_hz", value);
}
static inline int32_t sdrkit_vlf_msk_set_hint_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hint_hz", value);
}

// The setting "stations" (Station list) of the stack "vlf_msk". frequency_hz=name pairs separated by semicolons, from the host's data file. Empty: no hint.
static inline int32_t sdrkit_vlf_msk_get_stations(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "stations", value);
}
static inline int32_t sdrkit_vlf_msk_set_stations(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "stations", value);
}

// The setting "gen_rate" (Bit rate) of the test signal generator of the stack "vlf_msk". Unit: bit/s.
static inline int32_t sdrkit_vlf_msk_gen_get_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rate", value);
}
static inline int32_t sdrkit_vlf_msk_gen_set_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rate", value);
}

// The setting "gen_fc_hz" (Carrier offset) of the test signal generator of the stack "vlf_msk". A frequency in Hz.
static inline int32_t sdrkit_vlf_msk_gen_get_fc_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fc_hz", value);
}
static inline int32_t sdrkit_vlf_msk_gen_set_fc_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fc_hz", value);
}

// The setting "gen_from_s" (On from) of the test signal generator of the stack "vlf_msk". A length of time in seconds.
static inline int32_t sdrkit_vlf_msk_gen_get_from_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_from_s", value);
}
static inline int32_t sdrkit_vlf_msk_gen_set_from_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_from_s", value);
}

// The setting "gen_to_s" (Off at) of the test signal generator of the stack "vlf_msk". A length of time in seconds. 0: on to the end.
static inline int32_t sdrkit_vlf_msk_gen_get_to_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_to_s", value);
}
static inline int32_t sdrkit_vlf_msk_gen_set_to_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_to_s", value);
}

// Fills `rows` with the changed rows of the table "telegrams" of the stack "efr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_efr_telegrams_rows(SDRKIT_HANDLE handle,
                                               sdrkit_efr_telegram_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.EfrTelegramRow", "telegrams", rows, sizeof(sdrkit_efr_telegram_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fsk.lock"
// (Sync). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdata_fsk_lock_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdata_fsk_lock_event *event) {
  return sdrkit_typed_event(handle, n, "fsk.lock", "hfdata.FskLockEvent", event, sizeof(sdrkit_hfdata_fsk_lock_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fsk.lost"
// (Sync lost). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdata_fsk_lost_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdata_fsk_lost_event *event) {
  return sdrkit_typed_event(handle, n, "fsk.lost", "hfdata.FskLostEvent", event, sizeof(sdrkit_hfdata_fsk_lost_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "efr.telegram"
// (Telegram). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_efr_telegram_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_efr_telegram_event *event) {
  return sdrkit_typed_event(handle, n, "efr.telegram", "hfdata.EfrTelegramEvent", event, sizeof(sdrkit_efr_telegram_event));
}

// Runs the command "clear" (Clear) of the stack "efr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_efr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "efr" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_efr_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "efr" are the sdrkit_efr_gen_*() functions
// after the settings of its receiver.

// The setting "shift_hz" (Shift) of the stack "efr". A frequency in Hz. 0 takes 340 Hz.
static inline int32_t sdrkit_efr_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "shift_hz", value);
}
static inline int32_t sdrkit_efr_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "shift_hz", value);
}

// The setting "polarity" (Polarity) of the stack "efr". A value of `sdrkit_efr_polarity`.
static inline int32_t sdrkit_efr_get_polarity(SDRKIT_HANDLE handle, sdrkit_efr_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_efr_polarity)index;
  return code;
}
static inline int32_t sdrkit_efr_set_polarity(SDRKIT_HANDLE handle, sdrkit_efr_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "polarity", (int32_t)value);
}

// The setting "baud" (Speed) of the stack "efr". Unit: Bd. Not confirmed by a public document.
static inline int32_t sdrkit_efr_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "baud", value);
}
static inline int32_t sdrkit_efr_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "baud", value);
}

// The setting "max_len" (Longest frame) of the stack "efr". Unit: bytes. Not confirmed by a public document.
static inline int32_t sdrkit_efr_get_max_len(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_len", value);
}
static inline int32_t sdrkit_efr_set_max_len(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_len", value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "efr". A frequency in Hz.
static inline int32_t sdrkit_efr_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_efr_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "gtor.frame"
// (Frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_gtor_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_gtor_frame_event *event) {
  return sdrkit_typed_event(handle, n, "gtor.frame", "hfdata.GtorFrameEvent", event, sizeof(sdrkit_gtor_frame_event));
}

// Runs the command "clear" (Clear) of the stack "gtor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_gtor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "gtor" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_gtor_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gtor" are the sdrkit_gtor_gen_*() functions
// after the settings of its receiver.

// The setting "baud" (Speed) of the stack "gtor". A value of `sdrkit_gtor_baud`.
static inline int32_t sdrkit_gtor_get_baud(SDRKIT_HANDLE handle, sdrkit_gtor_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gtor_baud)index;
  return code;
}
static inline int32_t sdrkit_gtor_set_baud(SDRKIT_HANDLE handle, sdrkit_gtor_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "baud", (int32_t)value);
}

// The setting "shift_hz" (Shift) of the stack "gtor". A frequency in Hz. 0 finds it from the signal.
static inline int32_t sdrkit_gtor_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "shift_hz", value);
}
static inline int32_t sdrkit_gtor_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "shift_hz", value);
}

// The setting "gen_baud" (Speed) of the test signal generator of the stack "gtor". A value of `sdrkit_gtor_gen_baud`. Automatic sends 100 Bd.
static inline int32_t sdrkit_gtor_gen_get_baud(SDRKIT_HANDLE handle, sdrkit_gtor_gen_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gtor_gen_baud)index;
  return code;
}
static inline int32_t sdrkit_gtor_gen_set_baud(SDRKIT_HANDLE handle, sdrkit_gtor_gen_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_baud", (int32_t)value);
}

// The setting "gen_golay" (Golay form) of the test signal generator of the stack "gtor".
static inline int32_t sdrkit_gtor_gen_get_golay(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_golay", value);
}
static inline int32_t sdrkit_gtor_gen_set_golay(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_golay", value);
}

// The setting "gen_shift_hz" (Shift) of the test signal generator of the stack "gtor". A frequency in Hz.
static inline int32_t sdrkit_gtor_gen_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", value);
}
static inline int32_t sdrkit_gtor_gen_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "gtor". A frequency in Hz.
static inline int32_t sdrkit_gtor_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_gtor_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Runs the command "clear" (Clear) of the stack "arqe" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_arqe_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "arqe" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_arqe_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "arqe" are the sdrkit_arqe_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Signal) of the stack "arqe". A value of `sdrkit_arqe_submode`.
static inline int32_t sdrkit_arqe_get_submode(SDRKIT_HANDLE handle, sdrkit_arqe_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_submode)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_submode(SDRKIT_HANDLE handle, sdrkit_arqe_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "baud" (Speed) of the stack "arqe". Unit: Bd. 0 finds it from the candidates of the submode.
static inline int32_t sdrkit_arqe_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "baud", value);
}
static inline int32_t sdrkit_arqe_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "baud", value);
}

// The setting "shift_hz" (Shift) of the stack "arqe". A frequency in Hz. 0 finds it from the signal.
static inline int32_t sdrkit_arqe_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "shift_hz", value);
}
static inline int32_t sdrkit_arqe_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "shift_hz", value);
}

// The setting "polarity" (Polarity) of the stack "arqe". A value of `sdrkit_arqe_polarity`.
static inline int32_t sdrkit_arqe_get_polarity(SDRKIT_HANDLE handle, sdrkit_arqe_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_polarity)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_polarity(SDRKIT_HANDLE handle, sdrkit_arqe_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "polarity", (int32_t)value);
}

// The setting "cycle" (Inversion cycle) of the stack "arqe". A value of `sdrkit_arqe_cycle`. ARQ-E only; ARQ-N inverts no word.
static inline int32_t sdrkit_arqe_get_cycle(SDRKIT_HANDLE handle, sdrkit_arqe_cycle *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "cycle", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_cycle)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_cycle(SDRKIT_HANDLE handle, sdrkit_arqe_cycle value) {
  return sdrkit_typed_setting_set_enum(handle, "cycle", (int32_t)value);
}

// The setting "alphabet" (Figures case) of the stack "arqe". A value of `sdrkit_arqe_alphabet`. The figures of the ITA2 alphabet: US teleprinter or ITU.
static inline int32_t sdrkit_arqe_get_alphabet(SDRKIT_HANDLE handle, sdrkit_arqe_alphabet *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "alphabet", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_alphabet)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_alphabet(SDRKIT_HANDLE handle, sdrkit_arqe_alphabet value) {
  return sdrkit_typed_setting_set_enum(handle, "alphabet", (int32_t)value);
}

// The setting "word_code" (Word code) of the stack "arqe". A value of `sdrkit_arqe_word_code`. Which 7-unit code the signal uses is not confirmed by a public document: an identification unit, ITA2 and odd parity, or ITU-R F.342.
static inline int32_t sdrkit_arqe_get_word_code(SDRKIT_HANDLE handle, sdrkit_arqe_word_code *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "word_code", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_word_code)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_word_code(SDRKIT_HANDLE handle, sdrkit_arqe_word_code value) {
  return sdrkit_typed_setting_set_enum(handle, "word_code", (int32_t)value);
}

// The setting "channels" (Channels) of the stack "arqe". A value of `sdrkit_arqe_channels`. ITU-R F.342: 2 channels send their characters in turn; 4 channels send A and C, then B and D, element by element.
static inline int32_t sdrkit_arqe_get_channels(SDRKIT_HANDLE handle, sdrkit_arqe_channels *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channels", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_channels)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_channels(SDRKIT_HANDLE handle, sdrkit_arqe_channels value) {
  return sdrkit_typed_setting_set_enum(handle, "channels", (int32_t)value);
}

// The setting "channel" (Channel shown) of the stack "arqe". A value of `sdrkit_arqe_channel`. The channel whose text is shown when the signal has 2 or 4 channels.
static inline int32_t sdrkit_arqe_get_channel(SDRKIT_HANDLE handle, sdrkit_arqe_channel *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_channel)index;
  return code;
}
static inline int32_t sdrkit_arqe_set_channel(SDRKIT_HANDLE handle, sdrkit_arqe_channel value) {
  return sdrkit_typed_setting_set_enum(handle, "channel", (int32_t)value);
}

// The setting "gen_baud" (Speed) of the test signal generator of the stack "arqe". Unit: Bd.
static inline int32_t sdrkit_arqe_gen_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud", value);
}
static inline int32_t sdrkit_arqe_gen_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud", value);
}

// The setting "gen_shift_hz" (Shift) of the test signal generator of the stack "arqe". A frequency in Hz.
static inline int32_t sdrkit_arqe_gen_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", value);
}
static inline int32_t sdrkit_arqe_gen_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value);
}

// The setting "gen_cycle" (Inversion cycle) of the test signal generator of the stack "arqe". A value of `sdrkit_arqe_gen_cycle`. Automatic sends 8. ARQ-N inverts no word.
static inline int32_t sdrkit_arqe_gen_get_cycle(SDRKIT_HANDLE handle, sdrkit_arqe_gen_cycle *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_cycle", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_gen_cycle)index;
  return code;
}
static inline int32_t sdrkit_arqe_gen_set_cycle(SDRKIT_HANDLE handle, sdrkit_arqe_gen_cycle value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_cycle", (int32_t)value);
}

// The setting "gen_channels" (Channels) of the test signal generator of the stack "arqe". A value of `sdrkit_arqe_gen_channels`. Automatic sends one. Channel A sends the test text, the others the test text after their name.
static inline int32_t sdrkit_arqe_gen_get_channels(SDRKIT_HANDLE handle, sdrkit_arqe_gen_channels *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_channels", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_gen_channels)index;
  return code;
}
static inline int32_t sdrkit_arqe_gen_set_channels(SDRKIT_HANDLE handle, sdrkit_arqe_gen_channels value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_channels", (int32_t)value);
}

// The setting "gen_word_code" (Word code) of the test signal generator of the stack "arqe". A value of `sdrkit_arqe_gen_word_code`.
static inline int32_t sdrkit_arqe_gen_get_word_code(SDRKIT_HANDLE handle, sdrkit_arqe_gen_word_code *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_word_code", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arqe_gen_word_code)index;
  return code;
}
static inline int32_t sdrkit_arqe_gen_set_word_code(SDRKIT_HANDLE handle, sdrkit_arqe_gen_word_code value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_word_code", (int32_t)value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "arqe". A frequency in Hz.
static inline int32_t sdrkit_arqe_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_arqe_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Runs the command "clear" (Clear) of the stack "autospec" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_autospec_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "autospec" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_autospec_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "autospec" are the sdrkit_autospec_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Signal) of the stack "autospec". A value of `sdrkit_autospec_submode`.
static inline int32_t sdrkit_autospec_get_submode(SDRKIT_HANDLE handle, sdrkit_autospec_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_autospec_submode)index;
  return code;
}
static inline int32_t sdrkit_autospec_set_submode(SDRKIT_HANDLE handle, sdrkit_autospec_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "baud" (Speed) of the stack "autospec". Unit: Bd. 0 finds it from the candidates of the submode.
static inline int32_t sdrkit_autospec_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "baud", value);
}
static inline int32_t sdrkit_autospec_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "baud", value);
}

// The setting "shift_hz" (Shift) of the stack "autospec". A frequency in Hz. 0 finds it from the signal.
static inline int32_t sdrkit_autospec_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "shift_hz", value);
}
static inline int32_t sdrkit_autospec_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "shift_hz", value);
}

// The setting "polarity" (Polarity) of the stack "autospec". A value of `sdrkit_autospec_polarity`.
static inline int32_t sdrkit_autospec_get_polarity(SDRKIT_HANDLE handle, sdrkit_autospec_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_autospec_polarity)index;
  return code;
}
static inline int32_t sdrkit_autospec_set_polarity(SDRKIT_HANDLE handle, sdrkit_autospec_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "polarity", (int32_t)value);
}

// The setting "depth" (Spread depth) of the stack "autospec". A value of `sdrkit_autospec_depth`. SPREAD only.
static inline int32_t sdrkit_autospec_get_depth(SDRKIT_HANDLE handle, sdrkit_autospec_depth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "depth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_autospec_depth)index;
  return code;
}
static inline int32_t sdrkit_autospec_set_depth(SDRKIT_HANDLE handle, sdrkit_autospec_depth value) {
  return sdrkit_typed_setting_set_enum(handle, "depth", (int32_t)value);
}

// The setting "alphabet" (Figures case) of the stack "autospec". A value of `sdrkit_autospec_alphabet`. The figures of the ITA2 alphabet: US teleprinter or ITU.
static inline int32_t sdrkit_autospec_get_alphabet(SDRKIT_HANDLE handle, sdrkit_autospec_alphabet *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "alphabet", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_autospec_alphabet)index;
  return code;
}
static inline int32_t sdrkit_autospec_set_alphabet(SDRKIT_HANDLE handle, sdrkit_autospec_alphabet value) {
  return sdrkit_typed_setting_set_enum(handle, "alphabet", (int32_t)value);
}

// The setting "idle_code" (Idle character) of the stack "autospec". The ITA2 code sent while idle; a run of three or more is not shown. Not confirmed by a public document.
static inline int32_t sdrkit_autospec_get_idle_code(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "idle_code", value);
}
static inline int32_t sdrkit_autospec_set_idle_code(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "idle_code", value);
}

// The setting "gen_baud" (Speed) of the test signal generator of the stack "autospec". Unit: Bd.
static inline int32_t sdrkit_autospec_gen_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud", value);
}
static inline int32_t sdrkit_autospec_gen_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud", value);
}

// The setting "gen_shift_hz" (Shift) of the test signal generator of the stack "autospec". A frequency in Hz. 0 sends 270 Hz for AUTOSPEC and 410 Hz for SPREAD.
static inline int32_t sdrkit_autospec_gen_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", value);
}
static inline int32_t sdrkit_autospec_gen_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value);
}

// The setting "gen_depth" (Spread depth) of the test signal generator of the stack "autospec". A value of `sdrkit_autospec_gen_depth`. Automatic sends 11.
static inline int32_t sdrkit_autospec_gen_get_depth(SDRKIT_HANDLE handle, sdrkit_autospec_gen_depth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_depth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_autospec_gen_depth)index;
  return code;
}
static inline int32_t sdrkit_autospec_gen_set_depth(SDRKIT_HANDLE handle, sdrkit_autospec_gen_depth value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_depth", (int32_t)value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "autospec". A frequency in Hz.
static inline int32_t sdrkit_autospec_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_autospec_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "piccolo.pairs"
// (Tone pairs). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_piccolo_pairs_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_piccolo_pairs_event *event) {
  return sdrkit_typed_event(handle, n, "piccolo.pairs", "hfdata.PiccoloPairsEvent", event, sizeof(sdrkit_piccolo_pairs_event));
}

// Runs the command "clear" (Clear) of the stack "piccolo" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_piccolo_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "piccolo" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_piccolo_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "piccolo" are the sdrkit_piccolo_gen_*() functions
// after the settings of its receiver.

// The setting "alphabet" (Figures case) of the stack "piccolo". A value of `sdrkit_piccolo_alphabet`. The figures of the ITA2 alphabet: US teleprinter or ITU.
static inline int32_t sdrkit_piccolo_get_alphabet(SDRKIT_HANDLE handle, sdrkit_piccolo_alphabet *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "alphabet", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_piccolo_alphabet)index;
  return code;
}
static inline int32_t sdrkit_piccolo_set_alphabet(SDRKIT_HANDLE handle, sdrkit_piccolo_alphabet value) {
  return sdrkit_typed_setting_set_enum(handle, "alphabet", (int32_t)value);
}

// The setting "spacing_hz" (Tone spacing) of the stack "piccolo". A frequency in Hz. Not confirmed by a public document.
static inline int32_t sdrkit_piccolo_get_spacing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "spacing_hz", value);
}
static inline int32_t sdrkit_piccolo_set_spacing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "spacing_hz", value);
}

// The setting "tone_ms" (Tone length) of the stack "piccolo". Unit: ms. Not confirmed by a public document.
static inline int32_t sdrkit_piccolo_get_tone_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tone_ms", value);
}
static inline int32_t sdrkit_piccolo_set_tone_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tone_ms", value);
}

// The setting "gen_base_hz" (Tone A) of the test signal generator of the stack "piccolo". A frequency in Hz.
static inline int32_t sdrkit_piccolo_gen_get_base_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_base_hz", value);
}
static inline int32_t sdrkit_piccolo_gen_set_base_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_base_hz", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "coquelet.pairs"
// (Tone pairs). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_coquelet_pairs_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_coquelet_pairs_event *event) {
  return sdrkit_typed_event(handle, n, "coquelet.pairs", "hfdata.CoqueletPairsEvent", event, sizeof(sdrkit_coquelet_pairs_event));
}

// Runs the command "clear" (Clear) of the stack "coquelet" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_coquelet_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "coquelet" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_coquelet_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "coquelet" are the sdrkit_coquelet_gen_*() functions
// after the settings of its receiver.

// The setting "tone_ms" (Tone length) of the stack "coquelet". A value of `sdrkit_coquelet_tone_ms`.
static inline int32_t sdrkit_coquelet_get_tone_ms(SDRKIT_HANDLE handle, sdrkit_coquelet_tone_ms *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tone_ms", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_coquelet_tone_ms)index;
  return code;
}
static inline int32_t sdrkit_coquelet_set_tone_ms(SDRKIT_HANDLE handle, sdrkit_coquelet_tone_ms value) {
  return sdrkit_typed_setting_set_enum(handle, "tone_ms", (int32_t)value);
}

// The setting "alphabet" (Figures case) of the stack "coquelet". A value of `sdrkit_coquelet_alphabet`. The figures of the ITA2 alphabet: US teleprinter or ITU.
static inline int32_t sdrkit_coquelet_get_alphabet(SDRKIT_HANDLE handle, sdrkit_coquelet_alphabet *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "alphabet", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_coquelet_alphabet)index;
  return code;
}
static inline int32_t sdrkit_coquelet_set_alphabet(SDRKIT_HANDLE handle, sdrkit_coquelet_alphabet value) {
  return sdrkit_typed_setting_set_enum(handle, "alphabet", (int32_t)value);
}

// The setting "spacing_hz" (Tone spacing) of the stack "coquelet". A frequency in Hz. Not confirmed by a public document.
static inline int32_t sdrkit_coquelet_get_spacing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "spacing_hz", value);
}
static inline int32_t sdrkit_coquelet_set_spacing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "spacing_hz", value);
}

// The setting "gen_tone_ms" (Tone length) of the test signal generator of the stack "coquelet". A value of `sdrkit_coquelet_gen_tone_ms`. Automatic sends 50 ms.
static inline int32_t sdrkit_coquelet_gen_get_tone_ms(SDRKIT_HANDLE handle, sdrkit_coquelet_gen_tone_ms *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_tone_ms", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_coquelet_gen_tone_ms)index;
  return code;
}
static inline int32_t sdrkit_coquelet_gen_set_tone_ms(SDRKIT_HANDLE handle, sdrkit_coquelet_gen_tone_ms value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_tone_ms", (int32_t)value);
}

// The setting "gen_base_hz" (Tone 1) of the test signal generator of the stack "coquelet". A frequency in Hz.
static inline int32_t sdrkit_coquelet_gen_get_base_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_base_hz", value);
}
static inline int32_t sdrkit_coquelet_gen_set_base_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_base_hz", value);
}

// Fills `rows` with the changed rows of the table "pdus" of the stack "ale3g" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale3g_pdus_rows(SDRKIT_HANDLE handle,
                                            sdrkit_ale3g_pdu_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.Ale3gPduRow", "pdus", rows, sizeof(sdrkit_ale3g_pdu_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ale3g.pdu"
// (PDU). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ale3g_pdu_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ale3g_pdu_event *event) {
  return sdrkit_typed_event(handle, n, "ale3g.pdu", "hfdata.Ale3gPduEvent", event, sizeof(sdrkit_ale3g_pdu_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ale3g.datagram"
// (Datagram). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ale3g_datagram_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ale3g_datagram_event *event) {
  return sdrkit_typed_event(handle, n, "ale3g.datagram", "hfdata.Ale3gDatagramEvent", event, sizeof(sdrkit_ale3g_datagram_event));
}

// Runs the command "clear" (Clear) of the stack "ale3g" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ale3g_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ale3g" are the sdrkit_ale3g_gen_*() functions
// after the settings of its receiver.

// The setting "bw2_taps" (BW2 PN taps) of the stack "ale3g". The register bits (as a 16-bit mask) whose parity is the new bit of the BW2 PN generator. The default is Figure C-12 as drawn (bits 15, 14, 5 and 0); no recording has confirmed it.
static inline int32_t sdrkit_ale3g_get_bw2_taps(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "bw2_taps", value);
}
static inline int32_t sdrkit_ale3g_set_bw2_taps(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "bw2_taps", value);
}

// The setting "gen_set" (Bursts) of the test signal generator of the stack "ale3g". A value of `sdrkit_ale3g_gen_set`.
static inline int32_t sdrkit_ale3g_gen_get_set(SDRKIT_HANDLE handle, sdrkit_ale3g_gen_set *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_set", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ale3g_gen_set)index;
  return code;
}
static inline int32_t sdrkit_ale3g_gen_set_set(SDRKIT_HANDLE handle, sdrkit_ale3g_gen_set value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_set", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "s4481" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_s4481_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_s4481_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.S4481ChannelRow", "channels", rows, sizeof(sdrkit_s4481_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "s4481.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_s4481_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_s4481_event *event) {
  return sdrkit_typed_event(handle, n, "s4481.event", "hfdata.S4481Event", event, sizeof(sdrkit_s4481_event));
}

// Runs the command "clear" (Clear) of the stack "s4481" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_s4481_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "s4481" are the sdrkit_s4481_gen_*() functions
// after the settings of its receiver.

// The setting "channel_mode" (Channels) of the stack "s4481". A value of `sdrkit_s4481_channel_mode`.
static inline int32_t sdrkit_s4481_get_channel_mode(SDRKIT_HANDLE handle, sdrkit_s4481_channel_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4481_channel_mode)index;
  return code;
}
static inline int32_t sdrkit_s4481_set_channel_mode(SDRKIT_HANDLE handle, sdrkit_s4481_channel_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "channel_mode", (int32_t)value);
}

// The setting "baud" (Speed) of the stack "s4481". A value of `sdrkit_s4481_baud`.
static inline int32_t sdrkit_s4481_get_baud(SDRKIT_HANDLE handle, sdrkit_s4481_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4481_baud)index;
  return code;
}
static inline int32_t sdrkit_s4481_set_baud(SDRKIT_HANDLE handle, sdrkit_s4481_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "baud", (int32_t)value);
}

// The setting "alphabet" (Figures case) of the stack "s4481". A value of `sdrkit_s4481_alphabet`.
static inline int32_t sdrkit_s4481_get_alphabet(SDRKIT_HANDLE handle, sdrkit_s4481_alphabet *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "alphabet", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4481_alphabet)index;
  return code;
}
static inline int32_t sdrkit_s4481_set_alphabet(SDRKIT_HANDLE handle, sdrkit_s4481_alphabet value) {
  return sdrkit_typed_setting_set_enum(handle, "alphabet", (int32_t)value);
}

// The setting "gen_multi" (Multi-channel) of the test signal generator of the stack "s4481".
static inline int32_t sdrkit_s4481_gen_get_multi(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_multi", value);
}
static inline int32_t sdrkit_s4481_gen_set_multi(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_multi", value);
}

// The setting "gen_baud" (Speed) of the test signal generator of the stack "s4481". A value of `sdrkit_s4481_gen_baud`. Automatic sends 75 Bd.
static inline int32_t sdrkit_s4481_gen_get_baud(SDRKIT_HANDLE handle, sdrkit_s4481_gen_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4481_gen_baud)index;
  return code;
}
static inline int32_t sdrkit_s4481_gen_set_baud(SDRKIT_HANDLE handle, sdrkit_s4481_gen_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_baud", (int32_t)value);
}

// The setting "gen_single_mark_hz" (Single-channel mark) of the test signal generator of the stack "s4481". A frequency in Hz.
static inline int32_t sdrkit_s4481_gen_get_single_mark_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_single_mark_hz", value);
}
static inline int32_t sdrkit_s4481_gen_set_single_mark_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_single_mark_hz", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "link11" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_link11_messages_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_link11_message_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.Link11MessageRow", "messages", rows, sizeof(sdrkit_link11_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "link11.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_link11_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_link11_event *event) {
  return sdrkit_typed_event(handle, n, "link11.event", "hfdata.Link11Event", event, sizeof(sdrkit_link11_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "link11.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_link11_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_link11_message_event *event) {
  return sdrkit_typed_event(handle, n, "link11.message", "hfdata.Link11MessageEvent", event, sizeof(sdrkit_link11_message_event));
}

// Runs the command "clear" (Clear) of the stack "link11" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_link11_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "link11" are the sdrkit_link11_gen_*() functions
// after the settings of its receiver.

// The setting "frame_rate" (Frame rate) of the stack "link11". A value of `sdrkit_link11_frame_rate`.
static inline int32_t sdrkit_link11_get_frame_rate(SDRKIT_HANDLE handle, sdrkit_link11_frame_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_link11_frame_rate)index;
  return code;
}
static inline int32_t sdrkit_link11_set_frame_rate(SDRKIT_HANDLE handle, sdrkit_link11_frame_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_rate", (int32_t)value);
}

// The setting "gen_rate" (Frame rate) of the test signal generator of the stack "link11". A value of `sdrkit_link11_gen_rate`. Automatic sends fast frames.
static inline int32_t sdrkit_link11_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_link11_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_link11_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_link11_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_link11_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// The setting "gen_offset_deg" (Phase step offset) of the test signal generator of the stack "link11". Unit: deg.
static inline int32_t sdrkit_link11_gen_get_offset_deg(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_offset_deg", value);
}
static inline int32_t sdrkit_link11_gen_set_offset_deg(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_offset_deg", value);
}

// The setting "gen_preamble_frames" (Preamble frames) of the test signal generator of the stack "link11".
static inline int32_t sdrkit_link11_gen_get_preamble_frames(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_preamble_frames", value);
}
static inline int32_t sdrkit_link11_gen_set_preamble_frames(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_preamble_frames", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "cis_hf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cis_hf_messages_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_cis_hf_message_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.CisHfMessageRow", "messages", rows, sizeof(sdrkit_cis_hf_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "cis_hf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cis_hf_carriers_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_cis_hf_carrier_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.CisHfCarrierRow", "carriers", rows, sizeof(sdrkit_cis_hf_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cis.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cis_hf_cis_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cis_hf_cis_message_event *event) {
  return sdrkit_typed_event(handle, n, "cis.message", "hfdata.CisMessageEvent", event, sizeof(sdrkit_cis_hf_cis_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cis.ident"
// (Identified). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cis_hf_cis_ident_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cis_hf_cis_ident_event *event) {
  return sdrkit_typed_event(handle, n, "cis.ident", "hfdata.CisIdentEvent", event, sizeof(sdrkit_cis_hf_cis_ident_event));
}

// Runs the command "clear" (Clear) of the stack "cis_hf" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cis_hf_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "cis_hf" on the session of `handle`. Searches the signal again; keeps the text and the tables.
static inline int32_t sdrkit_cis_hf_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cis_hf" are the sdrkit_cis_hf_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Signal) of the stack "cis_hf". A value of `sdrkit_cis_hf_submode`.
static inline int32_t sdrkit_cis_hf_get_submode(SDRKIT_HANDLE handle, sdrkit_cis_hf_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cis_hf_submode)index;
  return code;
}
static inline int32_t sdrkit_cis_hf_set_submode(SDRKIT_HANDLE handle, sdrkit_cis_hf_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "baud" (Speed) of the stack "cis_hf". Unit: Bd. bee and cis14: 0 tries every candidate speed; a speed replaces them.
static inline int32_t sdrkit_cis_hf_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "baud", value);
}
static inline int32_t sdrkit_cis_hf_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "baud", value);
}

// The setting "shift_hz" (Shift) of the stack "cis_hf". A frequency in Hz. bee and cis14: 0 finds it from the signal.
static inline int32_t sdrkit_cis_hf_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "shift_hz", value);
}
static inline int32_t sdrkit_cis_hf_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "shift_hz", value);
}

// The setting "roll_off" (CIS-3000 roll-off) of the stack "cis_hf". The pulse of CIS-3000 is not published; a root-raised cosine of this roll-off is assumed.
static inline int32_t sdrkit_cis_hf_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_cis_hf_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// The setting "gen_variant" (BEE variant) of the test signal generator of the stack "cis_hf". A value of `sdrkit_cis_hf_gen_variant`.
static inline int32_t sdrkit_cis_hf_gen_get_variant(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cis_hf_gen_variant)index;
  return code;
}
static inline int32_t sdrkit_cis_hf_gen_set_variant(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_variant", (int32_t)value);
}

// The setting "gen_lower_one" (BEE 1 on the lower tone) of the test signal generator of the stack "cis_hf".
static inline int32_t sdrkit_cis_hf_gen_get_lower_one(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_lower_one", value);
}
static inline int32_t sdrkit_cis_hf_gen_set_lower_one(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_lower_one", value);
}

// The setting "gen_centre_hz" (Centre) of the test signal generator of the stack "cis_hf". A frequency in Hz. 0 takes 1700 Hz for BEE and 1500 Hz for CIS-14.
static inline int32_t sdrkit_cis_hf_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_cis_hf_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// The setting "gen_shift_hz" (CIS-14 shift) of the test signal generator of the stack "cis_hf". A frequency in Hz.
static inline int32_t sdrkit_cis_hf_gen_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", value);
}
static inline int32_t sdrkit_cis_hf_gen_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value);
}

// The setting "gen_order" (CIS-12 order) of the test signal generator of the stack "cis_hf". A value of `sdrkit_cis_hf_gen_order`.
static inline int32_t sdrkit_cis_hf_gen_get_order(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_order *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_order", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cis_hf_gen_order)index;
  return code;
}
static inline int32_t sdrkit_cis_hf_gen_set_order(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_order value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_order", (int32_t)value);
}

// The setting "gen_rate" (CIS-36 tone rate) of the test signal generator of the stack "cis_hf". A value of `sdrkit_cis_hf_gen_rate`.
static inline int32_t sdrkit_cis_hf_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cis_hf_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_cis_hf_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// The setting "gen_ofdm" (OFDM signal) of the test signal generator of the stack "cis_hf". A value of `sdrkit_cis_hf_gen_ofdm`.
static inline int32_t sdrkit_cis_hf_gen_get_ofdm(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_ofdm *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_ofdm", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cis_hf_gen_ofdm)index;
  return code;
}
static inline int32_t sdrkit_cis_hf_gen_set_ofdm(SDRKIT_HANDLE handle, sdrkit_cis_hf_gen_ofdm value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_ofdm", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "candidates" of the stack "hfid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfid_candidates_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_hfid_candidate_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfidCandidateRow", "candidates", rows, sizeof(sdrkit_hfid_candidate_row), capacity, info);
}

// The settings of the test signal generator of the stack "hfid" are the sdrkit_hfid_gen_*() functions
// after the settings of its receiver.

// The setting "gen_calls" (Calls) of the test signal generator of the stack "hfid". A value of `sdrkit_hfid_gen_calls`.
static inline int32_t sdrkit_hfid_gen_get_calls(SDRKIT_HANDLE handle, sdrkit_hfid_gen_calls *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_calls", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hfid_gen_calls)index;
  return code;
}
static inline int32_t sdrkit_hfid_gen_set_calls(SDRKIT_HANDLE handle, sdrkit_hfid_gen_calls value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_calls", (int32_t)value);
}

// The setting "gen_ber" (Channel bit errors) of the test signal generator of the stack "hfid". The share of channel bits turned over before they become tones.
static inline int32_t sdrkit_hfid_gen_get_ber(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ber", value);
}
static inline int32_t sdrkit_hfid_gen_set_ber(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ber", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "ale" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale_events_rows(SDRKIT_HANDLE handle,
                                            sdrkit_ale_event_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.AleEventRow", "events", rows, sizeof(sdrkit_ale_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "ale" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ale_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.AleStationRow", "stations", rows, sizeof(sdrkit_ale_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "words" of the stack "ale" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale_words_rows(SDRKIT_HANDLE handle,
                                           sdrkit_ale_word_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.AleWordRow", "words", rows, sizeof(sdrkit_ale_word_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "learned_nets" of the stack "ale" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale_learned_nets_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_ale_net_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.AleNetRow", "learned_nets", rows, sizeof(sdrkit_ale_net_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "lqa" of the stack "ale" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale_lqa_rows(SDRKIT_HANDLE handle,
                                         sdrkit_ale_lqa_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.AleLqaRow", "lqa", rows, sizeof(sdrkit_ale_lqa_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "ale" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ale_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ale_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.AleMessageRow", "messages", rows, sizeof(sdrkit_ale_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ale.call"
// (ALE call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ale_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ale_call_event *event) {
  return sdrkit_typed_event(handle, n, "ale.call", "hfdata.AleCallEvent", event, sizeof(sdrkit_ale_call_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ale.tx"
// (ALE transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ale_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ale_tx_event *event) {
  return sdrkit_typed_event(handle, n, "ale.tx", "hfdata.AleTxEvent", event, sizeof(sdrkit_ale_tx_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ale.link"
// (ALE link). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ale_link_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ale_link_event *event) {
  return sdrkit_typed_event(handle, n, "ale.link", "hfdata.AleLinkEvent", event, sizeof(sdrkit_ale_link_event));
}

// Runs the command "clear" (Clear) of the stack "ale" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ale_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ale" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ale_remove(SDRKIT_HANDLE handle, const sdrkit_ale_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfdata.AleStationArgs", args, NULL, NULL, 0);
}

// Runs the command "call" (Call) of the stack "ale" on the session of `handle`. Sends one individual call on this channel: the called address in TO words the calling cycle repeats times, then the own address in TIS words. Transmit must be on and the own address set. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ale_call(SDRKIT_HANDLE handle, const sdrkit_ale_call_args *args) {
  return sdrkit_typed_command(handle, "call", "hfdata.AleCallArgs", args, NULL, NULL, 0);
}

// Runs the command "sound" (Sound) of the stack "ale" on the session of `handle`. Sends one sounding on this channel: the own address in TWS or TIS words, the calling cycle repeats times. Transmit must be on and the own address set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ale_sound(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "sound", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "ale" on the session of `handle`. Ends the call or the sounding at once.
static inline int32_t sdrkit_ale_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "amd" (Send AMD text) of the stack "ale" on the session of `handle`. Sends an AMD text. To the linked station: the called address twice, the text, the own address in TWS words.          To another station: a call with the text before the own address, which starts the three-way handshake.          Transmit must be on and the own address set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_ale_amd(SDRKIT_HANDLE handle, const sdrkit_ale_amd_args *args) {
  return sdrkit_typed_command(handle, "amd", "hfdata.AleAmdArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ale" are the sdrkit_ale_gen_*() functions
// after the settings of its receiver.

// The setting "nets" (Net addresses) of the stack "ale". Net addresses you know, separated by commas. Calls to them are net calls at once. Others are found when a station other than the one called answers.
static inline int32_t sdrkit_ale_get_nets(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "nets", value);
}
static inline int32_t sdrkit_ale_set_nets(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "nets", value);
}

// The setting "listen" (Listen) of the stack "ale". Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way.
static inline int32_t sdrkit_ale_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_ale_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "demod" (Demodulator) of the stack "ale". A value of `sdrkit_ale_demod`. USB below 30 MHz, NFM above (Auto). The decoders see the same audio whether or not you listen.
static inline int32_t sdrkit_ale_get_demod(SDRKIT_HANDLE handle, sdrkit_ale_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ale_demod)index;
  return code;
}
static inline int32_t sdrkit_ale_set_demod(SDRKIT_HANDLE handle, sdrkit_ale_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "self_address" (Own address) of the stack "ale". 1 to 15 characters of A-Z and 0-9, sent in the TIS or TWS words. Empty refuses call and sound.
static inline int32_t sdrkit_ale_get_self_address(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "self_address", value);
}
static inline int32_t sdrkit_ale_set_self_address(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "self_address", value);
}

// The setting "call_repeats" (Calling cycle repeats) of the stack "ale". How many times a call sends the called address before the own address, and a sounding the own address. 6 is the figure of the specification for one channel.
static inline int32_t sdrkit_ale_get_call_repeats(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "call_repeats", value);
}
static inline int32_t sdrkit_ale_set_call_repeats(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "call_repeats", value);
}

// The setting "sounding" (Sounding word) of the stack "ale". A value of `sdrkit_ale_sounding`.
static inline int32_t sdrkit_ale_get_sounding(SDRKIT_HANDLE handle, sdrkit_ale_sounding *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sounding", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ale_sounding)index;
  return code;
}
static inline int32_t sdrkit_ale_set_sounding(SDRKIT_HANDLE handle, sdrkit_ale_sounding value) {
  return sdrkit_typed_setting_set_enum(handle, "sounding", (int32_t)value);
}

// The setting "scan_channels" (Scanned channels) of the stack "ale". The channels in the scan list of the called station. A call first sends the called address for the scan time (channels x dwell), so that the station finds the call when it next listens on this channel. 0 sends no scanning part.
static inline int32_t sdrkit_ale_get_scan_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "scan_channels", value);
}
static inline int32_t sdrkit_ale_set_scan_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "scan_channels", value);
}

// The setting "scan_dwell_s" (Dwell on a channel) of the stack "ale". Unit: s. How long the called station listens on each channel of its scan: 0.5 s at 2 channels a second, 0.2 s at 5.
static inline int32_t sdrkit_ale_get_scan_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_dwell_s", value);
}
static inline int32_t sdrkit_ale_set_scan_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_dwell_s", value);
}

// The setting "auto_answer" (Answer calls) of the stack "ale". Answers a call to the own address that ends in TIS with the response, and so completes the three-way handshake, with no key press. Off at the start, so that nothing is sent unattended. The engine's transmit switch must be on.
static inline int32_t sdrkit_ale_get_auto_answer(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_answer", value);
}
static inline int32_t sdrkit_ale_set_auto_answer(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_answer", value);
}

// The setting "lqa" (Exchange link quality) of the stack "ale". A call carries the LQA command, so that the response carries the figures of the call as heard; the acknowledgement then carries the figures of the response. Answers carry them also when the other station asks.
static inline int32_t sdrkit_ale_get_lqa(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "lqa", value);
}
static inline int32_t sdrkit_ale_set_lqa(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "lqa", value);
}

// The setting "answer_wait_s" (Wait for an answer) of the stack "ale". Unit: s. The longest time from the end of a call or a response to the start of its answer. The other station hears the end of a transmission two words (0.784 s) after its last word.
static inline int32_t sdrkit_ale_get_answer_wait_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "answer_wait_s", value);
}
static inline int32_t sdrkit_ale_set_answer_wait_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "answer_wait_s", value);
}

// The setting "gen_calls" (Calls) of the test signal generator of the stack "ale". A value of `sdrkit_ale_gen_calls`.
static inline int32_t sdrkit_ale_gen_get_calls(SDRKIT_HANDLE handle, sdrkit_ale_gen_calls *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_calls", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ale_gen_calls)index;
  return code;
}
static inline int32_t sdrkit_ale_gen_set_calls(SDRKIT_HANDLE handle, sdrkit_ale_gen_calls value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_calls", (int32_t)value);
}

// The setting "gen_ber" (Channel bit errors) of the test signal generator of the stack "ale". The share of channel bits turned over before they become tones.
static inline int32_t sdrkit_ale_gen_get_ber(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ber", value);
}
static inline int32_t sdrkit_ale_gen_set_ber(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ber", value);
}

// Fills `rows` with the changed rows of the table "radars" of the stack "hfradar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfradar_radars_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hfradar_radar_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfradarRadarRow", "radars", rows, sizeof(sdrkit_hfradar_radar_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "heard" of the stack "hfradar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfradar_heard_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfradar_heard_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfradarHeardRow", "heard", rows, sizeof(sdrkit_hfradar_heard_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bands" of the stack "hfradar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfradar_bands_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hfradar_band_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfradarBandRow", "bands", rows, sizeof(sdrkit_hfradar_band_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfradar.radar_heard"
// (Radar heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfradar_radar_heard_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfradar_radar_heard_event *event) {
  return sdrkit_typed_event(handle, n, "hfradar.radar_heard", "hfdata.HfradarRadarHeardEvent", event, sizeof(sdrkit_hfradar_radar_heard_event));
}

// Runs the command "clear" (Clear) of the stack "hfradar" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_hfradar_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "band.tune" (Tune to a band) of the stack "hfradar" on the session of `handle`. Asks the host to tune to the middle of an oceanographic radar band. The radio takes in one part of the band at a time (1.8 MHz at 2 Msps, so a whole band at once from about 12 Msps for 12 - 14 MHz): a wide radio sees all radars of a band together, a narrow one needs the scan. Refused while the scan is on.
static inline int32_t sdrkit_hfradar_band_tune(SDRKIT_HANDLE handle, const sdrkit_hfradar_band_args *args) {
  return sdrkit_typed_command(handle, "band.tune", "hfdata.HfradarBandArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hfradar" are the sdrkit_hfradar_gen_*() functions
// after the settings of its receiver.

// The setting "scan" (Scan the radar bands) of the stack "hfradar". Step through the radar bands in parts that the sample rate takes in, and listen to each for the dwell time. A radar heard on one part is not heard on the others meanwhile. Off: one position.
static inline int32_t sdrkit_hfradar_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_hfradar_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "dwell_s" (Dwell) of the stack "hfradar". A length of time in seconds.
static inline int32_t sdrkit_hfradar_get_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell_s", value);
}
static inline int32_t sdrkit_hfradar_set_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell_s", value);
}

// The setting "gen_signal" (Signal) of the test signal generator of the stack "hfradar". A value of `sdrkit_hfradar_gen_signal`.
static inline int32_t sdrkit_hfradar_gen_get_signal(SDRKIT_HANDLE handle, sdrkit_hfradar_gen_signal *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hfradar_gen_signal)index;
  return code;
}
static inline int32_t sdrkit_hfradar_gen_set_signal(SDRKIT_HANDLE handle, sdrkit_hfradar_gen_signal value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_signal", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "pactor2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor2_carriers_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_hfdata_pactorhf_carrier_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.PactorhfCarrierRow", "carriers", rows, sizeof(sdrkit_hfdata_pactorhf_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "pactor2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor2_events_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hfdata_pactorhf_event_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.PactorhfEventRow", "events", rows, sizeof(sdrkit_hfdata_pactorhf_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "pactorhf.analysis"
// (PACTOR signal). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfdata_pactorhf_analysis_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfdata_pactorhf_analysis_event *event) {
  return sdrkit_typed_event(handle, n, "pactorhf.analysis", "hfdata.PactorhfAnalysisEvent", event, sizeof(sdrkit_hfdata_pactorhf_analysis_event));
}

// Runs the command "clear" (Clear) of the stack "pactor2" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_pactor2_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "analysis_interval_s" (Analysis interval) of the stack "pactor2". A length of time in seconds.
static inline int32_t sdrkit_pactor2_get_analysis_interval_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", value);
}
static inline int32_t sdrkit_pactor2_set_analysis_interval_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "pactor3" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor3_carriers_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_hfdata_pactorhf_carrier_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.PactorhfCarrierRow", "carriers", rows, sizeof(sdrkit_hfdata_pactorhf_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "pactor3" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor3_events_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hfdata_pactorhf_event_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.PactorhfEventRow", "events", rows, sizeof(sdrkit_hfdata_pactorhf_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "pactor3" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_pactor3_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "pactor3" are the sdrkit_pactor3_gen_*() functions
// after the settings of its receiver.

// The setting "analysis_interval_s" (Analysis interval) of the stack "pactor3". A length of time in seconds.
static inline int32_t sdrkit_pactor3_get_analysis_interval_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", value);
}
static inline int32_t sdrkit_pactor3_set_analysis_interval_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value);
}

// The setting "gen_level" (Speed level) of the test signal generator of the stack "pactor3".
static inline int32_t sdrkit_pactor3_gen_get_level(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_level", value);
}
static inline int32_t sdrkit_pactor3_gen_set_level(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_level", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "pactor4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor4_carriers_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_hfdata_pactorhf_carrier_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.PactorhfCarrierRow", "carriers", rows, sizeof(sdrkit_hfdata_pactorhf_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "pactor4" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor4_events_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hfdata_pactorhf_event_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.PactorhfEventRow", "events", rows, sizeof(sdrkit_hfdata_pactorhf_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "pactor4" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_pactor4_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "pactor4" are the sdrkit_pactor4_gen_*() functions
// after the settings of its receiver.

// The setting "analysis_interval_s" (Analysis interval) of the stack "pactor4". A length of time in seconds.
static inline int32_t sdrkit_pactor4_get_analysis_interval_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", value);
}
static inline int32_t sdrkit_pactor4_set_analysis_interval_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value);
}

// The setting "gen_constellation" (Constellation) of the test signal generator of the stack "pactor4". A value of `sdrkit_pactor4_gen_constellation`.
static inline int32_t sdrkit_pactor4_gen_get_constellation(SDRKIT_HANDLE handle, sdrkit_pactor4_gen_constellation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pactor4_gen_constellation)index;
  return code;
}
static inline int32_t sdrkit_pactor4_gen_set_constellation(SDRKIT_HANDLE handle, sdrkit_pactor4_gen_constellation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_constellation", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "pactor1" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor1_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_pactor1_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.Pactor1FrameRow", "frames", rows, sizeof(sdrkit_pactor1_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "pactor1" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_pactor1_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_pactor1_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.Pactor1StationRow", "stations", rows, sizeof(sdrkit_pactor1_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "pactor.frame"
// (PACTOR-I frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_pactor1_pactor_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_pactor1_pactor_frame_event *event) {
  return sdrkit_typed_event(handle, n, "pactor.frame", "hfdata.PactorFrameEvent", event, sizeof(sdrkit_pactor1_pactor_frame_event));
}

// Runs the command "clear" (Clear) of the stack "pactor1" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_pactor1_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "pactor1" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_pactor1_remove(SDRKIT_HANDLE handle, const sdrkit_pactor1_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "hfdata.Pactor1StationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "pactor1" are the sdrkit_pactor1_gen_*() functions
// after the settings of its receiver.

// The setting "listen" (Listen) of the stack "pactor1". Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way.
static inline int32_t sdrkit_pactor1_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_pactor1_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "demod" (Demodulator) of the stack "pactor1". A value of `sdrkit_pactor1_demod`. USB below 30 MHz, NFM above (Auto). The decoders see the same audio whether or not you listen.
static inline int32_t sdrkit_pactor1_get_demod(SDRKIT_HANDLE handle, sdrkit_pactor1_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pactor1_demod)index;
  return code;
}
static inline int32_t sdrkit_pactor1_set_demod(SDRKIT_HANDLE handle, sdrkit_pactor1_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "huffman_table" (Huffman table) of the stack "pactor1". The full table of the public ARRL PACTOR page (Annex, section 9): one entry a line, the character or its decimal code, a tab, the code bits. Empty: the built-in partial table. Unknown codes show as ~.
static inline int32_t sdrkit_pactor1_get_huffman_table(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "huffman_table", value);
}
static inline int32_t sdrkit_pactor1_set_huffman_table(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "huffman_table", value);
}

// The setting "gen_script" (Script) of the test signal generator of the stack "pactor1". A value of `sdrkit_pactor1_gen_script`.
static inline int32_t sdrkit_pactor1_gen_get_script(SDRKIT_HANDLE handle, sdrkit_pactor1_gen_script *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_script", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_pactor1_gen_script)index;
  return code;
}
static inline int32_t sdrkit_pactor1_gen_set_script(SDRKIT_HANDLE handle, sdrkit_pactor1_gen_script value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_script", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "beacons" of the stack "hfbeacons" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfbeacons_beacons_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_hfbeacons_beacon_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfbeaconsBeaconRow", "beacons", rows, sizeof(sdrkit_hfbeacons_beacon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bands" of the stack "hfbeacons" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfbeacons_bands_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_hfbeacons_band_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfbeaconsBandRow", "bands", rows, sizeof(sdrkit_hfbeacons_band_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "propagation" of the stack "hfbeacons" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hfbeacons_propagation_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_hfbeacons_propagation_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.HfbeaconsPropagationRow", "propagation", rows, sizeof(sdrkit_hfbeacons_propagation_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfb.detection"
// (Beacon heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfbeacons_hfb_detection_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfbeacons_hfb_detection_event *event) {
  return sdrkit_typed_event(handle, n, "hfb.detection", "hfdata.HfbDetectionEvent", event, sizeof(sdrkit_hfbeacons_hfb_detection_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfb.tx"
// (Own beacon). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfbeacons_hfb_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfbeacons_hfb_tx_event *event) {
  return sdrkit_typed_event(handle, n, "hfb.tx", "hfdata.HfbTxEvent", event, sizeof(sdrkit_hfbeacons_hfb_tx_event));
}

// Runs the command "clear" (Clear) of the stack "hfbeacons" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_hfbeacons_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "list.restore" (Restore the built-in list) of the stack "hfbeacons" on the session of `handle`. Goes back to the built-in beacon list.
static inline int32_t sdrkit_hfbeacons_list_restore(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "list.restore", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "start" (Start the beacon) of the stack "hfbeacons" on the session of `handle`. Sends the message in Morse at offset_s after the next interval boundary of UTC (at once when that is now), then again each interval, until stop or count sendings. Each sending is one transmission; between sendings the transmitter is free. Transmit must be on and the call sign set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_hfbeacons_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop the beacon) of the stack "hfbeacons" on the session of `handle`. Ends the sending on air within 50 ms, with its fall, and ends the schedule.
static inline int32_t sdrkit_hfbeacons_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send the beacon once) of the stack "hfbeacons" on the session of `handle`. Sends the message in Morse once, at once, outside the schedule. Transmit must be on and the call sign set. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_hfbeacons_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hfbeacons" are the sdrkit_hfbeacons_gen_*() functions
// after the settings of its receiver.

// The setting "ppm" (Frequency error) of the stack "hfbeacons". Unit: ppm. The frequency error of the radio's oscillator: 1 ppm is 14 Hz at 14 MHz. Signals are looked for at f x (1 + ppm 10^-6).
static inline int32_t sdrkit_hfbeacons_get_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ppm", value);
}
static inline int32_t sdrkit_hfbeacons_set_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ppm", value);
}

// The setting "hop" (Step through the bands) of the stack "hfbeacons". Step through the bands when the capture is narrower than the list.
static inline int32_t sdrkit_hfbeacons_get_hop(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop", value);
}
static inline int32_t sdrkit_hfbeacons_set_hop(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop", value);
}

// The setting "dwell_s" (Dwell) of the stack "hfbeacons". A length of time in seconds. Time per stop. 180 s per stop hears all 18 NCDXF beacons on every band in the capture. The Long Dashes cycle is 27 s. Letters repeat within seconds.
static inline int32_t sdrkit_hfbeacons_get_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell_s", value);
}
static inline int32_t sdrkit_hfbeacons_set_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell_s", value);
}

// The setting "watch_ncdxf" (NCDXF/IARU) of the stack "hfbeacons".
static inline int32_t sdrkit_hfbeacons_get_watch_ncdxf(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "watch_ncdxf", value);
}
static inline int32_t sdrkit_hfbeacons_set_watch_ncdxf(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "watch_ncdxf", value);
}

// The setting "watch_dashes" (Long Dashes) of the stack "hfbeacons".
static inline int32_t sdrkit_hfbeacons_get_watch_dashes(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "watch_dashes", value);
}
static inline int32_t sdrkit_hfbeacons_set_watch_dashes(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "watch_dashes", value);
}

// The setting "watch_letters" (Single letters) of the stack "hfbeacons".
static inline int32_t sdrkit_hfbeacons_get_watch_letters(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "watch_letters", value);
}
static inline int32_t sdrkit_hfbeacons_set_watch_letters(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "watch_letters", value);
}

// The setting "watch_carriers" (Your carriers) of the stack "hfbeacons".
static inline int32_t sdrkit_hfbeacons_get_watch_carriers(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "watch_carriers", value);
}
static inline int32_t sdrkit_hfbeacons_set_watch_carriers(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "watch_carriers", value);
}

// The setting "list_json" (Beacon list) of the stack "hfbeacons". A beacon list in the JSON format of the list file. Empty: the built-in list.
static inline int32_t sdrkit_hfbeacons_get_list_json(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "list_json", value);
}
static inline int32_t sdrkit_hfbeacons_set_list_json(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "list_json", value);
}

// The setting "clock_offset_s" (Clock offset) of the stack "hfbeacons". Unit: s. How far the computer clock runs ahead of true UTC. The NCDXF slots need it right to about a second.
static inline int32_t sdrkit_hfbeacons_get_clock_offset_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock_offset_s", value);
}
static inline int32_t sdrkit_hfbeacons_set_clock_offset_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock_offset_s", value);
}

// The setting "start_utc_s" (Recording start (UTC seconds; 0 unknown)) of the stack "hfbeacons". Unit: s.
static inline int32_t sdrkit_hfbeacons_get_start_utc_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc_s", value);
}
static inline int32_t sdrkit_hfbeacons_set_start_utc_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc_s", value);
}

// The setting "listen" (Listen) of the stack "hfbeacons". Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way.
static inline int32_t sdrkit_hfbeacons_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_hfbeacons_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "listen_khz" (Listen to the beacon on) of the stack "hfbeacons". Unit: kHz.
static inline int32_t sdrkit_hfbeacons_get_listen_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "listen_khz", value);
}
static inline int32_t sdrkit_hfbeacons_set_listen_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "listen_khz", value);
}

// The setting "call" (Call sign) of the stack "hfbeacons". 1 to 12 characters of A-Z, 0-9 and /. Empty refuses start.
static inline int32_t sdrkit_hfbeacons_get_call(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "call", value);
}
static inline int32_t sdrkit_hfbeacons_set_call(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "call", value);
}

// The setting "message" (Message) of the stack "hfbeacons". Up to 60 characters of A-Z, 0-9, space and / ? . , =. {call} stands for the call sign; a message without it is followed by DE and the call sign; empty sends the call sign alone.
static inline int32_t sdrkit_hfbeacons_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "message", value);
}
static inline int32_t sdrkit_hfbeacons_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "message", value);
}

// The setting "wpm" (Speed) of the stack "hfbeacons". Unit: wpm. Words a minute of the word PARIS: a dot is 1.2 / wpm s.
static inline int32_t sdrkit_hfbeacons_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "wpm", value);
}
static inline int32_t sdrkit_hfbeacons_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "wpm", value);
}

// The setting "pitch_hz" (Tone) of the stack "hfbeacons". A frequency in Hz. The tone above the dial: the beacon is on the dial plus this frequency.
static inline int32_t sdrkit_hfbeacons_get_pitch_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pitch_hz", value);
}
static inline int32_t sdrkit_hfbeacons_set_pitch_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pitch_hz", value);
}

// The setting "long_dash_s" (Long dash) of the stack "hfbeacons". Unit: s. A steady carrier after the message, a word gap after it, for level checks. 0 sends none.
static inline int32_t sdrkit_hfbeacons_get_long_dash_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "long_dash_s", value);
}
static inline int32_t sdrkit_hfbeacons_set_long_dash_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "long_dash_s", value);
}

// The setting "interval_min" (Interval) of the stack "hfbeacons". Unit: min. From the start of one sending to the start of the next. A sending longer than the interval is followed at once by the next.
static inline int32_t sdrkit_hfbeacons_get_interval_min(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "interval_min", value);
}
static inline int32_t sdrkit_hfbeacons_set_interval_min(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "interval_min", value);
}

// The setting "power_steps" (Power steps) of the stack "hfbeacons". Four dashes of 1 s at 0, -10, -20 and -30 dB after the long dash, with no gap, as in the slot of the international beacon project.
static inline int32_t sdrkit_hfbeacons_get_power_steps(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "power_steps", value);
}
static inline int32_t sdrkit_hfbeacons_set_power_steps(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "power_steps", value);
}

// The setting "offset_s" (Start in the interval) of the stack "hfbeacons". Unit: s. The sendings of start begin this many seconds after each interval boundary of UTC (an interval of 10 min: on the hour, 10 past, ...). Taken modulo the interval.
static inline int32_t sdrkit_hfbeacons_get_offset_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "offset_s", value);
}
static inline int32_t sdrkit_hfbeacons_set_offset_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "offset_s", value);
}

// The setting "count" (Transmissions) of the stack "hfbeacons". The beacon ends after this many sendings. 0 sends until stop.
static inline int32_t sdrkit_hfbeacons_get_count(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "count", value);
}
static inline int32_t sdrkit_hfbeacons_set_count(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "count", value);
}

// The setting "gen_signal" (Signal) of the test signal generator of the stack "hfbeacons". A value of `sdrkit_hfbeacons_gen_signal`.
static inline int32_t sdrkit_hfbeacons_gen_get_signal(SDRKIT_HANDLE handle, sdrkit_hfbeacons_gen_signal *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hfbeacons_gen_signal)index;
  return code;
}
static inline int32_t sdrkit_hfbeacons_gen_set_signal(SDRKIT_HANDLE handle, sdrkit_hfbeacons_gen_signal value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_signal", (int32_t)value);
}

// The setting "gen_centre_hz" (Centre (0 = 3.65 MHz)) of the test signal generator of the stack "hfbeacons". A frequency in Hz.
static inline int32_t sdrkit_hfbeacons_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_hfbeacons_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "spectrum" of the stack "wavebuoy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wavebuoy_spectrum_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_wavebuoy_spectrum_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.WavebuoySpectrumRow", "spectrum", rows, sizeof(sdrkit_wavebuoy_spectrum_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "system_words" of the stack "wavebuoy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wavebuoy_system_words_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_wavebuoy_system_word_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.WavebuoySystemWordRow", "system_words", rows, sizeof(sdrkit_wavebuoy_system_word_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "wavebuoy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_wavebuoy_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wavebuoy_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.WavebuoyFrameRow", "frames", rows, sizeof(sdrkit_wavebuoy_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wavebuoy.vector"
// (HXV vector). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wavebuoy_vector_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wavebuoy_vector_event *event) {
  return sdrkit_typed_event(handle, n, "wavebuoy.vector", "hfdata.WavebuoyVectorEvent", event, sizeof(sdrkit_wavebuoy_vector_event));
}

// Runs the command "clear" (Clear) of the stack "wavebuoy" on the session of `handle`. Forgets the bits, frames, vectors and system words.
static inline int32_t sdrkit_wavebuoy_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wavebuoy" are the sdrkit_wavebuoy_gen_*() functions
// after the settings of its receiver.

// The setting "format" (Format) of the stack "wavebuoy". A value of `sdrkit_wavebuoy_format`. A buoy that sends another format than the stack expects shows as a wrong-format signal. Use this choice to force one.
static inline int32_t sdrkit_wavebuoy_get_format(SDRKIT_HANDLE handle, sdrkit_wavebuoy_format *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "format", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wavebuoy_format)index;
  return code;
}
static inline int32_t sdrkit_wavebuoy_set_format(SDRKIT_HANDLE handle, sdrkit_wavebuoy_format value) {
  return sdrkit_typed_setting_set_enum(handle, "format", (int32_t)value);
}

// The setting "gen_signal" (Signal) of the test signal generator of the stack "wavebuoy". A value of `sdrkit_wavebuoy_gen_signal`.
static inline int32_t sdrkit_wavebuoy_gen_get_signal(SDRKIT_HANDLE handle, sdrkit_wavebuoy_gen_signal *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_signal", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_wavebuoy_gen_signal)index;
  return code;
}
static inline int32_t sdrkit_wavebuoy_gen_set_signal(SDRKIT_HANDLE handle, sdrkit_wavebuoy_gen_signal value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_signal", (int32_t)value);
}

// The setting "gen_inverted_tones" (Inverted tones) of the test signal generator of the stack "wavebuoy".
static inline int32_t sdrkit_wavebuoy_gen_get_inverted_tones(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_inverted_tones", value);
}
static inline int32_t sdrkit_wavebuoy_gen_set_inverted_tones(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_inverted_tones", value);
}

// The setting "gen_wrsg" (WR-SG) of the test signal generator of the stack "wavebuoy".
static inline int32_t sdrkit_wavebuoy_gen_get_wrsg(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_wrsg", value);
}
static inline int32_t sdrkit_wavebuoy_gen_set_wrsg(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_wrsg", value);
}

// Fills `rows` with the changed rows of the table "dpdus" of the stack "ms110" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ms110_dpdus_rows(SDRKIT_HANDLE handle,
                                             sdrkit_hfdata_dpdu_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.DpduRow", "dpdus", rows, sizeof(sdrkit_hfdata_dpdu_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ms110.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ms110_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ms110_event *event) {
  return sdrkit_typed_event(handle, n, "ms110.event", "hfdata.Ms110Event", event, sizeof(sdrkit_ms110_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ms110.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ms110_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ms110_message_event *event) {
  return sdrkit_typed_event(handle, n, "ms110.message", "hfdata.Ms110MessageEvent", event, sizeof(sdrkit_ms110_message_event));
}

// Runs the command "clear" (Clear) of the stack "ms110" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ms110_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ms110" are the sdrkit_ms110_gen_*() functions
// after the settings of its receiver.

// The setting "gen_bps" (Data rate) of the test signal generator of the stack "ms110". A value of `sdrkit_ms110_gen_bps`.
static inline int32_t sdrkit_ms110_gen_get_bps(SDRKIT_HANDLE handle, sdrkit_ms110_gen_bps *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bps", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ms110_gen_bps)index;
  return code;
}
static inline int32_t sdrkit_ms110_gen_set_bps(SDRKIT_HANDLE handle, sdrkit_ms110_gen_bps value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bps", (int32_t)value);
}

// The setting "gen_interleaver" (Interleaver) of the test signal generator of the stack "ms110". A value of `sdrkit_ms110_gen_interleaver`.
static inline int32_t sdrkit_ms110_gen_get_interleaver(SDRKIT_HANDLE handle, sdrkit_ms110_gen_interleaver *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_interleaver", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ms110_gen_interleaver)index;
  return code;
}
static inline int32_t sdrkit_ms110_gen_set_interleaver(SDRKIT_HANDLE handle, sdrkit_ms110_gen_interleaver value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_interleaver", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "dpdus" of the stack "s4285" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_s4285_dpdus_rows(SDRKIT_HANDLE handle,
                                             sdrkit_hfdata_dpdu_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.DpduRow", "dpdus", rows, sizeof(sdrkit_hfdata_dpdu_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "s4285.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_s4285_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_s4285_event *event) {
  return sdrkit_typed_event(handle, n, "s4285.event", "hfdata.S4285Event", event, sizeof(sdrkit_s4285_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "s4285.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_s4285_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_s4285_message_event *event) {
  return sdrkit_typed_event(handle, n, "s4285.message", "hfdata.S4285MessageEvent", event, sizeof(sdrkit_s4285_message_event));
}

// Runs the command "clear" (Clear) of the stack "s4285" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_s4285_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "s4285" are the sdrkit_s4285_gen_*() functions
// after the settings of its receiver.

// The setting "profile" (Rate and interleaver) of the stack "s4285". A value of `sdrkit_s4285_profile`. Automatic tries every coded profile and reads the one whose start-of-message pattern is found. An uncoded profile has no such pattern, so it must be set.
static inline int32_t sdrkit_s4285_get_profile(SDRKIT_HANDLE handle, sdrkit_s4285_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4285_profile)index;
  return code;
}
static inline int32_t sdrkit_s4285_set_profile(SDRKIT_HANDLE handle, sdrkit_s4285_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "profile", (int32_t)value);
}

// The setting "scrambler_start" (Scrambler start) of the stack "s4285". A value of `sdrkit_s4285_scrambler_start`. The state of the 9-stage scrambler at the start of each frame. No public document settles it.
static inline int32_t sdrkit_s4285_get_scrambler_start(SDRKIT_HANDLE handle, sdrkit_s4285_scrambler_start *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "scrambler_start", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4285_scrambler_start)index;
  return code;
}
static inline int32_t sdrkit_s4285_set_scrambler_start(SDRKIT_HANDLE handle, sdrkit_s4285_scrambler_start value) {
  return sdrkit_typed_setting_set_enum(handle, "scrambler_start", (int32_t)value);
}

// The setting "gen_profile" (Profile) of the test signal generator of the stack "s4285". A value of `sdrkit_s4285_gen_profile`. Automatic sends 600 bit/s coded, short.
static inline int32_t sdrkit_s4285_gen_get_profile(SDRKIT_HANDLE handle, sdrkit_s4285_gen_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4285_gen_profile)index;
  return code;
}
static inline int32_t sdrkit_s4285_gen_set_profile(SDRKIT_HANDLE handle, sdrkit_s4285_gen_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_profile", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "dpdus" of the stack "s4539" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_s4539_dpdus_rows(SDRKIT_HANDLE handle,
                                             sdrkit_hfdata_dpdu_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.DpduRow", "dpdus", rows, sizeof(sdrkit_hfdata_dpdu_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "s4539.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_s4539_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_s4539_event *event) {
  return sdrkit_typed_event(handle, n, "s4539.event", "hfdata.S4539Event", event, sizeof(sdrkit_s4539_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "s4539.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_s4539_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_s4539_message_event *event) {
  return sdrkit_typed_event(handle, n, "s4539.message", "hfdata.S4539MessageEvent", event, sizeof(sdrkit_s4539_message_event));
}

// Runs the command "clear" (Clear) of the stack "s4539" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_s4539_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "s4539" are the sdrkit_s4539_gen_*() functions
// after the settings of its receiver.

// The setting "equaliser" (Multipath equaliser) of the stack "s4539". Off: one channel tap between the probes, for a channel without multipath.
static inline int32_t sdrkit_s4539_get_equaliser(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "equaliser", value);
}
static inline int32_t sdrkit_s4539_set_equaliser(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "equaliser", value);
}

// The setting "gen_bps" (Data rate) of the test signal generator of the stack "s4539". A value of `sdrkit_s4539_gen_bps`.
static inline int32_t sdrkit_s4539_gen_get_bps(SDRKIT_HANDLE handle, sdrkit_s4539_gen_bps *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bps", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4539_gen_bps)index;
  return code;
}
static inline int32_t sdrkit_s4539_gen_set_bps(SDRKIT_HANDLE handle, sdrkit_s4539_gen_bps value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bps", (int32_t)value);
}

// The setting "gen_frames" (Interleaver) of the test signal generator of the stack "s4539". A value of `sdrkit_s4539_gen_frames`. 12800 bit/s always has one frame.
static inline int32_t sdrkit_s4539_gen_get_frames(SDRKIT_HANDLE handle, sdrkit_s4539_gen_frames *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_frames", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_s4539_gen_frames)index;
  return code;
}
static inline int32_t sdrkit_s4539_gen_set_frames(SDRKIT_HANDLE handle, sdrkit_s4539_gen_frames value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_frames", (int32_t)value);
}

// The setting "gen_agc_blocks" (AGC blocks) of the test signal generator of the stack "s4539".
static inline int32_t sdrkit_s4539_gen_get_agc_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_agc_blocks", value);
}
static inline int32_t sdrkit_s4539_gen_set_agc_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_agc_blocks", value);
}

// The setting "gen_eom" (End of message) of the test signal generator of the stack "s4539".
static inline int32_t sdrkit_s4539_gen_get_eom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_eom", value);
}
static inline int32_t sdrkit_s4539_gen_set_eom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_eom", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "rpr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rpr_carriers_rows(SDRKIT_HANDLE handle,
                                              sdrkit_rpr_carrier_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.RprCarrierRow", "carriers", rows, sizeof(sdrkit_rpr_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "rpr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rpr_bursts_rows(SDRKIT_HANDLE handle,
                                            sdrkit_rpr_burst_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.RprBurstRow", "bursts", rows, sizeof(sdrkit_rpr_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "rpr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rpr_events_rows(SDRKIT_HANDLE handle,
                                            sdrkit_rpr_event_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "hfdata.RprEventRow", "events", rows, sizeof(sdrkit_rpr_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "rpr.burst"
// (Robust Packet burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_rpr_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_rpr_burst_event *event) {
  return sdrkit_typed_event(handle, n, "rpr.burst", "hfdata.RprBurstEvent", event, sizeof(sdrkit_rpr_burst_event));
}

// Runs the command "clear" (Clear) of the stack "rpr" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_rpr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "rpr" are the sdrkit_rpr_gen_*() functions
// after the settings of its receiver.

// The setting "analysis_interval_s" (Analysis interval) of the stack "rpr". A length of time in seconds.
static inline int32_t sdrkit_rpr_get_analysis_interval_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "analysis_interval_s", value);
}
static inline int32_t sdrkit_rpr_set_analysis_interval_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "analysis_interval_s", value);
}

// The setting "audio_centre_hz" (Audio centre) of the stack "rpr". A frequency in Hz. The audio frequency of the middle of the eight carriers when the dial is right.
static inline int32_t sdrkit_rpr_get_audio_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "audio_centre_hz", value);
}
static inline int32_t sdrkit_rpr_set_audio_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "audio_centre_hz", value);
}

// The setting "sideband" (Sideband) of the stack "rpr". A value of `sdrkit_rpr_sideband`. The sideband the signal is sent on.
static inline int32_t sdrkit_rpr_get_sideband(SDRKIT_HANDLE handle, sdrkit_rpr_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rpr_sideband)index;
  return code;
}
static inline int32_t sdrkit_rpr_set_sideband(SDRKIT_HANDLE handle, sdrkit_rpr_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "sideband", (int32_t)value);
}

// The setting "gen_speed" (Speed level) of the test signal generator of the stack "rpr". A value of `sdrkit_rpr_gen_speed`.
static inline int32_t sdrkit_rpr_gen_get_speed(SDRKIT_HANDLE handle, sdrkit_rpr_gen_speed *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_speed", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rpr_gen_speed)index;
  return code;
}
static inline int32_t sdrkit_rpr_gen_set_speed(SDRKIT_HANDLE handle, sdrkit_rpr_gen_speed value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_speed", (int32_t)value);
}

// The setting "gen_gap_s" (Gap between bursts) of the test signal generator of the stack "rpr". A length of time in seconds.
static inline int32_t sdrkit_rpr_gen_get_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_s", value);
}
static inline int32_t sdrkit_rpr_gen_set_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_crowd36_fields_read(SDRKIT_HANDLE handle, sdrkit_crowd36_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.Crowd36Fields", fields, sizeof(sdrkit_crowd36_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfdata_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_hfdata_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.DecoderFields", fields, sizeof(sdrkit_hfdata_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_xpa_fields_read(SDRKIT_HANDLE handle, sdrkit_xpa_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.XpaFields", fields, sizeof(sdrkit_xpa_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fsk200_fields_read(SDRKIT_HANDLE handle, sdrkit_fsk200_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.Fsk200Fields", fields, sizeof(sdrkit_fsk200_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gwfsk_fields_read(SDRKIT_HANDLE handle, sdrkit_gwfsk_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.GwfskFields", fields, sizeof(sdrkit_gwfsk_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fsk_modems_fields_read(SDRKIT_HANDLE handle, sdrkit_fsk_modems_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.FskModemsFields", fields, sizeof(sdrkit_fsk_modems_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_uvb76_fields_read(SDRKIT_HANDLE handle, sdrkit_uvb76_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.Uvb76Fields", fields, sizeof(sdrkit_uvb76_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_vlf_msk_fields_read(SDRKIT_HANDLE handle, sdrkit_vlf_msk_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.VlfMskFields", fields, sizeof(sdrkit_vlf_msk_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfdata_teleprinter_fields_read(SDRKIT_HANDLE handle, sdrkit_hfdata_teleprinter_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.TeleprinterFields", fields, sizeof(sdrkit_hfdata_teleprinter_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ale3g_fields_read(SDRKIT_HANDLE handle, sdrkit_ale3g_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.Ale3gFields", fields, sizeof(sdrkit_ale3g_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_s4481_fields_read(SDRKIT_HANDLE handle, sdrkit_s4481_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.S4481Fields", fields, sizeof(sdrkit_s4481_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_link11_fields_read(SDRKIT_HANDLE handle, sdrkit_link11_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.Link11Fields", fields, sizeof(sdrkit_link11_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cis_hf_fields_read(SDRKIT_HANDLE handle, sdrkit_cis_hf_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.CisHfFields", fields, sizeof(sdrkit_cis_hf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfid_fields_read(SDRKIT_HANDLE handle, sdrkit_hfid_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.HfidFields", fields, sizeof(sdrkit_hfid_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ale_fields_read(SDRKIT_HANDLE handle, sdrkit_ale_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.AleFields", fields, sizeof(sdrkit_ale_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ale_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_ale_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.AleTxFields", fields, sizeof(sdrkit_ale_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfradar_fields_read(SDRKIT_HANDLE handle, sdrkit_hfradar_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.HfradarFields", fields, sizeof(sdrkit_hfradar_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfdata_pactorhf_fields_read(SDRKIT_HANDLE handle, sdrkit_hfdata_pactorhf_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.PactorhfFields", fields, sizeof(sdrkit_hfdata_pactorhf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_pactor1_fields_read(SDRKIT_HANDLE handle, sdrkit_pactor1_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.Pactor1Fields", fields, sizeof(sdrkit_pactor1_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfbeacons_fields_read(SDRKIT_HANDLE handle, sdrkit_hfbeacons_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.HfbeaconsFields", fields, sizeof(sdrkit_hfbeacons_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfbeacons_hfb_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_hfbeacons_hfb_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.HfbTxFields", fields, sizeof(sdrkit_hfbeacons_hfb_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wavebuoy_fields_read(SDRKIT_HANDLE handle, sdrkit_wavebuoy_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.WavebuoyFields", fields, sizeof(sdrkit_wavebuoy_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfdata_serial_fields_read(SDRKIT_HANDLE handle, sdrkit_hfdata_serial_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.SerialFields", fields, sizeof(sdrkit_hfdata_serial_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_rpr_fields_read(SDRKIT_HANDLE handle, sdrkit_rpr_fields *fields) {
  return sdrkit_typed_fields(handle, "hfdata.RprFields", fields, sizeof(sdrkit_rpr_fields));
}

#endif
