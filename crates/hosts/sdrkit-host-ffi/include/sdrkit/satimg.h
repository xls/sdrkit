/*
 * sdrkit: the typed interface of the stack family "satimg", version 1.2. MIT licence.
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

#ifndef SDRKIT_SATIMG_H
#define SDRKIT_SATIMG_H

#include "../sdrkit.h"
#include "image.h"
#include "dvbs2.h"
#include "ipcast.h"

// The version of the family that this header describes.
#define SDRKIT_SATIMG_VERSION_MAJOR 1
#define SDRKIT_SATIMG_VERSION_MINOR 2

// The stack "meteor_lrpt": its id, and the keys of its event types.
#define SDRKIT_STACK_METEOR_LRPT "meteor_lrpt"

// The stack "goes_hrit": its id, and the keys of its event types.
#define SDRKIT_STACK_GOES_HRIT "goes_hrit"
#define SDRKIT_EVENT_SATIMG_PRODUCT "satimg.product"
#define SDRKIT_EVENT_SATIMG_TEXT "satimg.text"

// The stack "gk2a_lrit": its id, and the keys of its event types.
#define SDRKIT_STACK_GK2A_LRIT "gk2a_lrit"

// The stack "gk2a_hrit": its id, and the keys of its event types.
#define SDRKIT_STACK_GK2A_HRIT "gk2a_hrit"

// The stack "fy2_svissr": its id, and the keys of its event types.
#define SDRKIT_STACK_FY2_SVISSR "fy2_svissr"

// The stack "fy3_ahrpt": its id, and the keys of its event types.
#define SDRKIT_STACK_FY3_AHRPT "fy3_ahrpt"

// The stack "fy3_mpt": its id, and the keys of its event types.
#define SDRKIT_STACK_FY3_MPT "fy3_mpt"

// The stack "noaa_apt": its id, and the keys of its event types.
#define SDRKIT_STACK_NOAA_APT "noaa_apt"

// The stack "noaa_dsb": its id, and the keys of its event types.
#define SDRKIT_STACK_NOAA_DSB "noaa_dsb"

// The stack "goes_gvar": its id, and the keys of its event types.
#define SDRKIT_STACK_GOES_GVAR "goes_gvar"

// The stack "aws": its id, and the keys of its event types.
#define SDRKIT_STACK_AWS "aws"

// The stack "noaa_gac": its id, and the keys of its event types.
#define SDRKIT_STACK_NOAA_GAC "noaa_gac"

// The stack "msg_raw": its id, and the keys of its event types.
#define SDRKIT_STACK_MSG_RAW "msg_raw"

// The stack "goes_cda": its id, and the keys of its event types.
#define SDRKIT_STACK_GOES_CDA "goes_cda"

// The stack "goes_mdl": its id, and the keys of its event types.
#define SDRKIT_STACK_GOES_MDL "goes_mdl"

// The stack "gk2a_cdas": its id, and the keys of its event types.
#define SDRKIT_STACK_GK2A_CDAS "gk2a_cdas"

// The stack "sband_ccsds": its id, and the keys of its event types.
#define SDRKIT_STACK_SBAND_CCSDS "sband_ccsds"

// The stack "rdas": its id, and the keys of its event types.
#define SDRKIT_STACK_RDAS "rdas"

// The stack "gk2a_uhrit": its id, and the keys of its event types.
#define SDRKIT_STACK_GK2A_UHRIT "gk2a_uhrit"

// The stack "fy4_lrit": its id, and the keys of its event types.
#define SDRKIT_STACK_FY4_LRIT "fy4_lrit"

// The stack "fy4a_hrit": its id, and the keys of its event types.
#define SDRKIT_STACK_FY4A_HRIT "fy4a_hrit"

// The stack "goes_grb": its id, and the keys of its event types.
#define SDRKIT_STACK_GOES_GRB "goes_grb"

// The stack "sat_filecast": its id, and the keys of its event types.
#define SDRKIT_STACK_SAT_FILECAST "sat_filecast"

// The stack "elektro_l": its id, and the keys of its event types.
#define SDRKIT_STACK_ELEKTRO_L "elektro_l"
#define SDRKIT_EVENT_SATIMG_GGAK_FRAME "satimg.ggak_frame"

// The stack "hrpt": its id, and the keys of its event types.
#define SDRKIT_STACK_HRPT "hrpt"

// The stack "eodb": its id, and the keys of its event types.
#define SDRKIT_STACK_EODB "eodb"

// The options of the setting "frequency_preset" of the stack "meteor_lrpt".
typedef enum sdrkit_meteor_lrpt_frequency_preset {
  // 137.900 MHz (N2-4, N2-3)
  SDRKIT_METEOR_LRPT_FREQUENCY_PRESET_137_900 = 0,
  // 137.100 MHz
  SDRKIT_METEOR_LRPT_FREQUENCY_PRESET_137_100 = 1,
} sdrkit_meteor_lrpt_frequency_preset;

// The options of the setting "modulation" of the stack "meteor_lrpt".
typedef enum sdrkit_meteor_lrpt_modulation {
  // Automatic
  SDRKIT_METEOR_LRPT_MODULATION_AUTO = 0,
  // OQPSK 72k (N2-3, N2-4)
  SDRKIT_METEOR_LRPT_MODULATION_OQPSK72 = 1,
  // QPSK 72k (N2)
  SDRKIT_METEOR_LRPT_MODULATION_QPSK72 = 2,
  // OQPSK 80k interleaved
  SDRKIT_METEOR_LRPT_MODULATION_OQPSK80 = 3,
} sdrkit_meteor_lrpt_modulation;

// The options of the setting "view" of the stack "meteor_lrpt".
typedef enum sdrkit_meteor_lrpt_view {
  // Colour (automatic)
  SDRKIT_METEOR_LRPT_VIEW_AUTO = 0,
  // Colour 123
  SDRKIT_METEOR_LRPT_VIEW_RGB123 = 1,
  // Colour 125
  SDRKIT_METEOR_LRPT_VIEW_RGB125 = 2,
  // Channel 1
  SDRKIT_METEOR_LRPT_VIEW_CH1 = 3,
  // Channel 2
  SDRKIT_METEOR_LRPT_VIEW_CH2 = 4,
  // Channel 3
  SDRKIT_METEOR_LRPT_VIEW_CH3 = 5,
  // Channel 4
  SDRKIT_METEOR_LRPT_VIEW_CH4 = 6,
  // Channel 5
  SDRKIT_METEOR_LRPT_VIEW_CH5 = 7,
  // Channel 6
  SDRKIT_METEOR_LRPT_VIEW_CH6 = 8,
} sdrkit_meteor_lrpt_view;

// The options of the setting "enhance" of the stack "meteor_lrpt".
typedef enum sdrkit_meteor_lrpt_enhance {
  // As received
  SDRKIT_METEOR_LRPT_ENHANCE_NONE = 0,
  // Contrast stretch
  SDRKIT_METEOR_LRPT_ENHANCE_STRETCH = 1,
  // Stretch and brighten
  SDRKIT_METEOR_LRPT_ENHANCE_BRIGHTEN = 2,
  // Thermal colours
  SDRKIT_METEOR_LRPT_ENHANCE_THERMAL = 3,
  // Satellite preset
  SDRKIT_METEOR_LRPT_ENHANCE_SATELLITE = 4,
} sdrkit_meteor_lrpt_enhance;

// The options of the setting "gen_modulation" of the stack "meteor_lrpt".
typedef enum sdrkit_meteor_lrpt_gen_modulation {
  // OQPSK 72k (N2-3, N2-4)
  SDRKIT_METEOR_LRPT_GEN_MODULATION_OQPSK72 = 0,
  // QPSK 72k (N2)
  SDRKIT_METEOR_LRPT_GEN_MODULATION_QPSK72 = 1,
  // OQPSK 80k interleaved
  SDRKIT_METEOR_LRPT_GEN_MODULATION_OQPSK80 = 2,
} sdrkit_meteor_lrpt_gen_modulation;

// The options of the setting "filter" of the stack "goes_hrit".
typedef enum sdrkit_goes_hrit_filter {
  // All products
  SDRKIT_GOES_HRIT_FILTER_ALL = 0,
  // Images
  SDRKIT_GOES_HRIT_FILTER_IMAGES = 1,
  // Text
  SDRKIT_GOES_HRIT_FILTER_TEXT = 2,
} sdrkit_goes_hrit_filter;

// The options of the setting "gen_basis" of the stack "goes_hrit".
typedef enum sdrkit_goes_hrit_gen_basis {
  // Dual
  SDRKIT_GOES_HRIT_GEN_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_GOES_HRIT_GEN_BASIS_CONVENTIONAL = 1,
} sdrkit_goes_hrit_gen_basis;

// The options of the setting "filter" of the stack "gk2a_lrit".
typedef enum sdrkit_gk2a_lrit_filter {
  // All products
  SDRKIT_GK2A_LRIT_FILTER_ALL = 0,
  // Images
  SDRKIT_GK2A_LRIT_FILTER_IMAGES = 1,
  // Text
  SDRKIT_GK2A_LRIT_FILTER_TEXT = 2,
} sdrkit_gk2a_lrit_filter;

// The options of the setting "gen_basis" of the stack "gk2a_lrit".
typedef enum sdrkit_gk2a_lrit_gen_basis {
  // Dual
  SDRKIT_GK2A_LRIT_GEN_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_GK2A_LRIT_GEN_BASIS_CONVENTIONAL = 1,
} sdrkit_gk2a_lrit_gen_basis;

// The options of the setting "filter" of the stack "gk2a_hrit".
typedef enum sdrkit_gk2a_hrit_filter {
  // All products
  SDRKIT_GK2A_HRIT_FILTER_ALL = 0,
  // Images
  SDRKIT_GK2A_HRIT_FILTER_IMAGES = 1,
  // Text
  SDRKIT_GK2A_HRIT_FILTER_TEXT = 2,
} sdrkit_gk2a_hrit_filter;

// The options of the setting "gen_basis" of the stack "gk2a_hrit".
typedef enum sdrkit_gk2a_hrit_gen_basis {
  // Dual
  SDRKIT_GK2A_HRIT_GEN_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_GK2A_HRIT_GEN_BASIS_CONVENTIONAL = 1,
} sdrkit_gk2a_hrit_gen_basis;

// The options of the setting "preset" of the stack "fy3_ahrpt".
typedef enum sdrkit_fy3_ahrpt_preset {
  // Automatic
  SDRKIT_FY3_AHRPT_PRESET_AUTO = 0,
  // 1701.400 FY-3C AHRPT (2.6 Msym/s)
  SDRKIT_FY3_AHRPT_PRESET_FY3C = 1,
  // 1704.500 FY-3A / FY-3B AHRPT (2.8 Msym/s)
  SDRKIT_FY3_AHRPT_PRESET_FY3AB = 2,
  // 7790 FY-3G (6 Msym/s, X-band, needs a down-converter)
  SDRKIT_FY3_AHRPT_PRESET_FY3G = 3,
  // 7820 FY-3D (30 Msym/s, X-band, needs a down-converter and 40 Msps)
  SDRKIT_FY3_AHRPT_PRESET_FY3D = 4,
  // 7790 FY-3F (38.4 Msym/s, X-band)
  SDRKIT_FY3_AHRPT_PRESET_FY3F = 5,
  // 7860 FY-3E / FY-3H (38.4 Msym/s, X-band)
  SDRKIT_FY3_AHRPT_PRESET_FY3EH = 6,
} sdrkit_fy3_ahrpt_preset;

// The options of the setting "instrument" of the stack "fy3_ahrpt".
typedef enum sdrkit_fy3_ahrpt_instrument {
  // VIRR
  SDRKIT_FY3_AHRPT_INSTRUMENT_V_I_R_R = 0,
  // MERSI
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_E_R_S_I = 1,
  // MERSI-II
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_E_R_S_I_I_I = 2,
  // MERSI-3
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_E_R_S_I_3 = 3,
  // MERSI-LL
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_E_R_S_I_L_L = 4,
  // MERSI-RM
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_E_R_S_I_R_M = 5,
  // MWRI
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_W_R_I = 6,
  // MWRI-2
  SDRKIT_FY3_AHRPT_INSTRUMENT_M_W_R_I_2 = 7,
} sdrkit_fy3_ahrpt_instrument;

// The options of the setting "view" of the stack "fy3_ahrpt".
typedef enum sdrkit_fy3_ahrpt_view {
  // True colour (1, 9, 7)
  SDRKIT_FY3_AHRPT_VIEW_TRUE = 0,
  // False colour (2, 1, 4 inverted)
  SDRKIT_FY3_AHRPT_VIEW_FALSE = 1,
  // Channel 1 (0.63 um red)
  SDRKIT_FY3_AHRPT_VIEW_CH1 = 2,
  // Channel 2 (0.865 um)
  SDRKIT_FY3_AHRPT_VIEW_CH2 = 3,
  // Channel 3 (3.74 um)
  SDRKIT_FY3_AHRPT_VIEW_CH3 = 4,
  // Channel 4 (10.8 um)
  SDRKIT_FY3_AHRPT_VIEW_CH4 = 5,
  // Channel 5 (12 um)
  SDRKIT_FY3_AHRPT_VIEW_CH5 = 6,
  // Channel 6 (1.6 um)
  SDRKIT_FY3_AHRPT_VIEW_CH6 = 7,
  // Channel 7 (0.455 um blue)
  SDRKIT_FY3_AHRPT_VIEW_CH7 = 8,
  // Channel 8 (0.505 um)
  SDRKIT_FY3_AHRPT_VIEW_CH8 = 9,
  // Channel 9 (0.555 um green)
  SDRKIT_FY3_AHRPT_VIEW_CH9 = 10,
  // Channel 10 (1.36 um)
  SDRKIT_FY3_AHRPT_VIEW_CH10 = 11,
  // Channel 11 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH11 = 12,
  // Channel 12 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH12 = 13,
  // Channel 13 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH13 = 14,
  // Channel 14 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH14 = 15,
  // Channel 15 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH15 = 16,
  // Channel 16 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH16 = 17,
  // Channel 17 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH17 = 18,
  // Channel 18 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH18 = 19,
  // Channel 19 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH19 = 20,
  // Channel 20 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH20 = 21,
  // Channel 21 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH21 = 22,
  // Channel 22 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH22 = 23,
  // Channel 23 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH23 = 24,
  // Channel 24 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH24 = 25,
  // Channel 25 (1 km)
  SDRKIT_FY3_AHRPT_VIEW_CH25 = 26,
  // Channel 26
  SDRKIT_FY3_AHRPT_VIEW_CH26 = 27,
} sdrkit_fy3_ahrpt_view;

// The options of the setting "preset" of the stack "fy3_mpt".
typedef enum sdrkit_fy3_mpt_preset {
  // Automatic
  SDRKIT_FY3_MPT_PRESET_AUTO = 0,
  // 7750 FY-3A / FY-3B MPT (18.7 Msym/s, X-band, needs a down-converter and 28 Msps)
  SDRKIT_FY3_MPT_PRESET_MPT_AB = 1,
  // 7780 FY-3C MPT (18.7 Msym/s, X-band)
  SDRKIT_FY3_MPT_PRESET_MPT_C = 2,
} sdrkit_fy3_mpt_preset;

// The options of the setting "instrument" of the stack "fy3_mpt".
typedef enum sdrkit_fy3_mpt_instrument {
  // MERSI
  SDRKIT_FY3_MPT_INSTRUMENT_M_E_R_S_I = 0,
  // MERSI-II
  SDRKIT_FY3_MPT_INSTRUMENT_M_E_R_S_I_I_I = 1,
  // MERSI-3
  SDRKIT_FY3_MPT_INSTRUMENT_M_E_R_S_I_3 = 2,
  // MERSI-LL
  SDRKIT_FY3_MPT_INSTRUMENT_M_E_R_S_I_L_L = 3,
  // MERSI-RM
  SDRKIT_FY3_MPT_INSTRUMENT_M_E_R_S_I_R_M = 4,
  // VIRR
  SDRKIT_FY3_MPT_INSTRUMENT_V_I_R_R = 5,
  // MWRI
  SDRKIT_FY3_MPT_INSTRUMENT_M_W_R_I = 6,
  // MWRI-2
  SDRKIT_FY3_MPT_INSTRUMENT_M_W_R_I_2 = 7,
} sdrkit_fy3_mpt_instrument;

// The options of the setting "view" of the stack "fy3_mpt".
typedef enum sdrkit_fy3_mpt_view {
  // True colour (3, 2, 1)
  SDRKIT_FY3_MPT_VIEW_TRUE = 0,
  // Channel 1 (0.47 um blue)
  SDRKIT_FY3_MPT_VIEW_CH1 = 1,
  // Channel 2 (0.55 um green)
  SDRKIT_FY3_MPT_VIEW_CH2 = 2,
  // Channel 3 (0.65 um red)
  SDRKIT_FY3_MPT_VIEW_CH3 = 3,
  // Channel 4 (0.865 um)
  SDRKIT_FY3_MPT_VIEW_CH4 = 4,
  // Channel 5 (10.8 um)
  SDRKIT_FY3_MPT_VIEW_CH5 = 5,
  // Channel 6 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH6 = 6,
  // Channel 7 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH7 = 7,
  // Channel 8 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH8 = 8,
  // Channel 9 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH9 = 9,
  // Channel 10 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH10 = 10,
  // Channel 11 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH11 = 11,
  // Channel 12 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH12 = 12,
  // Channel 13 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH13 = 13,
  // Channel 14 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH14 = 14,
  // Channel 15 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH15 = 15,
  // Channel 16 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH16 = 16,
  // Channel 17 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH17 = 17,
  // Channel 18 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH18 = 18,
  // Channel 19 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH19 = 19,
  // Channel 20 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH20 = 20,
  // Channel 21 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH21 = 21,
  // Channel 22 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH22 = 22,
  // Channel 23 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH23 = 23,
  // Channel 24 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH24 = 24,
  // Channel 25 (1 km)
  SDRKIT_FY3_MPT_VIEW_CH25 = 25,
  // False colour (2, 1, 4 inverted)
  SDRKIT_FY3_MPT_VIEW_FALSE = 26,
  // Channel 26
  SDRKIT_FY3_MPT_VIEW_CH26 = 27,
} sdrkit_fy3_mpt_view;

// The options of the setting "picture" of the stack "noaa_apt".
typedef enum sdrkit_noaa_apt_picture {
  // APT
  SDRKIT_NOAA_APT_PICTURE_A_P_T = 0,
  // APT channel A
  SDRKIT_NOAA_APT_PICTURE_A_P_T_CHANNEL_A = 1,
  // APT channel B
  SDRKIT_NOAA_APT_PICTURE_A_P_T_CHANNEL_B = 2,
} sdrkit_noaa_apt_picture;

// The options of the setting "view" of the stack "noaa_dsb".
typedef enum sdrkit_noaa_dsb_view {
  // Colour (2, 1, 4)
  SDRKIT_NOAA_DSB_VIEW_RGB214 = 0,
  // HIRS channel 1
  SDRKIT_NOAA_DSB_VIEW_CH1 = 1,
  // HIRS channel 2
  SDRKIT_NOAA_DSB_VIEW_CH2 = 2,
  // HIRS channel 3
  SDRKIT_NOAA_DSB_VIEW_CH3 = 3,
  // HIRS channel 4
  SDRKIT_NOAA_DSB_VIEW_CH4 = 4,
  // HIRS channel 5
  SDRKIT_NOAA_DSB_VIEW_CH5 = 5,
  // HIRS channel 6
  SDRKIT_NOAA_DSB_VIEW_CH6 = 6,
  // HIRS channel 7
  SDRKIT_NOAA_DSB_VIEW_CH7 = 7,
  // HIRS channel 8
  SDRKIT_NOAA_DSB_VIEW_CH8 = 8,
  // HIRS channel 9
  SDRKIT_NOAA_DSB_VIEW_CH9 = 9,
  // HIRS channel 10
  SDRKIT_NOAA_DSB_VIEW_CH10 = 10,
  // HIRS channel 11
  SDRKIT_NOAA_DSB_VIEW_CH11 = 11,
  // HIRS channel 12
  SDRKIT_NOAA_DSB_VIEW_CH12 = 12,
  // HIRS channel 13
  SDRKIT_NOAA_DSB_VIEW_CH13 = 13,
  // HIRS channel 14
  SDRKIT_NOAA_DSB_VIEW_CH14 = 14,
  // HIRS channel 15
  SDRKIT_NOAA_DSB_VIEW_CH15 = 15,
  // HIRS channel 16
  SDRKIT_NOAA_DSB_VIEW_CH16 = 16,
  // HIRS channel 17
  SDRKIT_NOAA_DSB_VIEW_CH17 = 17,
  // HIRS channel 18
  SDRKIT_NOAA_DSB_VIEW_CH18 = 18,
  // HIRS channel 19
  SDRKIT_NOAA_DSB_VIEW_CH19 = 19,
  // HIRS channel 20
  SDRKIT_NOAA_DSB_VIEW_CH20 = 20,
} sdrkit_noaa_dsb_view;

// The options of the setting "picture" of the stack "goes_gvar".
typedef enum sdrkit_goes_gvar_picture {
  // Imager visible
  SDRKIT_GOES_GVAR_PICTURE_IMAGER_VISIBLE = 0,
  // Imager infrared
  SDRKIT_GOES_GVAR_PICTURE_IMAGER_INFRARED = 1,
} sdrkit_goes_gvar_picture;

// The options of the setting "view" of the stack "goes_gvar".
typedef enum sdrkit_goes_gvar_view {
  // Visible (channel 1)
  SDRKIT_GOES_GVAR_VIEW_VISIBLE = 0,
  // Channel 2 (3.9 um)
  SDRKIT_GOES_GVAR_VIEW_IR2 = 1,
  // Channel 3 (6.5 um)
  SDRKIT_GOES_GVAR_VIEW_IR3 = 2,
  // Channel 4 (10.7 um)
  SDRKIT_GOES_GVAR_VIEW_IR4 = 3,
  // Channel 6 (13.3 um) or 5 (12 um)
  SDRKIT_GOES_GVAR_VIEW_IR6 = 4,
} sdrkit_goes_gvar_view;

// The options of the setting "view" of the stack "aws".
typedef enum sdrkit_aws_view {
  // Channel 1 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH1 = 0,
  // Channel 2 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH2 = 1,
  // Channel 3 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH3 = 2,
  // Channel 4 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH4 = 3,
  // Channel 5 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH5 = 4,
  // Channel 6 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH6 = 5,
  // Channel 7 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH7 = 6,
  // Channel 8 (54 GHz receiver)
  SDRKIT_AWS_VIEW_CH8 = 7,
  // Channel 9 (89 GHz receiver)
  SDRKIT_AWS_VIEW_CH9 = 8,
  // Channel 10 (183 GHz receiver)
  SDRKIT_AWS_VIEW_CH10 = 9,
  // Channel 11 (183 GHz receiver)
  SDRKIT_AWS_VIEW_CH11 = 10,
  // Channel 12 (183 GHz receiver)
  SDRKIT_AWS_VIEW_CH12 = 11,
  // Channel 13 (183 GHz receiver)
  SDRKIT_AWS_VIEW_CH13 = 12,
  // Channel 14 (183 GHz receiver)
  SDRKIT_AWS_VIEW_CH14 = 13,
  // Channel 15 (183 GHz receiver)
  SDRKIT_AWS_VIEW_CH15 = 14,
  // Channel 16 (325 GHz receiver)
  SDRKIT_AWS_VIEW_CH16 = 15,
  // Channel 17 (325 GHz receiver)
  SDRKIT_AWS_VIEW_CH17 = 16,
  // Channel 18 (325 GHz receiver)
  SDRKIT_AWS_VIEW_CH18 = 17,
  // Channel 19 (325 GHz receiver)
  SDRKIT_AWS_VIEW_CH19 = 18,
} sdrkit_aws_view;

// The options of the setting "picture" of the stack "noaa_gac".
typedef enum sdrkit_noaa_gac_picture {
  // AVHRR GAC
  SDRKIT_NOAA_GAC_PICTURE_A_V_H_R_R_G_A_C = 0,
  // HIRS
  SDRKIT_NOAA_GAC_PICTURE_H_I_R_S = 1,
} sdrkit_noaa_gac_picture;

// The options of the setting "view" of the stack "noaa_gac".
typedef enum sdrkit_noaa_gac_view {
  // Colour (2, 1, 4)
  SDRKIT_NOAA_GAC_VIEW_RGB214 = 0,
  // Channel 1
  SDRKIT_NOAA_GAC_VIEW_CH1 = 1,
  // Channel 2
  SDRKIT_NOAA_GAC_VIEW_CH2 = 2,
  // Channel 3
  SDRKIT_NOAA_GAC_VIEW_CH3 = 3,
  // Channel 4
  SDRKIT_NOAA_GAC_VIEW_CH4 = 4,
  // Channel 5
  SDRKIT_NOAA_GAC_VIEW_CH5 = 5,
  // HIRS channel 6
  SDRKIT_NOAA_GAC_VIEW_CH6 = 6,
  // HIRS channel 7
  SDRKIT_NOAA_GAC_VIEW_CH7 = 7,
  // HIRS channel 8
  SDRKIT_NOAA_GAC_VIEW_CH8 = 8,
  // HIRS channel 9
  SDRKIT_NOAA_GAC_VIEW_CH9 = 9,
  // HIRS channel 10
  SDRKIT_NOAA_GAC_VIEW_CH10 = 10,
  // HIRS channel 11
  SDRKIT_NOAA_GAC_VIEW_CH11 = 11,
  // HIRS channel 12
  SDRKIT_NOAA_GAC_VIEW_CH12 = 12,
  // HIRS channel 13
  SDRKIT_NOAA_GAC_VIEW_CH13 = 13,
  // HIRS channel 14
  SDRKIT_NOAA_GAC_VIEW_CH14 = 14,
  // HIRS channel 15
  SDRKIT_NOAA_GAC_VIEW_CH15 = 15,
  // HIRS channel 16
  SDRKIT_NOAA_GAC_VIEW_CH16 = 16,
  // HIRS channel 17
  SDRKIT_NOAA_GAC_VIEW_CH17 = 17,
  // HIRS channel 18
  SDRKIT_NOAA_GAC_VIEW_CH18 = 18,
  // HIRS channel 19
  SDRKIT_NOAA_GAC_VIEW_CH19 = 19,
  // HIRS channel 20
  SDRKIT_NOAA_GAC_VIEW_CH20 = 20,
} sdrkit_noaa_gac_view;

// The options of the setting "picture" of the stack "msg_raw".
typedef enum sdrkit_msg_raw_picture {
  // SEVIRI
  SDRKIT_MSG_RAW_PICTURE_S_E_V_I_R_I = 0,
  // SEVIRI HRV
  SDRKIT_MSG_RAW_PICTURE_S_E_V_I_R_I_H_R_V = 1,
} sdrkit_msg_raw_picture;

// The options of the setting "view" of the stack "msg_raw".
typedef enum sdrkit_msg_raw_view {
  // Colour (2, 1, 4)
  SDRKIT_MSG_RAW_VIEW_RGB214 = 0,
  // Channel 1
  SDRKIT_MSG_RAW_VIEW_CH1 = 1,
  // Channel 2
  SDRKIT_MSG_RAW_VIEW_CH2 = 2,
  // Channel 3
  SDRKIT_MSG_RAW_VIEW_CH3 = 3,
  // Channel 4
  SDRKIT_MSG_RAW_VIEW_CH4 = 4,
  // Channel 5
  SDRKIT_MSG_RAW_VIEW_CH5 = 5,
  // Channel 6
  SDRKIT_MSG_RAW_VIEW_CH6 = 6,
  // Channel 7
  SDRKIT_MSG_RAW_VIEW_CH7 = 7,
  // Channel 8
  SDRKIT_MSG_RAW_VIEW_CH8 = 8,
  // Channel 9
  SDRKIT_MSG_RAW_VIEW_CH9 = 9,
  // Channel 10
  SDRKIT_MSG_RAW_VIEW_CH10 = 10,
  // Channel 11
  SDRKIT_MSG_RAW_VIEW_CH11 = 11,
  // HRV
  SDRKIT_MSG_RAW_VIEW_HRV = 12,
} sdrkit_msg_raw_view;

// The options of the setting "modulation" of the stack "sband_ccsds".
typedef enum sdrkit_sband_ccsds_modulation {
  // BPSK
  SDRKIT_SBAND_CCSDS_MODULATION_BPSK = 0,
  // QPSK
  SDRKIT_SBAND_CCSDS_MODULATION_QPSK = 1,
  // OQPSK
  SDRKIT_SBAND_CCSDS_MODULATION_OQPSK = 2,
  // PM (residual carrier)
  SDRKIT_SBAND_CCSDS_MODULATION_PM = 3,
  // FSK
  SDRKIT_SBAND_CCSDS_MODULATION_FSK = 4,
} sdrkit_sband_ccsds_modulation;

// The options of the setting "randomiser" of the stack "sband_ccsds".
typedef enum sdrkit_sband_ccsds_randomiser {
  // None
  SDRKIT_SBAND_CCSDS_RANDOMISER_NONE = 0,
  // On the coded frame
  SDRKIT_SBAND_CCSDS_RANDOMISER_FRAME = 1,
  // Inside the Reed-Solomon code
  SDRKIT_SBAND_CCSDS_RANDOMISER_INSIDE = 2,
} sdrkit_sband_ccsds_randomiser;

// The options of the setting "rs" of the stack "sband_ccsds".
typedef enum sdrkit_sband_ccsds_rs {
  // None
  SDRKIT_SBAND_CCSDS_RS_NONE = 0,
  // (255,223)
  SDRKIT_SBAND_CCSDS_RS_CODE_255_223 = 1,
  // (255,239)
  SDRKIT_SBAND_CCSDS_RS_CODE_255_239 = 2,
} sdrkit_sband_ccsds_rs;

// The options of the setting "rs_basis" of the stack "sband_ccsds".
typedef enum sdrkit_sband_ccsds_rs_basis {
  // Dual
  SDRKIT_SBAND_CCSDS_RS_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_SBAND_CCSDS_RS_BASIS_CONVENTIONAL = 1,
} sdrkit_sband_ccsds_rs_basis;

// The options of the setting "roll_off" of the stack "gk2a_uhrit".
typedef enum sdrkit_gk2a_uhrit_roll_off {
  // Auto
  SDRKIT_GK2A_UHRIT_ROLL_OFF_AUTO = 0,
  // 0.35
  SDRKIT_GK2A_UHRIT_ROLL_OFF_0_35 = 1,
  // 0.25
  SDRKIT_GK2A_UHRIT_ROLL_OFF_0_25 = 2,
  // 0.20
  SDRKIT_GK2A_UHRIT_ROLL_OFF_0_20 = 3,
  // 0.15
  SDRKIT_GK2A_UHRIT_ROLL_OFF_0_15 = 4,
  // 0.10
  SDRKIT_GK2A_UHRIT_ROLL_OFF_0_10 = 5,
  // 0.05
  SDRKIT_GK2A_UHRIT_ROLL_OFF_0_05 = 6,
} sdrkit_gk2a_uhrit_roll_off;

// The options of the setting "filter" of the stack "gk2a_uhrit".
typedef enum sdrkit_gk2a_uhrit_filter {
  // All products
  SDRKIT_GK2A_UHRIT_FILTER_ALL = 0,
  // Images
  SDRKIT_GK2A_UHRIT_FILTER_IMAGES = 1,
  // Text
  SDRKIT_GK2A_UHRIT_FILTER_TEXT = 2,
} sdrkit_gk2a_uhrit_filter;

// The options of the setting "modcod_rule" of the stack "gk2a_uhrit".
typedef enum sdrkit_gk2a_uhrit_modcod_rule {
  // Any
  SDRKIT_GK2A_UHRIT_MODCOD_RULE_ANY = 0,
  // Fixed
  SDRKIT_GK2A_UHRIT_MODCOD_RULE_FIXED = 1,
  // Set
  SDRKIT_GK2A_UHRIT_MODCOD_RULE_SET = 2,
} sdrkit_gk2a_uhrit_modcod_rule;

// The options of the setting "frame_length" of the stack "gk2a_uhrit".
typedef enum sdrkit_gk2a_uhrit_frame_length {
  // Any
  SDRKIT_GK2A_UHRIT_FRAME_LENGTH_ANY = 0,
  // Normal
  SDRKIT_GK2A_UHRIT_FRAME_LENGTH_NORMAL = 1,
  // Short
  SDRKIT_GK2A_UHRIT_FRAME_LENGTH_SHORT = 2,
} sdrkit_gk2a_uhrit_frame_length;

// The options of the setting "pilots" of the stack "gk2a_uhrit".
typedef enum sdrkit_gk2a_uhrit_pilots {
  // Any
  SDRKIT_GK2A_UHRIT_PILOTS_ANY = 0,
  // On
  SDRKIT_GK2A_UHRIT_PILOTS_ON = 1,
  // Off
  SDRKIT_GK2A_UHRIT_PILOTS_OFF = 2,
} sdrkit_gk2a_uhrit_pilots;

// The options of the setting "satellite" of the stack "fy4_lrit".
typedef enum sdrkit_fy4_lrit_satellite {
  // FY-4A (90 ksym/s)
  SDRKIT_FY4_LRIT_SATELLITE_FY4A = 0,
  // FY-4B (120 ksym/s)
  SDRKIT_FY4_LRIT_SATELLITE_FY4B = 1,
} sdrkit_fy4_lrit_satellite;

// The options of the setting "roll_off" of the stack "fy4_lrit".
typedef enum sdrkit_fy4_lrit_roll_off {
  // Auto
  SDRKIT_FY4_LRIT_ROLL_OFF_AUTO = 0,
  // 0.35
  SDRKIT_FY4_LRIT_ROLL_OFF_0_35 = 1,
  // 0.25
  SDRKIT_FY4_LRIT_ROLL_OFF_0_25 = 2,
  // 0.20
  SDRKIT_FY4_LRIT_ROLL_OFF_0_20 = 3,
  // 0.15
  SDRKIT_FY4_LRIT_ROLL_OFF_0_15 = 4,
  // 0.10
  SDRKIT_FY4_LRIT_ROLL_OFF_0_10 = 5,
  // 0.05
  SDRKIT_FY4_LRIT_ROLL_OFF_0_05 = 6,
} sdrkit_fy4_lrit_roll_off;

// The options of the setting "modcod_rule" of the stack "fy4_lrit".
typedef enum sdrkit_fy4_lrit_modcod_rule {
  // Any
  SDRKIT_FY4_LRIT_MODCOD_RULE_ANY = 0,
  // Fixed
  SDRKIT_FY4_LRIT_MODCOD_RULE_FIXED = 1,
  // Set
  SDRKIT_FY4_LRIT_MODCOD_RULE_SET = 2,
} sdrkit_fy4_lrit_modcod_rule;

// The options of the setting "frame_length" of the stack "fy4_lrit".
typedef enum sdrkit_fy4_lrit_frame_length {
  // Any
  SDRKIT_FY4_LRIT_FRAME_LENGTH_ANY = 0,
  // Normal
  SDRKIT_FY4_LRIT_FRAME_LENGTH_NORMAL = 1,
  // Short
  SDRKIT_FY4_LRIT_FRAME_LENGTH_SHORT = 2,
} sdrkit_fy4_lrit_frame_length;

// The options of the setting "pilots" of the stack "fy4_lrit".
typedef enum sdrkit_fy4_lrit_pilots {
  // Any
  SDRKIT_FY4_LRIT_PILOTS_ANY = 0,
  // On
  SDRKIT_FY4_LRIT_PILOTS_ON = 1,
  // Off
  SDRKIT_FY4_LRIT_PILOTS_OFF = 2,
} sdrkit_fy4_lrit_pilots;

// The options of the setting "filter" of the stack "fy4_lrit".
typedef enum sdrkit_fy4_lrit_filter {
  // All products
  SDRKIT_FY4_LRIT_FILTER_ALL = 0,
  // Images
  SDRKIT_FY4_LRIT_FILTER_IMAGES = 1,
  // Text
  SDRKIT_FY4_LRIT_FILTER_TEXT = 2,
} sdrkit_fy4_lrit_filter;

// The options of the setting "roll_off" of the stack "fy4a_hrit".
typedef enum sdrkit_fy4a_hrit_roll_off {
  // Auto
  SDRKIT_FY4A_HRIT_ROLL_OFF_AUTO = 0,
  // 0.35
  SDRKIT_FY4A_HRIT_ROLL_OFF_0_35 = 1,
  // 0.25
  SDRKIT_FY4A_HRIT_ROLL_OFF_0_25 = 2,
  // 0.20
  SDRKIT_FY4A_HRIT_ROLL_OFF_0_20 = 3,
  // 0.15
  SDRKIT_FY4A_HRIT_ROLL_OFF_0_15 = 4,
  // 0.10
  SDRKIT_FY4A_HRIT_ROLL_OFF_0_10 = 5,
  // 0.05
  SDRKIT_FY4A_HRIT_ROLL_OFF_0_05 = 6,
} sdrkit_fy4a_hrit_roll_off;

// The options of the setting "modcod_rule" of the stack "fy4a_hrit".
typedef enum sdrkit_fy4a_hrit_modcod_rule {
  // Any
  SDRKIT_FY4A_HRIT_MODCOD_RULE_ANY = 0,
  // Fixed
  SDRKIT_FY4A_HRIT_MODCOD_RULE_FIXED = 1,
  // Set
  SDRKIT_FY4A_HRIT_MODCOD_RULE_SET = 2,
} sdrkit_fy4a_hrit_modcod_rule;

// The options of the setting "frame_length" of the stack "fy4a_hrit".
typedef enum sdrkit_fy4a_hrit_frame_length {
  // Any
  SDRKIT_FY4A_HRIT_FRAME_LENGTH_ANY = 0,
  // Normal
  SDRKIT_FY4A_HRIT_FRAME_LENGTH_NORMAL = 1,
  // Short
  SDRKIT_FY4A_HRIT_FRAME_LENGTH_SHORT = 2,
} sdrkit_fy4a_hrit_frame_length;

// The options of the setting "pilots" of the stack "fy4a_hrit".
typedef enum sdrkit_fy4a_hrit_pilots {
  // Any
  SDRKIT_FY4A_HRIT_PILOTS_ANY = 0,
  // On
  SDRKIT_FY4A_HRIT_PILOTS_ON = 1,
  // Off
  SDRKIT_FY4A_HRIT_PILOTS_OFF = 2,
} sdrkit_fy4a_hrit_pilots;

// The options of the setting "filter" of the stack "fy4a_hrit".
typedef enum sdrkit_fy4a_hrit_filter {
  // All products
  SDRKIT_FY4A_HRIT_FILTER_ALL = 0,
  // Images
  SDRKIT_FY4A_HRIT_FILTER_IMAGES = 1,
  // Text
  SDRKIT_FY4A_HRIT_FILTER_TEXT = 2,
} sdrkit_fy4a_hrit_filter;

// The options of the setting "roll_off" of the stack "goes_grb".
typedef enum sdrkit_goes_grb_roll_off {
  // Auto
  SDRKIT_GOES_GRB_ROLL_OFF_AUTO = 0,
  // 0.35
  SDRKIT_GOES_GRB_ROLL_OFF_0_35 = 1,
  // 0.25
  SDRKIT_GOES_GRB_ROLL_OFF_0_25 = 2,
  // 0.20
  SDRKIT_GOES_GRB_ROLL_OFF_0_20 = 3,
  // 0.15
  SDRKIT_GOES_GRB_ROLL_OFF_0_15 = 4,
  // 0.10
  SDRKIT_GOES_GRB_ROLL_OFF_0_10 = 5,
  // 0.05
  SDRKIT_GOES_GRB_ROLL_OFF_0_05 = 6,
} sdrkit_goes_grb_roll_off;

// The options of the setting "modcod_rule" of the stack "goes_grb".
typedef enum sdrkit_goes_grb_modcod_rule {
  // Any
  SDRKIT_GOES_GRB_MODCOD_RULE_ANY = 0,
  // Fixed
  SDRKIT_GOES_GRB_MODCOD_RULE_FIXED = 1,
  // Set
  SDRKIT_GOES_GRB_MODCOD_RULE_SET = 2,
} sdrkit_goes_grb_modcod_rule;

// The options of the setting "frame_length" of the stack "goes_grb".
typedef enum sdrkit_goes_grb_frame_length {
  // Any
  SDRKIT_GOES_GRB_FRAME_LENGTH_ANY = 0,
  // Normal
  SDRKIT_GOES_GRB_FRAME_LENGTH_NORMAL = 1,
  // Short
  SDRKIT_GOES_GRB_FRAME_LENGTH_SHORT = 2,
} sdrkit_goes_grb_frame_length;

// The options of the setting "pilots" of the stack "goes_grb".
typedef enum sdrkit_goes_grb_pilots {
  // Any
  SDRKIT_GOES_GRB_PILOTS_ANY = 0,
  // On
  SDRKIT_GOES_GRB_PILOTS_ON = 1,
  // Off
  SDRKIT_GOES_GRB_PILOTS_OFF = 2,
} sdrkit_goes_grb_pilots;

// The options of the setting "roll_off" of the stack "sat_filecast".
typedef enum sdrkit_sat_filecast_roll_off {
  // Auto
  SDRKIT_SAT_FILECAST_ROLL_OFF_AUTO = 0,
  // 0.35
  SDRKIT_SAT_FILECAST_ROLL_OFF_0_35 = 1,
  // 0.25
  SDRKIT_SAT_FILECAST_ROLL_OFF_0_25 = 2,
  // 0.20
  SDRKIT_SAT_FILECAST_ROLL_OFF_0_20 = 3,
  // 0.15
  SDRKIT_SAT_FILECAST_ROLL_OFF_0_15 = 4,
  // 0.10
  SDRKIT_SAT_FILECAST_ROLL_OFF_0_10 = 5,
  // 0.05
  SDRKIT_SAT_FILECAST_ROLL_OFF_0_05 = 6,
} sdrkit_sat_filecast_roll_off;

// The options of the setting "modcod_rule" of the stack "sat_filecast".
typedef enum sdrkit_sat_filecast_modcod_rule {
  // Any
  SDRKIT_SAT_FILECAST_MODCOD_RULE_ANY = 0,
  // Fixed
  SDRKIT_SAT_FILECAST_MODCOD_RULE_FIXED = 1,
  // Set
  SDRKIT_SAT_FILECAST_MODCOD_RULE_SET = 2,
} sdrkit_sat_filecast_modcod_rule;

// The options of the setting "frame_length" of the stack "sat_filecast".
typedef enum sdrkit_sat_filecast_frame_length {
  // Any
  SDRKIT_SAT_FILECAST_FRAME_LENGTH_ANY = 0,
  // Normal
  SDRKIT_SAT_FILECAST_FRAME_LENGTH_NORMAL = 1,
  // Short
  SDRKIT_SAT_FILECAST_FRAME_LENGTH_SHORT = 2,
} sdrkit_sat_filecast_frame_length;

// The options of the setting "pilots" of the stack "sat_filecast".
typedef enum sdrkit_sat_filecast_pilots {
  // Any
  SDRKIT_SAT_FILECAST_PILOTS_ANY = 0,
  // On
  SDRKIT_SAT_FILECAST_PILOTS_ON = 1,
  // Off
  SDRKIT_SAT_FILECAST_PILOTS_OFF = 2,
} sdrkit_sat_filecast_pilots;

// The options of the setting "craft" of the stack "elektro_l".
typedef enum sdrkit_elektro_l_craft {
  // Elektro-L N3 (76°E)
  SDRKIT_ELEKTRO_L_CRAFT_N3 = 0,
  // Elektro-L N4 (165.8°E)
  SDRKIT_ELEKTRO_L_CRAFT_N4 = 1,
  // Elektro-L N5 (14.5°W)
  SDRKIT_ELEKTRO_L_CRAFT_N5 = 2,
} sdrkit_elektro_l_craft;

// The options of the setting "link" of the stack "elektro_l".
typedef enum sdrkit_elektro_l_link {
  // HRIT 665.4 ksym/s
  SDRKIT_ELEKTRO_L_LINK_HRIT665 = 0,
  // HRIT 1.0 Msym/s
  SDRKIT_ELEKTRO_L_LINK_HRIT1000 = 1,
  // LRIT 128 ksym/s
  SDRKIT_ELEKTRO_L_LINK_LRIT128 = 2,
  // LRIT 64 ksym/s
  SDRKIT_ELEKTRO_L_LINK_LRIT64 = 3,
  // GGAK 5 ksym/s
  SDRKIT_ELEKTRO_L_LINK_GGAK = 4,
} sdrkit_elektro_l_link;

// The options of the setting "filter" of the stack "elektro_l".
typedef enum sdrkit_elektro_l_filter {
  // All products
  SDRKIT_ELEKTRO_L_FILTER_ALL = 0,
  // Images
  SDRKIT_ELEKTRO_L_FILTER_IMAGES = 1,
  // Text
  SDRKIT_ELEKTRO_L_FILTER_TEXT = 2,
} sdrkit_elektro_l_filter;

// The options of the setting "gen_basis" of the stack "elektro_l".
typedef enum sdrkit_elektro_l_gen_basis {
  // Dual
  SDRKIT_ELEKTRO_L_GEN_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_ELEKTRO_L_GEN_BASIS_CONVENTIONAL = 1,
} sdrkit_elektro_l_gen_basis;

// The options of the setting "gen_link" of the stack "elektro_l".
typedef enum sdrkit_elektro_l_gen_link {
  // HRIT 665.4 ksym/s
  SDRKIT_ELEKTRO_L_GEN_LINK_HRIT665 = 0,
  // HRIT 1.0 Msym/s
  SDRKIT_ELEKTRO_L_GEN_LINK_HRIT1000 = 1,
  // LRIT 128 ksym/s
  SDRKIT_ELEKTRO_L_GEN_LINK_LRIT128 = 2,
  // LRIT 64 ksym/s
  SDRKIT_ELEKTRO_L_GEN_LINK_LRIT64 = 3,
  // GGAK 5 ksym/s
  SDRKIT_ELEKTRO_L_GEN_LINK_GGAK = 4,
} sdrkit_elektro_l_gen_link;

// The options of the setting "frequency_preset" of the stack "hrpt".
typedef enum sdrkit_hrpt_frequency_preset {
  // 1701.300 MetOp-B / MetOp-C AHRPT
  SDRKIT_HRPT_FREQUENCY_PRESET_1701_300 = 0,
  // 1701.400 FY-3 AHRPT (check the satellite)
  SDRKIT_HRPT_FREQUENCY_PRESET_1701_400 = 1,
  // 1698.000 NOAA-19 HRPT (retired)
  SDRKIT_HRPT_FREQUENCY_PRESET_1698_000 = 2,
  // 1702.500 NOAA-15 HRPT (retired)
  SDRKIT_HRPT_FREQUENCY_PRESET_1702_500 = 3,
  // 1707.000 NOAA-18 HRPT (retired)
  SDRKIT_HRPT_FREQUENCY_PRESET_1707_000 = 4,
} sdrkit_hrpt_frequency_preset;

// The options of the setting "variant" of the stack "hrpt".
typedef enum sdrkit_hrpt_variant {
  // Automatic
  SDRKIT_HRPT_VARIANT_AUTO = 0,
  // MetOp AHRPT (QPSK 2.33 Msym/s)
  SDRKIT_HRPT_VARIANT_METOP = 1,
  // FY-3 AHRPT (QPSK 2.8 Msym/s)
  SDRKIT_HRPT_VARIANT_FY3 = 2,
  // NOAA HRPT (retired)
  SDRKIT_HRPT_VARIANT_NOAA = 3,
} sdrkit_hrpt_variant;

// The options of the setting "view" of the stack "hrpt".
typedef enum sdrkit_hrpt_view {
  // Colour (2, 1, 4)
  SDRKIT_HRPT_VIEW_RGB214 = 0,
  // Channel 1 (visible)
  SDRKIT_HRPT_VIEW_CH1 = 1,
  // Channel 2 (near infrared)
  SDRKIT_HRPT_VIEW_CH2 = 2,
  // Channel 3 (3A / 3B)
  SDRKIT_HRPT_VIEW_CH3 = 3,
  // Channel 4 (thermal)
  SDRKIT_HRPT_VIEW_CH4 = 4,
  // Channel 5 (thermal)
  SDRKIT_HRPT_VIEW_CH5 = 5,
} sdrkit_hrpt_view;

// The options of the setting "hirs_view" of the stack "hrpt".
typedef enum sdrkit_hrpt_hirs_view {
  // Colour (2, 1, 4)
  SDRKIT_HRPT_HIRS_VIEW_RGB214 = 0,
  // HIRS channel 1
  SDRKIT_HRPT_HIRS_VIEW_CH1 = 1,
  // HIRS channel 2
  SDRKIT_HRPT_HIRS_VIEW_CH2 = 2,
  // HIRS channel 3
  SDRKIT_HRPT_HIRS_VIEW_CH3 = 3,
  // HIRS channel 4
  SDRKIT_HRPT_HIRS_VIEW_CH4 = 4,
  // HIRS channel 5
  SDRKIT_HRPT_HIRS_VIEW_CH5 = 5,
  // HIRS channel 6
  SDRKIT_HRPT_HIRS_VIEW_CH6 = 6,
  // HIRS channel 7
  SDRKIT_HRPT_HIRS_VIEW_CH7 = 7,
  // HIRS channel 8
  SDRKIT_HRPT_HIRS_VIEW_CH8 = 8,
  // HIRS channel 9
  SDRKIT_HRPT_HIRS_VIEW_CH9 = 9,
  // HIRS channel 10
  SDRKIT_HRPT_HIRS_VIEW_CH10 = 10,
  // HIRS channel 11
  SDRKIT_HRPT_HIRS_VIEW_CH11 = 11,
  // HIRS channel 12
  SDRKIT_HRPT_HIRS_VIEW_CH12 = 12,
  // HIRS channel 13
  SDRKIT_HRPT_HIRS_VIEW_CH13 = 13,
  // HIRS channel 14
  SDRKIT_HRPT_HIRS_VIEW_CH14 = 14,
  // HIRS channel 15
  SDRKIT_HRPT_HIRS_VIEW_CH15 = 15,
  // HIRS channel 16
  SDRKIT_HRPT_HIRS_VIEW_CH16 = 16,
  // HIRS channel 17
  SDRKIT_HRPT_HIRS_VIEW_CH17 = 17,
  // HIRS channel 18
  SDRKIT_HRPT_HIRS_VIEW_CH18 = 18,
  // HIRS channel 19
  SDRKIT_HRPT_HIRS_VIEW_CH19 = 19,
  // HIRS channel 20
  SDRKIT_HRPT_HIRS_VIEW_CH20 = 20,
} sdrkit_hrpt_hirs_view;

// The options of the setting "gen_variant" of the stack "hrpt".
typedef enum sdrkit_hrpt_gen_variant {
  // MetOp AHRPT (QPSK 2.33 Msym/s)
  SDRKIT_HRPT_GEN_VARIANT_METOP = 0,
  // FY-3 AHRPT (QPSK 2.8 Msym/s)
  SDRKIT_HRPT_GEN_VARIANT_FY3 = 1,
  // NOAA HRPT (retired)
  SDRKIT_HRPT_GEN_VARIANT_NOAA = 2,
} sdrkit_hrpt_gen_variant;

// The fields of the link layers that every coded stack publishes: the front end, the demodulator, the code, the frames and the packets. A field whose report has not come has no value.
typedef struct sdrkit_satimg_link_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the demodulator, dB. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset: the energy location, the search and the loop. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock, ppm. Unit: ppm.
  double clock_ppm;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The nominal symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `frames_ok` holds a value.
  bool has_frames_ok;
  // Frames decoded.
  int64_t frames_ok;
  // True if `frames_bad` holds a value.
  bool has_frames_bad;
  // Frames that failed their check.
  int64_t frames_bad;
  // True if `ber` holds a value.
  bool has_ber;
  // The bit error rate before the code.
  double ber;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
  // True if `input_rate_hz` holds a value.
  bool has_input_rate_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // True if `lock_metric` holds a value.
  bool has_lock_metric;
  // The carrier lock metric, 0 to 1.
  double lock_metric;
  // True if `phase_ambiguity` holds a value.
  bool has_phase_ambiguity;
  // The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late.
  int64_t phase_ambiguity;
  // True if `code_locked` holds a value.
  bool has_code_locked;
  // The code synchroniser holds its lock.
  bool code_locked;
  // True if `nrzm` holds a value.
  bool has_nrzm;
  // NRZ-M was found before the code.
  bool nrzm;
  // True if `marker_errors` holds a value.
  bool has_marker_errors;
  // Wrong bits of the last coded marker compared (of 52).
  int64_t marker_errors;
  // True if `bytes_corrected` holds a value.
  bool has_bytes_corrected;
  // Reed-Solomon symbols corrected.
  int64_t bytes_corrected;
  // True if `markers` holds a value.
  bool has_markers;
  // Frame markers found.
  int64_t markers;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft of the last frame.
  int64_t spacecraft;
  // True if `vcid` holds a value.
  bool has_vcid;
  // The virtual channel of the last frame.
  int64_t vcid;
  // True if `frame_count` holds a value.
  bool has_frame_count;
  // The virtual channel frame count of the last frame.
  int64_t frame_count;
  // True if `packets` holds a value.
  bool has_packets;
  // Space packets given out.
  int64_t packets;
  // True if `packets_lost` holds a value.
  bool has_packets_lost;
  // Space packets lost.
  int64_t packets_lost;
} sdrkit_satimg_link_fields;

// The options of `modulation` of `sdrkit_meteor_lrpt_fields`.
typedef enum sdrkit_meteor_lrpt_fields_modulation {
  // OQPSK 72k (N2-3, N2-4)
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_OQPSK72 = 0,
  // QPSK 72k (N2)
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_QPSK72 = 1,
  // OQPSK 80k interleaved
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_OQPSK80 = 2,
} sdrkit_meteor_lrpt_fields_modulation;

// The options of `modulation_setting` of `sdrkit_meteor_lrpt_fields`.
typedef enum sdrkit_meteor_lrpt_fields_modulation_setting {
  // Automatic
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_SETTING_AUTO = 0,
  // OQPSK 72k (N2-3, N2-4)
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_SETTING_OQPSK72 = 1,
  // QPSK 72k (N2)
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_SETTING_QPSK72 = 2,
  // OQPSK 80k interleaved
  SDRKIT_METEOR_LRPT_FIELDS_MODULATION_SETTING_OQPSK80 = 3,
} sdrkit_meteor_lrpt_fields_modulation_setting;

// The fields of Meteor-M LRPT beside the link layers.
typedef struct sdrkit_meteor_lrpt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The running modulation. A value of `sdrkit_meteor_lrpt_fields_modulation`.
  int32_t modulation;
  // The modulation setting: automatic or fixed. A value of `sdrkit_meteor_lrpt_fields_modulation_setting`.
  int32_t modulation_setting;
  // The deinterleaver of the 80k link holds the markers.
  bool interleaver_locked;
  // True if `onboard_time` holds a value.
  bool has_onboard_time;
  // The on-board time of the last image packet, seconds of the day (Moscow time). A length of time in seconds.
  double onboard_time;
  // True if `quality` holds a value.
  bool has_quality;
  // The quality factor of the last image packet.
  int64_t quality;
  // Rows of 8 lines in the picture.
  int64_t picture_rows;
  // Lines of 1568 pixels in the picture.
  int64_t picture_lines;
  // True if `row_time` holds a value.
  bool has_row_time;
  // The row time learnt from the on-board times. A length of time in seconds.
  double row_time;
  // True if `first_decoding` holds a value.
  bool has_first_decoding;
  // The signal time at which frames first decoded. A length of time in seconds.
  double first_decoding;
  // True if `colour_channels` holds a value.
  bool has_colour_channels;
  // The channels the automatic colour view shows ("1,2,3").
  sdrkit_string colour_channels;
} sdrkit_meteor_lrpt_fields;

// The packets of one channel of Meteor-M LRPT. The row key is the channel.
typedef struct sdrkit_meteor_lrpt_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of an image channel or the telemetry.
  sdrkit_string channel;
  // The packets of a channel.
  int64_t packets;
} sdrkit_meteor_lrpt_channel_row;

// A finished image. The row key names the image. The picture itself leaves on the picture stream.
typedef struct sdrkit_satimg_image_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The product, or the instrument, that an image shows.
  sdrkit_string product;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel that an image shows, when the product names one.
  sdrkit_string channel;
  // True if `time` holds a value.
  bool has_time;
  // The signal time at which the image started. A length of time in seconds.
  double time;
  // The image width in pixels. Unit: px.
  int64_t width;
  // True if `height` holds a value.
  bool has_height;
  // The image height in pixels (rows received for a picture that grows). Unit: px.
  int64_t height;
} sdrkit_satimg_image_row;

// The options of `rs_basis` of `sdrkit_satimg_xrit_fields`.
typedef enum sdrkit_satimg_xrit_fields_rs_basis {
  // Dual
  SDRKIT_SATIMG_XRIT_FIELDS_RS_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_SATIMG_XRIT_FIELDS_RS_BASIS_CONVENTIONAL = 1,
} sdrkit_satimg_xrit_fields_rs_basis;

// The fields of an xRIT stack (GOES HRIT, GK-2A LRIT and HRIT, Elektro-L) beside the link layers.
typedef struct sdrkit_satimg_xrit_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The name of the running link ("HRIT", "LRIT", "HRIT 665.4 ksym/s", "GGAK 5 ksym/s").
  sdrkit_string link;
  // True if `carrier_locked` holds a value.
  bool has_carrier_locked;
  // The carrier is locked (also while frames decode).
  bool carrier_locked;
  // True if `symbol_rate_measured_hz` holds a value.
  bool has_symbol_rate_measured_hz;
  // The symbol rate the timing loop measures. A frequency in Hz.
  double symbol_rate_measured_hz;
  // True if `g2_inverted` holds a value.
  bool has_g2_inverted;
  // The second code output arrives inverted (as CCSDS sends it).
  bool g2_inverted;
  // True if `rs_basis` holds a value.
  bool has_rs_basis;
  // The basis of the Reed-Solomon symbols. A value of `sdrkit_satimg_xrit_fields_rs_basis`.
  int32_t rs_basis;
  // True if `byte_errors_pct` holds a value.
  bool has_byte_errors_pct;
  // Bytes corrected over the last second, percent. Unit: %.
  double byte_errors_pct;
  // True if `transfer_frames` holds a value.
  bool has_transfer_frames;
  // Transfer frames (fill frames not counted).
  int64_t transfer_frames;
  // True if `fill_frames` holds a value.
  bool has_fill_frames;
  // Fill frames.
  int64_t fill_frames;
  // True if `frame_gaps` holds a value.
  bool has_frame_gaps;
  // Gaps of the frame counts.
  int64_t frame_gaps;
  // True if `crc_errors` holds a value.
  bool has_crc_errors;
  // Packets with a wrong CRC.
  int64_t crc_errors;
  // True if `files` holds a value.
  bool has_files;
  // Files.
  int64_t files;
  // True if `file_errors` holds a value.
  bool has_file_errors;
  // Files with errors.
  int64_t file_errors;
  // True if `length_errors` holds a value.
  bool has_length_errors;
  // Files whose length differs from their transport header.
  int64_t length_errors;
  // True if `products` holds a value.
  bool has_products;
  // Products kept.
  int64_t products;
  // True if `keys` holds a value.
  bool has_keys;
  // GK-2A: keys loaded.
  int64_t keys;
  // True if `segments_locked` holds a value.
  bool has_segments_locked;
  // GK-2A: segments waiting for a key.
  int64_t segments_locked;
  // What the user can do (empty when all is well).
  sdrkit_string hint;
  // True if `first_decoding` holds a value.
  bool has_first_decoding;
  // The signal time at which frames first decoded. A length of time in seconds.
  double first_decoding;
  // True if `cadus_imported` holds a value.
  bool has_cadus_imported;
  // CADUs imported from a file.
  int64_t cadus_imported;
  // True if `ggak_frames` holds a value.
  bool has_ggak_frames;
  // GGAK frames.
  int64_t ggak_frames;
  // True if `ggak_good` holds a value.
  bool has_ggak_good;
  // GGAK frames with a good CRC.
  int64_t ggak_good;
  // True if `ggak_bad` holds a value.
  bool has_ggak_bad;
  // GGAK frames with a bad CRC.
  int64_t ggak_bad;
  // True if `ggak_marker` holds a value.
  bool has_ggak_marker;
  // The marker of the last GGAK frame.
  int64_t ggak_marker;
  // True if `ggak_field` holds a value.
  bool has_ggak_field;
  // |B| of the last plausible GGAK field.
  double ggak_field;
} sdrkit_satimg_xrit_fields;

// The frames of one virtual channel. The row key is the virtual channel.
typedef struct sdrkit_satimg_virtual_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The virtual channel of a frame header.
  int64_t vcid;
  // The good frames of a virtual channel.
  int64_t frames;
} sdrkit_satimg_virtual_channel_row;

// The options of `kind` of `sdrkit_satimg_product_row`.
typedef enum sdrkit_satimg_product_row_kind {
  // Admin message
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_ADMIN_MESSAGE = 0,
  // DCS
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_DCS = 1,
  // EMWIN
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_EMWIN = 2,
  // Mesoscale
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_MESOSCALE = 3,
  // Full disk
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_FULL_DISK = 4,
  // CONUS
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_CONUS = 5,
  // Image
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_IMAGE = 6,
  // Text
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_TEXT = 7,
  // Other
  SDRKIT_SATIMG_PRODUCT_ROW_KIND_OTHER = 8,
} sdrkit_satimg_product_row_kind;

// A product of an xRIT stack. The row key is the product id.
typedef struct sdrkit_satimg_product_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of a product in the `products` table.
  sdrkit_string product;
  // The kind of a product in the `products` table. A value of `sdrkit_satimg_product_row_kind`.
  int32_t kind;
  // How much of a product arrived: rows or segments for images, a size for others, "..." while it arrives, "errors".
  sdrkit_string done;
} sdrkit_satimg_product_row;

// The fields of FY-2 S-VISSR beside the link layers.
typedef struct sdrkit_fy2_svissr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Lines decoded.
  int64_t lines;
  // Sync windows found.
  int64_t syncs;
  // Frames refused.
  int64_t rejected;
  // Frames with under 2 % preamble errors.
  int64_t usable;
  // The preamble bit error ratio of the last frame.
  double preamble_ber;
  // The estimate of the data bit error ratio (the set bits of the spare area).
  double data_ber;
  // True if `line_counter` holds a value.
  bool has_line_counter;
  // The last line counter.
  int64_t line_counter;
  // Pictures started.
  int64_t pictures_started;
  // Frames imported.
  int64_t imported;
  // What to look at when nothing decodes.
  sdrkit_string hint;
} sdrkit_fy2_svissr_fields;

// The options of `product` of `sdrkit_satimg_fy3_fields`.
typedef enum sdrkit_satimg_fy3_fields_product {
  // AHRPT
  SDRKIT_SATIMG_FY3_FIELDS_PRODUCT_AHRPT = 0,
  // MPT
  SDRKIT_SATIMG_FY3_FIELDS_PRODUCT_MPT = 1,
} sdrkit_satimg_fy3_fields_product;

// The fields of FY-3 AHRPT and MPT beside the link layers.
typedef struct sdrkit_satimg_fy3_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The product: AHRPT or MPT. A value of `sdrkit_satimg_fy3_fields_product`.
  int32_t product;
  // The preset setting (its choice key, "auto" for automatic).
  sdrkit_string preset;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The satellite heard (from the spacecraft id).
  sdrkit_string satellite;
  // True if `measured_symbol_rate_hz` holds a value.
  bool has_measured_symbol_rate_hz;
  // The symbol rate measured from the signal. A frequency in Hz.
  double measured_symbol_rate_hz;
  // True if `rails_locked` holds a value.
  bool has_rails_locked;
  // Both rails have their phase.
  bool rails_locked;
  // True if `rail_i_quality` holds a value.
  bool has_rail_i_quality;
  // The lock quality of the I rail, 0 to 1.
  double rail_i_quality;
  // True if `rail_q_quality` holds a value.
  bool has_rail_q_quality;
  // The lock quality of the Q rail, 0 to 1.
  double rail_q_quality;
  // True if `differential_hypothesis` holds a value.
  bool has_differential_hypothesis;
  // The differential hypothesis locked (0 to 7).
  int64_t differential_hypothesis;
  // The rows of the tallest picture.
  int64_t picture_rows;
  // What to look at when nothing decodes.
  sdrkit_string hint;
} sdrkit_satimg_fy3_fields;

// An instrument of FY-3. The row key names the instrument.
typedef struct sdrkit_satimg_fy3_instrument_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // An instrument.
  sdrkit_string instrument;
  // The frames of an instrument.
  int64_t frames;
  // The lines or scans of an instrument.
  int64_t rows;
} sdrkit_satimg_fy3_instrument_row;

// The fields of NOAA APT.
typedef struct sdrkit_noaa_apt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
  // The carrier offset: the energy location, the search and the loop. A frequency in Hz.
  double offset_hz;
  // The error of the symbol clock, ppm. Unit: ppm.
  double clock_ppm;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
  // True if `input_rate_hz` holds a value.
  bool has_input_rate_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // The line lock holds.
  bool locked;
  // Lines decoded.
  int64_t lines;
  // Lines taken at the expected place because their sync was not found.
  int64_t lines_missed;
  // The mean subcarrier amplitude (full deviation is 1).
  double subcarrier_level;
  // The correlation of the last line sync with its pattern.
  double sync_quality;
  // Words a second. A frequency in Hz.
  double word_rate_hz;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_noaa_apt_fields;

// The fields of NOAA DSB.
typedef struct sdrkit_noaa_dsb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the demodulator, dB. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset: the energy location, the search and the loop. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock, ppm. Unit: ppm.
  double clock_ppm;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The nominal symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // Frames decoded.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
  // True if `input_rate_hz` holds a value.
  bool has_input_rate_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // True if `lock_metric` holds a value.
  bool has_lock_metric;
  // The carrier lock metric, 0 to 1.
  double lock_metric;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft of the last frame.
  int64_t spacecraft;
  // Sync words found.
  int64_t sync_hits;
  // TIP minor frames.
  int64_t tip_frames;
  // HIRS lines.
  int64_t lines;
  // True if `minor_frame` holds a value.
  bool has_minor_frame;
  // The minor frame counter of the last frame, 0 to 319.
  int64_t minor_frame;
  // True if `chip_pairing` holds a value.
  bool has_chip_pairing;
  // Which chip starts a bit: 0 or 1.
  int64_t chip_pairing;
  // The data arrive inverted.
  bool inverted;
  // HIRS elements without odd parity.
  int64_t hirs_parity_errors;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_noaa_dsb_fields;

// The fields of GOES GVAR.
typedef struct sdrkit_goes_gvar_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the demodulator, dB. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset: the energy location, the search and the loop. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock, ppm. Unit: ppm.
  double clock_ppm;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The nominal symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // Frames decoded.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
  // True if `input_rate_hz` holds a value.
  bool has_input_rate_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // True if `lock_metric` holds a value.
  bool has_lock_metric;
  // The carrier lock metric, 0 to 1.
  double lock_metric;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft of the last frame.
  int64_t spacecraft;
  // Synchronisation codes found.
  int64_t sync_hits;
  // Idle blocks.
  int64_t idle_blocks;
  // Blocks whose data field failed its CRC.
  int64_t data_crc_errors;
  // True if `block_id` holds a value.
  bool has_block_id;
  // The id of the last block.
  int64_t block_id;
  // True if `block_count` holds a value.
  bool has_block_count;
  // The block count of the last block.
  int64_t block_count;
  // The scan count of the last imager block.
  int64_t scan;
  // Imager frames whose start was seen.
  int64_t frame_starts;
  // Rows of the visible picture.
  int64_t visible_rows;
  // Rows of the infrared picture.
  int64_t infrared_rows;
  // Detector records left out.
  int64_t records_refused;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_goes_gvar_fields;

// A block of GOES GVAR. The row key is the place of the block.
typedef struct sdrkit_goes_gvar_block_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The block count of the last block.
  int64_t block_count;
  // The id of the last block.
  int64_t block_id;
  // The word size of a block. Unit: bit.
  int64_t word_size;
  // The word count of a block.
  int64_t word_count;
  // A header copy of a block had a good CRC.
  bool header_ok;
  // The data field of a block had a good CRC.
  bool data_ok;
} sdrkit_goes_gvar_block_row;

// The fields of AWS beside the link layers.
typedef struct sdrkit_aws_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Scans of the radiometer.
  int64_t lines;
  // Science packets.
  int64_t science_packets;
  // Packets whose error control field was wrong.
  int64_t packet_crc_errors;
  // Scan records that were not 7599 bytes long.
  int64_t odd_records;
  // Idle frames.
  int64_t idle_frames;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_aws_fields;

// The frames of one virtual channel of AWS. The row key is the virtual channel.
typedef struct sdrkit_aws_virtual_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A virtual channel.
  int64_t vcid;
  // The frames of a virtual channel.
  int64_t frames;
} sdrkit_aws_virtual_channel_row;

// The packets of one application process of AWS. The row key is the APID.
typedef struct sdrkit_aws_apid_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // An application process.
  int64_t apid;
  // The packets of an application process.
  int64_t packets;
} sdrkit_aws_apid_row;

// The fields of NOAA GAC.
typedef struct sdrkit_noaa_gac_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the demodulator, dB. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset: the energy location, the search and the loop. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock, ppm. Unit: ppm.
  double clock_ppm;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The nominal symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // Frames decoded.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
  // True if `input_rate_hz` holds a value.
  bool has_input_rate_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // True if `lock_metric` holds a value.
  bool has_lock_metric;
  // The carrier lock metric, 0 to 1.
  double lock_metric;
  // A confirmed frame lock is held.
  bool frame_lock;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft of the last frame.
  int64_t spacecraft;
  // True if `id_code` holds a value.
  bool has_id_code;
  // The frame code of the identification word: 0 in a GAC frame.
  int64_t id_code;
  // Sync words found.
  int64_t sync_hits;
  // TIP minor frames.
  int64_t tip_frames;
  // AVHRR lines.
  int64_t lines;
  // HIRS lines.
  int64_t hirs_lines;
  // The data arrive inverted.
  bool inverted;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_noaa_gac_fields;

// The fields of the raw Meteosat downlink beside the link layers.
typedef struct sdrkit_msg_raw_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `satellite_number` holds a value.
  bool has_satellite_number;
  // The first ten bits of the last frame, read as one number.
  int64_t satellite_number;
  // Lines of part 0 stored.
  int64_t lines;
  // Rows of single channels written to the SEVIRI picture.
  int64_t channel_rows;
  // Rows of the HRV picture.
  int64_t hrv_rows;
  // SEVIRI packets taken.
  int64_t seviri_packets;
  // SEVIRI packets not placed: too short, or of a satellite number without clock numbers.
  int64_t packets_refused;
  // Discs finished.
  int64_t discs;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_msg_raw_fields;

// The fields of every stack that lists frames: the link, the frame lock and the note.
typedef struct sdrkit_satimg_frame_table_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the demodulator, dB. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, dBFS. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset: the energy location, the search and the loop. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock, ppm. Unit: ppm.
  double clock_ppm;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The nominal symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // Frames decoded.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
  // True if `input_rate_hz` holds a value.
  bool has_input_rate_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // True if `lock_metric` holds a value.
  bool has_lock_metric;
  // The carrier lock metric, 0 to 1.
  double lock_metric;
  // A confirmed frame lock is held.
  bool frame_lock;
  // Sync words found.
  int64_t sync_hits;
  // Reed-Solomon symbols corrected.
  int64_t bytes_corrected;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft of a frame header.
  int64_t spacecraft;
  // True if `vcid` holds a value.
  bool has_vcid;
  // The virtual channel of a frame header.
  int64_t vcid;
  // True if `stream` holds a value.
  bool has_stream;
  // The QPSK stream in use, 0 to 3.
  int64_t stream;
  // The data arrive inverted.
  bool inverted;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_satimg_frame_table_fields;

// A frame of a stack that lists frames. The row key is the place of the frame in the stream.
typedef struct sdrkit_satimg_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time of a frame. A length of time in seconds.
  double time;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft of a frame header.
  int64_t spacecraft;
  // True if `vcid` holds a value.
  bool has_vcid;
  // The virtual channel of a frame header.
  int64_t vcid;
  // Bytes of a frame with its sync word.
  int64_t bytes;
  // The check of a frame: good, check failed, or none.
  sdrkit_string check;
  // The first 24 bytes of a frame after decoding.
  sdrkit_string hex;
} sdrkit_satimg_frame_row;

// The fields of the S-band CCSDS stack beside those of the frame table.
typedef struct sdrkit_sband_ccsds_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The name of the link.
  sdrkit_string link;
  // True if `carrier_locked` holds a value.
  bool has_carrier_locked;
  // The carrier is locked (also while frames decode).
  bool carrier_locked;
  // True if `code_locked` holds a value.
  bool has_code_locked;
  // The code synchroniser holds its lock.
  bool code_locked;
  // True if `phase_ambiguity` holds a value.
  bool has_phase_ambiguity;
  // The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late.
  int64_t phase_ambiguity;
  // What the user can do (empty when all is well).
  sdrkit_string hint;
} sdrkit_sband_ccsds_fields;

// The fields of RDAS beside those of the frame table.
typedef struct sdrkit_rdas_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The carrier is locked (also while frames decode).
  bool carrier_locked;
  // MSU-GS frames found.
  int64_t packets;
  // Infrared lines.
  int64_t lines_ir;
  // Reduced visible lines.
  int64_t lines_visible;
  // True if `gate_skipped` holds a value.
  bool has_gate_skipped;
  // Input samples let go by the gate.
  int64_t gate_skipped;
  // True if `gate_skipped_pct` holds a value.
  bool has_gate_skipped_pct;
  // The share of the input let go by the gate, percent. Unit: %.
  double gate_skipped_pct;
  // True if `gate_line` holds a value.
  bool has_gate_line;
  // The height of the last gate line over the mean.
  double gate_line;
  // What the user can do (empty when all is well).
  sdrkit_string hint;
} sdrkit_rdas_fields;

// The counts of the transfer frames in the data fields of a DVB-S2 carrier.
typedef struct sdrkit_satimg_cadu_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // CADUs found.
  int64_t cadus;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft id of the frames. No value before a frame is read.
  int64_t spacecraft;
  // True if `vcid` holds a value.
  bool has_vcid;
  // The virtual channel of the last frame.
  int64_t vcid;
  // The frames of each virtual channel, for people.
  sdrkit_string frames_per_vc;
  // Space packets.
  int64_t space_packets;
  // Space packets lost.
  int64_t packets_lost;
  // Breaks in the stream of data fields.
  int64_t field_breaks;
} sdrkit_satimg_cadu_fields;

// The result fields of the file layer of `gk2a_uhrit`.
typedef struct sdrkit_gk2a_uhrit_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a bad CRC-16.
  int64_t frames_crc_bad;
  // Packets with a bad CRC-32.
  int64_t packets_crc_bad;
  // Files.
  int64_t files;
  // The last file name.
  sdrkit_string last_file;
  // True if `products` holds a value.
  bool has_products;
  // Products kept by the file layer. No value before the file layer reports.
  int64_t products;
  // What the user can do.
  sdrkit_string hint;
  // A note on the state of the stack.
  sdrkit_string note;
} sdrkit_gk2a_uhrit_fields;

// A row of the packets of each APID of `gk2a_uhrit`. The row key is the APID.
typedef struct sdrkit_gk2a_uhrit_apid_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The APID.
  int64_t apid;
  // The packets of the APID.
  int64_t packets;
} sdrkit_gk2a_uhrit_apid_row;

// A row of the frames of each virtual channel of `gk2a_uhrit`. The row key is the virtual channel.
typedef struct sdrkit_satimg_uhrit_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The virtual channel.
  int64_t vcid;
  // The frames of the virtual channel.
  int64_t frames;
} sdrkit_satimg_uhrit_frame_row;

// The result fields of `fy4_lrit` and `fy4a_hrit` beside those of the DVB-S2 receiver. A count of a layer that has not reported has no value.
typedef struct sdrkit_satimg_fy4_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The name of the running link.
  sdrkit_string link;
  // The carrier of the link: the standard, the MODCOD and the symbol rate.
  sdrkit_string signal;
  // Transport packets.
  int64_t ts_packets;
  // Transport errors.
  int64_t ts_errors;
  // CADUs found in the transport stream or imported.
  int64_t cadus;
  // CADUs imported from a file.
  int64_t cadus_imported;
  // True if `transfer_frames` holds a value.
  bool has_transfer_frames;
  // Transfer frames (fill frames not counted).
  int64_t transfer_frames;
  // True if `fill_frames` holds a value.
  bool has_fill_frames;
  // Fill frames.
  int64_t fill_frames;
  // True if `frame_gaps` holds a value.
  bool has_frame_gaps;
  // Gaps of the frame counts.
  int64_t frame_gaps;
  // True if `packets` holds a value.
  bool has_packets;
  // Space packets.
  int64_t packets;
  // True if `files` holds a value.
  bool has_files;
  // Files.
  int64_t files;
  // True if `file_errors` holds a value.
  bool has_file_errors;
  // Files with errors.
  int64_t file_errors;
  // True if `encrypted_files` holds a value.
  bool has_encrypted_files;
  // Encrypted files.
  int64_t encrypted_files;
  // True if `image_segments` holds a value.
  bool has_image_segments;
  // Image segments.
  int64_t image_segments;
  // True if `pictures_complete` holds a value.
  bool has_pictures_complete;
  // Pictures complete.
  int64_t pictures_complete;
  // True if `products` holds a value.
  bool has_products;
  // Products kept.
  int64_t products;
  // What the user can do (empty when all is well).
  sdrkit_string hint;
  // The seconds of signal decoded. A length of time in seconds.
  double signal_time;
} sdrkit_satimg_fy4_fields;

// The result fields of `goes_grb` beside those of the DVB-S2 receiver and the CADUs.
typedef struct sdrkit_goes_grb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Packets with a bad CRC.
  int64_t packets_crc_bad;
  // Payloads.
  int64_t payloads;
  // Picture blocks.
  int64_t picture_blocks;
  // JPEG 2000 failures.
  int64_t j2k_failures;
  // Files.
  int64_t files;
  // Rows of the picture shown.
  int64_t picture_lines;
  // The product shown.
  sdrkit_string shown_product;
  // What the user can do.
  sdrkit_string hint;
  // A note on the state of the stack.
  sdrkit_string note;
} sdrkit_goes_grb_fields;

// The GLM lightning results of `goes_grb`.
typedef struct sdrkit_goes_grb_glm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Complete GLM payloads (APIDs 300 to 303).
  int64_t glm_payloads;
  // GLM payloads dropped because a packet of them was lost or damaged.
  int64_t glm_dropped;
  // Packets missing from the sequence counts of the GLM APIDs.
  int64_t glm_packets_lost;
  // Compressed GLM payloads, saved as files and not decoded.
  int64_t glm_not_decoded;
  // Flash and event units whose length does not match their count.
  int64_t glm_bad_length;
  // Group units whose length does not match their count.
  int64_t glm_group_bad_length;
  // Flashes decoded.
  int64_t flash_count;
  // Groups decoded.
  int64_t group_count;
  // Events counted.
  int64_t event_count;
  // Flash and group records with an impossible value.
  int64_t glm_rejected;
  // True if `newest_flash` holds a value.
  bool has_newest_flash;
  // The time of the first event of the newest flash (UTC). Seconds since 1970 (UTC).
  double newest_flash;
  // Flashes in the 60 s of flash time up to the newest flash. Unit: 1/min.
  double flash_rate;
} sdrkit_goes_grb_glm_fields;

// A picture product of `goes_grb`. The row key is the APID.
typedef struct sdrkit_goes_grb_product_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The APID.
  int64_t apid;
  // The name of the product of the APID.
  sdrkit_string name;
  // The signal time of the last update. A length of time in seconds.
  double updated;
} sdrkit_goes_grb_product_row;

// A row of the packets of each APID of `goes_grb`. The row key is the APID.
typedef struct sdrkit_goes_grb_apid_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The APID.
  int64_t apid;
  // The name of the product of the APID.
  sdrkit_string name;
  // The packets of the APID.
  int64_t packets;
} sdrkit_goes_grb_apid_row;

// The options of `quality` of `sdrkit_goes_grb_glm_flash_row`.
typedef enum sdrkit_goes_grb_glm_flash_row_quality {
  // Good
  SDRKIT_GOES_GRB_GLM_FLASH_ROW_QUALITY_GOOD = 0,
  // Degraded: events out of time order
  SDRKIT_GOES_GRB_GLM_FLASH_ROW_QUALITY_TIME_ORDER = 1,
  // Degraded: too many events
  SDRKIT_GOES_GRB_GLM_FLASH_ROW_QUALITY_EVENT_COUNT = 2,
  // Degraded: duration too long
  SDRKIT_GOES_GRB_GLM_FLASH_ROW_QUALITY_DURATION = 3,
} sdrkit_goes_grb_glm_flash_row_quality;

// A lightning flash of `goes_grb`. The row key is the product time (microseconds since J2000) and the flash id.
typedef struct sdrkit_goes_grb_glm_flash_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The product-unique flash id.
  int64_t flash;
  // The time of the first event (UTC). Seconds since 1970 (UTC).
  double time;
  // From the first to the last event. A length of time in seconds.
  double duration;
  // The energy-weighted centroid, latitude first. Unit: °.
  sdrkit_position position;
  // True if `area_km2` holds a value.
  bool has_area_km2;
  // The area covered. Unit: km².
  double area_km2;
  // True if `energy_fj` holds a value.
  bool has_energy_fj;
  // The radiant energy. Unit: fJ.
  double energy_fj;
  // True if `quality` holds a value.
  bool has_quality;
  // The data quality. A value of `sdrkit_goes_grb_glm_flash_row_quality`.
  int32_t quality;
  // Groups received with this flash as parent.
  int64_t groups;
  // Events received in those groups.
  int64_t events;
} sdrkit_goes_grb_glm_flash_row;

// The options of `quality` of `sdrkit_goes_grb_glm_group_row`.
typedef enum sdrkit_goes_grb_glm_group_row_quality {
  // Good
  SDRKIT_GOES_GRB_GLM_GROUP_ROW_QUALITY_GOOD = 0,
  // Degraded: events out of time order
  SDRKIT_GOES_GRB_GLM_GROUP_ROW_QUALITY_TIME_ORDER = 1,
  // Degraded: too many events
  SDRKIT_GOES_GRB_GLM_GROUP_ROW_QUALITY_EVENT_COUNT = 2,
  // Degraded: duration too long
  SDRKIT_GOES_GRB_GLM_GROUP_ROW_QUALITY_DURATION = 3,
} sdrkit_goes_grb_glm_group_row_quality;

// A lightning group of `goes_grb`. The row key is the product time (microseconds since J2000) and the group id.
typedef struct sdrkit_goes_grb_glm_group_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The product-unique group id.
  int64_t group;
  // The mean time of the events (UTC). Seconds since 1970 (UTC).
  double time;
  // The energy-weighted centroid, latitude first. Unit: °.
  sdrkit_position position;
  // True if `area_km2` holds a value.
  bool has_area_km2;
  // The area covered. Unit: km².
  double area_km2;
  // True if `energy_fj` holds a value.
  bool has_energy_fj;
  // The radiant energy. Unit: fJ.
  double energy_fj;
  // The parent flash id.
  int64_t flash;
  // True if `quality` holds a value.
  bool has_quality;
  // The data quality. A value of `sdrkit_goes_grb_glm_group_row_quality`.
  int32_t quality;
  // Events received with this group as parent.
  int64_t events;
} sdrkit_goes_grb_glm_group_row;

// The result fields of `sat_filecast` beside those of the DVB-S2 receiver.
typedef struct sdrkit_sat_filecast_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The PID read.
  int64_t pid;
  // Transport packets on the PID.
  int64_t pid_packets;
  // IP datagrams.
  int64_t datagrams;
  // Datagrams that are not UDP.
  int64_t not_udp;
  // File-broadcast packets.
  int64_t broadcast_packets;
  // File-broadcast packets refused.
  int64_t bad_packets;
  // Complete files.
  int64_t files_complete;
  // Incomplete files.
  int64_t files_incomplete;
  // Files whose bzip2 data failed.
  int64_t files_failed;
  // Files dropped (the queue was full).
  int64_t files_dropped;
  // Bytes of files.
  int64_t bytes;
  // Files open.
  int64_t open_files;
  // The last error.
  sdrkit_string last_error;
  // Pictures finished.
  int64_t pictures_finished;
  // What the user can do.
  sdrkit_string hint;
  // A note on the state of the stack.
  sdrkit_string note;
} sdrkit_sat_filecast_fields;

// A file of `sat_filecast`. The row key is the number of the file.
typedef struct sdrkit_sat_filecast_recent_file_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the file.
  sdrkit_string name;
  // The size of the file. Unit: bytes.
  int64_t size_;
  // The parts of the file.
  int64_t parts;
  // The parts received.
  int64_t received;
  // Every part arrived.
  bool complete;
} sdrkit_sat_filecast_recent_file_row;

// A picture of `sat_filecast`. The row key is the channel.
typedef struct sdrkit_sat_filecast_picture_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel.
  sdrkit_string channel;
  // The time stamp of the picture.
  sdrkit_string stamp;
  // The width of the picture. Unit: px.
  int64_t width;
  // The height of the picture. Unit: px.
  int64_t height;
  // The segments received.
  int64_t segments;
  // The segments in all.
  int64_t total;
} sdrkit_sat_filecast_picture_row;

// The options of `variant` of `sdrkit_hrpt_fields`.
typedef enum sdrkit_hrpt_fields_variant {
  // MetOp AHRPT (QPSK 2.33 Msym/s)
  SDRKIT_HRPT_FIELDS_VARIANT_METOP = 0,
  // FY-3 AHRPT (QPSK 2.8 Msym/s)
  SDRKIT_HRPT_FIELDS_VARIANT_FY3 = 1,
  // NOAA HRPT (retired)
  SDRKIT_HRPT_FIELDS_VARIANT_NOAA = 2,
} sdrkit_hrpt_fields_variant;

// The options of `variant_setting` of `sdrkit_hrpt_fields`.
typedef enum sdrkit_hrpt_fields_variant_setting {
  // Automatic
  SDRKIT_HRPT_FIELDS_VARIANT_SETTING_AUTO = 0,
  // MetOp AHRPT (QPSK 2.33 Msym/s)
  SDRKIT_HRPT_FIELDS_VARIANT_SETTING_METOP = 1,
  // FY-3 AHRPT (QPSK 2.8 Msym/s)
  SDRKIT_HRPT_FIELDS_VARIANT_SETTING_FY3 = 2,
  // NOAA HRPT (retired)
  SDRKIT_HRPT_FIELDS_VARIANT_SETTING_NOAA = 3,
} sdrkit_hrpt_fields_variant_setting;

// The fields of `hrpt` beside the link layers.
typedef struct sdrkit_hrpt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The running variant. A value of `sdrkit_hrpt_fields_variant`.
  int32_t variant;
  // The variant setting: automatic or fixed. A value of `sdrkit_hrpt_fields_variant_setting`.
  int32_t variant_setting;
  // True if `phase_ambiguity_mirrored` holds a value.
  bool has_phase_ambiguity_mirrored;
  // The code synchroniser undid a mirrored transform (the second output inverted). No value for NOAA.
  bool phase_ambiguity_mirrored;
  // True if `noaa_rotation` holds a value.
  bool has_noaa_rotation;
  // NOAA: the polarity plus 2 times the chip phase of the framer. No value for the other satellites.
  int64_t noaa_rotation;
  // AVHRR packets of APID 103.
  int64_t packets_apid_103;
  // AVHRR packets of APID 104.
  int64_t packets_apid_104;
  // Other packets.
  int64_t other_packets;
  // NOAA minor frames.
  int64_t noaa_frames;
  // AVHRR lines decoded.
  int64_t avhrr_lines;
  // Lines of 2048 pixels in the picture.
  int64_t picture_lines;
  // True if `onboard_time` holds a value.
  bool has_onboard_time;
  // The on-board time of the last line, seconds of the day. A length of time in seconds.
  double onboard_time;
  // HIRS lines given out (NOAA).
  int64_t hirs_lines;
  // HIRS elements put into a line.
  int64_t hirs_valid;
  // HIRS elements without odd parity.
  int64_t hirs_parity_errors;
} sdrkit_hrpt_fields;

// The fields of EODB beside those of the frame table.
typedef struct sdrkit_eodb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The carrier is locked (also while frames decode).
  bool carrier_locked;
  // Packets.
  int64_t packets;
  // Space packets lost.
  int64_t packets_lost;
  // Space packets that went into a picture.
  int64_t instrument_packets;
  // Picture lines.
  int64_t lines;
  // The satellite of the last good frame.
  sdrkit_string satellite;
  // The code hypothesis that holds the lock.
  sdrkit_string hypothesis;
  // The input rate is below the least rate of the link.
  bool rate_too_low;
  // CADUs of frame files taken.
  int64_t imported;
  // True if `gate_skipped` holds a value.
  bool has_gate_skipped;
  // Input samples let go by the gate.
  int64_t gate_skipped;
  // True if `gate_skipped_pct` holds a value.
  bool has_gate_skipped_pct;
  // The share of the input let go by the gate, percent. Unit: %.
  double gate_skipped_pct;
  // True if `gate_line` holds a value.
  bool has_gate_line;
  // The height of the last gate line over the mean.
  double gate_line;
  // What the user can do (empty when all is well).
  sdrkit_string hint;
} sdrkit_eodb_fields;

// The options of `family` of `sdrkit_satimg_product_event`.
typedef enum sdrkit_satimg_product_event_family {
  // Aircraft
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SATIMG_PRODUCT_EVENT_FAMILY_OTHER = 12,
} sdrkit_satimg_product_event_family;

// The options of `kind` of `sdrkit_satimg_product_event`.
typedef enum sdrkit_satimg_product_event_kind {
  // Admin message
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_ADMIN_MESSAGE = 0,
  // DCS
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_DCS = 1,
  // EMWIN
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_EMWIN = 2,
  // Mesoscale
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_MESOSCALE = 3,
  // Full disk
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_FULL_DISK = 4,
  // CONUS
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_CONUS = 5,
  // Image
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_IMAGE = 6,
  // Text
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_TEXT = 7,
  // Other
  SDRKIT_SATIMG_PRODUCT_EVENT_KIND_OTHER = 8,
} sdrkit_satimg_product_event_kind;

// A product that changed.
typedef struct sdrkit_satimg_product_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Where the product comes from: its name.
  sdrkit_string source;
  // The text: the name of a product, the text of a text product.
  sdrkit_string text;
  // The family of the source: a satellite. A value of `sdrkit_satimg_product_event_family`.
  int32_t family;
  // The product id.
  int64_t id;
  // The kind of a product. A value of `sdrkit_satimg_product_event_kind`.
  int32_t kind;
  // The name of a product.
  sdrkit_string name;
  // The virtual channel of a product.
  int64_t vcid;
  // The APID of a product.
  int64_t apid;
  // Rows received.
  int64_t rows;
  // The picture width.
  int64_t width;
  // The picture height.
  int64_t height;
  // Segments received.
  int64_t segments;
  // Segments in all.
  int64_t segments_total;
  // Every segment arrived.
  bool complete;
  // A file of the product had errors.
  bool errors;
  // Grows with every change of the product.
  int64_t version;
  // True if `note` holds a value.
  bool has_note;
  // A note ("picture not decoded: ...").
  sdrkit_string note;
} sdrkit_satimg_product_event;

// The options of `family` of `sdrkit_satimg_text_event`.
typedef enum sdrkit_satimg_text_event_family {
  // Aircraft
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SATIMG_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_satimg_text_event_family;

// A text product.
typedef struct sdrkit_satimg_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Where the product comes from: its name.
  sdrkit_string source;
  // The text: the name of a product, the text of a text product.
  sdrkit_string text;
  // The family of the source: a satellite. A value of `sdrkit_satimg_text_event_family`.
  int32_t family;
  // The name of a product.
  sdrkit_string name;
  // True if `inner_name` holds a value.
  bool has_inner_name;
  // The file name inside a ZIP archive.
  sdrkit_string inner_name;
  // The product id.
  int64_t id;
} sdrkit_satimg_text_event;

// The options of `family` of `sdrkit_satimg_ggak_frame_event`.
typedef enum sdrkit_satimg_ggak_frame_event_family {
  // Aircraft
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SATIMG_GGAK_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_satimg_ggak_frame_event_family;

// A GGAK telemetry frame of Elektro-L.
typedef struct sdrkit_satimg_ggak_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The family of the source: a satellite. A value of `sdrkit_satimg_ggak_frame_event_family`.
  int32_t family;
  // The marker of a GGAK frame.
  int64_t marker;
  // The sub-frame of a GGAK frame.
  int64_t subframe;
  // The time stamp of a GGAK frame.
  int64_t timestamp;
  // The CRC of a GGAK frame is right.
  bool crc_ok;
  // The polarity of a GGAK frame was inverted.
  bool inverted;
  // Bx of the first triple of a GGAK frame.
  int64_t bx;
  // By of the first triple of a GGAK frame.
  int64_t by;
  // Bz of the first triple of a GGAK frame.
  int64_t bz;
} sdrkit_satimg_ggak_frame_event;

// The arguments of a command that reads a file.
typedef struct sdrkit_satimg_file_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // A file in the host's storage.
  sdrkit_string path;
} sdrkit_satimg_file_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SATIMG_FILE_ARGS_INIT {sizeof(sdrkit_satimg_file_args), {0}}

// The arguments of `add_keys`.
typedef struct sdrkit_satimg_keys_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Lines "index:hexkey".
  sdrkit_string keys;
} sdrkit_satimg_keys_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SATIMG_KEYS_ARGS_INIT {sizeof(sdrkit_satimg_keys_args), {0}}

// The arguments of `product` of EODB.
typedef struct sdrkit_eodb_product_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The key of a product of the link's instrument.
  sdrkit_string product;
  // True if `width` holds a value.
  bool has_width;
  // The width limit of the product, 0 for full size. Unit: px.
  int64_t width;
} sdrkit_eodb_product_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_EODB_PRODUCT_ARGS_INIT {sizeof(sdrkit_eodb_product_args), {0}, 0, 0}

// Fills `rows` with the changed rows of the table "channels" of the stack "meteor_lrpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_meteor_lrpt_channels_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_meteor_lrpt_channel_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.MeteorLrptChannelRow", "channels", rows, sizeof(sdrkit_meteor_lrpt_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "meteor_lrpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_meteor_lrpt_images_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_image_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "meteor_lrpt" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_meteor_lrpt_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "meteor_lrpt" are the sdrkit_meteor_lrpt_gen_*() functions
// after the settings of its receiver.

// The setting "frequency_preset" (Frequency) of the stack "meteor_lrpt". A value of `sdrkit_meteor_lrpt_frequency_preset`. Meteor-M N2-3 and N2-4 send LRPT on 137.900 or 137.100 MHz. Which one each satellite uses changes, so use a current list.
static inline int32_t sdrkit_meteor_lrpt_get_frequency_preset(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_frequency_preset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frequency_preset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meteor_lrpt_frequency_preset)index;
  return code;
}
static inline int32_t sdrkit_meteor_lrpt_set_frequency_preset(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_frequency_preset value) {
  return sdrkit_typed_setting_set_enum(handle, "frequency_preset", (int32_t)value);
}

// The setting "modulation" (Modulation) of the stack "meteor_lrpt". A value of `sdrkit_meteor_lrpt_modulation`. Automatic tries OQPSK 72k (N2-3, N2-4), then QPSK 72k, then the interleaved 80k link, until frames decode.
static inline int32_t sdrkit_meteor_lrpt_get_modulation(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meteor_lrpt_modulation)index;
  return code;
}
static inline int32_t sdrkit_meteor_lrpt_set_modulation(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "modulation", (int32_t)value);
}

// The setting "view" (View) of the stack "meteor_lrpt". A value of `sdrkit_meteor_lrpt_view`. What the picture shows.
static inline int32_t sdrkit_meteor_lrpt_get_view(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meteor_lrpt_view)index;
  return code;
}
static inline int32_t sdrkit_meteor_lrpt_set_view(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "enhance" (Enhancement) of the stack "meteor_lrpt". A value of `sdrkit_meteor_lrpt_enhance`. Thermal colours apply to a single channel view.
static inline int32_t sdrkit_meteor_lrpt_get_enhance(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_enhance *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "enhance", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meteor_lrpt_enhance)index;
  return code;
}
static inline int32_t sdrkit_meteor_lrpt_set_enhance(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_enhance value) {
  return sdrkit_typed_setting_set_enum(handle, "enhance", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "meteor_lrpt". Keeps the newest lines in view.
static inline int32_t sdrkit_meteor_lrpt_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_meteor_lrpt_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_modulation" (Test modulation) of the test signal generator of the stack "meteor_lrpt". A value of `sdrkit_meteor_lrpt_gen_modulation`.
static inline int32_t sdrkit_meteor_lrpt_gen_get_modulation(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_gen_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meteor_lrpt_gen_modulation)index;
  return code;
}
static inline int32_t sdrkit_meteor_lrpt_gen_set_modulation(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_gen_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_modulation", (int32_t)value);
}

// The setting "gen_night" (Test night channels) of the test signal generator of the stack "meteor_lrpt".
static inline int32_t sdrkit_meteor_lrpt_gen_get_night(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_night", value);
}
static inline int32_t sdrkit_meteor_lrpt_gen_set_night(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_night", value);
}

// The setting "gen_rows" (Test picture rows) of the test signal generator of the stack "meteor_lrpt".
static inline int32_t sdrkit_meteor_lrpt_gen_get_rows(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_rows", value);
}
static inline int32_t sdrkit_meteor_lrpt_gen_set_rows(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_rows", value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "goes_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_hrit_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_satimg_virtual_channel_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "goes_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_hrit_products_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_product_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "goes_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_hrit_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "satimg.product"
// (Product). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_satimg_product_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_satimg_product_event *event) {
  return sdrkit_typed_event(handle, n, "satimg.product", "satimg.ProductEvent", event, sizeof(sdrkit_satimg_product_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "satimg.text"
// (Text product). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_satimg_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_satimg_text_event *event) {
  return sdrkit_typed_event(handle, n, "satimg.text", "satimg.TextEvent", event, sizeof(sdrkit_satimg_text_event));
}

// Runs the command "open_cadu" (Open CADU) of the stack "goes_hrit" on the session of `handle`. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
static inline int32_t sdrkit_goes_hrit_open_cadu(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_cadu", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "goes_hrit" on the session of `handle`. Forgets the newest frame.
static inline int32_t sdrkit_goes_hrit_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "goes_hrit" are the sdrkit_goes_hrit_gen_*() functions
// after the settings of its receiver.

// The setting "filter" (Show) of the stack "goes_hrit". A value of `sdrkit_goes_hrit_filter`.
static inline int32_t sdrkit_goes_hrit_get_filter(SDRKIT_HANDLE handle, sdrkit_goes_hrit_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_hrit_filter)index;
  return code;
}
static inline int32_t sdrkit_goes_hrit_set_filter(SDRKIT_HANDLE handle, sdrkit_goes_hrit_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "goes_hrit".
static inline int32_t sdrkit_goes_hrit_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_goes_hrit_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// The setting "gen_invert_g2" (Test G2 inverted) of the test signal generator of the stack "goes_hrit".
static inline int32_t sdrkit_goes_hrit_gen_get_invert_g2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", value);
}
static inline int32_t sdrkit_goes_hrit_gen_set_invert_g2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value);
}

// The setting "gen_nrzm" (Test NRZ-M) of the test signal generator of the stack "goes_hrit".
static inline int32_t sdrkit_goes_hrit_gen_get_nrzm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nrzm", value);
}
static inline int32_t sdrkit_goes_hrit_gen_set_nrzm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value);
}

// The setting "gen_basis" (Test Reed-Solomon basis) of the test signal generator of the stack "goes_hrit". A value of `sdrkit_goes_hrit_gen_basis`.
static inline int32_t sdrkit_goes_hrit_gen_get_basis(SDRKIT_HANDLE handle, sdrkit_goes_hrit_gen_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_hrit_gen_basis)index;
  return code;
}
static inline int32_t sdrkit_goes_hrit_gen_set_basis(SDRKIT_HANDLE handle, sdrkit_goes_hrit_gen_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_basis", (int32_t)value);
}

// The setting "gen_fill" (Test fill frames) of the test signal generator of the stack "goes_hrit".
static inline int32_t sdrkit_goes_hrit_gen_get_fill(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fill", value);
}
static inline int32_t sdrkit_goes_hrit_gen_set_fill(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fill", value);
}

// The setting "gen_once" (Test files once) of the test signal generator of the stack "goes_hrit".
static inline int32_t sdrkit_goes_hrit_gen_get_once(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_once", value);
}
static inline int32_t sdrkit_goes_hrit_gen_set_once(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_once", value);
}

// The setting "gen_counter" (Test frame count start) of the test signal generator of the stack "goes_hrit".
static inline int32_t sdrkit_goes_hrit_gen_get_counter(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_counter", value);
}
static inline int32_t sdrkit_goes_hrit_gen_set_counter(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_counter", value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "gk2a_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_lrit_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_satimg_virtual_channel_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "gk2a_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_lrit_products_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_product_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "gk2a_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_lrit_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "open_cadu" (Open CADU) of the stack "gk2a_lrit" on the session of `handle`. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
static inline int32_t sdrkit_gk2a_lrit_open_cadu(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_cadu", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "gk2a_lrit" on the session of `handle`. Forgets the newest frame.
static inline int32_t sdrkit_gk2a_lrit_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "import_keys" (Import keys) of the stack "gk2a_lrit" on the session of `handle`. Reads a KMA key file.
static inline int32_t sdrkit_gk2a_lrit_import_keys(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "import_keys", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "add_keys" (Add keys) of the stack "gk2a_lrit" on the session of `handle`. Adds keys typed as text.
static inline int32_t sdrkit_gk2a_lrit_add_keys(SDRKIT_HANDLE handle, const sdrkit_satimg_keys_args *args) {
  return sdrkit_typed_command(handle, "add_keys", "satimg.KeysArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gk2a_lrit" are the sdrkit_gk2a_lrit_gen_*() functions
// after the settings of its receiver.

// The setting "filter" (Show) of the stack "gk2a_lrit". A value of `sdrkit_gk2a_lrit_filter`.
static inline int32_t sdrkit_gk2a_lrit_get_filter(SDRKIT_HANDLE handle, sdrkit_gk2a_lrit_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_lrit_filter)index;
  return code;
}
static inline int32_t sdrkit_gk2a_lrit_set_filter(SDRKIT_HANDLE handle, sdrkit_gk2a_lrit_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_gk2a_lrit_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// The setting "gen_invert_g2" (Test G2 inverted) of the test signal generator of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_gen_get_invert_g2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", value);
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_invert_g2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value);
}

// The setting "gen_nrzm" (Test NRZ-M) of the test signal generator of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_gen_get_nrzm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nrzm", value);
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_nrzm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value);
}

// The setting "gen_basis" (Test Reed-Solomon basis) of the test signal generator of the stack "gk2a_lrit". A value of `sdrkit_gk2a_lrit_gen_basis`.
static inline int32_t sdrkit_gk2a_lrit_gen_get_basis(SDRKIT_HANDLE handle, sdrkit_gk2a_lrit_gen_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_lrit_gen_basis)index;
  return code;
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_basis(SDRKIT_HANDLE handle, sdrkit_gk2a_lrit_gen_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_basis", (int32_t)value);
}

// The setting "gen_fill" (Test fill frames) of the test signal generator of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_gen_get_fill(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fill", value);
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_fill(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fill", value);
}

// The setting "gen_once" (Test files once) of the test signal generator of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_gen_get_once(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_once", value);
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_once(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_once", value);
}

// The setting "gen_counter" (Test frame count start) of the test signal generator of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_gen_get_counter(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_counter", value);
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_counter(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_counter", value);
}

// The setting "gen_encrypt" (Test encrypted VI006) of the test signal generator of the stack "gk2a_lrit".
static inline int32_t sdrkit_gk2a_lrit_gen_get_encrypt(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_encrypt", value);
}
static inline int32_t sdrkit_gk2a_lrit_gen_set_encrypt(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_encrypt", value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "gk2a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_hrit_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_satimg_virtual_channel_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "gk2a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_hrit_products_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_product_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "gk2a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_hrit_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "open_cadu" (Open CADU) of the stack "gk2a_hrit" on the session of `handle`. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
static inline int32_t sdrkit_gk2a_hrit_open_cadu(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_cadu", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "gk2a_hrit" on the session of `handle`. Forgets the newest frame.
static inline int32_t sdrkit_gk2a_hrit_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "import_keys" (Import keys) of the stack "gk2a_hrit" on the session of `handle`. Reads a KMA key file.
static inline int32_t sdrkit_gk2a_hrit_import_keys(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "import_keys", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "add_keys" (Add keys) of the stack "gk2a_hrit" on the session of `handle`. Adds keys typed as text.
static inline int32_t sdrkit_gk2a_hrit_add_keys(SDRKIT_HANDLE handle, const sdrkit_satimg_keys_args *args) {
  return sdrkit_typed_command(handle, "add_keys", "satimg.KeysArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gk2a_hrit" are the sdrkit_gk2a_hrit_gen_*() functions
// after the settings of its receiver.

// The setting "filter" (Show) of the stack "gk2a_hrit". A value of `sdrkit_gk2a_hrit_filter`.
static inline int32_t sdrkit_gk2a_hrit_get_filter(SDRKIT_HANDLE handle, sdrkit_gk2a_hrit_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_hrit_filter)index;
  return code;
}
static inline int32_t sdrkit_gk2a_hrit_set_filter(SDRKIT_HANDLE handle, sdrkit_gk2a_hrit_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_gk2a_hrit_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// The setting "gen_invert_g2" (Test G2 inverted) of the test signal generator of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_gen_get_invert_g2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", value);
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_invert_g2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value);
}

// The setting "gen_nrzm" (Test NRZ-M) of the test signal generator of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_gen_get_nrzm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nrzm", value);
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_nrzm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value);
}

// The setting "gen_basis" (Test Reed-Solomon basis) of the test signal generator of the stack "gk2a_hrit". A value of `sdrkit_gk2a_hrit_gen_basis`.
static inline int32_t sdrkit_gk2a_hrit_gen_get_basis(SDRKIT_HANDLE handle, sdrkit_gk2a_hrit_gen_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_hrit_gen_basis)index;
  return code;
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_basis(SDRKIT_HANDLE handle, sdrkit_gk2a_hrit_gen_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_basis", (int32_t)value);
}

// The setting "gen_fill" (Test fill frames) of the test signal generator of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_gen_get_fill(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fill", value);
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_fill(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fill", value);
}

// The setting "gen_once" (Test files once) of the test signal generator of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_gen_get_once(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_once", value);
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_once(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_once", value);
}

// The setting "gen_counter" (Test frame count start) of the test signal generator of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_gen_get_counter(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_counter", value);
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_counter(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_counter", value);
}

// The setting "gen_encrypt" (Test encrypted VI006) of the test signal generator of the stack "gk2a_hrit".
static inline int32_t sdrkit_gk2a_hrit_gen_get_encrypt(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_encrypt", value);
}
static inline int32_t sdrkit_gk2a_hrit_gen_set_encrypt(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_encrypt", value);
}

// Fills `rows` with the changed rows of the table "images" of the stack "fy2_svissr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy2_svissr_images_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_satimg_image_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "open_svissr" (Open .svissr) of the stack "fy2_svissr" on the session of `handle`. Decodes a file of clear 44 356-byte frames at the next input.
static inline int32_t sdrkit_fy2_svissr_open_svissr(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_svissr", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (New picture) of the stack "fy2_svissr" on the session of `handle`. Forgets every picture and starts new ones.
static inline int32_t sdrkit_fy2_svissr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fy2_svissr" are the sdrkit_fy2_svissr_gen_*() functions
// after the settings of its receiver.

// The setting "symbol_rate" (Symbol rate) of the stack "fy2_svissr". Unit: ksym/s.
static inline int32_t sdrkit_fy2_svissr_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_fy2_svissr_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "follow" (Newest) of the stack "fy2_svissr".
static inline int32_t sdrkit_fy2_svissr_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_fy2_svissr_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_symbol_rate" (Test symbol rate) of the test signal generator of the stack "fy2_svissr". A frequency in Hz.
static inline int32_t sdrkit_fy2_svissr_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_fy2_svissr_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_first_line" (Test first line) of the test signal generator of the stack "fy2_svissr".
static inline int32_t sdrkit_fy2_svissr_gen_get_first_line(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_first_line", value);
}
static inline int32_t sdrkit_fy2_svissr_gen_set_first_line(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_first_line", value);
}

// The setting "gen_lines" (Test lines) of the test signal generator of the stack "fy2_svissr".
static inline int32_t sdrkit_fy2_svissr_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_fy2_svissr_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// Fills `rows` with the changed rows of the table "instruments" of the stack "fy3_ahrpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy3_ahrpt_instruments_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_satimg_fy3_instrument_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.Fy3InstrumentRow", "instruments", rows, sizeof(sdrkit_satimg_fy3_instrument_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "fy3_ahrpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy3_ahrpt_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "fy3_ahrpt" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_fy3_ahrpt_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fy3_ahrpt" are the sdrkit_fy3_ahrpt_gen_*() functions
// after the settings of its receiver.

// The setting "preset" (Satellite) of the stack "fy3_ahrpt". A value of `sdrkit_fy3_ahrpt_preset`. Retunes to the preset's frequency; restarts.
static inline int32_t sdrkit_fy3_ahrpt_get_preset(SDRKIT_HANDLE handle, sdrkit_fy3_ahrpt_preset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "preset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy3_ahrpt_preset)index;
  return code;
}
static inline int32_t sdrkit_fy3_ahrpt_set_preset(SDRKIT_HANDLE handle, sdrkit_fy3_ahrpt_preset value) {
  return sdrkit_typed_setting_set_enum(handle, "preset", (int32_t)value);
}

// The setting "symbol_rate" (Symbol rate) of the stack "fy3_ahrpt". Unit: Msym/s. 0 = automatic. Restarts.
static inline int32_t sdrkit_fy3_ahrpt_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_fy3_ahrpt_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "fy3_ahrpt". 0 = automatic. Restarts.
static inline int32_t sdrkit_fy3_ahrpt_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_fy3_ahrpt_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// The setting "instrument" (Instrument) of the stack "fy3_ahrpt". A value of `sdrkit_fy3_ahrpt_instrument`. The instrument whose picture is shown.
static inline int32_t sdrkit_fy3_ahrpt_get_instrument(SDRKIT_HANDLE handle, sdrkit_fy3_ahrpt_instrument *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "instrument", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy3_ahrpt_instrument)index;
  return code;
}
static inline int32_t sdrkit_fy3_ahrpt_set_instrument(SDRKIT_HANDLE handle, sdrkit_fy3_ahrpt_instrument value) {
  return sdrkit_typed_setting_set_enum(handle, "instrument", (int32_t)value);
}

// The setting "view" (View) of the stack "fy3_ahrpt". A value of `sdrkit_fy3_ahrpt_view`. The composite, else a channel. A view that the instrument shown lacks is refused.
static inline int32_t sdrkit_fy3_ahrpt_get_view(SDRKIT_HANDLE handle, sdrkit_fy3_ahrpt_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy3_ahrpt_view)index;
  return code;
}
static inline int32_t sdrkit_fy3_ahrpt_set_view(SDRKIT_HANDLE handle, sdrkit_fy3_ahrpt_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "fy3_ahrpt".
static inline int32_t sdrkit_fy3_ahrpt_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_fy3_ahrpt_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_satellite" (Test spacecraft id) of the test signal generator of the stack "fy3_ahrpt".
static inline int32_t sdrkit_fy3_ahrpt_gen_get_satellite(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_satellite", value);
}
static inline int32_t sdrkit_fy3_ahrpt_gen_set_satellite(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_satellite", value);
}

// The setting "gen_symbol_rate" (Test symbol rate) of the test signal generator of the stack "fy3_ahrpt". Unit: Msym/s.
static inline int32_t sdrkit_fy3_ahrpt_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_fy3_ahrpt_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_qneg" (Test Q negated) of the test signal generator of the stack "fy3_ahrpt".
static inline int32_t sdrkit_fy3_ahrpt_gen_get_qneg(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_qneg", value);
}
static inline int32_t sdrkit_fy3_ahrpt_gen_set_qneg(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_qneg", value);
}

// The setting "gen_swap" (Test I and Q exchanged) of the test signal generator of the stack "fy3_ahrpt".
static inline int32_t sdrkit_fy3_ahrpt_gen_get_swap(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_swap", value);
}
static inline int32_t sdrkit_fy3_ahrpt_gen_set_swap(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_swap", value);
}

// The setting "gen_fill_every" (Test fill frame every) of the test signal generator of the stack "fy3_ahrpt". 0: the instrument's own share of fill frames.
static inline int32_t sdrkit_fy3_ahrpt_gen_get_fill_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fill_every", value);
}
static inline int32_t sdrkit_fy3_ahrpt_gen_set_fill_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fill_every", value);
}

// Fills `rows` with the changed rows of the table "instruments" of the stack "fy3_mpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy3_mpt_instruments_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_satimg_fy3_instrument_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.Fy3InstrumentRow", "instruments", rows, sizeof(sdrkit_satimg_fy3_instrument_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "fy3_mpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy3_mpt_images_rows(SDRKIT_HANDLE handle,
                                                sdrkit_satimg_image_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "fy3_mpt" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_fy3_mpt_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fy3_mpt" are the sdrkit_fy3_mpt_gen_*() functions
// after the settings of its receiver.

// The setting "preset" (Satellite) of the stack "fy3_mpt". A value of `sdrkit_fy3_mpt_preset`. Retunes to the preset's frequency; restarts.
static inline int32_t sdrkit_fy3_mpt_get_preset(SDRKIT_HANDLE handle, sdrkit_fy3_mpt_preset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "preset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy3_mpt_preset)index;
  return code;
}
static inline int32_t sdrkit_fy3_mpt_set_preset(SDRKIT_HANDLE handle, sdrkit_fy3_mpt_preset value) {
  return sdrkit_typed_setting_set_enum(handle, "preset", (int32_t)value);
}

// The setting "symbol_rate" (Symbol rate) of the stack "fy3_mpt". Unit: Msym/s. 0 = automatic. Restarts.
static inline int32_t sdrkit_fy3_mpt_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_fy3_mpt_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "fy3_mpt". 0 = automatic. Restarts.
static inline int32_t sdrkit_fy3_mpt_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_fy3_mpt_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// The setting "instrument" (Instrument) of the stack "fy3_mpt". A value of `sdrkit_fy3_mpt_instrument`. The instrument whose picture is shown.
static inline int32_t sdrkit_fy3_mpt_get_instrument(SDRKIT_HANDLE handle, sdrkit_fy3_mpt_instrument *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "instrument", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy3_mpt_instrument)index;
  return code;
}
static inline int32_t sdrkit_fy3_mpt_set_instrument(SDRKIT_HANDLE handle, sdrkit_fy3_mpt_instrument value) {
  return sdrkit_typed_setting_set_enum(handle, "instrument", (int32_t)value);
}

// The setting "view" (View) of the stack "fy3_mpt". A value of `sdrkit_fy3_mpt_view`. The composite, else a channel. A view that the instrument shown lacks is refused.
static inline int32_t sdrkit_fy3_mpt_get_view(SDRKIT_HANDLE handle, sdrkit_fy3_mpt_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy3_mpt_view)index;
  return code;
}
static inline int32_t sdrkit_fy3_mpt_set_view(SDRKIT_HANDLE handle, sdrkit_fy3_mpt_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "fy3_mpt".
static inline int32_t sdrkit_fy3_mpt_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_fy3_mpt_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_satellite" (Test spacecraft id) of the test signal generator of the stack "fy3_mpt".
static inline int32_t sdrkit_fy3_mpt_gen_get_satellite(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_satellite", value);
}
static inline int32_t sdrkit_fy3_mpt_gen_set_satellite(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_satellite", value);
}

// The setting "gen_symbol_rate" (Test symbol rate) of the test signal generator of the stack "fy3_mpt". Unit: Msym/s.
static inline int32_t sdrkit_fy3_mpt_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_fy3_mpt_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_qneg" (Test Q negated) of the test signal generator of the stack "fy3_mpt".
static inline int32_t sdrkit_fy3_mpt_gen_get_qneg(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_qneg", value);
}
static inline int32_t sdrkit_fy3_mpt_gen_set_qneg(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_qneg", value);
}

// The setting "gen_swap" (Test I and Q exchanged) of the test signal generator of the stack "fy3_mpt".
static inline int32_t sdrkit_fy3_mpt_gen_get_swap(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_swap", value);
}
static inline int32_t sdrkit_fy3_mpt_gen_set_swap(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_swap", value);
}

// The setting "gen_fill_every" (Test fill frame every) of the test signal generator of the stack "fy3_mpt". 0: the instrument's own share of fill frames.
static inline int32_t sdrkit_fy3_mpt_gen_get_fill_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fill_every", value);
}
static inline int32_t sdrkit_fy3_mpt_gen_set_fill_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fill_every", value);
}

// Fills `rows` with the changed rows of the table "images" of the stack "noaa_apt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noaa_apt_images_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_image_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "noaa_apt" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_noaa_apt_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "noaa_apt" are the sdrkit_noaa_apt_gen_*() functions
// after the settings of its receiver.

// The setting "picture" (Picture) of the stack "noaa_apt". A value of `sdrkit_noaa_apt_picture`. The full line, or one of the two channels of the line.
static inline int32_t sdrkit_noaa_apt_get_picture(SDRKIT_HANDLE handle, sdrkit_noaa_apt_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_noaa_apt_picture)index;
  return code;
}
static inline int32_t sdrkit_noaa_apt_set_picture(SDRKIT_HANDLE handle, sdrkit_noaa_apt_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "picture", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "noaa_apt".
static inline int32_t sdrkit_noaa_apt_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_noaa_apt_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "line_filter" (Line filter) of the stack "noaa_apt". Removes clicks and smooths noise along each line of both pictures, with the noise measured in space A.
static inline int32_t sdrkit_noaa_apt_get_line_filter(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "line_filter", value);
}
static inline int32_t sdrkit_noaa_apt_set_line_filter(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "line_filter", value);
}

// The setting "gen_lines" (Test picture lines) of the test signal generator of the stack "noaa_apt".
static inline int32_t sdrkit_noaa_apt_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_noaa_apt_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// Fills `rows` with the changed rows of the table "images" of the stack "noaa_dsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noaa_dsb_images_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_image_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "noaa_dsb" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_noaa_dsb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "noaa_dsb" are the sdrkit_noaa_dsb_gen_*() functions
// after the settings of its receiver.

// The setting "view" (View) of the stack "noaa_dsb". A value of `sdrkit_noaa_dsb_view`. What the HIRS picture shows.
static inline int32_t sdrkit_noaa_dsb_get_view(SDRKIT_HANDLE handle, sdrkit_noaa_dsb_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_noaa_dsb_view)index;
  return code;
}
static inline int32_t sdrkit_noaa_dsb_set_view(SDRKIT_HANDLE handle, sdrkit_noaa_dsb_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "noaa_dsb".
static inline int32_t sdrkit_noaa_dsb_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_noaa_dsb_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_scid" (Test spacecraft address) of the test signal generator of the stack "noaa_dsb".
static inline int32_t sdrkit_noaa_dsb_gen_get_scid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_scid", value);
}
static inline int32_t sdrkit_noaa_dsb_gen_set_scid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_scid", value);
}

// The setting "gen_pm_index" (Test phase index) of the test signal generator of the stack "noaa_dsb". Unit: deg.
static inline int32_t sdrkit_noaa_dsb_gen_get_pm_index(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pm_index", value);
}
static inline int32_t sdrkit_noaa_dsb_gen_set_pm_index(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pm_index", value);
}

// The setting "gen_lines" (Test picture lines) of the test signal generator of the stack "noaa_dsb".
static inline int32_t sdrkit_noaa_dsb_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_noaa_dsb_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// Fills `rows` with the changed rows of the table "blocks" of the stack "goes_gvar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_gvar_blocks_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_goes_gvar_block_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.GoesGvarBlockRow", "blocks", rows, sizeof(sdrkit_goes_gvar_block_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "goes_gvar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_gvar_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "goes_gvar" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_goes_gvar_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "goes_gvar" are the sdrkit_goes_gvar_gen_*() functions
// after the settings of its receiver.

// The setting "picture" (Picture) of the stack "goes_gvar". A value of `sdrkit_goes_gvar_picture`. The visible picture or the infrared picture.
static inline int32_t sdrkit_goes_gvar_get_picture(SDRKIT_HANDLE handle, sdrkit_goes_gvar_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_gvar_picture)index;
  return code;
}
static inline int32_t sdrkit_goes_gvar_set_picture(SDRKIT_HANDLE handle, sdrkit_goes_gvar_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "picture", (int32_t)value);
}

// The setting "view" (View) of the stack "goes_gvar". A value of `sdrkit_goes_gvar_view`. The channel shown. The visible picture has one view.
static inline int32_t sdrkit_goes_gvar_get_view(SDRKIT_HANDLE handle, sdrkit_goes_gvar_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_gvar_view)index;
  return code;
}
static inline int32_t sdrkit_goes_gvar_set_view(SDRKIT_HANDLE handle, sdrkit_goes_gvar_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "goes_gvar".
static inline int32_t sdrkit_goes_gvar_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_goes_gvar_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_lines" (Test scans) of the test signal generator of the stack "goes_gvar".
static inline int32_t sdrkit_goes_gvar_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_goes_gvar_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "aws" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aws_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_aws_virtual_channel_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.AwsVirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_aws_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "apids" of the stack "aws" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aws_apids_rows(SDRKIT_HANDLE handle,
                                           sdrkit_aws_apid_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.AwsApidRow", "apids", rows, sizeof(sdrkit_aws_apid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "aws" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aws_images_rows(SDRKIT_HANDLE handle,
                                            sdrkit_satimg_image_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "aws" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_aws_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "aws" are the sdrkit_aws_gen_*() functions
// after the settings of its receiver.

// The setting "view" (View) of the stack "aws". A value of `sdrkit_aws_view`. The radiometer channel shown.
static inline int32_t sdrkit_aws_get_view(SDRKIT_HANDLE handle, sdrkit_aws_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_aws_view)index;
  return code;
}
static inline int32_t sdrkit_aws_set_view(SDRKIT_HANDLE handle, sdrkit_aws_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "aws".
static inline int32_t sdrkit_aws_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_aws_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_lines" (Test picture scans) of the test signal generator of the stack "aws".
static inline int32_t sdrkit_aws_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_aws_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// Fills `rows` with the changed rows of the table "images" of the stack "noaa_gac" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noaa_gac_images_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_image_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "noaa_gac" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_noaa_gac_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "noaa_gac" are the sdrkit_noaa_gac_gen_*() functions
// after the settings of its receiver.

// The setting "picture" (Picture) of the stack "noaa_gac". A value of `sdrkit_noaa_gac_picture`. The AVHRR picture or the HIRS picture.
static inline int32_t sdrkit_noaa_gac_get_picture(SDRKIT_HANDLE handle, sdrkit_noaa_gac_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_noaa_gac_picture)index;
  return code;
}
static inline int32_t sdrkit_noaa_gac_set_picture(SDRKIT_HANDLE handle, sdrkit_noaa_gac_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "picture", (int32_t)value);
}

// The setting "view" (View) of the stack "noaa_gac". A value of `sdrkit_noaa_gac_view`. What the picture shows.
static inline int32_t sdrkit_noaa_gac_get_view(SDRKIT_HANDLE handle, sdrkit_noaa_gac_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_noaa_gac_view)index;
  return code;
}
static inline int32_t sdrkit_noaa_gac_set_view(SDRKIT_HANDLE handle, sdrkit_noaa_gac_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "noaa_gac".
static inline int32_t sdrkit_noaa_gac_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_noaa_gac_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_scid" (Test spacecraft address) of the test signal generator of the stack "noaa_gac".
static inline int32_t sdrkit_noaa_gac_gen_get_scid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_scid", value);
}
static inline int32_t sdrkit_noaa_gac_gen_set_scid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_scid", value);
}

// The setting "gen_lines" (Test picture lines) of the test signal generator of the stack "noaa_gac".
static inline int32_t sdrkit_noaa_gac_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_noaa_gac_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "msg_raw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msg_raw_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_satimg_virtual_channel_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "msg_raw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_msg_raw_images_rows(SDRKIT_HANDLE handle,
                                                sdrkit_satimg_image_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "msg_raw" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_msg_raw_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "msg_raw" are the sdrkit_msg_raw_gen_*() functions
// after the settings of its receiver.

// The setting "picture" (Picture) of the stack "msg_raw". A value of `sdrkit_msg_raw_picture`. The eleven standard channels or the HRV channel.
static inline int32_t sdrkit_msg_raw_get_picture(SDRKIT_HANDLE handle, sdrkit_msg_raw_picture *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "picture", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_msg_raw_picture)index;
  return code;
}
static inline int32_t sdrkit_msg_raw_set_picture(SDRKIT_HANDLE handle, sdrkit_msg_raw_picture value) {
  return sdrkit_typed_setting_set_enum(handle, "picture", (int32_t)value);
}

// The setting "view" (View) of the stack "msg_raw". A value of `sdrkit_msg_raw_view`. What the picture shows.
static inline int32_t sdrkit_msg_raw_get_view(SDRKIT_HANDLE handle, sdrkit_msg_raw_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_msg_raw_view)index;
  return code;
}
static inline int32_t sdrkit_msg_raw_set_view(SDRKIT_HANDLE handle, sdrkit_msg_raw_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "msg_raw".
static inline int32_t sdrkit_msg_raw_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_msg_raw_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_scid" (Test satellite number) of the test signal generator of the stack "msg_raw".
static inline int32_t sdrkit_msg_raw_gen_get_scid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_scid", value);
}
static inline int32_t sdrkit_msg_raw_gen_set_scid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_scid", value);
}

// The setting "gen_lines" (Test picture lines) of the test signal generator of the stack "msg_raw".
static inline int32_t sdrkit_msg_raw_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_msg_raw_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// The setting "gen_start" (Test first line) of the test signal generator of the stack "msg_raw".
static inline int32_t sdrkit_msg_raw_gen_get_start(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_start", value);
}
static inline int32_t sdrkit_msg_raw_gen_set_start(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_start", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "goes_cda" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_goes_cda_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FrameRow", "frames", rows, sizeof(sdrkit_satimg_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "goes_cda" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_cda_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                           sdrkit_satimg_virtual_channel_row *rows,
                                                           uint32_t capacity,
                                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "goes_cda" on the session of `handle`. Empties the frame table and the counters; the synchronisation stays.
static inline int32_t sdrkit_goes_cda_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "goes_cda" are the sdrkit_goes_cda_gen_*() functions
// after the settings of its receiver.

// The setting "gen_pm_index" (Test phase index (GOES-N)) of the test signal generator of the stack "goes_cda". Unit: deg.
static inline int32_t sdrkit_goes_cda_gen_get_pm_index(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pm_index", value);
}
static inline int32_t sdrkit_goes_cda_gen_set_pm_index(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pm_index", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "goes_mdl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_goes_mdl_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FrameRow", "frames", rows, sizeof(sdrkit_satimg_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "goes_mdl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_mdl_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                           sdrkit_satimg_virtual_channel_row *rows,
                                                           uint32_t capacity,
                                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "goes_mdl" on the session of `handle`. Empties the frame table and the counters; the synchronisation stays.
static inline int32_t sdrkit_goes_mdl_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "gk2a_cdas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_gk2a_cdas_frames_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_frame_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FrameRow", "frames", rows, sizeof(sdrkit_satimg_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "gk2a_cdas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_cdas_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_satimg_virtual_channel_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "gk2a_cdas" on the session of `handle`. Empties the frame table and the counters; the synchronisation stays.
static inline int32_t sdrkit_gk2a_cdas_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "sband_ccsds" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_sband_ccsds_frames_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_frame_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FrameRow", "frames", rows, sizeof(sdrkit_satimg_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "sband_ccsds" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sband_ccsds_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                              sdrkit_satimg_virtual_channel_row *rows,
                                                              uint32_t capacity,
                                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "sband_ccsds" on the session of `handle`. Empties the frame table and the counters; the synchronisation stays.
static inline int32_t sdrkit_sband_ccsds_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "link_name" (Link name) of the stack "sband_ccsds". The name in the summary.
static inline int32_t sdrkit_sband_ccsds_get_link_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "link_name", value);
}
static inline int32_t sdrkit_sband_ccsds_set_link_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "link_name", value);
}

// The setting "modulation" (Modulation) of the stack "sband_ccsds". A value of `sdrkit_sband_ccsds_modulation`.
static inline int32_t sdrkit_sband_ccsds_get_modulation(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sband_ccsds_modulation)index;
  return code;
}
static inline int32_t sdrkit_sband_ccsds_set_modulation(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "modulation", (int32_t)value);
}

// The setting "symbol_rate" (Symbol rate) of the stack "sband_ccsds". A frequency in Hz.
static inline int32_t sdrkit_sband_ccsds_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_sband_ccsds_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Matched filter roll-off) of the stack "sband_ccsds".
static inline int32_t sdrkit_sband_ccsds_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_sband_ccsds_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// The setting "conv" (K = 7 code) of the stack "sband_ccsds". The rate 1/2 code of CCSDS, second output inverted, over the frames and their sync words.
static inline int32_t sdrkit_sband_ccsds_get_conv(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "conv", value);
}
static inline int32_t sdrkit_sband_ccsds_set_conv(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "conv", value);
}

// The setting "frame_bits" (Frame length) of the stack "sband_ccsds". Unit: bits. The frame with its sync word.
static inline int32_t sdrkit_sband_ccsds_get_frame_bits(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "frame_bits", value);
}
static inline int32_t sdrkit_sband_ccsds_set_frame_bits(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "frame_bits", value);
}

// The setting "sync_word" (Sync word) of the stack "sband_ccsds". 32 bits, 8 hexadecimal digits.
static inline int32_t sdrkit_sband_ccsds_get_sync_word(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "sync_word", value);
}
static inline int32_t sdrkit_sband_ccsds_set_sync_word(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "sync_word", value);
}

// The setting "nrzm" (NRZ-M) of the stack "sband_ccsds". On a coded link before the code; on uncoded QPSK the four-phase differential coding.
static inline int32_t sdrkit_sband_ccsds_get_nrzm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "nrzm", value);
}
static inline int32_t sdrkit_sband_ccsds_set_nrzm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "nrzm", value);
}

// The setting "randomiser" (CCSDS randomiser) of the stack "sband_ccsds". A value of `sdrkit_sband_ccsds_randomiser`.
static inline int32_t sdrkit_sband_ccsds_get_randomiser(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_randomiser *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "randomiser", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sband_ccsds_randomiser)index;
  return code;
}
static inline int32_t sdrkit_sband_ccsds_set_randomiser(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_randomiser value) {
  return sdrkit_typed_setting_set_enum(handle, "randomiser", (int32_t)value);
}

// The setting "rs" (Reed-Solomon) of the stack "sband_ccsds". A value of `sdrkit_sband_ccsds_rs`.
static inline int32_t sdrkit_sband_ccsds_get_rs(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_rs *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rs", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sband_ccsds_rs)index;
  return code;
}
static inline int32_t sdrkit_sband_ccsds_set_rs(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_rs value) {
  return sdrkit_typed_setting_set_enum(handle, "rs", (int32_t)value);
}

// The setting "rs_interleave" (Interleave) of the stack "sband_ccsds".
static inline int32_t sdrkit_sband_ccsds_get_rs_interleave(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "rs_interleave", value);
}
static inline int32_t sdrkit_sband_ccsds_set_rs_interleave(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "rs_interleave", value);
}

// The setting "rs_basis" (Basis) of the stack "sband_ccsds". A value of `sdrkit_sband_ccsds_rs_basis`.
static inline int32_t sdrkit_sband_ccsds_get_rs_basis(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_rs_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rs_basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sband_ccsds_rs_basis)index;
  return code;
}
static inline int32_t sdrkit_sband_ccsds_set_rs_basis(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_rs_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "rs_basis", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "rdas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_rdas_frames_rows(SDRKIT_HANDLE handle,
                                             sdrkit_satimg_frame_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FrameRow", "frames", rows, sizeof(sdrkit_satimg_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "rdas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rdas_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_satimg_virtual_channel_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "rdas" on the session of `handle`. Empties the frame table and the counters; the synchronisation stays.
static inline int32_t sdrkit_rdas_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "symbol_rate" (Symbol rate) of the stack "rdas". A frequency in Hz. Not confirmed by a public document.
static inline int32_t sdrkit_rdas_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_rdas_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Matched filter roll-off) of the stack "rdas". Not confirmed by a public document.
static inline int32_t sdrkit_rdas_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_rdas_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_carriers_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dvbs2_carrier_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2CarrierRow", "carriers", rows, sizeof(sdrkit_dvbs2_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_streams_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvbs2_stream_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamRow", "streams", rows, sizeof(sdrkit_dvbs2_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stream_modcods" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_stream_modcods_rows(SDRKIT_HANDLE handle,
                                                           sdrkit_dvbs2_stream_modcod_row *rows,
                                                           uint32_t capacity,
                                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamModcodRow", "stream_modcods", rows, sizeof(sdrkit_dvbs2_stream_modcod_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets_by_apid" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_packets_by_apid_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_gk2a_uhrit_apid_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.UhritApidRow", "packets_by_apid", rows, sizeof(sdrkit_gk2a_uhrit_apid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_frames_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_satimg_uhrit_frame_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.UhritFrameRow", "frames", rows, sizeof(sdrkit_satimg_uhrit_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_products_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_satimg_product_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "gk2a_uhrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gk2a_uhrit_images_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_satimg_image_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "import_keys" (Import keys) of the stack "gk2a_uhrit" on the session of `handle`. Reads a KMA key file.
static inline int32_t sdrkit_gk2a_uhrit_import_keys(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "import_keys", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "add_keys" (Add keys) of the stack "gk2a_uhrit" on the session of `handle`. Adds keys typed as text.
static inline int32_t sdrkit_gk2a_uhrit_add_keys(SDRKIT_HANDLE handle, const sdrkit_satimg_keys_args *args) {
  return sdrkit_typed_command(handle, "add_keys", "satimg.KeysArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "gk2a_uhrit" on the session of `handle`. Forgets every product.
static inline int32_t sdrkit_gk2a_uhrit_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "symbol_rate" (Symbol rate) of the stack "gk2a_uhrit". A frequency in Hz.
static inline int32_t sdrkit_gk2a_uhrit_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "gk2a_uhrit". A value of `sdrkit_gk2a_uhrit_roll_off`.
static inline int32_t sdrkit_gk2a_uhrit_get_roll_off(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_roll_off *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "roll_off", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_uhrit_roll_off)index;
  return code;
}
static inline int32_t sdrkit_gk2a_uhrit_set_roll_off(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_roll_off value) {
  return sdrkit_typed_setting_set_enum(handle, "roll_off", (int32_t)value);
}

// The setting "carrier_offset" (Carrier) of the stack "gk2a_uhrit". Unit: Hz. Offset in Hz from the centre. Empty: the carrier nearest the centre.
static inline int32_t sdrkit_gk2a_uhrit_get_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "carrier_offset", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "carrier_offset", value);
}

// The setting "uncounted_secondary" (Secondary header outside the length) of the stack "gk2a_uhrit". Unit: bytes. A packet with its secondary header flag set is this much longer than its length field says.
static inline int32_t sdrkit_gk2a_uhrit_get_uncounted_secondary(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "uncounted_secondary", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_uncounted_secondary(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "uncounted_secondary", value);
}

// The setting "filter" (Show) of the stack "gk2a_uhrit". A value of `sdrkit_gk2a_uhrit_filter`.
static inline int32_t sdrkit_gk2a_uhrit_get_filter(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_uhrit_filter)index;
  return code;
}
static inline int32_t sdrkit_gk2a_uhrit_set_filter(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "gk2a_uhrit".
static inline int32_t sdrkit_gk2a_uhrit_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// The setting "modcod_rule" (MODCODs taken) of the stack "gk2a_uhrit". A value of `sdrkit_gk2a_uhrit_modcod_rule`. Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.
static inline int32_t sdrkit_gk2a_uhrit_get_modcod_rule(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_modcod_rule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_uhrit_modcod_rule)index;
  return code;
}
static inline int32_t sdrkit_gk2a_uhrit_set_modcod_rule(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_modcod_rule value) {
  return sdrkit_typed_setting_set_enum(handle, "modcod_rule", (int32_t)value);
}

// The setting "modcod" (MODCOD) of the stack "gk2a_uhrit".
static inline int32_t sdrkit_gk2a_uhrit_get_modcod(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "modcod", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_modcod(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "modcod", value);
}

// The setting "modcods" (MODCOD set) of the stack "gk2a_uhrit". MODCOD numbers with commas between them, for the rule "set".
static inline int32_t sdrkit_gk2a_uhrit_get_modcods(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "modcods", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_modcods(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "modcods", value);
}

// The setting "frame_length" (Frame length) of the stack "gk2a_uhrit". A value of `sdrkit_gk2a_uhrit_frame_length`. Frames of the other length are counted and skipped.
static inline int32_t sdrkit_gk2a_uhrit_get_frame_length(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_frame_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_uhrit_frame_length)index;
  return code;
}
static inline int32_t sdrkit_gk2a_uhrit_set_frame_length(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_frame_length value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_length", (int32_t)value);
}

// The setting "pilots" (Pilots) of the stack "gk2a_uhrit". A value of `sdrkit_gk2a_uhrit_pilots`. A check only: a frame with other pilots is counted and decoded.
static inline int32_t sdrkit_gk2a_uhrit_get_pilots(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gk2a_uhrit_pilots)index;
  return code;
}
static inline int32_t sdrkit_gk2a_uhrit_set_pilots(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "pilots", (int32_t)value);
}

// The setting "pl_scrambling" (PL scrambling code) of the stack "gk2a_uhrit".
static inline int32_t sdrkit_gk2a_uhrit_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pl_scrambling", value);
}
static inline int32_t sdrkit_gk2a_uhrit_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "fy4_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4_lrit_carriers_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvbs2_carrier_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2CarrierRow", "carriers", rows, sizeof(sdrkit_dvbs2_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "fy4_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4_lrit_streams_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvbs2_stream_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamRow", "streams", rows, sizeof(sdrkit_dvbs2_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stream_modcods" of the stack "fy4_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4_lrit_stream_modcods_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_dvbs2_stream_modcod_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamModcodRow", "stream_modcods", rows, sizeof(sdrkit_dvbs2_stream_modcod_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "fy4_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4_lrit_products_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_satimg_product_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "fy4_lrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4_lrit_images_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_image_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "open_cadu" (Open CADU) of the stack "fy4_lrit" on the session of `handle`. Decodes a file of 1024-byte CADUs at the next input.
static inline int32_t sdrkit_fy4_lrit_open_cadu(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_cadu", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "fy4_lrit" on the session of `handle`. Forgets every product.
static inline int32_t sdrkit_fy4_lrit_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "satellite" (Satellite) of the stack "fy4_lrit". A value of `sdrkit_fy4_lrit_satellite`. A running LRIT stack switches at once.
static inline int32_t sdrkit_fy4_lrit_get_satellite(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_satellite *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "satellite", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4_lrit_satellite)index;
  return code;
}
static inline int32_t sdrkit_fy4_lrit_set_satellite(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_satellite value) {
  return sdrkit_typed_setting_set_enum(handle, "satellite", (int32_t)value);
}

// The setting "symbol_rate" (Symbol rate) of the stack "fy4_lrit". A frequency in Hz.
static inline int32_t sdrkit_fy4_lrit_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_fy4_lrit_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "fy4_lrit". A value of `sdrkit_fy4_lrit_roll_off`.
static inline int32_t sdrkit_fy4_lrit_get_roll_off(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_roll_off *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "roll_off", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4_lrit_roll_off)index;
  return code;
}
static inline int32_t sdrkit_fy4_lrit_set_roll_off(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_roll_off value) {
  return sdrkit_typed_setting_set_enum(handle, "roll_off", (int32_t)value);
}

// The setting "carrier_offset" (Carrier) of the stack "fy4_lrit". Unit: Hz. Offset in Hz from the centre. Empty: the carrier nearest the centre.
static inline int32_t sdrkit_fy4_lrit_get_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "carrier_offset", value);
}
static inline int32_t sdrkit_fy4_lrit_set_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "carrier_offset", value);
}

// The setting "isi" (Input stream) of the stack "fy4_lrit". -1: the first stream seen.
static inline int32_t sdrkit_fy4_lrit_get_isi(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "isi", value);
}
static inline int32_t sdrkit_fy4_lrit_set_isi(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "isi", value);
}

// The setting "modcod_rule" (MODCODs taken) of the stack "fy4_lrit". A value of `sdrkit_fy4_lrit_modcod_rule`. Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.
static inline int32_t sdrkit_fy4_lrit_get_modcod_rule(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_modcod_rule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4_lrit_modcod_rule)index;
  return code;
}
static inline int32_t sdrkit_fy4_lrit_set_modcod_rule(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_modcod_rule value) {
  return sdrkit_typed_setting_set_enum(handle, "modcod_rule", (int32_t)value);
}

// The setting "modcod" (MODCOD) of the stack "fy4_lrit".
static inline int32_t sdrkit_fy4_lrit_get_modcod(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "modcod", value);
}
static inline int32_t sdrkit_fy4_lrit_set_modcod(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "modcod", value);
}

// The setting "modcods" (MODCOD set) of the stack "fy4_lrit". MODCOD numbers with commas between them, for the rule "set".
static inline int32_t sdrkit_fy4_lrit_get_modcods(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "modcods", value);
}
static inline int32_t sdrkit_fy4_lrit_set_modcods(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "modcods", value);
}

// The setting "frame_length" (Frame length) of the stack "fy4_lrit". A value of `sdrkit_fy4_lrit_frame_length`. Frames of the other length are counted and skipped.
static inline int32_t sdrkit_fy4_lrit_get_frame_length(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_frame_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4_lrit_frame_length)index;
  return code;
}
static inline int32_t sdrkit_fy4_lrit_set_frame_length(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_frame_length value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_length", (int32_t)value);
}

// The setting "pilots" (Pilots) of the stack "fy4_lrit". A value of `sdrkit_fy4_lrit_pilots`. A check only: a frame with other pilots is counted and decoded.
static inline int32_t sdrkit_fy4_lrit_get_pilots(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4_lrit_pilots)index;
  return code;
}
static inline int32_t sdrkit_fy4_lrit_set_pilots(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "pilots", (int32_t)value);
}

// The setting "pl_scrambling" (PL scrambling code) of the stack "fy4_lrit".
static inline int32_t sdrkit_fy4_lrit_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pl_scrambling", value);
}
static inline int32_t sdrkit_fy4_lrit_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value);
}

// The setting "filter" (Show) of the stack "fy4_lrit". A value of `sdrkit_fy4_lrit_filter`.
static inline int32_t sdrkit_fy4_lrit_get_filter(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4_lrit_filter)index;
  return code;
}
static inline int32_t sdrkit_fy4_lrit_set_filter(SDRKIT_HANDLE handle, sdrkit_fy4_lrit_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "fy4_lrit".
static inline int32_t sdrkit_fy4_lrit_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_fy4_lrit_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "fy4a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4a_hrit_carriers_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dvbs2_carrier_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2CarrierRow", "carriers", rows, sizeof(sdrkit_dvbs2_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "fy4a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4a_hrit_streams_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvbs2_stream_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamRow", "streams", rows, sizeof(sdrkit_dvbs2_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stream_modcods" of the stack "fy4a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4a_hrit_stream_modcods_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_dvbs2_stream_modcod_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamModcodRow", "stream_modcods", rows, sizeof(sdrkit_dvbs2_stream_modcod_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "fy4a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4a_hrit_products_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_product_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "fy4a_hrit" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fy4a_hrit_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "open_cadu" (Open CADU) of the stack "fy4a_hrit" on the session of `handle`. Decodes a file of 1024-byte CADUs at the next input.
static inline int32_t sdrkit_fy4a_hrit_open_cadu(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_cadu", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "fy4a_hrit" on the session of `handle`. Forgets every product.
static inline int32_t sdrkit_fy4a_hrit_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "symbol_rate" (Symbol rate) of the stack "fy4a_hrit". A frequency in Hz.
static inline int32_t sdrkit_fy4a_hrit_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "fy4a_hrit". A value of `sdrkit_fy4a_hrit_roll_off`.
static inline int32_t sdrkit_fy4a_hrit_get_roll_off(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_roll_off *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "roll_off", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4a_hrit_roll_off)index;
  return code;
}
static inline int32_t sdrkit_fy4a_hrit_set_roll_off(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_roll_off value) {
  return sdrkit_typed_setting_set_enum(handle, "roll_off", (int32_t)value);
}

// The setting "carrier_offset" (Carrier) of the stack "fy4a_hrit". Unit: Hz. Offset in Hz from the centre. Empty: the carrier nearest the centre.
static inline int32_t sdrkit_fy4a_hrit_get_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "carrier_offset", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "carrier_offset", value);
}

// The setting "isi" (Input stream) of the stack "fy4a_hrit". -1: the first stream seen.
static inline int32_t sdrkit_fy4a_hrit_get_isi(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "isi", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_isi(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "isi", value);
}

// The setting "modcod_rule" (MODCODs taken) of the stack "fy4a_hrit". A value of `sdrkit_fy4a_hrit_modcod_rule`. Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.
static inline int32_t sdrkit_fy4a_hrit_get_modcod_rule(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_modcod_rule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4a_hrit_modcod_rule)index;
  return code;
}
static inline int32_t sdrkit_fy4a_hrit_set_modcod_rule(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_modcod_rule value) {
  return sdrkit_typed_setting_set_enum(handle, "modcod_rule", (int32_t)value);
}

// The setting "modcod" (MODCOD) of the stack "fy4a_hrit".
static inline int32_t sdrkit_fy4a_hrit_get_modcod(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "modcod", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_modcod(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "modcod", value);
}

// The setting "modcods" (MODCOD set) of the stack "fy4a_hrit". MODCOD numbers with commas between them, for the rule "set".
static inline int32_t sdrkit_fy4a_hrit_get_modcods(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "modcods", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_modcods(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "modcods", value);
}

// The setting "frame_length" (Frame length) of the stack "fy4a_hrit". A value of `sdrkit_fy4a_hrit_frame_length`. Frames of the other length are counted and skipped.
static inline int32_t sdrkit_fy4a_hrit_get_frame_length(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_frame_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4a_hrit_frame_length)index;
  return code;
}
static inline int32_t sdrkit_fy4a_hrit_set_frame_length(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_frame_length value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_length", (int32_t)value);
}

// The setting "pilots" (Pilots) of the stack "fy4a_hrit". A value of `sdrkit_fy4a_hrit_pilots`. A check only: a frame with other pilots is counted and decoded.
static inline int32_t sdrkit_fy4a_hrit_get_pilots(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4a_hrit_pilots)index;
  return code;
}
static inline int32_t sdrkit_fy4a_hrit_set_pilots(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "pilots", (int32_t)value);
}

// The setting "pl_scrambling" (PL scrambling code) of the stack "fy4a_hrit".
static inline int32_t sdrkit_fy4a_hrit_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pl_scrambling", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value);
}

// The setting "filter" (Show) of the stack "fy4a_hrit". A value of `sdrkit_fy4a_hrit_filter`.
static inline int32_t sdrkit_fy4a_hrit_get_filter(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fy4a_hrit_filter)index;
  return code;
}
static inline int32_t sdrkit_fy4a_hrit_set_filter(SDRKIT_HANDLE handle, sdrkit_fy4a_hrit_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "fy4a_hrit".
static inline int32_t sdrkit_fy4a_hrit_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_fy4a_hrit_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_grb_carriers_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dvbs2_carrier_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2CarrierRow", "carriers", rows, sizeof(sdrkit_dvbs2_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_grb_streams_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dvbs2_stream_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamRow", "streams", rows, sizeof(sdrkit_dvbs2_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stream_modcods" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_grb_stream_modcods_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_dvbs2_stream_modcod_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamModcodRow", "stream_modcods", rows, sizeof(sdrkit_dvbs2_stream_modcod_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "grb_products" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_goes_grb_products_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_goes_grb_product_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.GrbProductRow", "grb_products", rows, sizeof(sdrkit_goes_grb_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets_by_apid" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_goes_grb_packets_by_apid_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_goes_grb_apid_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.GrbApidRow", "packets_by_apid", rows, sizeof(sdrkit_goes_grb_apid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_grb_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_satimg_uhrit_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.UhritFrameRow", "frames", rows, sizeof(sdrkit_satimg_uhrit_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "flashes" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `position`.
static inline int32_t sdrkit_goes_grb_flashes_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_goes_grb_glm_flash_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.GlmFlashRow", "flashes", rows, sizeof(sdrkit_goes_grb_glm_flash_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "groups" of the stack "goes_grb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `position`.
static inline int32_t sdrkit_goes_grb_groups_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_goes_grb_glm_group_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.GlmGroupRow", "groups", rows, sizeof(sdrkit_goes_grb_glm_group_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "goes_grb" on the session of `handle`. Forgets the pictures, the payloads and the counters.
static inline int32_t sdrkit_goes_grb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "symbol_rate" (Symbol rate) of the stack "goes_grb". A frequency in Hz.
static inline int32_t sdrkit_goes_grb_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_goes_grb_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "goes_grb". A value of `sdrkit_goes_grb_roll_off`.
static inline int32_t sdrkit_goes_grb_get_roll_off(SDRKIT_HANDLE handle, sdrkit_goes_grb_roll_off *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "roll_off", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_grb_roll_off)index;
  return code;
}
static inline int32_t sdrkit_goes_grb_set_roll_off(SDRKIT_HANDLE handle, sdrkit_goes_grb_roll_off value) {
  return sdrkit_typed_setting_set_enum(handle, "roll_off", (int32_t)value);
}

// The setting "carrier_offset" (Carrier) of the stack "goes_grb". Unit: Hz. Offset in Hz from the centre. Empty: the carrier nearest the centre.
static inline int32_t sdrkit_goes_grb_get_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "carrier_offset", value);
}
static inline int32_t sdrkit_goes_grb_set_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "carrier_offset", value);
}

// The setting "product" (Product shown) of the stack "goes_grb". An APID, or -1 for the picture updated last.
static inline int32_t sdrkit_goes_grb_get_product(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "product", value);
}
static inline int32_t sdrkit_goes_grb_set_product(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "product", value);
}

// The setting "channels" (Virtual channels) of the stack "goes_grb". Virtual channel numbers with commas between them. Empty: every channel but 63.
static inline int32_t sdrkit_goes_grb_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_goes_grb_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "modcod_rule" (MODCODs taken) of the stack "goes_grb". A value of `sdrkit_goes_grb_modcod_rule`. Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.
static inline int32_t sdrkit_goes_grb_get_modcod_rule(SDRKIT_HANDLE handle, sdrkit_goes_grb_modcod_rule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_grb_modcod_rule)index;
  return code;
}
static inline int32_t sdrkit_goes_grb_set_modcod_rule(SDRKIT_HANDLE handle, sdrkit_goes_grb_modcod_rule value) {
  return sdrkit_typed_setting_set_enum(handle, "modcod_rule", (int32_t)value);
}

// The setting "modcod" (MODCOD) of the stack "goes_grb".
static inline int32_t sdrkit_goes_grb_get_modcod(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "modcod", value);
}
static inline int32_t sdrkit_goes_grb_set_modcod(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "modcod", value);
}

// The setting "modcods" (MODCOD set) of the stack "goes_grb". MODCOD numbers with commas between them, for the rule "set".
static inline int32_t sdrkit_goes_grb_get_modcods(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "modcods", value);
}
static inline int32_t sdrkit_goes_grb_set_modcods(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "modcods", value);
}

// The setting "frame_length" (Frame length) of the stack "goes_grb". A value of `sdrkit_goes_grb_frame_length`. Frames of the other length are counted and skipped.
static inline int32_t sdrkit_goes_grb_get_frame_length(SDRKIT_HANDLE handle, sdrkit_goes_grb_frame_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_grb_frame_length)index;
  return code;
}
static inline int32_t sdrkit_goes_grb_set_frame_length(SDRKIT_HANDLE handle, sdrkit_goes_grb_frame_length value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_length", (int32_t)value);
}

// The setting "pilots" (Pilots) of the stack "goes_grb". A value of `sdrkit_goes_grb_pilots`. A check only: a frame with other pilots is counted and decoded.
static inline int32_t sdrkit_goes_grb_get_pilots(SDRKIT_HANDLE handle, sdrkit_goes_grb_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_grb_pilots)index;
  return code;
}
static inline int32_t sdrkit_goes_grb_set_pilots(SDRKIT_HANDLE handle, sdrkit_goes_grb_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "pilots", (int32_t)value);
}

// The setting "pl_scrambling" (PL scrambling code) of the stack "goes_grb".
static inline int32_t sdrkit_goes_grb_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pl_scrambling", value);
}
static inline int32_t sdrkit_goes_grb_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "sat_filecast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sat_filecast_carriers_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_dvbs2_carrier_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2CarrierRow", "carriers", rows, sizeof(sdrkit_dvbs2_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "sat_filecast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sat_filecast_streams_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_dvbs2_stream_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamRow", "streams", rows, sizeof(sdrkit_dvbs2_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stream_modcods" of the stack "sat_filecast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sat_filecast_stream_modcods_rows(SDRKIT_HANDLE handle,
                                                             sdrkit_dvbs2_stream_modcod_row *rows,
                                                             uint32_t capacity,
                                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbs2.Dvbs2StreamModcodRow", "stream_modcods", rows, sizeof(sdrkit_dvbs2_stream_modcod_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "recent_files" of the stack "sat_filecast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_sat_filecast_recent_files_rows(SDRKIT_HANDLE handle,
                                                           sdrkit_sat_filecast_recent_file_row *rows,
                                                           uint32_t capacity,
                                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.RecentFileRow", "recent_files", rows, sizeof(sdrkit_sat_filecast_recent_file_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "pictures" of the stack "sat_filecast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sat_filecast_pictures_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_sat_filecast_picture_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FilecastPictureRow", "pictures", rows, sizeof(sdrkit_sat_filecast_picture_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "sat_filecast" on the session of `handle`. Forgets the open files and the counters; keeps the pictures.
static inline int32_t sdrkit_sat_filecast_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sat_filecast" are the sdrkit_sat_filecast_gen_*() functions
// after the settings of its receiver.

// The setting "symbol_rate" (Symbol rate) of the stack "sat_filecast". A frequency in Hz.
static inline int32_t sdrkit_sat_filecast_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_sat_filecast_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Roll-off) of the stack "sat_filecast". A value of `sdrkit_sat_filecast_roll_off`.
static inline int32_t sdrkit_sat_filecast_get_roll_off(SDRKIT_HANDLE handle, sdrkit_sat_filecast_roll_off *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "roll_off", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sat_filecast_roll_off)index;
  return code;
}
static inline int32_t sdrkit_sat_filecast_set_roll_off(SDRKIT_HANDLE handle, sdrkit_sat_filecast_roll_off value) {
  return sdrkit_typed_setting_set_enum(handle, "roll_off", (int32_t)value);
}

// The setting "carrier_offset" (Carrier) of the stack "sat_filecast". Unit: Hz. Offset in Hz from the centre. Empty: the carrier nearest the centre.
static inline int32_t sdrkit_sat_filecast_get_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "carrier_offset", value);
}
static inline int32_t sdrkit_sat_filecast_set_carrier_offset(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "carrier_offset", value);
}

// The setting "pid" (PID of the MPE stream) of the stack "sat_filecast". The transport stream PID that carries the datagram sections (256 in the test signal).
static inline int32_t sdrkit_sat_filecast_get_pid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pid", value);
}
static inline int32_t sdrkit_sat_filecast_set_pid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pid", value);
}

// The setting "part_bytes" (Part size) of the stack "sat_filecast". Unit: bytes. Bytes of each numbered part of a file (1408 in the test signal). Not confirmed by a public document.
static inline int32_t sdrkit_sat_filecast_get_part_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "part_bytes", value);
}
static inline int32_t sdrkit_sat_filecast_set_part_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "part_bytes", value);
}

// The setting "max_silence" (Give up on a file after) of the stack "sat_filecast". Unit: s.
static inline int32_t sdrkit_sat_filecast_get_max_silence(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_silence", value);
}
static inline int32_t sdrkit_sat_filecast_set_max_silence(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_silence", value);
}

// The setting "name_tag" (Name tag of GEONETCAST) of the stack "sat_filecast". Unit: characters. GEONETCAST names end with a tag of this many characters, which the stack removes.
static inline int32_t sdrkit_sat_filecast_get_name_tag(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "name_tag", value);
}
static inline int32_t sdrkit_sat_filecast_set_name_tag(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "name_tag", value);
}

// The setting "isi" (Input stream) of the stack "sat_filecast". -1: the first stream seen.
static inline int32_t sdrkit_sat_filecast_get_isi(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "isi", value);
}
static inline int32_t sdrkit_sat_filecast_set_isi(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "isi", value);
}

// The setting "modcod_rule" (MODCODs taken) of the stack "sat_filecast". A value of `sdrkit_sat_filecast_modcod_rule`. Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.
static inline int32_t sdrkit_sat_filecast_get_modcod_rule(SDRKIT_HANDLE handle, sdrkit_sat_filecast_modcod_rule *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sat_filecast_modcod_rule)index;
  return code;
}
static inline int32_t sdrkit_sat_filecast_set_modcod_rule(SDRKIT_HANDLE handle, sdrkit_sat_filecast_modcod_rule value) {
  return sdrkit_typed_setting_set_enum(handle, "modcod_rule", (int32_t)value);
}

// The setting "modcod" (MODCOD) of the stack "sat_filecast".
static inline int32_t sdrkit_sat_filecast_get_modcod(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "modcod", value);
}
static inline int32_t sdrkit_sat_filecast_set_modcod(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "modcod", value);
}

// The setting "modcods" (MODCOD set) of the stack "sat_filecast". MODCOD numbers with commas between them, for the rule "set".
static inline int32_t sdrkit_sat_filecast_get_modcods(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "modcods", value);
}
static inline int32_t sdrkit_sat_filecast_set_modcods(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "modcods", value);
}

// The setting "frame_length" (Frame length) of the stack "sat_filecast". A value of `sdrkit_sat_filecast_frame_length`. Frames of the other length are counted and skipped.
static inline int32_t sdrkit_sat_filecast_get_frame_length(SDRKIT_HANDLE handle, sdrkit_sat_filecast_frame_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sat_filecast_frame_length)index;
  return code;
}
static inline int32_t sdrkit_sat_filecast_set_frame_length(SDRKIT_HANDLE handle, sdrkit_sat_filecast_frame_length value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_length", (int32_t)value);
}

// The setting "pilots" (Pilots) of the stack "sat_filecast". A value of `sdrkit_sat_filecast_pilots`. A check only: a frame with other pilots is counted and decoded.
static inline int32_t sdrkit_sat_filecast_get_pilots(SDRKIT_HANDLE handle, sdrkit_sat_filecast_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sat_filecast_pilots)index;
  return code;
}
static inline int32_t sdrkit_sat_filecast_set_pilots(SDRKIT_HANDLE handle, sdrkit_sat_filecast_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "pilots", (int32_t)value);
}

// The setting "pl_scrambling" (PL scrambling code) of the stack "sat_filecast".
static inline int32_t sdrkit_sat_filecast_get_pl_scrambling(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "pl_scrambling", value);
}
static inline int32_t sdrkit_sat_filecast_set_pl_scrambling(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value);
}

// The setting "gen_symbol_rate" (Symbol rate) of the test signal generator of the stack "sat_filecast". A frequency in Hz.
static inline int32_t sdrkit_sat_filecast_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_sat_filecast_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "elektro_l" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_elektro_l_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_satimg_virtual_channel_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "elektro_l" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_elektro_l_products_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_satimg_product_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ProductRow", "products", rows, sizeof(sdrkit_satimg_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "elektro_l" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_elektro_l_images_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_satimg_image_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "satimg.ggak_frame"
// (GGAK frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_satimg_ggak_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_satimg_ggak_frame_event *event) {
  return sdrkit_typed_event(handle, n, "satimg.ggak_frame", "satimg.GgakFrameEvent", event, sizeof(sdrkit_satimg_ggak_frame_event));
}

// Runs the command "open_cadu" (Open CADU) of the stack "elektro_l" on the session of `handle`. Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.
static inline int32_t sdrkit_elektro_l_open_cadu(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_cadu", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "elektro_l" on the session of `handle`. Forgets the newest frame.
static inline int32_t sdrkit_elektro_l_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "elektro_l" are the sdrkit_elektro_l_gen_*() functions
// after the settings of its receiver.

// The setting "craft" (Satellite) of the stack "elektro_l". A value of `sdrkit_elektro_l_craft`. Names only (summary and product names).
static inline int32_t sdrkit_elektro_l_get_craft(SDRKIT_HANDLE handle, sdrkit_elektro_l_craft *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "craft", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_elektro_l_craft)index;
  return code;
}
static inline int32_t sdrkit_elektro_l_set_craft(SDRKIT_HANDLE handle, sdrkit_elektro_l_craft value) {
  return sdrkit_typed_setting_set_enum(handle, "craft", (int32_t)value);
}

// The setting "link" (Link) of the stack "elektro_l". A value of `sdrkit_elektro_l_link`. Restarts the demodulator (the products stay).
static inline int32_t sdrkit_elektro_l_get_link(SDRKIT_HANDLE handle, sdrkit_elektro_l_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_elektro_l_link)index;
  return code;
}
static inline int32_t sdrkit_elektro_l_set_link(SDRKIT_HANDLE handle, sdrkit_elektro_l_link value) {
  return sdrkit_typed_setting_set_enum(handle, "link", (int32_t)value);
}

// The setting "filter" (Show) of the stack "elektro_l". A value of `sdrkit_elektro_l_filter`.
static inline int32_t sdrkit_elektro_l_get_filter(SDRKIT_HANDLE handle, sdrkit_elektro_l_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_elektro_l_filter)index;
  return code;
}
static inline int32_t sdrkit_elektro_l_set_filter(SDRKIT_HANDLE handle, sdrkit_elektro_l_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "filter", (int32_t)value);
}

// The setting "follow_newest" (Newest) of the stack "elektro_l".
static inline int32_t sdrkit_elektro_l_get_follow_newest(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_newest", value);
}
static inline int32_t sdrkit_elektro_l_set_follow_newest(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_newest", value);
}

// The setting "gen_invert_g2" (Test G2 inverted) of the test signal generator of the stack "elektro_l".
static inline int32_t sdrkit_elektro_l_gen_get_invert_g2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert_g2", value);
}
static inline int32_t sdrkit_elektro_l_gen_set_invert_g2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert_g2", value);
}

// The setting "gen_nrzm" (Test NRZ-M) of the test signal generator of the stack "elektro_l".
static inline int32_t sdrkit_elektro_l_gen_get_nrzm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nrzm", value);
}
static inline int32_t sdrkit_elektro_l_gen_set_nrzm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nrzm", value);
}

// The setting "gen_basis" (Test Reed-Solomon basis) of the test signal generator of the stack "elektro_l". A value of `sdrkit_elektro_l_gen_basis`.
static inline int32_t sdrkit_elektro_l_gen_get_basis(SDRKIT_HANDLE handle, sdrkit_elektro_l_gen_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_elektro_l_gen_basis)index;
  return code;
}
static inline int32_t sdrkit_elektro_l_gen_set_basis(SDRKIT_HANDLE handle, sdrkit_elektro_l_gen_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_basis", (int32_t)value);
}

// The setting "gen_fill" (Test fill frames) of the test signal generator of the stack "elektro_l".
static inline int32_t sdrkit_elektro_l_gen_get_fill(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fill", value);
}
static inline int32_t sdrkit_elektro_l_gen_set_fill(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fill", value);
}

// The setting "gen_once" (Test files once) of the test signal generator of the stack "elektro_l".
static inline int32_t sdrkit_elektro_l_gen_get_once(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_once", value);
}
static inline int32_t sdrkit_elektro_l_gen_set_once(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_once", value);
}

// The setting "gen_counter" (Test frame count start) of the test signal generator of the stack "elektro_l".
static inline int32_t sdrkit_elektro_l_gen_get_counter(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_counter", value);
}
static inline int32_t sdrkit_elektro_l_gen_set_counter(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_counter", value);
}

// The setting "gen_link" (Test link) of the test signal generator of the stack "elektro_l". A value of `sdrkit_elektro_l_gen_link`.
static inline int32_t sdrkit_elektro_l_gen_get_link(SDRKIT_HANDLE handle, sdrkit_elektro_l_gen_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_elektro_l_gen_link)index;
  return code;
}
static inline int32_t sdrkit_elektro_l_gen_set_link(SDRKIT_HANDLE handle, sdrkit_elektro_l_gen_link value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_link", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "hrpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hrpt_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_satimg_virtual_channel_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "images" of the stack "hrpt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hrpt_images_rows(SDRKIT_HANDLE handle,
                                             sdrkit_satimg_image_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.ImageRow", "images", rows, sizeof(sdrkit_satimg_image_row), capacity, info);
}

// Runs the command "clear" (New picture) of the stack "hrpt" on the session of `handle`. Starts a new picture.
static inline int32_t sdrkit_hrpt_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hrpt" are the sdrkit_hrpt_gen_*() functions
// after the settings of its receiver.

// The setting "frequency_preset" (Frequency) of the stack "hrpt". A value of `sdrkit_hrpt_frequency_preset`. Active now: MetOp-B and MetOp-C (AHRPT 1701.3 MHz) and the FY-3 satellites (check the frequency of each one).
static inline int32_t sdrkit_hrpt_get_frequency_preset(SDRKIT_HANDLE handle, sdrkit_hrpt_frequency_preset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frequency_preset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hrpt_frequency_preset)index;
  return code;
}
static inline int32_t sdrkit_hrpt_set_frequency_preset(SDRKIT_HANDLE handle, sdrkit_hrpt_frequency_preset value) {
  return sdrkit_typed_setting_set_enum(handle, "frequency_preset", (int32_t)value);
}

// The setting "variant" (Satellite) of the stack "hrpt". A value of `sdrkit_hrpt_variant`. Restarts the demodulator.
static inline int32_t sdrkit_hrpt_get_variant(SDRKIT_HANDLE handle, sdrkit_hrpt_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hrpt_variant)index;
  return code;
}
static inline int32_t sdrkit_hrpt_set_variant(SDRKIT_HANDLE handle, sdrkit_hrpt_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "variant", (int32_t)value);
}

// The setting "view" (View) of the stack "hrpt". A value of `sdrkit_hrpt_view`. What the picture shows.
static inline int32_t sdrkit_hrpt_get_view(SDRKIT_HANDLE handle, sdrkit_hrpt_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hrpt_view)index;
  return code;
}
static inline int32_t sdrkit_hrpt_set_view(SDRKIT_HANDLE handle, sdrkit_hrpt_view value) {
  return sdrkit_typed_setting_set_enum(handle, "view", (int32_t)value);
}

// The setting "hirs_view" (HIRS view) of the stack "hrpt". A value of `sdrkit_hrpt_hirs_view`. What the HIRS picture of NOAA HRPT shows.
static inline int32_t sdrkit_hrpt_get_hirs_view(SDRKIT_HANDLE handle, sdrkit_hrpt_hirs_view *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "hirs_view", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hrpt_hirs_view)index;
  return code;
}
static inline int32_t sdrkit_hrpt_set_hirs_view(SDRKIT_HANDLE handle, sdrkit_hrpt_hirs_view value) {
  return sdrkit_typed_setting_set_enum(handle, "hirs_view", (int32_t)value);
}

// The setting "follow" (Follow) of the stack "hrpt".
static inline int32_t sdrkit_hrpt_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_hrpt_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_variant" (Test satellite) of the test signal generator of the stack "hrpt". A value of `sdrkit_hrpt_gen_variant`.
static inline int32_t sdrkit_hrpt_gen_get_variant(SDRKIT_HANDLE handle, sdrkit_hrpt_gen_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hrpt_gen_variant)index;
  return code;
}
static inline int32_t sdrkit_hrpt_gen_set_variant(SDRKIT_HANDLE handle, sdrkit_hrpt_gen_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_variant", (int32_t)value);
}

// The setting "gen_3b" (Test channel 3B (APID 104)) of the test signal generator of the stack "hrpt".
static inline int32_t sdrkit_hrpt_gen_get_3b(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_3b", value);
}
static inline int32_t sdrkit_hrpt_gen_set_3b(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_3b", value);
}

// The setting "gen_lines" (Test picture lines) of the test signal generator of the stack "hrpt".
static inline int32_t sdrkit_hrpt_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_hrpt_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// The setting "gen_polarity" (Test inverted NOAA polarity) of the test signal generator of the stack "hrpt".
static inline int32_t sdrkit_hrpt_gen_get_polarity(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_polarity", value);
}
static inline int32_t sdrkit_hrpt_gen_set_polarity(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_polarity", value);
}

// The setting "gen_pm_index" (Test phase index (Meteor)) of the test signal generator of the stack "hrpt". Unit: deg.
static inline int32_t sdrkit_hrpt_gen_get_pm_index(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pm_index", value);
}
static inline int32_t sdrkit_hrpt_gen_set_pm_index(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pm_index", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "eodb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `hex` names a row.
static inline int32_t sdrkit_eodb_frames_rows(SDRKIT_HANDLE handle,
                                             sdrkit_satimg_frame_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.FrameRow", "frames", rows, sizeof(sdrkit_satimg_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "virtual_channels" of the stack "eodb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_eodb_virtual_channels_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_satimg_virtual_channel_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satimg.VirtualChannelRow", "virtual_channels", rows, sizeof(sdrkit_satimg_virtual_channel_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "eodb" on the session of `handle`. Empties the frame table and the counters; the synchronisation stays.
static inline int32_t sdrkit_eodb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "open_frames" (Open frame file) of the stack "eodb" on the session of `handle`. Decodes a file of CADUs of the link's length, each starting with the marker 1ACFFC1D.
static inline int32_t sdrkit_eodb_open_frames(SDRKIT_HANDLE handle, const sdrkit_satimg_file_args *args) {
  return sdrkit_typed_command(handle, "open_frames", "satimg.FileArgs", args, NULL, NULL, 0);
}

// Runs the command "product" (Make product) of the stack "eodb" on the session of `handle`. Makes a colour product of the channel pictures.
static inline int32_t sdrkit_eodb_product(SDRKIT_HANDLE handle, const sdrkit_eodb_product_args *args) {
  return sdrkit_typed_command(handle, "product", "satimg.EodbProductArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "eodb" are the sdrkit_eodb_gen_*() functions
// after the settings of its receiver.

// The setting "symbol_rate" (Symbol rate) of the stack "eodb". A frequency in Hz. 0 takes the symbol rate of the profile, which no public document confirms.
static inline int32_t sdrkit_eodb_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "symbol_rate", value);
}
static inline int32_t sdrkit_eodb_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "symbol_rate", value);
}

// The setting "roll_off" (Matched filter roll-off) of the stack "eodb". 0 takes the roll-off of the profile, which no public document confirms.
static inline int32_t sdrkit_eodb_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_eodb_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// The setting "satellites" (Satellite list) of the stack "eodb". A file in the host's storage with the satellites of each link and their spacecraft ids (data/links/xband_downlinks.csv). Empty: no satellite names.
static inline int32_t sdrkit_eodb_get_satellites(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "satellites", value);
}
static inline int32_t sdrkit_eodb_set_satellites(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "satellites", value);
}

// The setting "avhrr_calibration" (AVHRR calibration) of the stack "eodb". A file in the host's storage with the AVHRR calibration coefficients of each satellite (data/satimg/avhrr_calibration.csv). Empty: no temperature products.
static inline int32_t sdrkit_eodb_get_avhrr_calibration(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "avhrr_calibration", value);
}
static inline int32_t sdrkit_eodb_set_avhrr_calibration(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "avhrr_calibration", value);
}

// The setting "gen_ambiguity" (Test signal ambiguity) of the test signal generator of the stack "eodb". Turns the test signal by 90 degrees this many times (modulo 4), mirrored from 4 on.
static inline int32_t sdrkit_eodb_gen_get_ambiguity(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ambiguity", value);
}
static inline int32_t sdrkit_eodb_gen_set_ambiguity(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ambiguity", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satimg_link_fields_read(SDRKIT_HANDLE handle, sdrkit_satimg_link_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.LinkFields", fields, sizeof(sdrkit_satimg_link_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_meteor_lrpt_fields_read(SDRKIT_HANDLE handle, sdrkit_meteor_lrpt_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.MeteorLrptFields", fields, sizeof(sdrkit_meteor_lrpt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satimg_xrit_fields_read(SDRKIT_HANDLE handle, sdrkit_satimg_xrit_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.XritFields", fields, sizeof(sdrkit_satimg_xrit_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fy2_svissr_fields_read(SDRKIT_HANDLE handle, sdrkit_fy2_svissr_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.Fy2SvissrFields", fields, sizeof(sdrkit_fy2_svissr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satimg_fy3_fields_read(SDRKIT_HANDLE handle, sdrkit_satimg_fy3_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.Fy3Fields", fields, sizeof(sdrkit_satimg_fy3_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_noaa_apt_fields_read(SDRKIT_HANDLE handle, sdrkit_noaa_apt_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.NoaaAptFields", fields, sizeof(sdrkit_noaa_apt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_noaa_dsb_fields_read(SDRKIT_HANDLE handle, sdrkit_noaa_dsb_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.NoaaDsbFields", fields, sizeof(sdrkit_noaa_dsb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_goes_gvar_fields_read(SDRKIT_HANDLE handle, sdrkit_goes_gvar_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.GoesGvarFields", fields, sizeof(sdrkit_goes_gvar_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_aws_fields_read(SDRKIT_HANDLE handle, sdrkit_aws_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.AwsFields", fields, sizeof(sdrkit_aws_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_noaa_gac_fields_read(SDRKIT_HANDLE handle, sdrkit_noaa_gac_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.NoaaGacFields", fields, sizeof(sdrkit_noaa_gac_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_msg_raw_fields_read(SDRKIT_HANDLE handle, sdrkit_msg_raw_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.MsgRawFields", fields, sizeof(sdrkit_msg_raw_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satimg_frame_table_fields_read(SDRKIT_HANDLE handle, sdrkit_satimg_frame_table_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.FrameTableFields", fields, sizeof(sdrkit_satimg_frame_table_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sband_ccsds_fields_read(SDRKIT_HANDLE handle, sdrkit_sband_ccsds_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.SbandCcsdsFields", fields, sizeof(sdrkit_sband_ccsds_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_rdas_fields_read(SDRKIT_HANDLE handle, sdrkit_rdas_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.RdasFields", fields, sizeof(sdrkit_rdas_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satimg_cadu_fields_read(SDRKIT_HANDLE handle, sdrkit_satimg_cadu_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.CaduFields", fields, sizeof(sdrkit_satimg_cadu_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gk2a_uhrit_fields_read(SDRKIT_HANDLE handle, sdrkit_gk2a_uhrit_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.UhritFields", fields, sizeof(sdrkit_gk2a_uhrit_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satimg_fy4_fields_read(SDRKIT_HANDLE handle, sdrkit_satimg_fy4_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.Fy4Fields", fields, sizeof(sdrkit_satimg_fy4_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_goes_grb_fields_read(SDRKIT_HANDLE handle, sdrkit_goes_grb_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.GrbFields", fields, sizeof(sdrkit_goes_grb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_goes_grb_glm_fields_read(SDRKIT_HANDLE handle, sdrkit_goes_grb_glm_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.GlmFields", fields, sizeof(sdrkit_goes_grb_glm_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sat_filecast_fields_read(SDRKIT_HANDLE handle, sdrkit_sat_filecast_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.FilecastFields", fields, sizeof(sdrkit_sat_filecast_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hrpt_fields_read(SDRKIT_HANDLE handle, sdrkit_hrpt_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.HrptFields", fields, sizeof(sdrkit_hrpt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_eodb_fields_read(SDRKIT_HANDLE handle, sdrkit_eodb_fields *fields) {
  return sdrkit_typed_fields(handle, "satimg.EodbFields", fields, sizeof(sdrkit_eodb_fields));
}

// Fills `fields` with the result fields of the component "s2_report" of the stack "gk2a_uhrit" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_report.<key>".
static inline int32_t sdrkit_gk2a_uhrit_s2_report_dvbs2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbs2.Dvbs2Fields@s2_report", fields, sizeof(sdrkit_dvbs2_fields));
}

// Fills `fields` with the result fields of the component "s2_report" of the stack "fy4_lrit" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_report.<key>".
static inline int32_t sdrkit_fy4_lrit_s2_report_dvbs2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbs2.Dvbs2Fields@s2_report", fields, sizeof(sdrkit_dvbs2_fields));
}

// Fills `fields` with the result fields of the component "s2_bbframe" of the stack "fy4_lrit" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_bbframe.<key>".
static inline int32_t sdrkit_fy4_lrit_s2_bbframe_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@s2_bbframe", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

// Fills `fields` with the result fields of the component "s2_report" of the stack "fy4a_hrit" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_report.<key>".
static inline int32_t sdrkit_fy4a_hrit_s2_report_dvbs2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbs2.Dvbs2Fields@s2_report", fields, sizeof(sdrkit_dvbs2_fields));
}

// Fills `fields` with the result fields of the component "s2_bbframe" of the stack "fy4a_hrit" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_bbframe.<key>".
static inline int32_t sdrkit_fy4a_hrit_s2_bbframe_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@s2_bbframe", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

// Fills `fields` with the result fields of the component "s2_report" of the stack "goes_grb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_report.<key>".
static inline int32_t sdrkit_goes_grb_s2_report_dvbs2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbs2.Dvbs2Fields@s2_report", fields, sizeof(sdrkit_dvbs2_fields));
}

// Fills `fields` with the result fields of the component "s2_report" of the stack "sat_filecast" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_report.<key>".
static inline int32_t sdrkit_sat_filecast_s2_report_dvbs2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbs2.Dvbs2Fields@s2_report", fields, sizeof(sdrkit_dvbs2_fields));
}

// Fills `fields` with the result fields of the component "s2_bbframe" of the stack "sat_filecast" in the snapshot of
// `handle`. A generic consumer reads them under the keys "s2_bbframe.<key>".
static inline int32_t sdrkit_sat_filecast_s2_bbframe_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@s2_bbframe", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

#endif
