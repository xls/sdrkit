/*
 * sdrkit: the typed interface of the stack family "analogtv", version 1.2. MIT licence.
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

#ifndef SDRKIT_ANALOGTV_H
#define SDRKIT_ANALOGTV_H

#include "../sdrkit.h"
#include "mpegts.h"

// The version of the family that this header describes.
#define SDRKIT_ANALOGTV_VERSION_MAJOR 1
#define SDRKIT_ANALOGTV_VERSION_MINOR 2

// The stack "atv": its id, and the keys of its event types.
#define SDRKIT_STACK_ATV "atv"
#define SDRKIT_EVENT_ATV_STATE "atv.state"

// The stack "mac": its id, and the keys of its event types.
#define SDRKIT_STACK_MAC "mac"
#define SDRKIT_EVENT_MAC_STATE "mac.state"

// The stack "fpv": its id, and the keys of its event types.
#define SDRKIT_STACK_FPV "fpv"
#define SDRKIT_EVENT_FPV_STATE "fpv.state"

// The options of the setting "modulation" of the stack "atv".
typedef enum sdrkit_atv_modulation {
  // AM (broadcast)
  SDRKIT_ATV_MODULATION_AM = 0,
  // FM video (FPV)
  SDRKIT_ATV_MODULATION_FM = 1,
  // NBTV / mechanical on AM
  SDRKIT_ATV_MODULATION_NBTV_AM = 2,
  // NBTV / mechanical on FM
  SDRKIT_ATV_MODULATION_NBTV_FM = 3,
} sdrkit_atv_modulation;

// The options of the setting "system" of the stack "atv".
typedef enum sdrkit_atv_system {
  // Automatic
  SDRKIT_ATV_SYSTEM_AUTO = 0,
  // B (7 MHz)
  SDRKIT_ATV_SYSTEM_B = 1,
  // G (8 MHz)
  SDRKIT_ATV_SYSTEM_G = 2,
  // I
  SDRKIT_ATV_SYSTEM_I = 3,
  // D/K
  SDRKIT_ATV_SYSTEM_DK = 4,
  // M
  SDRKIT_ATV_SYSTEM_M = 5,
  // N
  SDRKIT_ATV_SYSTEM_N = 6,
  // A (405 lines)
  SDRKIT_ATV_SYSTEM_A = 7,
  // C
  SDRKIT_ATV_SYSTEM_C = 8,
  // E (819 lines)
  SDRKIT_ATV_SYSTEM_E = 9,
  // H
  SDRKIT_ATV_SYSTEM_H = 10,
  // K'
  SDRKIT_ATV_SYSTEM_KP = 11,
  // L
  SDRKIT_ATV_SYSTEM_L = 12,
  // L' (band I)
  SDRKIT_ATV_SYSTEM_LP = 13,
} sdrkit_atv_system;

// The options of the setting "colour" of the stack "atv".
typedef enum sdrkit_atv_colour {
  // Automatic
  SDRKIT_ATV_COLOUR_AUTO = 0,
  // Mono
  SDRKIT_ATV_COLOUR_MONO = 1,
  // PAL
  SDRKIT_ATV_COLOUR_PAL = 2,
  // NTSC
  SDRKIT_ATV_COLOUR_NTSC = 3,
  // SECAM
  SDRKIT_ATV_COLOUR_SECAM = 4,
} sdrkit_atv_colour;

// The options of the setting "detector" of the stack "atv".
typedef enum sdrkit_atv_detector {
  // Automatic (synchronous when the carrier is locked)
  SDRKIT_ATV_DETECTOR_AUTO = 0,
  // Envelope only
  SDRKIT_ATV_DETECTOR_ENVELOPE = 1,
} sdrkit_atv_detector;

// The options of the setting "deinterlace" of the stack "atv".
typedef enum sdrkit_atv_deinterlace {
  // Weave
  SDRKIT_ATV_DEINTERLACE_WEAVE = 0,
  // Bob
  SDRKIT_ATV_DEINTERLACE_BOB = 1,
} sdrkit_atv_deinterlace;

// The options of the setting "noise_reduction" of the stack "atv".
typedef enum sdrkit_atv_noise_reduction {
  // Automatic
  SDRKIT_ATV_NOISE_REDUCTION_AUTO = 0,
  // Off
  SDRKIT_ATV_NOISE_REDUCTION_OFF = 1,
  // Light
  SDRKIT_ATV_NOISE_REDUCTION_LIGHT = 2,
  // Medium
  SDRKIT_ATV_NOISE_REDUCTION_MEDIUM = 3,
  // Strong
  SDRKIT_ATV_NOISE_REDUCTION_STRONG = 4,
} sdrkit_atv_noise_reduction;

// The options of the setting "black_setup" of the stack "atv".
typedef enum sdrkit_atv_black_setup {
  // By system
  SDRKIT_ATV_BLACK_SETUP_SYSTEM = 0,
  // None
  SDRKIT_ATV_BLACK_SETUP_NONE = 1,
  // 7.5 IRE
  SDRKIT_ATV_BLACK_SETUP_7_5_IRE = 2,
} sdrkit_atv_black_setup;

// The options of the setting "channel_width" of the stack "atv".
typedef enum sdrkit_atv_channel_width {
  // 6 MHz
  SDRKIT_ATV_CHANNEL_WIDTH_6_MHZ = 0,
  // 7 MHz
  SDRKIT_ATV_CHANNEL_WIDTH_7_MHZ = 1,
  // 8 MHz
  SDRKIT_ATV_CHANNEL_WIDTH_8_MHZ = 2,
} sdrkit_atv_channel_width;

// The options of the setting "sound_system" of the stack "atv".
typedef enum sdrkit_atv_sound_system {
  // Automatic (NICAM or A2 when there)
  SDRKIT_ATV_SOUND_SYSTEM_AUTO = 0,
  // FM / AM carrier only
  SDRKIT_ATV_SOUND_SYSTEM_CARRIER = 1,
} sdrkit_atv_sound_system;

// The options of the setting "dual_sound" of the stack "atv".
typedef enum sdrkit_atv_dual_sound {
  // First
  SDRKIT_ATV_DUAL_SOUND_FIRST = 0,
  // Second
  SDRKIT_ATV_DUAL_SOUND_SECOND = 1,
} sdrkit_atv_dual_sound;

// The options of the setting "gen_system" of the stack "atv".
typedef enum sdrkit_atv_gen_system {
  // B/G (8 MHz)
  SDRKIT_ATV_GEN_SYSTEM_G = 0,
  // B (7 MHz)
  SDRKIT_ATV_GEN_SYSTEM_B = 1,
  // I
  SDRKIT_ATV_GEN_SYSTEM_I = 2,
  // D/K
  SDRKIT_ATV_GEN_SYSTEM_DK = 3,
  // M
  SDRKIT_ATV_GEN_SYSTEM_M = 4,
  // N
  SDRKIT_ATV_GEN_SYSTEM_N = 5,
  // A (405)
  SDRKIT_ATV_GEN_SYSTEM_A = 6,
  // C
  SDRKIT_ATV_GEN_SYSTEM_C = 7,
  // E (819)
  SDRKIT_ATV_GEN_SYSTEM_E = 8,
  // H
  SDRKIT_ATV_GEN_SYSTEM_H = 9,
  // K'
  SDRKIT_ATV_GEN_SYSTEM_KP = 10,
  // L
  SDRKIT_ATV_GEN_SYSTEM_L = 11,
  // L'
  SDRKIT_ATV_GEN_SYSTEM_LP = 12,
  // NBTV on AM
  SDRKIT_ATV_GEN_SYSTEM_NBTV_AM = 13,
  // NBTV on narrow FM
  SDRKIT_ATV_GEN_SYSTEM_NBTV_FM = 14,
} sdrkit_atv_gen_system;

// The options of the setting "gen_colour" of the stack "atv".
typedef enum sdrkit_atv_gen_colour {
  // By system
  SDRKIT_ATV_GEN_COLOUR_SYSTEM = 0,
  // PAL
  SDRKIT_ATV_GEN_COLOUR_PAL = 1,
  // NTSC
  SDRKIT_ATV_GEN_COLOUR_NTSC = 2,
  // SECAM
  SDRKIT_ATV_GEN_COLOUR_SECAM = 3,
  // None
  SDRKIT_ATV_GEN_COLOUR_NONE = 4,
} sdrkit_atv_gen_colour;

// The options of the setting "gen_picture" of the stack "atv".
typedef enum sdrkit_atv_gen_picture {
  // Test card
  SDRKIT_ATV_GEN_PICTURE_CARD = 0,
  // Colour bars
  SDRKIT_ATV_GEN_PICTURE_BARS = 1,
  // Grey ramp
  SDRKIT_ATV_GEN_PICTURE_RAMP = 2,
} sdrkit_atv_gen_picture;

// The options of the setting "gen_sound" of the stack "atv".
typedef enum sdrkit_atv_gen_sound {
  // 1 kHz tone with gaps
  SDRKIT_ATV_GEN_SOUND_GAPS = 0,
  // Melody
  SDRKIT_ATV_GEN_SOUND_MELODY = 1,
  // Unmodulated carrier
  SDRKIT_ATV_GEN_SOUND_CARRIER = 2,
  // No sound carrier
  SDRKIT_ATV_GEN_SOUND_NONE = 3,
  // Continuous 1 kHz
  SDRKIT_ATV_GEN_SOUND_TONE = 4,
} sdrkit_atv_gen_sound;

// The options of the setting "gen_second" of the stack "atv".
typedef enum sdrkit_atv_gen_second {
  // None
  SDRKIT_ATV_GEN_SECOND_NONE = 0,
  // A2 stereo
  SDRKIT_ATV_GEN_SECOND_A2_STEREO = 1,
  // A2 two languages
  SDRKIT_ATV_GEN_SECOND_A2_DUAL = 2,
  // A2 mono (pilot)
  SDRKIT_ATV_GEN_SECOND_A2_MONO = 3,
  // NICAM stereo
  SDRKIT_ATV_GEN_SECOND_NICAM_STEREO = 4,
  // NICAM two languages
  SDRKIT_ATV_GEN_SECOND_NICAM_DUAL = 5,
} sdrkit_atv_gen_second;

// The options of the setting "gen_setup" of the stack "atv".
typedef enum sdrkit_atv_gen_setup {
  // 7.5 IRE
  SDRKIT_ATV_GEN_SETUP_7_5_IRE = 0,
  // None (Japan)
  SDRKIT_ATV_GEN_SETUP_NONE = 1,
} sdrkit_atv_gen_setup;

// The options of the setting "gen_secam_id" of the stack "atv".
typedef enum sdrkit_atv_gen_secam_id {
  // Both
  SDRKIT_ATV_GEN_SECAM_ID_BOTH = 0,
  // Back porch only
  SDRKIT_ATV_GEN_SECAM_ID_PORCH = 1,
  // Field lines only
  SDRKIT_ATV_GEN_SECAM_ID_LINES = 2,
} sdrkit_atv_gen_secam_id;

// The options of the setting "tx_system" of the stack "atv".
typedef enum sdrkit_atv_tx_system {
  // G (8 MHz)
  SDRKIT_ATV_TX_SYSTEM_G = 0,
  // B (7 MHz)
  SDRKIT_ATV_TX_SYSTEM_B = 1,
  // I
  SDRKIT_ATV_TX_SYSTEM_I = 2,
  // D/K
  SDRKIT_ATV_TX_SYSTEM_DK = 3,
  // M
  SDRKIT_ATV_TX_SYSTEM_M = 4,
  // N
  SDRKIT_ATV_TX_SYSTEM_N = 5,
  // A (405 lines)
  SDRKIT_ATV_TX_SYSTEM_A = 6,
  // C
  SDRKIT_ATV_TX_SYSTEM_C = 7,
  // E (819 lines)
  SDRKIT_ATV_TX_SYSTEM_E = 8,
  // H
  SDRKIT_ATV_TX_SYSTEM_H = 9,
  // K'
  SDRKIT_ATV_TX_SYSTEM_KP = 10,
  // L
  SDRKIT_ATV_TX_SYSTEM_L = 11,
  // L' (band I)
  SDRKIT_ATV_TX_SYSTEM_LP = 12,
} sdrkit_atv_tx_system;

// The options of the setting "tx_colour" of the stack "atv".
typedef enum sdrkit_atv_tx_colour {
  // By system
  SDRKIT_ATV_TX_COLOUR_SYSTEM = 0,
  // PAL
  SDRKIT_ATV_TX_COLOUR_PAL = 1,
  // NTSC
  SDRKIT_ATV_TX_COLOUR_NTSC = 2,
  // SECAM
  SDRKIT_ATV_TX_COLOUR_SECAM = 3,
  // None
  SDRKIT_ATV_TX_COLOUR_NONE = 4,
} sdrkit_atv_tx_colour;

// The options of the setting "tx_picture" of the stack "atv".
typedef enum sdrkit_atv_tx_picture {
  // Test card
  SDRKIT_ATV_TX_PICTURE_CARD = 0,
  // Colour bars
  SDRKIT_ATV_TX_PICTURE_BARS = 1,
  // Grey ramp
  SDRKIT_ATV_TX_PICTURE_RAMP = 2,
} sdrkit_atv_tx_picture;

// The options of the setting "tx_setup" of the stack "atv".
typedef enum sdrkit_atv_tx_setup {
  // 7.5 IRE
  SDRKIT_ATV_TX_SETUP_7_5_IRE = 0,
  // None (Japan)
  SDRKIT_ATV_TX_SETUP_NONE = 1,
} sdrkit_atv_tx_setup;

// The options of the setting "tx_secam_id" of the stack "atv".
typedef enum sdrkit_atv_tx_secam_id {
  // Both
  SDRKIT_ATV_TX_SECAM_ID_BOTH = 0,
  // Back porch only
  SDRKIT_ATV_TX_SECAM_ID_PORCH = 1,
  // Field lines only
  SDRKIT_ATV_TX_SECAM_ID_LINES = 2,
} sdrkit_atv_tx_secam_id;

// The options of the setting "gen_variant" of the stack "mac".
typedef enum sdrkit_mac_gen_variant {
  // D2-MAC/packet
  SDRKIT_MAC_GEN_VARIANT_D2 = 0,
  // D-MAC/packet
  SDRKIT_MAC_GEN_VARIANT_D = 1,
} sdrkit_mac_gen_variant;

// The options of the setting "gen_picture" of the stack "mac".
typedef enum sdrkit_mac_gen_picture {
  // Test card
  SDRKIT_MAC_GEN_PICTURE_CARD = 0,
  // Colour bars
  SDRKIT_MAC_GEN_PICTURE_BARS = 1,
  // Grey ramp
  SDRKIT_MAC_GEN_PICTURE_RAMP = 2,
} sdrkit_mac_gen_picture;

// The options of the setting "system" of the stack "fpv".
typedef enum sdrkit_fpv_system {
  // Automatic
  SDRKIT_FPV_SYSTEM_AUTO = 0,
  // PAL (625)
  SDRKIT_FPV_SYSTEM_PAL = 1,
  // NTSC (525)
  SDRKIT_FPV_SYSTEM_NTSC = 2,
} sdrkit_fpv_system;

// The options of the setting "sound_subcarrier" of the stack "fpv".
typedef enum sdrkit_fpv_sound_subcarrier {
  // Automatic
  SDRKIT_FPV_SOUND_SUBCARRIER_AUTO = 0,
  // 6.0 MHz
  SDRKIT_FPV_SOUND_SUBCARRIER_6_0 = 1,
  // 6.5 MHz
  SDRKIT_FPV_SOUND_SUBCARRIER_6_5 = 2,
} sdrkit_fpv_sound_subcarrier;

// The options of the setting "deinterlace" of the stack "fpv".
typedef enum sdrkit_fpv_deinterlace {
  // Weave
  SDRKIT_FPV_DEINTERLACE_WEAVE = 0,
  // Bob
  SDRKIT_FPV_DEINTERLACE_BOB = 1,
} sdrkit_fpv_deinterlace;

// The options of the setting "gen_system" of the stack "fpv".
typedef enum sdrkit_fpv_gen_system {
  // PAL (625)
  SDRKIT_FPV_GEN_SYSTEM_PAL = 0,
  // NTSC (525)
  SDRKIT_FPV_GEN_SYSTEM_NTSC = 1,
} sdrkit_fpv_gen_system;

// The options of the setting "gen_picture" of the stack "fpv".
typedef enum sdrkit_fpv_gen_picture {
  // Test card
  SDRKIT_FPV_GEN_PICTURE_CARD = 0,
  // Colour bars
  SDRKIT_FPV_GEN_PICTURE_BARS = 1,
} sdrkit_fpv_gen_picture;

// The options of the setting "gen_sound" of the stack "fpv".
typedef enum sdrkit_fpv_gen_sound {
  // Subcarrier 6.5 MHz
  SDRKIT_FPV_GEN_SOUND_6_5 = 0,
  // 6.0 MHz
  SDRKIT_FPV_GEN_SOUND_6_0 = 1,
  // None
  SDRKIT_FPV_GEN_SOUND_NONE = 2,
} sdrkit_fpv_gen_sound;

// The options of the setting "tx_system" of the stack "fpv".
typedef enum sdrkit_fpv_tx_system {
  // PAL (625)
  SDRKIT_FPV_TX_SYSTEM_PAL = 0,
  // NTSC (525)
  SDRKIT_FPV_TX_SYSTEM_NTSC = 1,
} sdrkit_fpv_tx_system;

// The options of the setting "tx_picture" of the stack "fpv".
typedef enum sdrkit_fpv_tx_picture {
  // Test card
  SDRKIT_FPV_TX_PICTURE_CARD = 0,
  // Colour bars
  SDRKIT_FPV_TX_PICTURE_BARS = 1,
} sdrkit_fpv_tx_picture;

// The options of the setting "tx_subcarrier" of the stack "fpv".
typedef enum sdrkit_fpv_tx_subcarrier {
  // 6.5 MHz
  SDRKIT_FPV_TX_SUBCARRIER_6_5 = 0,
  // 6.0 MHz
  SDRKIT_FPV_TX_SUBCARRIER_6_0 = 1,
} sdrkit_fpv_tx_subcarrier;

// The options of `state` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_state {
  // Searching
  SDRKIT_ATV_FIELDS_STATE_SEARCHING = 0,
  // Synchronising
  SDRKIT_ATV_FIELDS_STATE_SYNC = 1,
  // Locked
  SDRKIT_ATV_FIELDS_STATE_LOCKED = 2,
} sdrkit_atv_fields_state;

// The options of `colour_system` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_colour_system {
  // PAL
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_PAL = 0,
  // PAL-M
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_PAL_M = 1,
  // PAL-N
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_PAL_N = 2,
  // NTSC
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_NTSC = 3,
  // NTSC-N
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_NTSC_N = 4,
  // NTSC 4.43
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_NTSC443 = 5,
  // SECAM
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_SECAM = 6,
  // mono
  SDRKIT_ATV_FIELDS_COLOUR_SYSTEM_MONO = 7,
} sdrkit_atv_fields_colour_system;

// The options of `detector` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_detector {
  // synchronous
  SDRKIT_ATV_FIELDS_DETECTOR_SYNCHRONOUS = 0,
  // envelope
  SDRKIT_ATV_FIELDS_DETECTOR_ENVELOPE = 1,
  // FM discriminator
  SDRKIT_ATV_FIELDS_DETECTOR_FM = 2,
} sdrkit_atv_fields_detector;

// The options of `modulation` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_modulation {
  // AM (broadcast)
  SDRKIT_ATV_FIELDS_MODULATION_AM = 0,
  // FM video (FPV)
  SDRKIT_ATV_FIELDS_MODULATION_FM = 1,
  // NBTV / mechanical on AM
  SDRKIT_ATV_FIELDS_MODULATION_NBTV_AM = 2,
  // NBTV / mechanical on FM
  SDRKIT_ATV_FIELDS_MODULATION_NBTV_FM = 3,
} sdrkit_atv_fields_modulation;

// The options of `sound_system` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_sound_system {
  // FM
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_FM = 0,
  // AM
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_AM = 1,
  // FM mono (A2 pilot)
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_A2_MONO = 2,
  // A2 stereo
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_A2_STEREO = 3,
  // A2 dual, sound A
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_A2_DUAL_A = 4,
  // A2 dual, sound B
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_A2_DUAL_B = 5,
  // NICAM stereo
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_NICAM_STEREO = 6,
  // NICAM dual mono, channel 1
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_NICAM_DUAL_1 = 7,
  // NICAM dual mono, channel 2
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_NICAM_DUAL_2 = 8,
  // NICAM mono
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_NICAM_MONO = 9,
  // none found
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_NONE_FOUND = 10,
  // not decoded
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_NOT_DECODED = 11,
  // none (the audio is the picture)
  SDRKIT_ATV_FIELDS_SOUND_SYSTEM_PICTURE = 12,
} sdrkit_atv_fields_sound_system;

// The options of `fm_polarity` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_fm_polarity {
  // trying
  SDRKIT_ATV_FIELDS_FM_POLARITY_TRYING = 0,
  // sync tip is the lowest frequency
  SDRKIT_ATV_FIELDS_FM_POLARITY_TIP_LOW = 1,
  // sync tip is the highest frequency
  SDRKIT_ATV_FIELDS_FM_POLARITY_TIP_HIGH = 2,
} sdrkit_atv_fields_fm_polarity;

// The options of `nicam_mode` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_nicam_mode {
  // stereo
  SDRKIT_ATV_FIELDS_NICAM_MODE_STEREO = 0,
  // dual mono
  SDRKIT_ATV_FIELDS_NICAM_MODE_DUAL_MONO = 1,
  // mono
  SDRKIT_ATV_FIELDS_NICAM_MODE_MONO = 2,
  // data
  SDRKIT_ATV_FIELDS_NICAM_MODE_DATA = 3,
} sdrkit_atv_fields_nicam_mode;

// The options of `a2_mode` of `sdrkit_atv_fields`.
typedef enum sdrkit_atv_fields_a2_mode {
  // mono
  SDRKIT_ATV_FIELDS_A2_MODE_MONO = 0,
  // stereo
  SDRKIT_ATV_FIELDS_A2_MODE_STEREO = 1,
  // dual
  SDRKIT_ATV_FIELDS_A2_MODE_DUAL = 2,
} sdrkit_atv_fields_a2_mode;

// The result fields of the `atv` receiver, which its status component writes.
typedef struct sdrkit_atv_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The state of the receiver. A value of `sdrkit_atv_fields_state`.
  int32_t state;
  // True if `system` holds a value.
  bool has_system;
  // The system and its raster, "PAL B/G 625/50". Absent while the receiver knows none.
  sdrkit_string system;
  // True if `colour_system` holds a value.
  bool has_colour_system;
  // The colour system in use. A value of `sdrkit_atv_fields_colour_system`.
  int32_t colour_system;
  // The picture has colour now.
  bool colour;
  // The colour killer is on (the burst is too weak).
  bool colour_killer;
  // Lines a frame.
  int64_t lines;
  // Fields a second. A frequency in Hz.
  double field_rate;
  // The line rate. A frequency in Hz.
  double line_rate;
  // The line rate against its nominal value. Unit: ppm.
  double line_error_ppm;
  // The detector in use. A value of `sdrkit_atv_fields_detector`.
  int32_t detector;
  // The modulation the receiver runs. A value of `sdrkit_atv_fields_modulation`.
  int32_t modulation;
  // The vision carrier, from the centre of the input. A frequency in Hz.
  double vision_carrier;
  // True if `sound_carrier` holds a value.
  bool has_sound_carrier;
  // The sound carrier, from the centre of the input. Absent while none is found. A frequency in Hz.
  double sound_carrier;
  // The sound carrier above the vision carrier (negative: below it). A frequency in Hz.
  double sound_spacing;
  // The offset of the vision carrier. A frequency in Hz.
  double offset_hz;
  // The level of the vision carrier. Unit: dBFS.
  double level_dbfs;
  // The peak vision carrier against the noise in 5 MHz. Unit: dB.
  double carrier_to_noise_db;
  // The SNR of the picture. Unit: dB.
  double snr_db;
  // A sound carrier is there.
  bool sound_present;
  // The peak deviation of the FM sound carrier. A frequency in Hz.
  double sound_deviation;
  // The audio level. Unit: dBFS.
  double audio_level_dbfs;
  // The sound that plays. A value of `sdrkit_atv_fields_sound_system`.
  int32_t sound_system;
  // The share of lines whose sync was found. Unit: %.
  double sync_found_pct;
  // Lines since the line loop started.
  int64_t line_count;
  // Field syncs found.
  int64_t field_count;
  // Pictures sent.
  int64_t pictures;
  // Snow pictures sent.
  int64_t snow_pictures;
  // The field completed last (1 or 2).
  int64_t last_field;
  // Fields decoded completely.
  int64_t fields_ok;
  // Fields damaged.
  int64_t fields_bad;
  // The sync depth, a share of the peak carrier. Unit: %.
  double sync_depth_pct;
  // The sync compression measured. Unit: %.
  double sync_compression_pct;
  // The white peak (1 is the nominal white).
  double white_peak;
  // The colour burst, a share of its nominal level. Unit: %.
  double burst_pct;
  // The phase error of the burst. Unit: °.
  double burst_phase_error_deg;
  // The SECAM colour subcarrier level, a share of its nominal level. Unit: %.
  double chroma_level_pct;
  // True if `secam_identification` holds a value.
  bool has_secam_identification;
  // How SECAM was found.
  sdrkit_string secam_identification;
  // True if `fm_polarity` holds a value.
  bool has_fm_polarity;
  // The polarity of FM video. A value of `sdrkit_atv_fields_fm_polarity`.
  int32_t fm_polarity;
  // The NBTV line loop is locked.
  bool nbtv_line_locked;
  // The NBTV picture is locked.
  bool nbtv_picture_locked;
  // True if `nicam_locked` holds a value.
  bool has_nicam_locked;
  // The NICAM frame alignment holds. Absent without a NICAM carrier.
  bool nicam_locked;
  // True if `nicam_valid` holds a value.
  bool has_nicam_valid;
  // The NICAM sound is valid and can play.
  bool nicam_valid;
  // True if `nicam_mode` holds a value.
  bool has_nicam_mode;
  // The NICAM sound mode. A value of `sdrkit_atv_fields_nicam_mode`.
  int32_t nicam_mode;
  // True if `nicam_parity_errors` holds a value.
  bool has_nicam_parity_errors;
  // NICAM sound words whose parity failed, since the lock.
  int64_t nicam_parity_errors;
  // True if `nicam_error_pct` holds a value.
  bool has_nicam_error_pct;
  // The share of NICAM sound words whose parity failed. Unit: %.
  double nicam_error_pct;
  // True if `a2_mode` holds a value.
  bool has_a2_mode;
  // The A2 identification found. A value of `sdrkit_atv_fields_a2_mode`.
  int32_t a2_mode;
} sdrkit_atv_fields;

// The result fields of the teletext decoder of the `atv` and `fpv` stacks. The counts have no value while no teletext line arrives.
typedef struct sdrkit_analogtv_teletext_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `teletext_header` holds a value.
  bool has_teletext_header;
  // The header of the last teletext page.
  sdrkit_string teletext_header;
  // True if `teletext_pages` holds a value.
  bool has_teletext_pages;
  // Teletext pages held.
  int64_t teletext_pages;
  // True if `teletext_packets` holds a value.
  bool has_teletext_packets;
  // Teletext packets received.
  int64_t teletext_packets;
  // True if `teletext_errors` holds a value.
  bool has_teletext_errors;
  // Teletext lines whose framing code or address failed.
  int64_t teletext_errors;
  // True if `teletext_parity_errors` holds a value.
  bool has_teletext_parity_errors;
  // Teletext characters whose parity failed.
  int64_t teletext_parity_errors;
  // The teletext page shown.
  int64_t teletext_page;
  // True if `page_text` holds a value.
  bool has_page_text;
  // The text of the teletext page shown, its rows joined with line feeds.
  sdrkit_string page_text;
} sdrkit_analogtv_teletext_fields;

// The result fields of the senders of `atv` and `fpv`.
typedef struct sdrkit_analogtv_tv_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the sender sends.
  bool tx_sending;
  // True if `tx_system` holds a value.
  bool has_tx_system;
  // The system sent, "PAL B/G 625/50". No value while nothing is sent.
  sdrkit_string tx_system;
  // The pictures of the host put on the air in the last transmission: one for each field that started with a new one.
  int64_t tx_frames;
  // The fields sent in the last transmission.
  int64_t tx_fields;
} sdrkit_analogtv_tv_tx_fields;

// A channel of the setting `channels`. The row key is the label.
typedef struct sdrkit_atv_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel label.
  sdrkit_string label;
  // The vision carrier of the channel. A frequency in Hz.
  double vision;
} sdrkit_atv_channel_row;

// The options of `state` of `sdrkit_mac_fields`.
typedef enum sdrkit_mac_fields_state {
  // Searching
  SDRKIT_MAC_FIELDS_STATE_SEARCHING = 0,
  // Synchronising
  SDRKIT_MAC_FIELDS_STATE_SYNC = 1,
  // Locked
  SDRKIT_MAC_FIELDS_STATE_LOCKED = 2,
} sdrkit_mac_fields_state;

// The options of `system` of `sdrkit_mac_fields`.
typedef enum sdrkit_mac_fields_system {
  // D2-MAC/packet
  SDRKIT_MAC_FIELDS_SYSTEM_D2 = 0,
  // D-MAC/packet
  SDRKIT_MAC_FIELDS_SYSTEM_D = 1,
} sdrkit_mac_fields_system;

// The options of `aspect` of `sdrkit_mac_fields`.
typedef enum sdrkit_mac_fields_aspect {
  // 4:3
  SDRKIT_MAC_FIELDS_ASPECT_NORMAL = 0,
  // 16:9
  SDRKIT_MAC_FIELDS_ASPECT_WIDE = 1,
} sdrkit_mac_fields_aspect;

// The options of `ca_system` of `sdrkit_mac_fields`.
typedef enum sdrkit_mac_fields_ca_system {
  // EuroCrypt
  SDRKIT_MAC_FIELDS_CA_SYSTEM_EUROCRYPT = 0,
  // conditional access
  SDRKIT_MAC_FIELDS_CA_SYSTEM_OTHER = 1,
} sdrkit_mac_fields_ca_system;

// The options of `sound_mode` of `sdrkit_mac_fields`.
typedef enum sdrkit_mac_fields_sound_mode {
  // companded 14/10, 32 kHz, first-level protection
  SDRKIT_MAC_FIELDS_SOUND_MODE_COMPANDED = 0,
  // scrambled (not played)
  SDRKIT_MAC_FIELDS_SOUND_MODE_SCRAMBLED = 1,
} sdrkit_mac_fields_sound_mode;

// The result fields of the MAC receiver.
typedef struct sdrkit_mac_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The state of the receiver. A value of `sdrkit_mac_fields_state`.
  int32_t state;
  // True if `system` holds a value.
  bool has_system;
  // The MAC variant. Absent while the receiver searches. A value of `sdrkit_mac_fields_system`.
  int32_t system;
  // The signal-to-noise ratio of the data burst. Unit: dB.
  double snr_db;
  // The SNR of the picture. Unit: dB.
  double video_snr_db;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The level of the signal, dBFS. Unit: dBFS.
  double level_dbfs;
  // The FM deviation, Hz a volt of the baseband. Unit: Hz/V.
  double deviation_hz_per_v;
  // The spectrum is inverted (and corrected).
  bool spectrum_inverted;
  // The line rate. A frequency in Hz.
  double line_rate;
  // The line rate against its nominal value. Unit: ppm.
  double line_error_ppm;
  // The share of lines whose sync word was found. Unit: %.
  double line_sync_pct;
  // Frame syncs found.
  int64_t frame_syncs;
  // Frames decoded.
  int64_t frames;
  // The bit error rate of the data burst.
  double ber;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel name. Absent until the service identification names it.
  sdrkit_string channel;
  // True if `network` holds a value.
  bool has_network;
  // The network name.
  sdrkit_string network;
  // True if `country` holds a value.
  bool has_country;
  // The country.
  sdrkit_string country;
  // True if `channel_id` holds a value.
  bool has_channel_id;
  // The channel identifier.
  int64_t channel_id;
  // True if `aspect` holds a value.
  bool has_aspect;
  // The picture's aspect ratio. A value of `sdrkit_mac_fields_aspect`.
  int32_t aspect;
  // The picture or sound is scrambled.
  bool scrambled;
  // True if `access` holds a value.
  bool has_access;
  // The access conditions, in words.
  sdrkit_string access;
  // True if `ca_system` holds a value.
  bool has_ca_system;
  // The conditional access system, when the channel names one. A value of `sdrkit_mac_fields_ca_system`.
  int32_t ca_system;
  // Sound words whose protection failed.
  int64_t sound_errors;
  // ECM packets received.
  int64_t ecm_packets;
  // Packet headers decoded.
  int64_t packet_headers;
  // Packet headers corrected.
  int64_t packets_corrected;
  // Dummy packets.
  int64_t packets_dummy;
  // Service identification groups whose CRC passed.
  int64_t si_groups_ok;
  // Service identification groups whose CRC failed.
  int64_t si_groups_bad;
  // True if `sound_address` holds a value.
  bool has_sound_address;
  // The packet address of the sound.
  int64_t sound_address;
  // The sound is there and plays.
  bool sound_present;
  // Sound packets received.
  int64_t sound_packets;
  // How the sound is coded. A value of `sdrkit_mac_fields_sound_mode`.
  int32_t sound_mode;
  // The audio level. Unit: dBFS.
  double audio_level_dbfs;
  // Sound blocks whose words failed their parity.
  int64_t blocks_bad;
  // Pictures sent.
  int64_t pictures;
} sdrkit_mac_fields;

// The options of `state` of `sdrkit_fpv_fields`.
typedef enum sdrkit_fpv_fields_state {
  // Searching
  SDRKIT_FPV_FIELDS_STATE_SEARCHING = 0,
  // Synchronising
  SDRKIT_FPV_FIELDS_STATE_SYNC = 1,
  // Locked
  SDRKIT_FPV_FIELDS_STATE_LOCKED = 2,
} sdrkit_fpv_fields_state;

// The options of `colour_system` of `sdrkit_fpv_fields`.
typedef enum sdrkit_fpv_fields_colour_system {
  // PAL
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_PAL = 0,
  // PAL-M
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_PAL_M = 1,
  // PAL-N
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_PAL_N = 2,
  // NTSC
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_NTSC = 3,
  // NTSC-N
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_NTSC_N = 4,
  // NTSC 4.43
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_NTSC443 = 5,
  // SECAM
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_SECAM = 6,
  // mono
  SDRKIT_FPV_FIELDS_COLOUR_SYSTEM_MONO = 7,
} sdrkit_fpv_fields_colour_system;

// The options of `polarity` of `sdrkit_fpv_fields`.
typedef enum sdrkit_fpv_fields_polarity {
  // trying
  SDRKIT_FPV_FIELDS_POLARITY_TRYING = 0,
  // sync tip is the lowest frequency
  SDRKIT_FPV_FIELDS_POLARITY_TIP_LOW = 1,
  // sync tip is the highest frequency
  SDRKIT_FPV_FIELDS_POLARITY_TIP_HIGH = 2,
} sdrkit_fpv_fields_polarity;

// The result fields of the `fpv` receiver, which its display writes.
typedef struct sdrkit_fpv_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The state of the receiver. A value of `sdrkit_fpv_fields_state`.
  int32_t state;
  // True if `channel` holds a value.
  bool has_channel;
  // The names of the channels at the tuned frequency. Absent off every channel.
  sdrkit_string channel;
  // The level of the channel. Unit: dBFS.
  double level_dbfs;
  // The level of the channel above the noise floor. Unit: dB.
  double over_noise_db;
  // The noise floor. Unit: dBFS.
  double noise_dbfs;
  // The offset of the transmitter from the channel. A frequency in Hz.
  double offset_hz;
  // True if `system` holds a value.
  bool has_system;
  // The system and its raster. Absent while the receiver knows none.
  sdrkit_string system;
  // True if `colour_system` holds a value.
  bool has_colour_system;
  // The colour system in use. A value of `sdrkit_fpv_fields_colour_system`.
  int32_t colour_system;
  // Lines a frame.
  int64_t lines;
  // Fields a second. A frequency in Hz.
  double field_rate;
  // The share of lines whose sync was found. Unit: %.
  double sync_found_pct;
  // Pictures sent.
  int64_t pictures;
  // Fields decoded completely.
  int64_t fields_ok;
  // Fields damaged.
  int64_t fields_bad;
  // The polarity of the FM video. A value of `sdrkit_fpv_fields_polarity`.
  int32_t polarity;
  // The SNR of the picture. Unit: dB.
  double snr_db;
  // A sound subcarrier is there.
  bool sound_present;
  // True if `sound_subcarrier` holds a value.
  bool has_sound_subcarrier;
  // The sound subcarrier, from the vision carrier. Absent while none is found. A frequency in Hz.
  double sound_subcarrier;
  // The 6.0 MHz subcarrier against the noise beside it. Unit: dB.
  double subcarrier_60_db;
  // The 6.5 MHz subcarrier against the noise beside it. Unit: dB.
  double subcarrier_65_db;
} sdrkit_fpv_fields;

// A channel of the FPV meter. The row key is the channel name ("R5").
typedef struct sdrkit_fpv_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the channel ("R5").
  sdrkit_string name;
  // The band letter.
  sdrkit_string band;
  // The number of the channel in its band.
  int64_t number;
  // The frequency of the channel. A frequency in Hz.
  double freq;
  // The level of the channel. Unit: dBFS.
  double level_dbfs;
  // The level of the channel above the noise floor. Unit: dB.
  double over_noise_db;
  // A transmitter is on the channel.
  bool present;
  // The channel is inside the capture.
  bool inside;
  // True if `heard` holds a value.
  bool has_heard;
  // The stream time at which a transmitter was last heard on the channel. A length of time in seconds.
  double heard;
} sdrkit_fpv_channel_row;

// The options of `state` of `sdrkit_atv_state_event`.
typedef enum sdrkit_atv_state_event_state {
  // Searching
  SDRKIT_ATV_STATE_EVENT_STATE_SEARCHING = 0,
  // Synchronising
  SDRKIT_ATV_STATE_EVENT_STATE_SYNC = 1,
  // Locked
  SDRKIT_ATV_STATE_EVENT_STATE_LOCKED = 2,
} sdrkit_atv_state_event_state;

// The options of `family` of `sdrkit_atv_state_event`.
typedef enum sdrkit_atv_state_event_family {
  // Aircraft
  SDRKIT_ATV_STATE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ATV_STATE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ATV_STATE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ATV_STATE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ATV_STATE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ATV_STATE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ATV_STATE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ATV_STATE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ATV_STATE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ATV_STATE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ATV_STATE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ATV_STATE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ATV_STATE_EVENT_FAMILY_OTHER = 12,
} sdrkit_atv_state_event_family;

// A change of the state of the `atv` receiver.
typedef struct sdrkit_atv_state_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The new state. A value of `sdrkit_atv_state_event_state`.
  int32_t state;
  // The system of the receiver.
  sdrkit_string text;
  // What the event is about: the receiver. A value of `sdrkit_atv_state_event_family`.
  int32_t family;
} sdrkit_atv_state_event;

// The options of `state` of `sdrkit_mac_state_event`.
typedef enum sdrkit_mac_state_event_state {
  // Searching
  SDRKIT_MAC_STATE_EVENT_STATE_SEARCHING = 0,
  // Synchronising
  SDRKIT_MAC_STATE_EVENT_STATE_SYNC = 1,
  // Locked
  SDRKIT_MAC_STATE_EVENT_STATE_LOCKED = 2,
} sdrkit_mac_state_event_state;

// The options of `family` of `sdrkit_mac_state_event`.
typedef enum sdrkit_mac_state_event_family {
  // Aircraft
  SDRKIT_MAC_STATE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MAC_STATE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MAC_STATE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MAC_STATE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MAC_STATE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MAC_STATE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MAC_STATE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MAC_STATE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MAC_STATE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MAC_STATE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MAC_STATE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MAC_STATE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MAC_STATE_EVENT_FAMILY_OTHER = 12,
} sdrkit_mac_state_event_family;

// A change of the state of the MAC receiver.
typedef struct sdrkit_mac_state_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The new state. A value of `sdrkit_mac_state_event_state`.
  int32_t state;
  // The summary of the receiver.
  sdrkit_string text;
  // What the event is about: the receiver. A value of `sdrkit_mac_state_event_family`.
  int32_t family;
} sdrkit_mac_state_event;

// The options of `state` of `sdrkit_fpv_state_event`.
typedef enum sdrkit_fpv_state_event_state {
  // Searching
  SDRKIT_FPV_STATE_EVENT_STATE_SEARCHING = 0,
  // Synchronising
  SDRKIT_FPV_STATE_EVENT_STATE_SYNC = 1,
  // Locked
  SDRKIT_FPV_STATE_EVENT_STATE_LOCKED = 2,
} sdrkit_fpv_state_event_state;

// The options of `family` of `sdrkit_fpv_state_event`.
typedef enum sdrkit_fpv_state_event_family {
  // Aircraft
  SDRKIT_FPV_STATE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FPV_STATE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FPV_STATE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FPV_STATE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FPV_STATE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FPV_STATE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FPV_STATE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FPV_STATE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FPV_STATE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FPV_STATE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FPV_STATE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FPV_STATE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FPV_STATE_EVENT_FAMILY_OTHER = 12,
} sdrkit_fpv_state_event_family;

// A change of the state of the `fpv` receiver.
typedef struct sdrkit_fpv_state_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The new state. A value of `sdrkit_fpv_state_event_state`.
  int32_t state;
  // The names of the channels at the tuned frequency.
  sdrkit_string text;
  // What the event is about: the receiver. A value of `sdrkit_fpv_state_event_family`.
  int32_t family;
} sdrkit_fpv_state_event;

// The arguments of `channel.tune` of the `atv` stack.
typedef struct sdrkit_atv_channel_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the channel in `channels`: its label.
  sdrkit_string row;
} sdrkit_atv_channel_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ATV_CHANNEL_ARGS_INIT {sizeof(sdrkit_atv_channel_args), {0}}

// The arguments of `frame`.
typedef struct sdrkit_analogtv_tv_frame_args {
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
} sdrkit_analogtv_tv_frame_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ANALOGTV_TV_FRAME_ARGS_INIT {sizeof(sdrkit_analogtv_tv_frame_args), {0}, 0, 0, 0}

// The arguments of `teletext.select`.
typedef struct sdrkit_analogtv_teletext_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the page in `teletext_pages`.
  sdrkit_string row;
} sdrkit_analogtv_teletext_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ANALOGTV_TELETEXT_SELECT_ARGS_INIT {sizeof(sdrkit_analogtv_teletext_select_args), {0}}

// The arguments of `channel.tune` of the `fpv` stack.
typedef struct sdrkit_fpv_channel_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the channel ("R5").
  sdrkit_string row;
} sdrkit_fpv_channel_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_FPV_CHANNEL_ARGS_INIT {sizeof(sdrkit_fpv_channel_args), {0}}

// The arguments of `scan` of the `fpv` stack.
typedef struct sdrkit_fpv_scan_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The plan that the scan visits. The index of an option.
  int32_t plan;
} sdrkit_fpv_scan_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_FPV_SCAN_ARGS_INIT {sizeof(sdrkit_fpv_scan_args), 0}

// Fills `rows` with the changed rows of the table "channels" of the stack "atv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `label` names a row.
static inline int32_t sdrkit_atv_channels_rows(SDRKIT_HANDLE handle,
                                              sdrkit_atv_channel_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "analogtv.AtvChannelRow", "channels", rows, sizeof(sdrkit_atv_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "atv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atv_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_teletext_page_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "atv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atv_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_teletext_subpage_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "atv.state"
// (State). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_atv_state_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_atv_state_event *event) {
  return sdrkit_typed_event(handle, n, "atv.state", "analogtv.AtvStateEvent", event, sizeof(sdrkit_atv_state_event));
}

// Runs the command "channel.tune" (Tune to a channel) of the stack "atv" on the session of `handle`. Asks the host to tune to a channel of the setting channels.
static inline int32_t sdrkit_atv_channel_tune(SDRKIT_HANDLE handle, const sdrkit_atv_channel_args *args) {
  return sdrkit_typed_command(handle, "channel.tune", "analogtv.AtvChannelArgs", args, NULL, NULL, 0);
}

// Runs the command "start" (Start sending) of the stack "atv" on the session of `handle`. Sends the television signal continuously from the start of a field: the picture of the command frame, or the test picture, and the host's audio. The transmit settings apply at each start. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_atv_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "atv" on the session of `handle`. Ends the transmission at the end of a field, with a fade of 5 ms.
static inline int32_t sdrkit_atv_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "frame" (Set the picture) of the stack "atv" on the session of `handle`. Sends this picture from the start of the next field until the next one: a still picture, or video when the host gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and below or at the sides.
static inline int32_t sdrkit_atv_frame(SDRKIT_HANDLE handle, const sdrkit_analogtv_tv_frame_args *args) {
  return sdrkit_typed_command(handle, "frame", "analogtv.TvFrameArgs", args, NULL, NULL, 0);
}

// Runs the command "teletext.select" (Select a teletext page) of the stack "atv" on the session of `handle`. Shows a teletext page.
static inline int32_t sdrkit_atv_teletext_select(SDRKIT_HANDLE handle, const sdrkit_analogtv_teletext_select_args *args) {
  return sdrkit_typed_command(handle, "teletext.select", "analogtv.TeletextSelectArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "atv" are the sdrkit_atv_gen_*() functions
// after the settings of its receiver.

// The setting "modulation" (Modulation) of the stack "atv". A value of `sdrkit_atv_modulation`. Broadcast television is AM. FM video needs 20 Msps.
static inline int32_t sdrkit_atv_get_modulation(SDRKIT_HANDLE handle, sdrkit_atv_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_modulation)index;
  return code;
}
static inline int32_t sdrkit_atv_set_modulation(SDRKIT_HANDLE handle, sdrkit_atv_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "modulation", (int32_t)value);
}

// The setting "channels" (Channels) of the stack "atv". The channels: entries separated by ";", each a label, "=" and the frequency in Hz, for example "E21=474000000" or "R5=5806000000". At most 256 entries for atv and 64 for fpv. The host fills it from a channel plan file. The table channels lists them and channel.tune takes a label. Empty: no channel.
static inline int32_t sdrkit_atv_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_atv_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "system" (System) of the stack "atv". A value of `sdrkit_atv_system`. Automatic finds the system from the signal. E needs 20 Msps for its 14 MHz channel.
static inline int32_t sdrkit_atv_get_system(SDRKIT_HANDLE handle, sdrkit_atv_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_system)index;
  return code;
}
static inline int32_t sdrkit_atv_set_system(SDRKIT_HANDLE handle, sdrkit_atv_system value) {
  return sdrkit_typed_setting_set_enum(handle, "system", (int32_t)value);
}

// The setting "colour" (Colour system) of the stack "atv". A value of `sdrkit_atv_colour`.
static inline int32_t sdrkit_atv_get_colour(SDRKIT_HANDLE handle, sdrkit_atv_colour *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "colour", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_colour)index;
  return code;
}
static inline int32_t sdrkit_atv_set_colour(SDRKIT_HANDLE handle, sdrkit_atv_colour value) {
  return sdrkit_typed_setting_set_enum(handle, "colour", (int32_t)value);
}

// The setting "colour_on" (Colour) of the stack "atv".
static inline int32_t sdrkit_atv_get_colour_on(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "colour_on", value);
}
static inline int32_t sdrkit_atv_set_colour_on(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "colour_on", value);
}

// The setting "saturation" (Saturation) of the stack "atv".
static inline int32_t sdrkit_atv_get_saturation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "saturation", value);
}
static inline int32_t sdrkit_atv_set_saturation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "saturation", value);
}

// The setting "tint" (Tint) of the stack "atv". Unit: °. NTSC only. PAL and SECAM correct themselves.
static inline int32_t sdrkit_atv_get_tint(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tint", value);
}
static inline int32_t sdrkit_atv_set_tint(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tint", value);
}

// The setting "detector" (Envelope detector) of the stack "atv". A value of `sdrkit_atv_detector`.
static inline int32_t sdrkit_atv_get_detector(SDRKIT_HANDLE handle, sdrkit_atv_detector *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "detector", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_detector)index;
  return code;
}
static inline int32_t sdrkit_atv_set_detector(SDRKIT_HANDLE handle, sdrkit_atv_detector value) {
  return sdrkit_typed_setting_set_enum(handle, "detector", (int32_t)value);
}

// The setting "deinterlace" (Fields) of the stack "atv". A value of `sdrkit_atv_deinterlace`.
static inline int32_t sdrkit_atv_get_deinterlace(SDRKIT_HANDLE handle, sdrkit_atv_deinterlace *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deinterlace", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_deinterlace)index;
  return code;
}
static inline int32_t sdrkit_atv_set_deinterlace(SDRKIT_HANDLE handle, sdrkit_atv_deinterlace value) {
  return sdrkit_typed_setting_set_enum(handle, "deinterlace", (int32_t)value);
}

// The setting "noise_reduction" (Noise reduction) of the stack "atv". A value of `sdrkit_atv_noise_reduction`.
static inline int32_t sdrkit_atv_get_noise_reduction(SDRKIT_HANDLE handle, sdrkit_atv_noise_reduction *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "noise_reduction", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_noise_reduction)index;
  return code;
}
static inline int32_t sdrkit_atv_set_noise_reduction(SDRKIT_HANDLE handle, sdrkit_atv_noise_reduction value) {
  return sdrkit_typed_setting_set_enum(handle, "noise_reduction", (int32_t)value);
}

// The setting "black_setup" (Black setup) of the stack "atv". A value of `sdrkit_atv_black_setup`.
static inline int32_t sdrkit_atv_get_black_setup(SDRKIT_HANDLE handle, sdrkit_atv_black_setup *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "black_setup", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_black_setup)index;
  return code;
}
static inline int32_t sdrkit_atv_set_black_setup(SDRKIT_HANDLE handle, sdrkit_atv_black_setup value) {
  return sdrkit_typed_setting_set_enum(handle, "black_setup", (int32_t)value);
}

// The setting "chroma_delay" (Chroma delay) of the stack "atv". Unit: ns. -1 is the standard's 170 ns.
static inline int32_t sdrkit_atv_get_chroma_delay(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "chroma_delay", value);
}
static inline int32_t sdrkit_atv_set_chroma_delay(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "chroma_delay", value);
}

// The setting "channel_width" (Channel width) of the stack "atv". A value of `sdrkit_atv_channel_width`. Only matters for the carrier offset with 5.5 MHz sound.
static inline int32_t sdrkit_atv_get_channel_width(SDRKIT_HANDLE handle, sdrkit_atv_channel_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel_width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_channel_width)index;
  return code;
}
static inline int32_t sdrkit_atv_set_channel_width(SDRKIT_HANDLE handle, sdrkit_atv_channel_width value) {
  return sdrkit_typed_setting_set_enum(handle, "channel_width", (int32_t)value);
}

// The setting "snow" (Show snow when there is no picture) of the stack "atv".
static inline int32_t sdrkit_atv_get_snow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "snow", value);
}
static inline int32_t sdrkit_atv_set_snow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "snow", value);
}

// The setting "sound_system" (Sound) of the stack "atv". A value of `sdrkit_atv_sound_system`.
static inline int32_t sdrkit_atv_get_sound_system(SDRKIT_HANDLE handle, sdrkit_atv_sound_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sound_system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_sound_system)index;
  return code;
}
static inline int32_t sdrkit_atv_set_sound_system(SDRKIT_HANDLE handle, sdrkit_atv_sound_system value) {
  return sdrkit_typed_setting_set_enum(handle, "sound_system", (int32_t)value);
}

// The setting "dual_sound" (Two languages) of the stack "atv". A value of `sdrkit_atv_dual_sound`.
static inline int32_t sdrkit_atv_get_dual_sound(SDRKIT_HANDLE handle, sdrkit_atv_dual_sound *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "dual_sound", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_dual_sound)index;
  return code;
}
static inline int32_t sdrkit_atv_set_dual_sound(SDRKIT_HANDLE handle, sdrkit_atv_dual_sound value) {
  return sdrkit_typed_setting_set_enum(handle, "dual_sound", (int32_t)value);
}

// The setting "nbtv_lines" (Lines) of the stack "atv".
static inline int32_t sdrkit_atv_get_nbtv_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "nbtv_lines", value);
}
static inline int32_t sdrkit_atv_set_nbtv_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "nbtv_lines", value);
}

// The setting "nbtv_rate" (Pictures/s) of the stack "atv".
static inline int32_t sdrkit_atv_get_nbtv_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "nbtv_rate", value);
}
static inline int32_t sdrkit_atv_set_nbtv_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "nbtv_rate", value);
}

// The setting "nbtv_deviation" (FM deviation) of the stack "atv". Unit: kHz.
static inline int32_t sdrkit_atv_get_nbtv_deviation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "nbtv_deviation", value);
}
static inline int32_t sdrkit_atv_set_nbtv_deviation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "nbtv_deviation", value);
}

// The setting "nbtv_vertical" (Vertical scan) of the stack "atv".
static inline int32_t sdrkit_atv_get_nbtv_vertical(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "nbtv_vertical", value);
}
static inline int32_t sdrkit_atv_set_nbtv_vertical(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "nbtv_vertical", value);
}

// The setting "tx_system" (System) of the stack "atv". A value of `sdrkit_atv_tx_system`.
static inline int32_t sdrkit_atv_get_tx_system(SDRKIT_HANDLE handle, sdrkit_atv_tx_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_tx_system)index;
  return code;
}
static inline int32_t sdrkit_atv_set_tx_system(SDRKIT_HANDLE handle, sdrkit_atv_tx_system value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_system", (int32_t)value);
}

// The setting "tx_colour" (Colour) of the stack "atv". A value of `sdrkit_atv_tx_colour`. By system: NTSC on M, SECAM on L, L' and K', none on A, C and E, PAL on the others.
static inline int32_t sdrkit_atv_get_tx_colour(SDRKIT_HANDLE handle, sdrkit_atv_tx_colour *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_colour", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_tx_colour)index;
  return code;
}
static inline int32_t sdrkit_atv_set_tx_colour(SDRKIT_HANDLE handle, sdrkit_atv_tx_colour value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_colour", (int32_t)value);
}

// The setting "tx_picture" (Test picture) of the stack "atv". A value of `sdrkit_atv_tx_picture`. What is sent until the host gives a picture with the command frame.
static inline int32_t sdrkit_atv_get_tx_picture(SDRKIT_HANDLE handle, sdrkit_atv_tx_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_tx_picture)index;
  return code;
}
static inline int32_t sdrkit_atv_set_tx_picture(SDRKIT_HANDLE handle, sdrkit_atv_tx_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_picture", (int32_t)value);
}

// The setting "tx_sound" (Sound) of the stack "atv". On: the host's audio (mono) on the sound carrier, an unmodulated carrier while it gives none. Off: no sound carrier.
static inline int32_t sdrkit_atv_get_tx_sound(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_sound", value);
}
static inline int32_t sdrkit_atv_set_tx_sound(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_sound", value);
}

// The setting "tx_compression" (Sync compression) of the stack "atv". Unit: %.
static inline int32_t sdrkit_atv_get_tx_compression(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_compression", value);
}
static inline int32_t sdrkit_atv_set_tx_compression(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_compression", value);
}

// The setting "tx_group_delay" (Group-delay pre-correction) of the stack "atv". The chrominance 170 ns early, as BT.470 asks of a PAL or NTSC transmitter.
static inline int32_t sdrkit_atv_get_tx_group_delay(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_group_delay", value);
}
static inline int32_t sdrkit_atv_set_tx_group_delay(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_group_delay", value);
}

// The setting "tx_nyquist" (Nyquist slope) of the stack "atv".
static inline int32_t sdrkit_atv_get_tx_nyquist(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_nyquist", value);
}
static inline int32_t sdrkit_atv_set_tx_nyquist(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_nyquist", value);
}

// The setting "tx_setup" (Black setup in 525 lines) of the stack "atv". A value of `sdrkit_atv_tx_setup`.
static inline int32_t sdrkit_atv_get_tx_setup(SDRKIT_HANDLE handle, sdrkit_atv_tx_setup *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_setup", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_tx_setup)index;
  return code;
}
static inline int32_t sdrkit_atv_set_tx_setup(SDRKIT_HANDLE handle, sdrkit_atv_tx_setup value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_setup", (int32_t)value);
}

// The setting "tx_secam_id" (SECAM identification) of the stack "atv". A value of `sdrkit_atv_tx_secam_id`.
static inline int32_t sdrkit_atv_get_tx_secam_id(SDRKIT_HANDLE handle, sdrkit_atv_tx_secam_id *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_secam_id", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_tx_secam_id)index;
  return code;
}
static inline int32_t sdrkit_atv_set_tx_secam_id(SDRKIT_HANDLE handle, sdrkit_atv_tx_secam_id value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_secam_id", (int32_t)value);
}

// The setting "gen_system" (System) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_system`.
static inline int32_t sdrkit_atv_gen_get_system(SDRKIT_HANDLE handle, sdrkit_atv_gen_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_system)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_system(SDRKIT_HANDLE handle, sdrkit_atv_gen_system value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_system", (int32_t)value);
}

// The setting "gen_colour" (Colour) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_colour`.
static inline int32_t sdrkit_atv_gen_get_colour(SDRKIT_HANDLE handle, sdrkit_atv_gen_colour *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_colour", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_colour)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_colour(SDRKIT_HANDLE handle, sdrkit_atv_gen_colour value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_colour", (int32_t)value);
}

// The setting "gen_picture" (Picture) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_picture`.
static inline int32_t sdrkit_atv_gen_get_picture(SDRKIT_HANDLE handle, sdrkit_atv_gen_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_picture)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_picture(SDRKIT_HANDLE handle, sdrkit_atv_gen_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_picture", (int32_t)value);
}

// The setting "gen_sound" (Sound) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_sound`.
static inline int32_t sdrkit_atv_gen_get_sound(SDRKIT_HANDLE handle, sdrkit_atv_gen_sound *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_sound", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_sound)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_sound(SDRKIT_HANDLE handle, sdrkit_atv_gen_sound value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_sound", (int32_t)value);
}

// The setting "gen_second" (Second sound) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_second`.
static inline int32_t sdrkit_atv_gen_get_second(SDRKIT_HANDLE handle, sdrkit_atv_gen_second *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_second", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_second)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_second(SDRKIT_HANDLE handle, sdrkit_atv_gen_second value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_second", (int32_t)value);
}

// The setting "gen_compression" (Sync compression) of the test signal generator of the stack "atv". Unit: %.
static inline int32_t sdrkit_atv_gen_get_compression(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_compression", value);
}
static inline int32_t sdrkit_atv_gen_set_compression(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_compression", value);
}

// The setting "gen_group_delay" (Group-delay pre-correction) of the test signal generator of the stack "atv".
static inline int32_t sdrkit_atv_gen_get_group_delay(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_group_delay", value);
}
static inline int32_t sdrkit_atv_gen_set_group_delay(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_group_delay", value);
}

// The setting "gen_nyquist" (Nyquist-slope transmitter) of the test signal generator of the stack "atv".
static inline int32_t sdrkit_atv_gen_get_nyquist(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nyquist", value);
}
static inline int32_t sdrkit_atv_gen_set_nyquist(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nyquist", value);
}

// The setting "gen_setup" (Black setup in 525 lines) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_setup`.
static inline int32_t sdrkit_atv_gen_get_setup(SDRKIT_HANDLE handle, sdrkit_atv_gen_setup *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_setup", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_setup)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_setup(SDRKIT_HANDLE handle, sdrkit_atv_gen_setup value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_setup", (int32_t)value);
}

// The setting "gen_secam_id" (SECAM identification) of the test signal generator of the stack "atv". A value of `sdrkit_atv_gen_secam_id`.
static inline int32_t sdrkit_atv_gen_get_secam_id(SDRKIT_HANDLE handle, sdrkit_atv_gen_secam_id *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_secam_id", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_atv_gen_secam_id)index;
  return code;
}
static inline int32_t sdrkit_atv_gen_set_secam_id(SDRKIT_HANDLE handle, sdrkit_atv_gen_secam_id value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_secam_id", (int32_t)value);
}

// The setting "gen_ident" (Ident text) of the test signal generator of the stack "atv".
static inline int32_t sdrkit_atv_gen_get_ident(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ident", value);
}
static inline int32_t sdrkit_atv_gen_set_ident(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ident", value);
}

// The setting "gen_teletext" (Teletext) of the test signal generator of the stack "atv". Three pages of teletext in the field blanking (625 lines).
static inline int32_t sdrkit_atv_gen_get_teletext(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_teletext", value);
}
static inline int32_t sdrkit_atv_gen_set_teletext(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_teletext", value);
}

// The setting "gen_start" (Clock start time) of the test signal generator of the stack "atv". Unit: s.
static inline int32_t sdrkit_atv_gen_get_start(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_start", value);
}
static inline int32_t sdrkit_atv_gen_set_start(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_start", value);
}

// The setting "gen_nbtv_lines" (NBTV lines) of the test signal generator of the stack "atv".
static inline int32_t sdrkit_atv_gen_get_nbtv_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_nbtv_lines", value);
}
static inline int32_t sdrkit_atv_gen_set_nbtv_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_nbtv_lines", value);
}

// The setting "gen_nbtv_rate" (NBTV pictures/s) of the test signal generator of the stack "atv".
static inline int32_t sdrkit_atv_gen_get_nbtv_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_nbtv_rate", value);
}
static inline int32_t sdrkit_atv_gen_set_nbtv_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_nbtv_rate", value);
}

// The setting "gen_nbtv_pattern" (NBTV picture) of the test signal generator of the stack "atv".
static inline int32_t sdrkit_atv_gen_get_nbtv_pattern(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_nbtv_pattern", value);
}
static inline int32_t sdrkit_atv_gen_set_nbtv_pattern(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_nbtv_pattern", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mac.state"
// (State). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mac_state_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mac_state_event *event) {
  return sdrkit_typed_event(handle, n, "mac.state", "analogtv.MacStateEvent", event, sizeof(sdrkit_mac_state_event));
}

// The settings of the test signal generator of the stack "mac" are the sdrkit_mac_gen_*() functions
// after the settings of its receiver.

// The setting "gen_variant" (Variant) of the test signal generator of the stack "mac". A value of `sdrkit_mac_gen_variant`.
static inline int32_t sdrkit_mac_gen_get_variant(SDRKIT_HANDLE handle, sdrkit_mac_gen_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mac_gen_variant)index;
  return code;
}
static inline int32_t sdrkit_mac_gen_set_variant(SDRKIT_HANDLE handle, sdrkit_mac_gen_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_variant", (int32_t)value);
}

// The setting "gen_scrambled" (Scrambled (EuroCrypt)) of the test signal generator of the stack "mac".
static inline int32_t sdrkit_mac_gen_get_scrambled(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_scrambled", value);
}
static inline int32_t sdrkit_mac_gen_set_scrambled(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_scrambled", value);
}

// The setting "gen_wide" (16:9) of the test signal generator of the stack "mac".
static inline int32_t sdrkit_mac_gen_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_wide", value);
}
static inline int32_t sdrkit_mac_gen_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_wide", value);
}

// The setting "gen_picture" (Picture) of the test signal generator of the stack "mac". A value of `sdrkit_mac_gen_picture`.
static inline int32_t sdrkit_mac_gen_get_picture(SDRKIT_HANDLE handle, sdrkit_mac_gen_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mac_gen_picture)index;
  return code;
}
static inline int32_t sdrkit_mac_gen_set_picture(SDRKIT_HANDLE handle, sdrkit_mac_gen_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_picture", (int32_t)value);
}

// The setting "gen_tone" (Tone) of the test signal generator of the stack "mac". A frequency in Hz.
static inline int32_t sdrkit_mac_gen_get_tone(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone", value);
}
static inline int32_t sdrkit_mac_gen_set_tone(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone", value);
}

// The setting "gen_channel" (Channel name) of the test signal generator of the stack "mac".
static inline int32_t sdrkit_mac_gen_get_channel(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_channel", value);
}
static inline int32_t sdrkit_mac_gen_set_channel(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_channel", value);
}

// The setting "gen_network" (Network name) of the test signal generator of the stack "mac".
static inline int32_t sdrkit_mac_gen_get_network(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_network", value);
}
static inline int32_t sdrkit_mac_gen_set_network(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_network", value);
}

// The setting "gen_country" (Country) of the test signal generator of the stack "mac".
static inline int32_t sdrkit_mac_gen_get_country(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_country", value);
}
static inline int32_t sdrkit_mac_gen_set_country(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_country", value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "fpv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_fpv_channels_rows(SDRKIT_HANDLE handle,
                                              sdrkit_fpv_channel_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "analogtv.FpvChannelRow", "channels", rows, sizeof(sdrkit_fpv_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "fpv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fpv_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_teletext_page_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "fpv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fpv_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_teletext_subpage_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fpv.state"
// (State). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fpv_state_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fpv_state_event *event) {
  return sdrkit_typed_event(handle, n, "fpv.state", "analogtv.FpvStateEvent", event, sizeof(sdrkit_fpv_state_event));
}

// Runs the command "clear" (Clear the channel levels) of the stack "fpv" on the session of `handle`. Forgets the channel levels, the noise floor and the level history.
static inline int32_t sdrkit_fpv_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "channel.tune" (Tune to a channel) of the stack "fpv" on the session of `handle`. Asks the host to tune to an FPV channel.
static inline int32_t sdrkit_fpv_channel_tune(SDRKIT_HANDLE handle, const sdrkit_fpv_channel_args *args) {
  return sdrkit_typed_command(handle, "channel.tune", "analogtv.FpvChannelArgs", args, NULL, NULL, 0);
}

// Runs the command "scan" (Scan) of the stack "fpv" on the session of `handle`. Asks the host to visit every channel of a plan for 0.7 s each.
static inline int32_t sdrkit_fpv_scan(SDRKIT_HANDLE handle, const sdrkit_fpv_scan_args *args) {
  return sdrkit_typed_command(handle, "scan", "analogtv.FpvScanArgs", args, NULL, NULL, 0);
}

// Runs the command "teletext.select" (Select a teletext page) of the stack "fpv" on the session of `handle`. Shows a teletext page.
static inline int32_t sdrkit_fpv_teletext_select(SDRKIT_HANDLE handle, const sdrkit_analogtv_teletext_select_args *args) {
  return sdrkit_typed_command(handle, "teletext.select", "analogtv.TeletextSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "start" (Start sending) of the stack "fpv" on the session of `handle`. Sends the television signal continuously from the start of a field: the picture of the command frame, or the test picture, and the host's audio. The transmit settings apply at each start. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_fpv_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "fpv" on the session of `handle`. Ends the transmission at the end of a field, with a fade of 5 ms.
static inline int32_t sdrkit_fpv_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "frame" (Set the picture) of the stack "fpv" on the session of `handle`. Sends this picture from the start of the next field until the next one: a still picture, or video when the host gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and below or at the sides.
static inline int32_t sdrkit_fpv_frame(SDRKIT_HANDLE handle, const sdrkit_analogtv_tv_frame_args *args) {
  return sdrkit_typed_command(handle, "frame", "analogtv.TvFrameArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fpv" are the sdrkit_fpv_gen_*() functions
// after the settings of its receiver.

// The setting "system" (System) of the stack "fpv". A value of `sdrkit_fpv_system`.
static inline int32_t sdrkit_fpv_get_system(SDRKIT_HANDLE handle, sdrkit_fpv_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_system)index;
  return code;
}
static inline int32_t sdrkit_fpv_set_system(SDRKIT_HANDLE handle, sdrkit_fpv_system value) {
  return sdrkit_typed_setting_set_enum(handle, "system", (int32_t)value);
}

// The setting "channels" (Channels) of the stack "fpv". The channels: entries separated by ";", each a label, "=" and the frequency in Hz, for example "E21=474000000" or "R5=5806000000". At most 256 entries for atv and 64 for fpv. The host fills it from a channel plan file. The table channels lists them and channel.tune takes a label. Empty: no channel.
static inline int32_t sdrkit_fpv_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_fpv_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "sound_subcarrier" (Sound subcarrier) of the stack "fpv". A value of `sdrkit_fpv_sound_subcarrier`.
static inline int32_t sdrkit_fpv_get_sound_subcarrier(SDRKIT_HANDLE handle, sdrkit_fpv_sound_subcarrier *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sound_subcarrier", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_sound_subcarrier)index;
  return code;
}
static inline int32_t sdrkit_fpv_set_sound_subcarrier(SDRKIT_HANDLE handle, sdrkit_fpv_sound_subcarrier value) {
  return sdrkit_typed_setting_set_enum(handle, "sound_subcarrier", (int32_t)value);
}

// The setting "colour_on" (Colour) of the stack "fpv".
static inline int32_t sdrkit_fpv_get_colour_on(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "colour_on", value);
}
static inline int32_t sdrkit_fpv_set_colour_on(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "colour_on", value);
}

// The setting "saturation" (Saturation) of the stack "fpv".
static inline int32_t sdrkit_fpv_get_saturation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "saturation", value);
}
static inline int32_t sdrkit_fpv_set_saturation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "saturation", value);
}

// The setting "deinterlace" (Fields) of the stack "fpv". A value of `sdrkit_fpv_deinterlace`.
static inline int32_t sdrkit_fpv_get_deinterlace(SDRKIT_HANDLE handle, sdrkit_fpv_deinterlace *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deinterlace", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_deinterlace)index;
  return code;
}
static inline int32_t sdrkit_fpv_set_deinterlace(SDRKIT_HANDLE handle, sdrkit_fpv_deinterlace value) {
  return sdrkit_typed_setting_set_enum(handle, "deinterlace", (int32_t)value);
}

// The setting "audio" (Audio) of the stack "fpv".
static inline int32_t sdrkit_fpv_get_audio(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio", value);
}
static inline int32_t sdrkit_fpv_set_audio(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio", value);
}

// The setting "tx_system" (System) of the stack "fpv". A value of `sdrkit_fpv_tx_system`.
static inline int32_t sdrkit_fpv_get_tx_system(SDRKIT_HANDLE handle, sdrkit_fpv_tx_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_tx_system)index;
  return code;
}
static inline int32_t sdrkit_fpv_set_tx_system(SDRKIT_HANDLE handle, sdrkit_fpv_tx_system value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_system", (int32_t)value);
}

// The setting "tx_picture" (Test picture) of the stack "fpv". A value of `sdrkit_fpv_tx_picture`. What is sent until the host gives a picture with the command frame.
static inline int32_t sdrkit_fpv_get_tx_picture(SDRKIT_HANDLE handle, sdrkit_fpv_tx_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_tx_picture)index;
  return code;
}
static inline int32_t sdrkit_fpv_set_tx_picture(SDRKIT_HANDLE handle, sdrkit_fpv_tx_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_picture", (int32_t)value);
}

// The setting "tx_sound" (Sound) of the stack "fpv". On: the host's audio (mono) on the sound carrier, an unmodulated carrier while it gives none. Off: no sound carrier.
static inline int32_t sdrkit_fpv_get_tx_sound(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_sound", value);
}
static inline int32_t sdrkit_fpv_set_tx_sound(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_sound", value);
}

// The setting "tx_subcarrier" (Sound subcarrier) of the stack "fpv". A value of `sdrkit_fpv_tx_subcarrier`. The subcarrier of the sound in the composite: FM, 25 kHz deviation at full scale.
static inline int32_t sdrkit_fpv_get_tx_subcarrier(SDRKIT_HANDLE handle, sdrkit_fpv_tx_subcarrier *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_subcarrier", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_tx_subcarrier)index;
  return code;
}
static inline int32_t sdrkit_fpv_set_tx_subcarrier(SDRKIT_HANDLE handle, sdrkit_fpv_tx_subcarrier value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_subcarrier", (int32_t)value);
}

// The setting "tx_deviation_mhz" (Deviation) of the stack "fpv". Unit: MHz. The swing from the middle to the sync tip and to peak white.
static inline int32_t sdrkit_fpv_get_tx_deviation_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_deviation_mhz", value);
}
static inline int32_t sdrkit_fpv_set_tx_deviation_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_deviation_mhz", value);
}

// The setting "tx_sync_high" (Sync tip at the highest frequency) of the stack "fpv".
static inline int32_t sdrkit_fpv_get_tx_sync_high(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_sync_high", value);
}
static inline int32_t sdrkit_fpv_set_tx_sync_high(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_sync_high", value);
}

// The setting "tx_ac_ms" (AC coupling time constant) of the stack "fpv". Unit: ms.
static inline int32_t sdrkit_fpv_get_tx_ac_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_ac_ms", value);
}
static inline int32_t sdrkit_fpv_set_tx_ac_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_ac_ms", value);
}

// The setting "gen_system" (System) of the test signal generator of the stack "fpv". A value of `sdrkit_fpv_gen_system`.
static inline int32_t sdrkit_fpv_gen_get_system(SDRKIT_HANDLE handle, sdrkit_fpv_gen_system *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_system", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_gen_system)index;
  return code;
}
static inline int32_t sdrkit_fpv_gen_set_system(SDRKIT_HANDLE handle, sdrkit_fpv_gen_system value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_system", (int32_t)value);
}

// The setting "gen_picture" (Picture) of the test signal generator of the stack "fpv". A value of `sdrkit_fpv_gen_picture`.
static inline int32_t sdrkit_fpv_gen_get_picture(SDRKIT_HANDLE handle, sdrkit_fpv_gen_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_gen_picture)index;
  return code;
}
static inline int32_t sdrkit_fpv_gen_set_picture(SDRKIT_HANDLE handle, sdrkit_fpv_gen_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_picture", (int32_t)value);
}

// The setting "gen_sound" (Sound) of the test signal generator of the stack "fpv". A value of `sdrkit_fpv_gen_sound`.
static inline int32_t sdrkit_fpv_gen_get_sound(SDRKIT_HANDLE handle, sdrkit_fpv_gen_sound *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_sound", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fpv_gen_sound)index;
  return code;
}
static inline int32_t sdrkit_fpv_gen_set_sound(SDRKIT_HANDLE handle, sdrkit_fpv_gen_sound value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_sound", (int32_t)value);
}

// The setting "gen_none" (No transmitter) of the test signal generator of the stack "fpv".
static inline int32_t sdrkit_fpv_gen_get_none(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_none", value);
}
static inline int32_t sdrkit_fpv_gen_set_none(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_none", value);
}

// The setting "gen_centre_mhz" (Capture centre from the transmitter) of the test signal generator of the stack "fpv". Unit: MHz.
static inline int32_t sdrkit_fpv_gen_get_centre_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_mhz", value);
}
static inline int32_t sdrkit_fpv_gen_set_centre_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_mhz", value);
}

// The setting "gen_deviation_mhz" (Deviation) of the test signal generator of the stack "fpv". Unit: MHz.
static inline int32_t sdrkit_fpv_gen_get_deviation_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_mhz", value);
}
static inline int32_t sdrkit_fpv_gen_set_deviation_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_mhz", value);
}

// The setting "gen_sync_high" (Sync tip at the highest frequency) of the test signal generator of the stack "fpv".
static inline int32_t sdrkit_fpv_gen_get_sync_high(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_sync_high", value);
}
static inline int32_t sdrkit_fpv_gen_set_sync_high(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_sync_high", value);
}

// The setting "gen_ac_ms" (AC coupling time constant) of the test signal generator of the stack "fpv". Unit: ms.
static inline int32_t sdrkit_fpv_gen_get_ac_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ac_ms", value);
}
static inline int32_t sdrkit_fpv_gen_set_ac_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ac_ms", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_atv_fields_read(SDRKIT_HANDLE handle, sdrkit_atv_fields *fields) {
  return sdrkit_typed_fields(handle, "analogtv.AtvFields", fields, sizeof(sdrkit_atv_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_analogtv_teletext_fields_read(SDRKIT_HANDLE handle, sdrkit_analogtv_teletext_fields *fields) {
  return sdrkit_typed_fields(handle, "analogtv.TeletextFields", fields, sizeof(sdrkit_analogtv_teletext_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_analogtv_tv_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_analogtv_tv_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "analogtv.TvTxFields", fields, sizeof(sdrkit_analogtv_tv_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mac_fields_read(SDRKIT_HANDLE handle, sdrkit_mac_fields *fields) {
  return sdrkit_typed_fields(handle, "analogtv.MacFields", fields, sizeof(sdrkit_mac_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fpv_fields_read(SDRKIT_HANDLE handle, sdrkit_fpv_fields *fields) {
  return sdrkit_typed_fields(handle, "analogtv.FpvFields", fields, sizeof(sdrkit_fpv_fields));
}

#endif
