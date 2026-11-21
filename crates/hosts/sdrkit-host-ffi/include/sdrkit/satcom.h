/*
 * sdrkit: the typed interface of the stack family "satcom", version 1.2. MIT licence.
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

#ifndef SDRKIT_SATCOM_H
#define SDRKIT_SATCOM_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_SATCOM_VERSION_MAJOR 1
#define SDRKIT_SATCOM_VERSION_MINOR 2

// The stack "inmarsatc": its id, and the keys of its event types.
#define SDRKIT_STACK_INMARSATC "inmarsatc"
#define SDRKIT_EVENT_INMARSATC_EGC "inmarsatc.egc"

// The stack "aero": its id, and the keys of its event types.
#define SDRKIT_STACK_AERO "aero"
#define SDRKIT_EVENT_AERO_MESSAGE "aero.message"
#define SDRKIT_EVENT_AERO_LOGON "aero.logon"

// The stack "inmarsat": its id, and the keys of its event types.
#define SDRKIT_STACK_INMARSAT "inmarsat"

// The stack "iridium": its id, and the keys of its event types.
#define SDRKIT_STACK_IRIDIUM "iridium"
#define SDRKIT_EVENT_IRIDIUM_RAW "iridium.raw"

// The stack "globalstar": its id, and the keys of its event types.
#define SDRKIT_STACK_GLOBALSTAR "globalstar"
#define SDRKIT_EVENT_GLOBALSTAR_ACTIVITY "globalstar.activity"

// The stack "goes_dcs": its id, and the keys of its event types.
#define SDRKIT_STACK_GOES_DCS "goes_dcs"
#define SDRKIT_EVENT_GOES_DCS_REPORT "goes_dcs.report"

// The stack "argos": its id, and the keys of its event types.
#define SDRKIT_STACK_ARGOS "argos"
#define SDRKIT_EVENT_ARGOS_PTT "argos.ptt"
#define SDRKIT_EVENT_ARGOS_DOWNLINK_FRAME "argos.downlink_frame"

// The stack "hrdcp": its id, and the keys of its event types.
#define SDRKIT_STACK_HRDCP "hrdcp"
#define SDRKIT_EVENT_HRDCP_REPORT "hrdcp.report"

// The stack "sarsat": its id, and the keys of its event types.
#define SDRKIT_STACK_SARSAT "sarsat"
#define SDRKIT_EVENT_SARSAT_BEACON "sarsat.beacon"

// The stack "lora_sat": its id, and the keys of its event types.
#define SDRKIT_STACK_LORA_SAT "lora_sat"
#define SDRKIT_EVENT_LORA_SAT_FRAME "lora_sat.frame"

// The stack "orbcomm": its id, and the keys of its event types.
#define SDRKIT_STACK_ORBCOMM "orbcomm"
#define SDRKIT_EVENT_ORBCOMM_MESSAGE "orbcomm.message"

// The stack "psk_demod": its id, and the keys of its event types.
#define SDRKIT_STACK_PSK_DEMOD "psk_demod"
#define SDRKIT_EVENT_PSK_DEMOD_LOCK "psk_demod.lock"
#define SDRKIT_EVENT_PSK_DEMOD_BYTES "psk_demod.bytes"

// The stack "qo100": its id, and the keys of its event types.
#define SDRKIT_STACK_QO100 "qo100"
#define SDRKIT_EVENT_QO100_LOCK "qo100.lock"
#define SDRKIT_EVENT_QO100_CONVERSATION "qo100.conversation"

// The stack "cubesat": its id, and the keys of its event types.
#define SDRKIT_STACK_CUBESAT "cubesat"
#define SDRKIT_EVENT_CUBESAT_FRAME "cubesat.frame"
#define SDRKIT_EVENT_CUBESAT_KISS "cubesat.kiss"
#define SDRKIT_EVENT_CUBESAT_TX "cubesat.tx"

// The stack "fleet_broadcast": its id, and the keys of its event types.
#define SDRKIT_STACK_FLEET_BROADCAST "fleet_broadcast"
#define SDRKIT_EVENT_FLEET_BROADCAST_STATE "fleet_broadcast.state"

// The stack "bgan": its id, and the keys of its event types.
#define SDRKIT_STACK_BGAN "bgan"
#define SDRKIT_EVENT_BGAN_SIGNALLING "bgan.signalling"

// The stack "sat_survey": its id, and the keys of its event types.
#define SDRKIT_STACK_SAT_SURVEY "sat_survey"
#define SDRKIT_EVENT_SURVEY_CARRIER "survey.carrier"

// The options of the setting "satellite" of the stack "inmarsatc".
typedef enum sdrkit_inmarsatc_satellite {
  // None (tune by hand)
  SDRKIT_INMARSATC_SATELLITE_NONE = 0,
  // Alphasat 25E (Europe, Africa, Indian Ocean) 1537.10 MHz IOR
  SDRKIT_INMARSATC_SATELLITE_ALPHASAT = 1,
  // Inmarsat-4 F3 98W (Americas) 1537.70 MHz AOR-W
  SDRKIT_INMARSATC_SATELLITE_I4F3 = 2,
  // Inmarsat-3 F5 54W (Atlantic East) 1541.45 MHz AOR-E
  SDRKIT_INMARSATC_SATELLITE_I3F5 = 3,
  // Inmarsat-4 F1 143.5E (Pacific, Asia) 1541.45 MHz POR (community value)
  SDRKIT_INMARSATC_SATELLITE_I4F1 = 4,
} sdrkit_inmarsatc_satellite;

// The options of the setting "min_priority" of the stack "inmarsatc".
typedef enum sdrkit_inmarsatc_min_priority {
  // All
  SDRKIT_INMARSATC_MIN_PRIORITY_ALL = 0,
  // Safety and higher
  SDRKIT_INMARSATC_MIN_PRIORITY_SAFETY_AND_HIGHER = 1,
  // Urgency and higher
  SDRKIT_INMARSATC_MIN_PRIORITY_URGENCY_AND_HIGHER = 2,
  // Distress
  SDRKIT_INMARSATC_MIN_PRIORITY_DISTRESS = 3,
} sdrkit_inmarsatc_min_priority;

// The options of the setting "service_filter" of the stack "inmarsatc".
typedef enum sdrkit_inmarsatc_service_filter {
  // All
  SDRKIT_INMARSATC_SERVICE_FILTER_ALL = 0,
  // SafetyNET
  SDRKIT_INMARSATC_SERVICE_FILTER_SAFETYNET = 1,
  // FleetNET
  SDRKIT_INMARSATC_SERVICE_FILTER_FLEETNET = 2,
} sdrkit_inmarsatc_service_filter;

// The options of the setting "satellite" of the stack "aero".
typedef enum sdrkit_aero_satellite {
  // None (tune by hand, 1545.57 MHz)
  SDRKIT_AERO_SATELLITE_NONE = 0,
  // Alphasat 25E (Europe, Africa)
  SDRKIT_AERO_SATELLITE_ALPHASAT = 1,
  // Inmarsat-4 F3 98W (Americas)
  SDRKIT_AERO_SATELLITE_I4F3 = 2,
  // Inmarsat-3 F5 54W (Atlantic, South America)
  SDRKIT_AERO_SATELLITE_I3F5 = 3,
  // Inmarsat-6 F1 83.5E (Indian Ocean, Asia-Pacific)
  SDRKIT_AERO_SATELLITE_I6F1 = 4,
  // Inmarsat-4 F1 143.5E (Pacific, Asia)
  SDRKIT_AERO_SATELLITE_I4F1 = 5,
} sdrkit_aero_satellite;

// The options of the setting "satellite" of the stack "inmarsat".
typedef enum sdrkit_inmarsat_satellite {
  // Automatic
  SDRKIT_INMARSAT_SATELLITE_AUTO = 0,
  // Alphasat 25E
  SDRKIT_INMARSAT_SATELLITE_ALPHASAT_25_E = 1,
  // Inmarsat-4 F3 98W
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_4_F3_98_W = 2,
  // Inmarsat-3 F5 54W (AOR-W)
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_3_F5_54_W_A_O_R_W = 3,
  // Inmarsat-6 F1 83.5E
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_6_F1_83_5_E = 4,
  // Inmarsat-4 F1 143.5E
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_4_F1_143_5_E = 5,
  // Inmarsat 15.5W (AOR-E)
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_15_5_W_A_O_R_E = 6,
  // Inmarsat 64.5E (IOR)
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_64_5_E_I_O_R = 7,
  // Inmarsat 178E (POR)
  SDRKIT_INMARSAT_SATELLITE_INMARSAT_178_E_P_O_R = 8,
} sdrkit_inmarsat_satellite;

// The options of the setting "gen_scenario" of the stack "globalstar".
typedef enum sdrkit_globalstar_gen_scenario {
  // A pass
  SDRKIT_GLOBALSTAR_GEN_SCENARIO_PASS = 0,
  // Steady
  SDRKIT_GLOBALSTAR_GEN_SCENARIO_STEADY = 1,
  // Silence
  SDRKIT_GLOBALSTAR_GEN_SCENARIO_SILENCE = 2,
} sdrkit_globalstar_gen_scenario;

// The options of the setting "gen_scenario" of the stack "goes_dcs".
typedef enum sdrkit_goes_dcs_gen_scenario {
  // Three platforms (100, 300, 1200 bit/s Manchester)
  SDRKIT_GOES_DCS_GEN_SCENARIO_THREE_PLATFORMS = 0,
  // Only the 300 bit/s platform
  SDRKIT_GOES_DCS_GEN_SCENARIO_ONLY_THE_300_BIT_S_PLATFORM = 1,
  // One of each rate, 300 and 1200 as NRZ
  SDRKIT_GOES_DCS_GEN_SCENARIO_ONE_OF_EACH_RATE_300_AND_1200_AS_NRZ = 2,
  // Three platforms and a steady carrier
  SDRKIT_GOES_DCS_GEN_SCENARIO_THREE_PLATFORMS_AND_A_STEADY_CARRIER = 3,
} sdrkit_goes_dcs_gen_scenario;

// The options of the setting "link" of the stack "argos".
typedef enum sdrkit_argos_link {
  // Automatic (from the dial)
  SDRKIT_ARGOS_LINK_AUTO = 0,
  // Uplink
  SDRKIT_ARGOS_LINK_UPLINK = 1,
  // Downlink
  SDRKIT_ARGOS_LINK_DOWNLINK = 2,
} sdrkit_argos_link;

// The options of the setting "gen_scene" of the stack "hrdcp".
typedef enum sdrkit_hrdcp_gen_scene {
  // Two platforms
  SDRKIT_HRDCP_GEN_SCENE_TWO_PLATFORMS = 0,
  // Platform A alone
  SDRKIT_HRDCP_GEN_SCENE_PLATFORM_A_ALONE = 1,
} sdrkit_hrdcp_gen_scene;

// The options of the setting "sf" of the stack "lora_sat".
typedef enum sdrkit_lora_sat_sf {
  // 7
  SDRKIT_LORA_SAT_SF_7 = 0,
  // 8
  SDRKIT_LORA_SAT_SF_8 = 1,
  // 9
  SDRKIT_LORA_SAT_SF_9 = 2,
  // 10
  SDRKIT_LORA_SAT_SF_10 = 3,
  // 11
  SDRKIT_LORA_SAT_SF_11 = 4,
  // 12
  SDRKIT_LORA_SAT_SF_12 = 5,
} sdrkit_lora_sat_sf;

// The options of the setting "bw_khz" of the stack "lora_sat".
typedef enum sdrkit_lora_sat_bw_khz {
  // 41.7 kHz
  SDRKIT_LORA_SAT_BW_KHZ_41_7 = 0,
  // 62.5 kHz
  SDRKIT_LORA_SAT_BW_KHZ_62_5 = 1,
  // 125 kHz
  SDRKIT_LORA_SAT_BW_KHZ_125_KHZ = 2,
  // 250 kHz
  SDRKIT_LORA_SAT_BW_KHZ_250_KHZ = 3,
  // 500 kHz
  SDRKIT_LORA_SAT_BW_KHZ_500_KHZ = 4,
} sdrkit_lora_sat_bw_khz;

// The options of the setting "sync" of the stack "lora_sat".
typedef enum sdrkit_lora_sat_sync {
  // Try 0x12 and 0x34
  SDRKIT_LORA_SAT_SYNC_BOTH = 0,
  // 0x12
  SDRKIT_LORA_SAT_SYNC_0X12 = 1,
  // 0x34
  SDRKIT_LORA_SAT_SYNC_0X34 = 2,
  // Other
  SDRKIT_LORA_SAT_SYNC_OTHER = 3,
} sdrkit_lora_sat_sync;

// The options of the setting "gen_band" of the stack "lora_sat".
typedef enum sdrkit_lora_sat_gen_band {
  // 436.8 MHz, SF11
  SDRKIT_LORA_SAT_GEN_BAND_436_8 = 0,
  // 868.1 MHz, SF10
  SDRKIT_LORA_SAT_GEN_BAND_868_1 = 1,
  // 137.055 MHz, SF8, 41.7 kHz
  SDRKIT_LORA_SAT_GEN_BAND_137_055 = 2,
} sdrkit_lora_sat_gen_band;

// The options of the setting "gen_sf" of the stack "lora_sat".
typedef enum sdrkit_lora_sat_gen_sf {
  // The pass's
  SDRKIT_LORA_SAT_GEN_SF_THE_PASS_S = 0,
  // 7
  SDRKIT_LORA_SAT_GEN_SF_7 = 1,
  // 8
  SDRKIT_LORA_SAT_GEN_SF_8 = 2,
  // 9
  SDRKIT_LORA_SAT_GEN_SF_9 = 3,
  // 10
  SDRKIT_LORA_SAT_GEN_SF_10 = 4,
  // 11
  SDRKIT_LORA_SAT_GEN_SF_11 = 5,
  // 12
  SDRKIT_LORA_SAT_GEN_SF_12 = 6,
} sdrkit_lora_sat_gen_sf;

// The options of the setting "gen_bw_khz" of the stack "lora_sat".
typedef enum sdrkit_lora_sat_gen_bw_khz {
  // The pass's
  SDRKIT_LORA_SAT_GEN_BW_KHZ_THE_PASS_S = 0,
  // 41.7 kHz
  SDRKIT_LORA_SAT_GEN_BW_KHZ_41_7 = 1,
  // 62.5 kHz
  SDRKIT_LORA_SAT_GEN_BW_KHZ_62_5 = 2,
  // 125 kHz
  SDRKIT_LORA_SAT_GEN_BW_KHZ_125_KHZ = 3,
  // 250 kHz
  SDRKIT_LORA_SAT_GEN_BW_KHZ_250_KHZ = 4,
  // 500 kHz
  SDRKIT_LORA_SAT_GEN_BW_KHZ_500_KHZ = 5,
} sdrkit_lora_sat_gen_bw_khz;

// The options of the setting "decoders" of the stack "orbcomm".
typedef enum sdrkit_orbcomm_decoders {
  // 1
  SDRKIT_ORBCOMM_DECODERS_1 = 0,
  // 2
  SDRKIT_ORBCOMM_DECODERS_2 = 1,
} sdrkit_orbcomm_decoders;

// The options of the setting "order" of the stack "psk_demod".
typedef enum sdrkit_psk_demod_order {
  // Found by the analyser
  SDRKIT_PSK_DEMOD_ORDER_AUTO = 0,
  // BPSK
  SDRKIT_PSK_DEMOD_ORDER_BPSK = 1,
  // QPSK
  SDRKIT_PSK_DEMOD_ORDER_QPSK = 2,
  // OQPSK
  SDRKIT_PSK_DEMOD_ORDER_OQPSK = 3,
  // 8PSK
  SDRKIT_PSK_DEMOD_ORDER_8PSK = 4,
} sdrkit_psk_demod_order;

// The options of the setting "shape" of the stack "psk_demod".
typedef enum sdrkit_psk_demod_shape {
  // Found by the analyser
  SDRKIT_PSK_DEMOD_SHAPE_AUTO = 0,
  // Rectangular
  SDRKIT_PSK_DEMOD_SHAPE_RECT = 1,
  // RRC 0.2
  SDRKIT_PSK_DEMOD_SHAPE_0_2 = 2,
  // RRC 0.25
  SDRKIT_PSK_DEMOD_SHAPE_0_25 = 3,
  // RRC 0.35
  SDRKIT_PSK_DEMOD_SHAPE_0_35 = 4,
  // RRC 0.5
  SDRKIT_PSK_DEMOD_SHAPE_0_5 = 5,
  // RRC 0.8
  SDRKIT_PSK_DEMOD_SHAPE_0_8 = 6,
  // RRC 1.0
  SDRKIT_PSK_DEMOD_SHAPE_1_0 = 7,
} sdrkit_psk_demod_shape;

// The options of the setting "differential" of the stack "psk_demod".
typedef enum sdrkit_psk_demod_differential {
  // None
  SDRKIT_PSK_DEMOD_DIFFERENTIAL_NONE = 0,
  // Differential PSK
  SDRKIT_PSK_DEMOD_DIFFERENTIAL_DPSK = 1,
  // NRZ-M
  SDRKIT_PSK_DEMOD_DIFFERENTIAL_NRZM = 2,
  // NRZ-S
  SDRKIT_PSK_DEMOD_DIFFERENTIAL_NRZS = 3,
} sdrkit_psk_demod_differential;

// The options of the setting "descrambler" of the stack "psk_demod".
typedef enum sdrkit_psk_demod_descrambler {
  // None
  SDRKIT_PSK_DEMOD_DESCRAMBLER_NONE = 0,
  // G3RUH
  SDRKIT_PSK_DEMOD_DESCRAMBLER_G3RUH = 1,
  // CCSDS
  SDRKIT_PSK_DEMOD_DESCRAMBLER_CCSDS = 2,
} sdrkit_psk_demod_descrambler;

// The options of the setting "gen_order" of the stack "psk_demod".
typedef enum sdrkit_psk_demod_gen_order {
  // BPSK
  SDRKIT_PSK_DEMOD_GEN_ORDER_BPSK = 0,
  // QPSK
  SDRKIT_PSK_DEMOD_GEN_ORDER_QPSK = 1,
  // OQPSK
  SDRKIT_PSK_DEMOD_GEN_ORDER_OQPSK = 2,
  // 8PSK
  SDRKIT_PSK_DEMOD_GEN_ORDER_8PSK = 3,
} sdrkit_psk_demod_gen_order;

// The options of the setting "gen_coding" of the stack "psk_demod".
typedef enum sdrkit_psk_demod_gen_coding {
  // None
  SDRKIT_PSK_DEMOD_GEN_CODING_NONE = 0,
  // Differential PSK
  SDRKIT_PSK_DEMOD_GEN_CODING_DPSK = 1,
  // NRZ-M
  SDRKIT_PSK_DEMOD_GEN_CODING_NRZM = 2,
  // NRZ-S
  SDRKIT_PSK_DEMOD_GEN_CODING_NRZS = 3,
  // G3RUH
  SDRKIT_PSK_DEMOD_GEN_CODING_G3RUH = 4,
  // CCSDS
  SDRKIT_PSK_DEMOD_GEN_CODING_CCSDS = 5,
} sdrkit_psk_demod_gen_coding;

// The options of the setting "lo" of the stack "qo100".
typedef enum sdrkit_qo100_lo {
  // 9750 MHz
  SDRKIT_QO100_LO_9750_MHZ = 0,
  // 9739 MHz
  SDRKIT_QO100_LO_9739_MHZ = 1,
  // 10000 MHz
  SDRKIT_QO100_LO_10000_MHZ = 2,
  // 10600 MHz (inverted)
  SDRKIT_QO100_LO_10600_MHZ = 3,
  // Custom
  SDRKIT_QO100_LO_CUSTOM = 4,
} sdrkit_qo100_lo;

// The options of the setting "auto_listen" of the stack "qo100".
typedef enum sdrkit_qo100_auto_listen {
  // Off
  SDRKIT_QO100_AUTO_LISTEN_OFF = 0,
  // The loudest
  SDRKIT_QO100_AUTO_LISTEN_LOUDEST = 1,
  // The newest
  SDRKIT_QO100_AUTO_LISTEN_NEWEST = 2,
} sdrkit_qo100_auto_listen;

// The options of the setting "gen_lo" of the stack "qo100".
typedef enum sdrkit_qo100_gen_lo {
  // 9750 MHz
  SDRKIT_QO100_GEN_LO_9750_MHZ = 0,
  // 9739 MHz
  SDRKIT_QO100_GEN_LO_9739_MHZ = 1,
  // 10000 MHz
  SDRKIT_QO100_GEN_LO_10000_MHZ = 2,
  // 10600 MHz (inverted)
  SDRKIT_QO100_GEN_LO_10600_MHZ = 3,
  // Custom
  SDRKIT_QO100_GEN_LO_CUSTOM = 4,
} sdrkit_qo100_gen_lo;

// The options of the setting "search" of the stack "cubesat".
typedef enum sdrkit_cubesat_search {
  // Table (the dial)
  SDRKIT_CUBESAT_SEARCH_TABLE = 0,
  // Smart (blind search)
  SDRKIT_CUBESAT_SEARCH_SMART = 1,
} sdrkit_cubesat_search;

// The options of the setting "extra_ax25" of the stack "cubesat".
typedef enum sdrkit_cubesat_extra_ax25 {
  // Off
  SDRKIT_CUBESAT_EXTRA_AX25_OFF = 0,
  // G3RUH
  SDRKIT_CUBESAT_EXTRA_AX25_G3RUH = 1,
  // NRZI
  SDRKIT_CUBESAT_EXTRA_AX25_NRZI = 2,
} sdrkit_cubesat_extra_ax25;

// The options of the setting "afsk_bitrate" of the stack "cubesat".
typedef enum sdrkit_cubesat_afsk_bitrate {
  // 1200 bit/s
  SDRKIT_CUBESAT_AFSK_BITRATE_1200_BIT_S = 0,
  // 2400 bit/s
  SDRKIT_CUBESAT_AFSK_BITRATE_2400_BIT_S = 1,
  // 4800 bit/s
  SDRKIT_CUBESAT_AFSK_BITRATE_4800_BIT_S = 2,
} sdrkit_cubesat_afsk_bitrate;

// The options of the setting "geoscan_length" of the stack "cubesat".
typedef enum sdrkit_cubesat_geoscan_length {
  // 66 bytes
  SDRKIT_CUBESAT_GEOSCAN_LENGTH_66_BYTES = 0,
  // 74 bytes
  SDRKIT_CUBESAT_GEOSCAN_LENGTH_74_BYTES = 1,
} sdrkit_cubesat_geoscan_length;

// The options of the setting "ccsds_basis" of the stack "cubesat".
typedef enum sdrkit_cubesat_ccsds_basis {
  // Dual
  SDRKIT_CUBESAT_CCSDS_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_CUBESAT_CCSDS_BASIS_CONVENTIONAL = 1,
} sdrkit_cubesat_ccsds_basis;

// The options of the setting "concat.basis" of the stack "cubesat".
typedef enum sdrkit_cubesat_concat_basis {
  // Dual
  SDRKIT_CUBESAT_CONCAT_BASIS_DUAL = 0,
  // Conventional
  SDRKIT_CUBESAT_CONCAT_BASIS_CONVENTIONAL = 1,
} sdrkit_cubesat_concat_basis;

// The options of the setting "concat.code" of the stack "cubesat".
typedef enum sdrkit_cubesat_concat_code {
  // CCSDS
  SDRKIT_CUBESAT_CONCAT_CODE_CCSDS = 0,
  // NASA-DSN
  SDRKIT_CUBESAT_CONCAT_CODE_NASA_DSN = 1,
  // CCSDS uninverted
  SDRKIT_CUBESAT_CONCAT_CODE_CCSDS_UNINVERTED = 2,
  // NASA-DSN uninverted
  SDRKIT_CUBESAT_CONCAT_CODE_NASA_DSN_UNINVERTED = 3,
} sdrkit_cubesat_concat_code;

// The options of the setting "mobitex_variant" of the stack "cubesat".
typedef enum sdrkit_cubesat_mobitex_variant {
  // Mobitex / Mobitex-NX
  SDRKIT_CUBESAT_MOBITEX_VARIANT_MOBITEX_NX = 0,
  // BEESAT-1
  SDRKIT_CUBESAT_MOBITEX_VARIANT_BEESAT1 = 1,
  // BEESAT-9
  SDRKIT_CUBESAT_MOBITEX_VARIANT_BEESAT9 = 2,
} sdrkit_cubesat_mobitex_variant;

// The options of the setting "framing_filter" of the stack "cubesat".
typedef enum sdrkit_cubesat_framing_filter {
  // Every framing
  SDRKIT_CUBESAT_FRAMING_FILTER_ALL = 0,
  // AX.25 AFSK
  SDRKIT_CUBESAT_FRAMING_FILTER_AX25_AFSK = 1,
  // AX.25 G3RUH
  SDRKIT_CUBESAT_FRAMING_FILTER_AX25_G3RUH = 2,
  // AX.25 NRZI
  SDRKIT_CUBESAT_FRAMING_FILTER_AX25_NRZI = 3,
  // AX100 Mode 5
  SDRKIT_CUBESAT_FRAMING_FILTER_AX100_MODE5 = 4,
  // AX100 Mode 6
  SDRKIT_CUBESAT_FRAMING_FILTER_AX100_MODE6 = 5,
  // CCSDS RS
  SDRKIT_CUBESAT_FRAMING_FILTER_CCSDS_RS = 6,
  // Geoscan
  SDRKIT_CUBESAT_FRAMING_FILTER_GEOSCAN = 7,
  // Lucky-7
  SDRKIT_CUBESAT_FRAMING_FILTER_LUCKY7 = 8,
  // Spino
  SDRKIT_CUBESAT_FRAMING_FILTER_SPINO = 9,
  // Erminaz
  SDRKIT_CUBESAT_FRAMING_FILTER_ERMINAZ = 10,
  // USP
  SDRKIT_CUBESAT_FRAMING_FILTER_USP = 11,
  // NGHam
  SDRKIT_CUBESAT_FRAMING_FILTER_NGHAM = 12,
  // U482C
  SDRKIT_CUBESAT_FRAMING_FILTER_U482C = 13,
  // Mobitex
  SDRKIT_CUBESAT_FRAMING_FILTER_MOBITEX = 14,
  // CCSDS concatenated
  SDRKIT_CUBESAT_FRAMING_FILTER_CCSDS_CONCAT = 15,
} sdrkit_cubesat_framing_filter;

// The options of the setting "tx_rate" of the stack "cubesat".
typedef enum sdrkit_cubesat_tx_rate {
  // 1200 bit/s AFSK
  SDRKIT_CUBESAT_TX_RATE_1200_BIT_S_AFSK = 0,
  // 9600 bit/s G3RUH
  SDRKIT_CUBESAT_TX_RATE_9600_BIT_S_G3RUH = 1,
} sdrkit_cubesat_tx_rate;

// The options of the setting "gen_framing" of the stack "cubesat".
typedef enum sdrkit_cubesat_gen_framing {
  // Every framing in turn
  SDRKIT_CUBESAT_GEN_FRAMING_EVERY_FRAMING_IN_TURN = 0,
  // AX.25 AFSK 1200
  SDRKIT_CUBESAT_GEN_FRAMING_AX_25_AFSK_1200 = 1,
  // AX.25 G3RUH 9600
  SDRKIT_CUBESAT_GEN_FRAMING_AX_25_G3RUH_9600 = 2,
  // AX100 Mode 5, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_AX100_MODE_5_9600 = 3,
  // AX100 Mode 5, 4800
  SDRKIT_CUBESAT_GEN_FRAMING_AX100_MODE_5_4800 = 4,
  // AX100 Mode 6, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_AX100_MODE_6_9600 = 5,
  // CCSDS RS, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_CCSDS_RS_9600 = 6,
  // Geoscan, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_GEOSCAN_9600 = 7,
  // Lucky-7, 4800
  SDRKIT_CUBESAT_GEN_FRAMING_LUCKY_7_4800 = 8,
  // Spino, 2400
  SDRKIT_CUBESAT_GEN_FRAMING_SPINO_2400 = 9,
  // Erminaz, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_ERMINAZ_9600 = 10,
  // USP, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_USP_9600 = 11,
  // NGHam, 4800
  SDRKIT_CUBESAT_GEN_FRAMING_NGHAM_4800 = 12,
  // U482C, AFSK 4800
  SDRKIT_CUBESAT_GEN_FRAMING_U482C_AFSK_4800 = 13,
  // Mobitex-NX, 4800
  SDRKIT_CUBESAT_GEN_FRAMING_MOBITEX_NX_4800 = 14,
  // CCSDS concatenated, 9600
  SDRKIT_CUBESAT_GEN_FRAMING_CCSDS_CONCATENATED_9600 = 15,
} sdrkit_cubesat_gen_framing;

// The options of the setting "gen_scenario" of the stack "fleet_broadcast".
typedef enum sdrkit_fleet_broadcast_gen_scenario {
  // Sync, reversals, idle and random channels
  SDRKIT_FLEET_BROADCAST_GEN_SCENARIO_SYNC_REVERSALS_IDLE_AND_RANDOM_CHANNELS = 0,
  // Sync, idle channels
  SDRKIT_FLEET_BROADCAST_GEN_SCENARIO_SYNC_IDLE_CHANNELS = 1,
  // Noise only
  SDRKIT_FLEET_BROADCAST_GEN_SCENARIO_NOISE_ONLY = 2,
  // A plain carrier
  SDRKIT_FLEET_BROADCAST_GEN_SCENARIO_A_PLAIN_CARRIER = 3,
  // Random bits everywhere
  SDRKIT_FLEET_BROADCAST_GEN_SCENARIO_RANDOM_BITS_EVERYWHERE = 4,
} sdrkit_fleet_broadcast_gen_scenario;

// The options of the setting "bearer" of the stack "bgan".
typedef enum sdrkit_bgan_bearer {
  // Any bearer
  SDRKIT_BGAN_BEARER_AUTO = 0,
  // F80T0.25Q-1B
  SDRKIT_BGAN_BEARER_F80T0_25Q_1B = 1,
  // F80T1Q-1B
  SDRKIT_BGAN_BEARER_F80T1Q_1B = 2,
  // F80T1Q-4B
  SDRKIT_BGAN_BEARER_F80T1Q_4B = 3,
  // F80T1X-4B
  SDRKIT_BGAN_BEARER_F80T1X_4B = 4,
  // FR80T2.5X4/16-5B
  SDRKIT_BGAN_BEARER_FR80T2_5X4_16_5B = 5,
  // FR80T2.5X16-5B
  SDRKIT_BGAN_BEARER_FR80T2_5X16_5B = 6,
  // FR80T2.5X32-6B
  SDRKIT_BGAN_BEARER_FR80T2_5X32_6B = 7,
  // FR80T2.5X64-7B
  SDRKIT_BGAN_BEARER_FR80T2_5X64_7B = 8,
  // F80T4.5X-8B
  SDRKIT_BGAN_BEARER_F80T4_5X_8B = 9,
  // FR80T5X4/16-9B
  SDRKIT_BGAN_BEARER_FR80T5X4_16_9B = 10,
  // FR80T5X16-9B
  SDRKIT_BGAN_BEARER_FR80T5X16_9B = 11,
  // FR80T5X32-11B
  SDRKIT_BGAN_BEARER_FR80T5X32_11B = 12,
  // FR80T5X64-13B
  SDRKIT_BGAN_BEARER_FR80T5X64_13B = 13,
} sdrkit_bgan_bearer;

// The options of the setting "uw_order" of the stack "bgan".
typedef enum sdrkit_bgan_uw_order {
  // First symbol is the most significant bit
  SDRKIT_BGAN_UW_ORDER_MSB_FIRST = 0,
  // First symbol is the least significant bit
  SDRKIT_BGAN_UW_ORDER_LSB_FIRST = 1,
} sdrkit_bgan_uw_order;

// The options of the setting "iq_labels" of the stack "bgan".
typedef enum sdrkit_bgan_iq_labels {
  // In column order
  SDRKIT_BGAN_IQ_LABELS_IN_ORDER = 0,
  // Swapped within each pair
  SDRKIT_BGAN_IQ_LABELS_SWAPPED = 1,
} sdrkit_bgan_iq_labels;

// The options of the setting "gen_bearer" of the stack "bgan".
typedef enum sdrkit_bgan_gen_bearer {
  // A group of carriers
  SDRKIT_BGAN_GEN_BEARER_A_GROUP_OF_CARRIERS = 0,
  // F80T0.25Q-1B
  SDRKIT_BGAN_GEN_BEARER_F80T0_25Q_1B = 1,
  // F80T1Q-1B
  SDRKIT_BGAN_GEN_BEARER_F80T1Q_1B = 2,
  // F80T1Q-4B
  SDRKIT_BGAN_GEN_BEARER_F80T1Q_4B = 3,
  // F80T1X-4B
  SDRKIT_BGAN_GEN_BEARER_F80T1X_4B = 4,
  // FR80T2.5X4/16-5B
  SDRKIT_BGAN_GEN_BEARER_FR80T2_5X4_16_5B = 5,
  // FR80T2.5X16-5B
  SDRKIT_BGAN_GEN_BEARER_FR80T2_5X16_5B = 6,
  // FR80T2.5X32-6B
  SDRKIT_BGAN_GEN_BEARER_FR80T2_5X32_6B = 7,
  // FR80T2.5X64-7B
  SDRKIT_BGAN_GEN_BEARER_FR80T2_5X64_7B = 8,
  // F80T4.5X-8B
  SDRKIT_BGAN_GEN_BEARER_F80T4_5X_8B = 9,
  // FR80T5X4/16-9B
  SDRKIT_BGAN_GEN_BEARER_FR80T5X4_16_9B = 10,
  // FR80T5X16-9B
  SDRKIT_BGAN_GEN_BEARER_FR80T5X16_9B = 11,
  // FR80T5X32-11B
  SDRKIT_BGAN_GEN_BEARER_FR80T5X32_11B = 12,
  // FR80T5X64-13B
  SDRKIT_BGAN_GEN_BEARER_FR80T5X64_13B = 13,
} sdrkit_bgan_gen_bearer;

// The options of the setting "gen_subtype" of the stack "bgan".
typedef enum sdrkit_bgan_gen_subtype {
  // L8
  SDRKIT_BGAN_GEN_SUBTYPE_L8 = 0,
  // L7
  SDRKIT_BGAN_GEN_SUBTYPE_L7 = 1,
  // L6
  SDRKIT_BGAN_GEN_SUBTYPE_L6 = 2,
  // L5
  SDRKIT_BGAN_GEN_SUBTYPE_L5 = 3,
  // L4
  SDRKIT_BGAN_GEN_SUBTYPE_L4 = 4,
  // L3
  SDRKIT_BGAN_GEN_SUBTYPE_L3 = 5,
  // L2
  SDRKIT_BGAN_GEN_SUBTYPE_L2 = 6,
  // L1
  SDRKIT_BGAN_GEN_SUBTYPE_L1 = 7,
  // R
  SDRKIT_BGAN_GEN_SUBTYPE_R = 8,
  // H1
  SDRKIT_BGAN_GEN_SUBTYPE_H1 = 9,
  // H2
  SDRKIT_BGAN_GEN_SUBTYPE_H2 = 10,
  // H3
  SDRKIT_BGAN_GEN_SUBTYPE_H3 = 11,
  // H4
  SDRKIT_BGAN_GEN_SUBTYPE_H4 = 12,
  // H5
  SDRKIT_BGAN_GEN_SUBTYPE_H5 = 13,
  // H6
  SDRKIT_BGAN_GEN_SUBTYPE_H6 = 14,
} sdrkit_bgan_gen_subtype;

// The result fields of Inmarsat-C.
typedef struct sdrkit_inmarsatc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The offset of the carrier of the first channel from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // The drift. Unit: Hz/s.
  double drift_hz_s;
  // The frames found.
  int64_t frames_found;
  // The packets good.
  int64_t packets_ok;
  // The packets bad.
  int64_t packets_bad;
  // The messages seen.
  int64_t message_count;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // The es/N0. Unit: dB.
  double esn0_db;
  // True if `ebn0_db` holds a value.
  bool has_ebn0_db;
  // The eb/N0. Unit: dB.
  double ebn0_db;
  // True if `uw_errors` holds a value.
  bool has_uw_errors;
  // The unique word errors.
  int64_t uw_errors;
  // True if `uw_errors_avg` holds a value.
  bool has_uw_errors_avg;
  // The unique word errors (average).
  double uw_errors_avg;
  // True if `symbol_error_rate` holds a value.
  bool has_symbol_error_rate;
  // The symbol error rate.
  double symbol_error_rate;
  // The polarity slips.
  int64_t polarity_slips;
  // True if `ocean_region_code` holds a value.
  bool has_ocean_region_code;
  // The ocean region code.
  int64_t ocean_region_code;
  // True if `region` holds a value.
  bool has_region;
  // The ocean region.
  sdrkit_string region;
  // True if `station_id` holds a value.
  bool has_station_id;
  // The station id.
  int64_t station_id;
  // True if `station_name` holds a value.
  bool has_station_name;
  // The station.
  sdrkit_string station_name;
  // True if `channel_type_code` holds a value.
  bool has_channel_type_code;
  // The channel type code.
  int64_t channel_type_code;
  // True if `channel_type` holds a value.
  bool has_channel_type;
  // The channel type.
  sdrkit_string channel_type;
  // True if `network_version` holds a value.
  bool has_network_version;
  // The network version.
  int64_t network_version;
  // True if `signalling_channel` holds a value.
  bool has_signalling_channel;
  // The signalling channel.
  int64_t signalling_channel;
  // True if `random_interval` holds a value.
  bool has_random_interval;
  // The random interval.
  int64_t random_interval;
  // True if `status_byte` holds a value.
  bool has_status_byte;
  // The status byte.
  int64_t status_byte;
  // True if `status_text` holds a value.
  bool has_status_text;
  // The status.
  sdrkit_string status_text;
  // True if `services_mask` holds a value.
  bool has_services_mask;
  // The services mask.
  int64_t services_mask;
  // True if `services_text` holds a value.
  bool has_services_text;
  // The services.
  sdrkit_string services_text;
  // True if `signalling_uplink_hz` holds a value.
  bool has_signalling_uplink_hz;
  // The signalling uplink. A frequency in Hz.
  double signalling_uplink_hz;
  // True if `frame_number` holds a value.
  bool has_frame_number;
  // The frame number.
  int64_t frame_number;
  // True if `frame_time` holds a value.
  bool has_frame_time;
  // The frame time of day. A length of time in seconds.
  double frame_time;
} sdrkit_inmarsatc_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_satcom_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_satcom_decoder_fields;

// An EGC message. The row key is the message number and the service.
typedef struct sdrkit_inmarsatc_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The first heard. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // The channel a message came on (0 the main one).
  int64_t channel;
  // The message number.
  int64_t message_number;
  // The EGC service code.
  int64_t service_code;
  // The service.
  sdrkit_string service;
  // The kind.
  sdrkit_string kind;
  // The priority.
  sdrkit_string priority;
  // The presentation.
  sdrkit_string presentation;
  // The packets of the message heard.
  int64_t packets;
  // Every packet of the message was heard.
  bool complete;
  // The heard complete.
  int64_t times_heard;
  // The area of a message.
  sdrkit_string area;
  // The subject of a message.
  sdrkit_string subject;
  // The warning.
  sdrkit_string warning_number;
  // The text of the message.
  sdrkit_string text;
  // The text of the message is cut short.
  bool truncated;
} sdrkit_inmarsatc_message_row;

// The options of `state` of `sdrkit_inmarsatc_channel_row`.
typedef enum sdrkit_inmarsatc_channel_row_state {
  // Searching
  SDRKIT_INMARSATC_CHANNEL_ROW_STATE_SEARCHING = 0,
  // Carrier
  SDRKIT_INMARSATC_CHANNEL_ROW_STATE_CARRIER = 1,
  // Frames
  SDRKIT_INMARSATC_CHANNEL_ROW_STATE_FRAMES = 2,
} sdrkit_inmarsatc_channel_row_state;

// A channel followed now. The row key is the channel index.
typedef struct sdrkit_inmarsatc_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel index.
  int64_t index;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // A carrier was found.
  bool carrier;
  // The state. A value of `sdrkit_inmarsatc_channel_row_state`.
  int32_t state;
  // The eb/N0. Unit: dB.
  double ebn0_db;
  // The unique word errors (average).
  double uw_errors_avg;
  // The frame number.
  int64_t frame_number;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The messages.
  int64_t messages;
  // The ocean region.
  sdrkit_string region;
  // The station.
  sdrkit_string station_name;
  // The channel type.
  sdrkit_string channel_type;
} sdrkit_inmarsatc_channel_row;

// The packets of one descriptor type. The row key is the descriptor in hexadecimal ("2A").
typedef struct sdrkit_inmarsatc_packet_count_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The descriptor.
  int64_t descriptor;
  // The packet type.
  sdrkit_string descriptor_name;
  // The packets good.
  int64_t packets_ok;
  // The packets bad.
  int64_t packets_bad;
} sdrkit_inmarsatc_packet_count_row;

// The result fields of the Aero display.
typedef struct sdrkit_aero_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The signal units with a good check.
  int64_t frames_ok;
  // The signal units with a bad check.
  int64_t frames_bad;
  // The frames read.
  int64_t frames;
  // The messages in the log.
  int64_t messages;
  // The positions heard.
  int64_t positions;
  // The aircraft in the table.
  int64_t aircraft;
  // The channels that decode frames.
  int64_t channels_decoding;
  // True if `ebn0_db` holds a value.
  bool has_ebn0_db;
  // The best Eb/N0 of the channels that decode. Absent while none decodes. Unit: dB.
  double ebn0_db;
} sdrkit_aero_fields;

// The options of `state` of `sdrkit_aero_channel_row`.
typedef enum sdrkit_aero_channel_row_state {
  // Seen
  SDRKIT_AERO_CHANNEL_ROW_STATE_SEEN = 0,
  // Carrier
  SDRKIT_AERO_CHANNEL_ROW_STATE_CARRIER = 1,
  // Frames
  SDRKIT_AERO_CHANNEL_ROW_STATE_FRAMES = 2,
  // Data
  SDRKIT_AERO_CHANNEL_ROW_STATE_DATA = 3,
} sdrkit_aero_channel_row_state;

// A row of the channel table of the Aero stacks. The row key is the place of the channel.
typedef struct sdrkit_aero_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The id of the channel.
  int64_t id;
  // The offset from the centre of the input. A frequency in Hz.
  double offset_hz;
  // The frequency of the channel. A frequency in Hz.
  double frequency;
  // The bit rate. Unit: bit/s.
  int64_t rate;
  // How far the channel decodes. A value of `sdrkit_aero_channel_row_state`.
  int32_t state;
  // The energy per bit over the noise density. Unit: dB.
  double ebn0_db;
  // The level of the carrier. Unit: dB.
  double level_db;
  // The frames read.
  int64_t frames;
  // The frames whose unique word was missed.
  int64_t uw_misses;
  // The signal units with a good check.
  int64_t sus_good;
  // The signal units with a bad check.
  int64_t sus_bad;
  // The bit error rate of the channel.
  double ber;
} sdrkit_aero_channel_row;

// The count of one SU type. The row key is the type in two hexadecimal digits (FE for SUs that failed their check).
typedef struct sdrkit_aero_su_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The SU type.
  int64_t su_type;
  // The name of the type.
  sdrkit_string su_name;
  // The SUs of the type.
  int64_t count;
} sdrkit_aero_su_type_row;

// A log-on or log-off. The row key is the number of the entry in the log.
typedef struct sdrkit_aero_logon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double at;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time, when known. Seconds since 1970 (UTC).
  double time;
  // The id of the channel.
  int64_t channel;
  // The AES id, six hexadecimal digits.
  sdrkit_string aes;
  // The GES id, two hexadecimal digits.
  sdrkit_string ges;
  // A log-on (else a log-off).
  bool logon;
} sdrkit_aero_logon_row;

// An ACARS message. The row key is the place of the message in the log.
typedef struct sdrkit_aero_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double at;
  // True if `time` holds a value.
  bool has_time;
  // The wall-clock time, when known. Seconds since 1970 (UTC).
  double time;
  // The id of the channel.
  int64_t channel;
  // The bit rate of the channel. Unit: bit/s.
  int64_t rate;
  // The frequency of the channel. A frequency in Hz.
  double frequency;
  // The AES id, six hexadecimal digits.
  sdrkit_string aes;
  // The GES id, two hexadecimal digits.
  sdrkit_string ges;
  // The registration.
  sdrkit_string registration;
  // The flight id.
  sdrkit_string flight;
  // The ACARS label.
  sdrkit_string label;
  // What the label means.
  sdrkit_string label_name;
  // The message sequence number.
  sdrkit_string message_number;
  // The text.
  sdrkit_string text;
  // The block check is good.
  bool check_ok;
  // Every block of the message was heard.
  bool complete;
  // The ADS-C report in words. Empty for a message that is not ADS-C.
  sdrkit_string adsc;
  // True if `position` holds a value.
  bool has_position;
  // The position in the message.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude in the message. Unit: ft.
  double alt_ft;
  // True if `reading` holds a value.
  bool has_reading;
  // The reading of the fixed format.
  sdrkit_string reading;
  // True if `reading_kind` holds a value.
  bool has_reading_kind;
  // The kind of the reading.
  sdrkit_string reading_kind;
  // True if `reading_category` holds a value.
  bool has_reading_category;
  // The category of the reading.
  sdrkit_string reading_category;
  // True if `cpdlc_imi` holds a value.
  bool has_cpdlc_imi;
  // The CPDLC envelope: AT1, CR1, CC1 or DR1.
  sdrkit_string cpdlc_imi;
  // True if `cpdlc_ground` holds a value.
  bool has_cpdlc_ground;
  // The CPDLC ground facility.
  sdrkit_string cpdlc_ground;
  // True if `cpdlc_aircraft` holds a value.
  bool has_cpdlc_aircraft;
  // The aircraft named in the CPDLC envelope.
  sdrkit_string cpdlc_aircraft;
  // True if `cpdlc_crc_ok` holds a value.
  bool has_cpdlc_crc_ok;
  // The CPDLC envelope check passed.
  bool cpdlc_crc_ok;
  // True if `cpdlc_parsed` holds a value.
  bool has_cpdlc_parsed;
  // The CPDLC message body decoded completely.
  bool cpdlc_parsed;
  // True if `cpdlc_text` holds a value.
  bool has_cpdlc_text;
  // The decoded CPDLC message.
  sdrkit_string cpdlc_text;
} sdrkit_aero_message_row;

// An aircraft heard: the standard aircraft columns, then the frequency last heard on. The row key is the address, else the registration.
typedef struct sdrkit_aero_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `address` holds a value.
  bool has_address;
  // The 24-bit address, six hex digits.
  sdrkit_string address;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration.
  sdrkit_string registration;
  // True if `country` holds a value.
  bool has_country;
  // The State the address is allocated to.
  sdrkit_string country;
  // True if `flight` holds a value.
  bool has_flight;
  // The flight id.
  sdrkit_string flight;
  // True if `label` holds a value.
  bool has_label;
  // The label of the last message.
  sdrkit_string label;
  // Messages heard.
  int64_t messages;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The altitude of the last position. Unit: ft.
  double alt_ft;
  // True if `track_deg` holds a value.
  bool has_track_deg;
  // The track, degrees true. Unit: deg.
  double track_deg;
  // True if `gs_kt` holds a value.
  bool has_gs_kt;
  // The ground speed. Unit: kt.
  double gs_kt;
  // True if `vs_fpm` holds a value.
  bool has_vs_fpm;
  // The vertical rate. Unit: ft/min.
  double vs_fpm;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard. Seconds since 1970 (UTC).
  double last_heard;
  // The frequency last heard on. A frequency in Hz.
  double frequency;
} sdrkit_aero_aircraft_row;

// A system fact (a satellite, the control of the P and R channels). The row key is the key of the fact.
typedef struct sdrkit_aero_fact_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The kind of the fact.
  sdrkit_string kind;
  // The fact in words.
  sdrkit_string text;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_aero_fact_row;

// The result fields of the merged Inmarsat display.
typedef struct sdrkit_inmarsat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The ACARS messages and the Inmarsat-C bulletin frames.
  int64_t frames_ok;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The satellite that the identification names. Absent while it names none.
  sdrkit_string satellite;
  // True if `satellite_confidence` holds a value.
  bool has_satellite_confidence;
  // How sure the identification is, 0 to 1.
  double satellite_confidence;
  // Two satellites fit about as well.
  bool satellite_ambiguous;
  // The beam, in words.
  sdrkit_string beam;
  // A note on the identification.
  sdrkit_string identification_note;
  // The ACARS messages.
  int64_t messages;
  // The ACARS messages with a position.
  int64_t positions;
  // The Inmarsat-C frames with a bulletin board.
  int64_t egc_frames;
  // The carriers found.
  int64_t carriers;
  // The carriers that decode.
  int64_t decoding;
} sdrkit_inmarsat_fields;

// The options of `class_` of `sdrkit_inmarsat_carrier_row`.
typedef enum sdrkit_inmarsat_carrier_row_class_ {
  // Narrow
  SDRKIT_INMARSAT_CARRIER_ROW_CLASS__NARROW = 0,
  // Medium
  SDRKIT_INMARSAT_CARRIER_ROW_CLASS__MEDIUM = 1,
  // Wide
  SDRKIT_INMARSAT_CARRIER_ROW_CLASS__WIDE = 2,
} sdrkit_inmarsat_carrier_row_class_;

// The options of `decodes` of `sdrkit_inmarsat_carrier_row`.
typedef enum sdrkit_inmarsat_carrier_row_decodes {
  // Unknown
  SDRKIT_INMARSAT_CARRIER_ROW_DECODES_UNKNOWN = 0,
  // Aero P
  SDRKIT_INMARSAT_CARRIER_ROW_DECODES_AERO_P = 1,
  // Inmarsat-C
  SDRKIT_INMARSAT_CARRIER_ROW_DECODES_INMARSAT_C = 2,
} sdrkit_inmarsat_carrier_row_decodes;

// A carrier found. The row key is the carrier id in five digits.
typedef struct sdrkit_inmarsat_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The id of the carrier.
  int64_t id;
  // The offset from the centre of the input. A frequency in Hz.
  double offset_hz;
  // The frequency of the carrier. A frequency in Hz.
  double frequency;
  // The width of the carrier. A frequency in Hz.
  double width;
  // The level over the floor. Unit: dB.
  double snr_db;
  // The class by the width. A value of `sdrkit_inmarsat_carrier_row_class_`.
  int32_t class_;
  // What decodes the carrier. A value of `sdrkit_inmarsat_carrier_row_decodes`.
  int32_t decodes;
  // The bit rate. Unit: bit/s.
  int64_t rate;
  // How far the carrier decodes: 0 seen, 1 carrier, 2 frames, 3 data.
  int64_t state;
  // The energy per bit over the noise density. Unit: dB.
  double ebn0_db;
  // The frames read.
  int64_t frames;
  // What the carrier is on the identified satellite.
  sdrkit_string satellite;
  // The window of the carrier is parked.
  bool parked;
  // The carrier has the focus.
  bool focused;
  // The window that holds the carrier.
  int64_t window;
  // The carrier has the width of a voice channel.
  bool voice_candidate;
} sdrkit_inmarsat_carrier_row;

// A satellite that the identification weighed. The row key is the name.
typedef struct sdrkit_inmarsat_candidate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the satellite.
  sdrkit_string name;
  // How sure the identification is of this satellite, 0 to 1.
  double satellite_confidence;
  // True if `content` holds a value.
  bool has_content;
  // The score of the decoded content.
  double content;
  // True if `frequency_score` holds a value.
  bool has_frequency_score;
  // The score of the frequencies.
  double frequency_score;
  // True if `geometry` holds a value.
  bool has_geometry;
  // The score of the geometry.
  double geometry;
  // True if `elevation_deg` holds a value.
  bool has_elevation_deg;
  // The elevation from the station. Unit: deg.
  double elevation_deg;
  // True if `azimuth_deg` holds a value.
  bool has_azimuth_deg;
  // The azimuth from the station. Unit: deg.
  double azimuth_deg;
} sdrkit_inmarsat_candidate_row;

// A reason of a candidate. The row key is `<candidate>/<n>`.
typedef struct sdrkit_inmarsat_candidate_reason_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The candidate.
  sdrkit_string candidate;
  // The reason in words.
  sdrkit_string why;
} sdrkit_inmarsat_candidate_reason_row;

// The result fields of Iridium.
typedef struct sdrkit_iridium_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts the detector found.
  int64_t bursts;
  // Bursts demodulated.
  int64_t demodulated;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Input items dropped because a helper thread did not keep up.
  int64_t dropped;
  // Bursts found twice and counted once.
  int64_t duplicates;
  // Bursts with the downlink unique word.
  int64_t downlink;
  // Bursts with the uplink unique word.
  int64_t uplink;
  // Bursts found per second (the last report interval). A frequency in Hz.
  double burst_rate;
  // Unique words found per second (the last report interval). A frequency in Hz.
  double uw_rate;
  // True if `confidence_pct` holds a value.
  bool has_confidence_pct;
  // The mean confidence of the bursts in the last interval. Unit: %.
  double confidence_pct;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // The mean Es/N0 of the bursts in the last interval. Unit: dB.
  double esn0_db;
  // True if `noise_db_hz` holds a value.
  bool has_noise_db_hz;
  // The detector's noise floor, dB relative to full scale in 1 Hz. Unit: dBFS/Hz.
  double noise_db_hz;
} sdrkit_iridium_fields;

// The options of `direction` of `sdrkit_iridium_raw_row`.
typedef enum sdrkit_iridium_raw_row_direction {
  // Downlink
  SDRKIT_IRIDIUM_RAW_ROW_DIRECTION_DOWNLINK = 0,
  // Uplink
  SDRKIT_IRIDIUM_RAW_ROW_DIRECTION_UPLINK = 1,
} sdrkit_iridium_raw_row_direction;

// A RAW line. The row key is the sequence number of the line.
typedef struct sdrkit_iridium_raw_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // At (signal time). A length of time in seconds.
  double at;
  // Frequency. A frequency in Hz.
  double frequency;
  // Direction. A value of `sdrkit_iridium_raw_row_direction`.
  int32_t direction;
  // Confidence. Unit: %.
  double confidence_pct;
  // RAW line.
  sdrkit_string line;
} sdrkit_iridium_raw_row;

// The result fields of Globalstar.
typedef struct sdrkit_globalstar_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Channels active now.
  int64_t active_channels;
  // Stretches of activity completed since the start (or the last Clear).
  int64_t activities;
  // True if `doppler` holds a value.
  bool has_doppler;
  // The mean Doppler of the active channels. A frequency in Hz.
  double doppler;
} sdrkit_globalstar_fields;

// A row of the 13 channels. The row key is the channel number ("07").
typedef struct sdrkit_globalstar_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A channel number, 1 to 13.
  int64_t channel;
  // The centre of the channel. A frequency in Hz.
  double centre;
  // The channel lies inside the input.
  bool in_view;
  // The channel is active now.
  bool active;
  // The power of the channel. Unit: dBFS.
  double power_dbfs;
  // The noise floor of the channel. Unit: dBFS.
  double floor_dbfs;
  // The signal to noise ratio of the channel. Unit: dB.
  double snr_db;
  // The share of time the channel was on (60 s average). Unit: %.
  double occupancy_pct;
  // The Doppler shift of the channel. A frequency in Hz.
  double doppler;
  // True if `last_active` holds a value.
  bool has_last_active;
  // The signal time when the channel was last active. A length of time in seconds.
  double last_active;
  // The stretches of activity on the channel.
  int64_t events;
} sdrkit_globalstar_channel_row;

// A stretch of activity on a channel. The row key is the sequence number of the stretch.
typedef struct sdrkit_globalstar_activity_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the start. A length of time in seconds.
  double start;
  // How long a stretch of activity lasted. A length of time in seconds.
  double duration;
  // A channel number, 1 to 13.
  int64_t channel;
  // The peak SNR. Unit: dB.
  double peak_snr_db;
  // The Doppler shift at the start. A frequency in Hz.
  double doppler_start;
  // The Doppler shift at the end. A frequency in Hz.
  double doppler_end;
  // True if `wall_start` holds a value.
  bool has_wall_start;
  // The wall-clock time of the start, when the samples are live. Seconds since 1970 (UTC).
  double wall_start;
} sdrkit_globalstar_activity_row;

// The result fields of GOES DCS.
typedef struct sdrkit_goes_dcs_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The reports read whole.
  int64_t frames_ok;
  // The reports cut off or lost.
  int64_t frames_bad;
  // The carriers followed now.
  int64_t carriers;
} sdrkit_goes_dcs_fields;

// The options of `format` of `sdrkit_goes_dcs_report_row`.
typedef enum sdrkit_goes_dcs_report_row_format {
  // ASCII
  SDRKIT_GOES_DCS_REPORT_ROW_FORMAT_ASCII = 0,
  // Pseudo-binary B
  SDRKIT_GOES_DCS_REPORT_ROW_FORMAT_PSEUDO_BINARY = 1,
  // Other
  SDRKIT_GOES_DCS_REPORT_ROW_FORMAT_OTHER = 2,
} sdrkit_goes_dcs_report_row_format;

// A platform report. The row key is the sequence number of the report.
typedef struct sdrkit_goes_dcs_report_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a report. A length of time in seconds.
  double at;
  // True if `heard_at` holds a value.
  bool has_heard_at;
  // The wall-clock time of a report (live radios only). Seconds since 1970 (UTC).
  double heard_at;
  // The address word (8 hex digits).
  sdrkit_string address;
  // The bit rate. A frequency in Hz.
  double bit_rate_hz;
  // Manchester coded.
  bool manchester;
  // The channel number.
  int64_t channel;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The flag byte, or -1.
  int64_t flags;
  // The data format. A value of `sdrkit_goes_dcs_report_row_format`.
  int32_t format;
  // The report ended with an EOT.
  bool eot;
  // Characters whose parity failed.
  int64_t parity_errors;
  // The data of the report.
  sdrkit_string text;
} sdrkit_goes_dcs_report_row;

// The options of `format` of `sdrkit_goes_dcs_platform_row`.
typedef enum sdrkit_goes_dcs_platform_row_format {
  // ASCII
  SDRKIT_GOES_DCS_PLATFORM_ROW_FORMAT_ASCII = 0,
  // Pseudo-binary B
  SDRKIT_GOES_DCS_PLATFORM_ROW_FORMAT_PSEUDO_BINARY = 1,
  // Other
  SDRKIT_GOES_DCS_PLATFORM_ROW_FORMAT_OTHER = 2,
} sdrkit_goes_dcs_platform_row_format;

// A platform heard. The row key is the address.
typedef struct sdrkit_goes_dcs_platform_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address word (8 hex digits).
  sdrkit_string address;
  // The channel number.
  int64_t channel;
  // The bit rate. A frequency in Hz.
  double bit_rate_hz;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The data format. A value of `sdrkit_goes_dcs_platform_row_format`.
  int32_t format;
  // The data of the report.
  sdrkit_string text;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // How many times it was heard.
  int64_t heard;
} sdrkit_goes_dcs_platform_row;

// A carrier followed now. The row key is its place in the list ("000").
typedef struct sdrkit_goes_dcs_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // The channel number.
  int64_t channel;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // True if `bit_rate_hz` holds a value.
  bool has_bit_rate_hz;
  // The bit rate. A frequency in Hz.
  double bit_rate_hz;
} sdrkit_goes_dcs_carrier_row;

// The options of `link` of `sdrkit_argos_fields`.
typedef enum sdrkit_argos_fields_link {
  // Automatic (from the dial)
  SDRKIT_ARGOS_FIELDS_LINK_AUTO = 0,
  // Uplink
  SDRKIT_ARGOS_FIELDS_LINK_UPLINK = 1,
  // Downlink
  SDRKIT_ARGOS_FIELDS_LINK_DOWNLINK = 2,
} sdrkit_argos_fields_link;

// The options of `coding` of `sdrkit_argos_fields`.
typedef enum sdrkit_argos_fields_coding {
  // NRZ
  SDRKIT_ARGOS_FIELDS_CODING_NRZ = 0,
  // Inverted NRZ
  SDRKIT_ARGOS_FIELDS_CODING_INVERTED_NRZ = 1,
  // NRZI
  SDRKIT_ARGOS_FIELDS_CODING_NRZI = 2,
} sdrkit_argos_fields_coding;

// The result fields of Argos.
typedef struct sdrkit_argos_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The link the display shows: uplink or downlink. A value of `sdrkit_argos_fields_link`.
  int32_t link;
  // The receiver holds lock.
  bool locked;
  // The downlink's carrier line over the noise. Unit: dB.
  double carrier_line_db;
  // Downlink bits received.
  int64_t downlink_bits;
  // True if `coding` holds a value.
  bool has_coding;
  // The downlink coding in use. A value of `sdrkit_argos_fields_coding`.
  int32_t coding;
} sdrkit_argos_fields;

// The options of `kind` of `sdrkit_argos_burst_row`.
typedef enum sdrkit_argos_burst_row_kind {
  // A2
  SDRKIT_ARGOS_BURST_ROW_KIND_A2 = 0,
  // Other
  SDRKIT_ARGOS_BURST_ROW_KIND_OTHER = 1,
} sdrkit_argos_burst_row_kind;

// An uplink burst. The row key is the sequence number of the burst.
typedef struct sdrkit_argos_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row's moment. A length of time in seconds.
  double at;
  // True if `heard_at` holds a value.
  bool has_heard_at;
  // The wall-clock time of the row's moment, when the samples were live. Seconds since 1970 (UTC).
  double heard_at;
  // The modulation of a burst. A value of `sdrkit_argos_burst_row_kind`.
  int32_t kind;
  // The carrier from the dial. A frequency in Hz.
  double offset_hz;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The unmodulated carrier. Unit: ms.
  double carrier_ms;
  // The whole burst. Unit: ms.
  double total_ms;
  // The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
  double quality;
  // True if `platform` holds a value.
  bool has_platform;
  // The platform id (7 hex digits).
  sdrkit_string platform;
  // True if `data_bits` holds a value.
  bool has_data_bits;
  // The number of user data bits.
  int64_t data_bits;
  // True if `data` holds a value.
  bool has_data;
  // The user data, the first bit the high bit of the first byte.
  sdrkit_bytes data;
} sdrkit_argos_burst_row;

// A platform heard. The row key is the platform id.
typedef struct sdrkit_argos_platform_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The platform id (7 hex digits).
  sdrkit_string platform;
  // The user data, the first bit the high bit of the first byte.
  sdrkit_bytes data;
  // The drift. Unit: Hz/s.
  double drift_hz_s;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // How many times it was heard.
  int64_t heard;
} sdrkit_argos_platform_row;

// The options of `coding` of `sdrkit_argos_frame_row`.
typedef enum sdrkit_argos_frame_row_coding {
  // NRZ
  SDRKIT_ARGOS_FRAME_ROW_CODING_NRZ = 0,
  // Inverted NRZ
  SDRKIT_ARGOS_FRAME_ROW_CODING_INVERTED_NRZ = 1,
  // NRZI
  SDRKIT_ARGOS_FRAME_ROW_CODING_NRZI = 2,
} sdrkit_argos_frame_row_coding;

// A downlink frame with a good FCS. The row key is the sequence number of the frame.
typedef struct sdrkit_argos_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row's moment. A length of time in seconds.
  double at;
  // The downlink coding in use. A value of `sdrkit_argos_frame_row_coding`.
  int32_t coding;
  // The length of a frame without its FCS.
  int64_t bytes;
  // A frame's bytes.
  sdrkit_bytes frame;
} sdrkit_argos_frame_row;

// The result fields of Meteosat HRDCP.
typedef struct sdrkit_hrdcp_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The carriers followed now.
  int64_t carriers;
  // The channel plan in use.
  sdrkit_string plan;
  // What is not confirmed by a public document.
  sdrkit_string note;
} sdrkit_hrdcp_fields;

// A message of the log. The row key is the sequence number of the message.
typedef struct sdrkit_hrdcp_report_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a message. A length of time in seconds.
  double at;
  // True if `heard_at` holds a value.
  bool has_heard_at;
  // The wall-clock time of a message (live radios only). Seconds since 1970 (UTC).
  double heard_at;
  // The address word (8 hex digits).
  sdrkit_string address;
  // The bits corrected.
  int64_t bits_corrected;
  // The sequence counter.
  int64_t sequence;
  // Self-timed or alert.
  sdrkit_string type;
  // The 10 health bits.
  sdrkit_string health;
  // The compression of the data.
  sdrkit_string compression;
  // The CRC passed.
  bool crc_ok;
  // Coded frames of the message.
  int64_t frames;
  // Frames whose words did not decode.
  int64_t frames_bad;
  // Reed-Solomon bytes corrected.
  int64_t rs_corrected;
  // Platform data bytes.
  int64_t bytes;
  // ASCII or binary.
  sdrkit_string format;
  // The bit rate. A frequency in Hz.
  double bit_rate_hz;
  // Manchester coded.
  bool manchester;
  // The frequency of the carrier. A frequency in Hz.
  double freq_hz;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel number of the plan.
  int64_t channel;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The data of the message.
  sdrkit_string text;
} sdrkit_hrdcp_report_row;

// A platform heard. The row key is the address.
typedef struct sdrkit_hrdcp_platform_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address word (8 hex digits).
  sdrkit_string address;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel number of the plan.
  int64_t channel;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // ASCII or binary.
  sdrkit_string format;
  // The data of the last message.
  sdrkit_string text;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // How many times it was heard.
  int64_t heard;
} sdrkit_hrdcp_platform_row;

// A carrier followed now. The row key is its place in the list ("000").
typedef struct sdrkit_hrdcp_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel number of the plan.
  int64_t channel;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The carrier state: 0 looking for the marker, 2 decoding.
  int64_t state;
  // True if `bit_rate_hz` holds a value.
  bool has_bit_rate_hz;
  // The bit rate. A frequency in Hz.
  double bit_rate_hz;
  // Platform data bytes.
  int64_t bytes;
} sdrkit_hrdcp_carrier_row;

// The result fields of COSPAS-SARSAT.
typedef struct sdrkit_sarsat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Bursts heard.
  int64_t bursts_heard;
  // First-generation finders running.
  int64_t finders;
  // The width the finders cover. A frequency in Hz.
  double span_hz;
} sdrkit_sarsat_fields;

// A burst of the log. The row key is the sequence number of the burst.
typedef struct sdrkit_sarsat_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a burst. A length of time in seconds.
  double at;
  // True if `heard_at` holds a value.
  bool has_heard_at;
  // The wall-clock time of the burst, when the samples were live. Seconds since 1970 (UTC).
  double heard_at;
  // The beacon generation, 1 or 2.
  int64_t generation;
  // The beacon's hex identity (15 digits, or 23 for the second generation).
  sdrkit_string hex_id;
  // The beacon type (EPIRB, ELT, PLB ...).
  sdrkit_string beacon_type;
  // The country of the beacon's MID.
  sdrkit_string country;
  // The protocol in words.
  sdrkit_string protocol;
  // The identification in words.
  sdrkit_string identification;
  // True if `position` holds a value.
  bool has_position;
  // The position that the beacon sends.
  sdrkit_position position;
  // The 121.5 MHz homing flag.
  bool homing;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
} sdrkit_sarsat_burst_row;

// A beacon heard, with its last burst. The row key is the hex identity.
typedef struct sdrkit_sarsat_beacon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The beacon's hex identity (15 digits, or 23 for the second generation).
  sdrkit_string hex_id;
  // The beacon generation, 1 or 2.
  int64_t generation;
  // The beacon type (EPIRB, ELT, PLB ...).
  sdrkit_string beacon_type;
  // The country of the beacon's MID.
  sdrkit_string country;
  // The identification in words.
  sdrkit_string identification;
  // True if `position` holds a value.
  bool has_position;
  // The position that the beacon sends.
  sdrkit_position position;
  // True if `combined` holds a value.
  bool has_combined;
  // Bursts whose soft bits were summed to decode the message.
  int64_t combined;
  // True if `confirmed` holds a value.
  bool has_confirmed;
  // A later burst decoded to the same identification before the message was reported.
  bool confirmed;
  // True if `repair_margin` holds a value.
  bool has_repair_margin;
  // The distance from the decoded word to the next one that passes, over the mean reliability of a bit.
  double repair_margin;
  // True if `repair_cost` holds a value.
  bool has_repair_cost;
  // The reliability of the repaired bits over the mean reliability of a bit.
  double repair_cost;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // How many times it was heard.
  int64_t heard;
} sdrkit_sarsat_beacon_row;

// The result fields of the LoRa satellite stack.
typedef struct sdrkit_lora_sat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The pass planner's name of the satellite overhead.
  sdrkit_string satellite;
  // True if `preset` holds a value.
  bool has_preset;
  // The name of the preset that runs.
  sdrkit_string preset;
  // The number of detectors without Doppler tracking.
  int64_t demodulators;
  // The largest Doppler shift expected. A frequency in Hz.
  double doppler_max_hz;
  // The largest Doppler rate expected. Unit: Hz/s.
  double doppler_rate_max_hz_s;
} sdrkit_lora_sat_fields;

// The options of `crc` of `sdrkit_lora_sat_frame_row`.
typedef enum sdrkit_lora_sat_frame_row_crc {
  // None
  SDRKIT_LORA_SAT_FRAME_ROW_CRC_NONE = 0,
  // Good
  SDRKIT_LORA_SAT_FRAME_ROW_CRC_OK = 1,
  // Bad
  SDRKIT_LORA_SAT_FRAME_ROW_CRC_BAD = 2,
} sdrkit_lora_sat_frame_row_crc;

// A frame of the log. The row key is the sequence number of the frame.
typedef struct sdrkit_lora_sat_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `heard_at` holds a value.
  bool has_heard_at;
  // The wall-clock time a frame was heard (live radios only). Seconds since 1970 (UTC).
  double heard_at;
  // The spreading factor of a frame.
  int64_t sf;
  // The bandwidth of a frame. A frequency in Hz.
  double bw_hz;
  // The coding rate of a frame: the denominator n of 4/n.
  int64_t cr;
  // The CRC of a frame. A value of `sdrkit_lora_sat_frame_row_crc`.
  int32_t crc;
  // The sync word of a frame.
  int64_t sync;
  // The payload length of a frame.
  int64_t bytes;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The carrier offset from the dial at the first data symbol. A frequency in Hz.
  double doppler_hz;
  // True if `doppler_rate_hz_s` holds a value.
  bool has_doppler_rate_hz_s;
  // The carrier rate during a frame. Unit: Hz/s.
  double doppler_rate_hz_s;
  // The frequency of the frame. A frequency in Hz.
  double freq_hz;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The pass planner's name of the satellite overhead.
  sdrkit_string satellite;
  // The payload of a frame.
  sdrkit_bytes payload;
  // The payload as printable text.
  sdrkit_string text;
} sdrkit_lora_sat_frame_row;

// A demodulator running now. The row key is its place in the list ("000").
typedef struct sdrkit_lora_sat_demodulator_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The spreading factor of a frame.
  int64_t sf;
  // The bandwidth of a frame. A frequency in Hz.
  double bw_hz;
  // Detector counters: preambles found.
  int64_t preambles;
  // Detector counters: preambles without a sync word or SFD.
  int64_t sync_failed;
  // Detector counters: headers that failed.
  int64_t header_failed;
  // Detector counters: frames decoded, good or not.
  int64_t frames;
  // Detector counters: frames whose CRC failed.
  int64_t crc_failed;
} sdrkit_lora_sat_demodulator_row;

// A sync word of a demodulator. The row key is the demodulator and the sync word ("000/18").
typedef struct sdrkit_lora_sat_demodulator_sync_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The detector a sync word belongs to.
  sdrkit_string demodulator;
  // The sync word of a frame.
  int64_t sync;
} sdrkit_lora_sat_demodulator_sync_row;

// The frames of one spreading factor. The row key is the SF ("07").
typedef struct sdrkit_lora_sat_sf_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The spreading factor of a frame.
  int64_t sf;
  // Detector counters: frames decoded, good or not.
  int64_t frames;
} sdrkit_lora_sat_sf_row;

// The result fields of Orbcomm.
typedef struct sdrkit_orbcomm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Messages joined from their parts.
  int64_t messages_joined;
  // Satellites heard.
  int64_t satellites;
} sdrkit_orbcomm_fields;

// A satellite heard. The row key is the satellite number.
typedef struct sdrkit_orbcomm_satellite_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The satellite.
  int64_t satellite;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The frequency. A frequency in Hz.
  double frequency;
  // True if `doppler` holds a value.
  bool has_doppler;
  // The doppler. A frequency in Hz.
  double doppler;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The frames.
  int64_t frames;
  // True if `heard` holds a value.
  bool has_heard;
  // How many times it was heard.
  int64_t heard;
  // The bad packets.
  int64_t bad;
  // True if `position` holds a value.
  bool has_position;
  // The position.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height. Unit: m.
  double height_m;
  // True if `inclination_deg` holds a value.
  bool has_inclination_deg;
  // The inclination. Unit: deg.
  double inclination_deg;
  // True if `period` holds a value.
  bool has_period;
  // The period. A length of time in seconds.
  double period;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_orbcomm_satellite_row;

// A channel that a satellite announces. The row key is `<satellite>/<u or d><n>`.
typedef struct sdrkit_orbcomm_satellite_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The satellite.
  int64_t satellite;
  // The channel is an uplink (else a downlink).
  bool uplink;
  // The frequency. A frequency in Hz.
  double frequency;
} sdrkit_orbcomm_satellite_channel_row;

// The options of `state` of `sdrkit_orbcomm_decoder_row`.
typedef enum sdrkit_orbcomm_decoder_row_state {
  // Free
  SDRKIT_ORBCOMM_DECODER_ROW_STATE_FREE = 0,
  // Acquiring
  SDRKIT_ORBCOMM_DECODER_ROW_STATE_ACQUIRING = 1,
  // Tracking
  SDRKIT_ORBCOMM_DECODER_ROW_STATE_TRACKING = 2,
  // Frame lock
  SDRKIT_ORBCOMM_DECODER_ROW_STATE_FRAME_LOCK = 3,
} sdrkit_orbcomm_decoder_row_state;

// A decoder. The row key is the decoder number.
typedef struct sdrkit_orbcomm_decoder_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The decoder.
  int64_t decoder;
  // The state. A value of `sdrkit_orbcomm_decoder_row_state`.
  int32_t state;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The frequency. A frequency in Hz.
  double frequency;
  // True if `offset` holds a value.
  bool has_offset;
  // The offset. A frequency in Hz.
  double offset;
  // True if `doppler` holds a value.
  bool has_doppler;
  // The doppler. A frequency in Hz.
  double doppler;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level. Unit: dB.
  double level_db;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The satellite.
  int64_t satellite;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The clock error. Unit: ppm.
  double clock_ppm;
  // True if `frames` holds a value.
  bool has_frames;
  // The frames.
  int64_t frames;
  // True if `frames_ok` holds a value.
  bool has_frames_ok;
  // Frames that passed their check.
  int64_t frames_ok;
  // True if `frames_bad` holds a value.
  bool has_frames_bad;
  // Frames that failed their check.
  int64_t frames_bad;
  // True if `sync_losses` holds a value.
  bool has_sync_losses;
  // The sync losses.
  int64_t sync_losses;
} sdrkit_orbcomm_decoder_row;

// The options of `type` of `sdrkit_orbcomm_packet_row`.
typedef enum sdrkit_orbcomm_packet_row_type {
  // Sync
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_SYNC = 0,
  // Message
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_MESSAGE = 1,
  // Uplink
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_UPLINK = 2,
  // Downlink
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_DOWNLINK = 3,
  // Network
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_NETWORK = 4,
  // Fill
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_FILL = 5,
  // Ephemeris
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_EPHEMERIS = 6,
  // Elements
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_ELEMENTS = 7,
  // Other
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_OTHER = 8,
  // Message joined
  SDRKIT_ORBCOMM_PACKET_ROW_TYPE_MESSAGE_JOINED = 9,
} sdrkit_orbcomm_packet_row_type;

// A packet of the log. The row key is the sequence number of the packet.
typedef struct sdrkit_orbcomm_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The at (signal time). A length of time in seconds.
  double at;
  // True if `wall` holds a value.
  bool has_wall;
  // The time. Seconds since 1970 (UTC).
  double wall;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The satellite.
  int64_t satellite;
  // The decoder.
  int64_t decoder;
  // The type. A value of `sdrkit_orbcomm_packet_row_type`.
  int32_t type;
  // True if `bytes` holds a value.
  bool has_bytes;
  // The bytes.
  sdrkit_bytes bytes;
  // What the packet says.
  sdrkit_string text;
} sdrkit_orbcomm_packet_row;

// The options of `type` of `sdrkit_orbcomm_packet_type_row`.
typedef enum sdrkit_orbcomm_packet_type_row_type {
  // Sync
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_SYNC = 0,
  // Message
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_MESSAGE = 1,
  // Uplink
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_UPLINK = 2,
  // Downlink
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_DOWNLINK = 3,
  // Network
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_NETWORK = 4,
  // Fill
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_FILL = 5,
  // Ephemeris
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_EPHEMERIS = 6,
  // Elements
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_ELEMENTS = 7,
  // Other
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_OTHER = 8,
  // Message joined
  SDRKIT_ORBCOMM_PACKET_TYPE_ROW_TYPE_MESSAGE_JOINED = 9,
} sdrkit_orbcomm_packet_type_row_type;

// The packets of one type. The row key is the key of the type.
typedef struct sdrkit_orbcomm_packet_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The type. A value of `sdrkit_orbcomm_packet_type_row_type`.
  int32_t type;
  // The count.
  int64_t count;
} sdrkit_orbcomm_packet_type_row;

// The options of `order` of `sdrkit_psk_demod_fields`.
typedef enum sdrkit_psk_demod_fields_order {
  // Found by the analyser
  SDRKIT_PSK_DEMOD_FIELDS_ORDER_AUTO = 0,
  // BPSK
  SDRKIT_PSK_DEMOD_FIELDS_ORDER_BPSK = 1,
  // QPSK
  SDRKIT_PSK_DEMOD_FIELDS_ORDER_QPSK = 2,
  // OQPSK
  SDRKIT_PSK_DEMOD_FIELDS_ORDER_OQPSK = 3,
  // 8PSK
  SDRKIT_PSK_DEMOD_FIELDS_ORDER_8PSK = 4,
} sdrkit_psk_demod_fields_order;

// The options of `shape` of `sdrkit_psk_demod_fields`.
typedef enum sdrkit_psk_demod_fields_shape {
  // Rectangular
  SDRKIT_PSK_DEMOD_FIELDS_SHAPE_RECT = 0,
  // Root raised cosine
  SDRKIT_PSK_DEMOD_FIELDS_SHAPE_RRC = 1,
} sdrkit_psk_demod_fields_shape;

// The options of `note` of `sdrkit_psk_demod_fields`.
typedef enum sdrkit_psk_demod_fields_note {
  // Too few samples
  SDRKIT_PSK_DEMOD_FIELDS_NOTE_TOO_SHORT = 0,
  // No symbol rate line
  SDRKIT_PSK_DEMOD_FIELDS_NOTE_NO_RATE_LINE = 1,
  // No trial demodulation
  SDRKIT_PSK_DEMOD_FIELDS_NOTE_NO_TRIAL = 2,
  // No constellation
  SDRKIT_PSK_DEMOD_FIELDS_NOTE_NO_CONSTELLATION = 3,
} sdrkit_psk_demod_fields_note;

// The result fields of the PSK demodulator.
typedef struct sdrkit_psk_demod_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `order` holds a value.
  bool has_order;
  // The modulation order the demodulator runs. A value of `sdrkit_psk_demod_fields_order`.
  int32_t order;
  // True if `lock` holds a value.
  bool has_lock;
  // The carrier loop's lock value (1 on the points).
  double lock;
  // True if `carrier_hz` holds a value.
  bool has_carrier_hz;
  // The carrier, from the dial. A frequency in Hz.
  double carrier_hz;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The drift. Unit: Hz/s.
  double drift_hz_s;
  // True if `evm_pct` holds a value.
  bool has_evm_pct;
  // The EVM. Unit: %.
  double evm_pct;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // Es/N0 from the error vector. Unit: dB.
  double esn0_db;
  // True if `ebn0_db` holds a value.
  bool has_ebn0_db;
  // Eb/N0 from the error vector. Unit: dB.
  double ebn0_db;
  // Symbols demodulated since the lock.
  int64_t symbols;
  // Bits since the start.
  int64_t bits;
  // Frames that passed their check.
  int64_t frames_ok;
  // The newest bits, oldest first, as the characters 0 and 1.
  sdrkit_string bit_stream;
  // The index of the first bit of `bit_stream`.
  int64_t bits_first;
  // The phase rotation the sync words resolved. Unit: deg.
  double rotation_deg;
  // Whether the sync words resolved a spectral inversion.
  bool conjugated;
  // Whether the sync words resolved an OQPSK rail shift.
  bool rail_shift;
  // Whether the phase ambiguity is resolved.
  bool resolved;
  // True if `signal_hz` holds a value.
  bool has_signal_hz;
  // The centre of the signal found, from the dial. A frequency in Hz.
  double signal_hz;
  // True if `width_3db_hz` holds a value.
  bool has_width_3db_hz;
  // Its width at 3 dB. A frequency in Hz.
  double width_3db_hz;
  // True if `width_10db_hz` holds a value.
  bool has_width_10db_hz;
  // Its width at 10 dB. A frequency in Hz.
  double width_10db_hz;
  // True if `occupied_hz` holds a value.
  bool has_occupied_hz;
  // Its 99 % occupied bandwidth. A frequency in Hz.
  double occupied_hz;
  // True if `signal_snr_db` holds a value.
  bool has_signal_snr_db;
  // Its in-band SNR. Unit: dB.
  double signal_snr_db;
  // True if `baud_raw_hz` holds a value.
  bool has_baud_raw_hz;
  // The symbol rate the analyser measured, before snapping. A frequency in Hz.
  double baud_raw_hz;
  // True if `snapped` holds a value.
  bool has_snapped;
  // Whether the rate was snapped to a standard one.
  bool snapped;
  // True if `rate_confidence` holds a value.
  bool has_rate_confidence;
  // The confidence of the rate, 0 to 1.
  double rate_confidence;
  // True if `order_confidence` holds a value.
  bool has_order_confidence;
  // The confidence of the order, 0 to 1.
  double order_confidence;
  // True if `shape` holds a value.
  bool has_shape;
  // The pulse shape. A value of `sdrkit_psk_demod_fields_shape`.
  int32_t shape;
  // True if `roll_off` holds a value.
  bool has_roll_off;
  // The roll-off of root raised cosine pulses.
  double roll_off;
  // True if `envelope_db` holds a value.
  bool has_envelope_db;
  // The envelope line over the median. Unit: dB.
  double envelope_db;
  // True if `line_x2_db` holds a value.
  bool has_line_x2_db;
  // The squared signal's line over the median. Unit: dB.
  double line_x2_db;
  // True if `line_x4_db` holds a value.
  bool has_line_x4_db;
  // The fourth power's line over the median. Unit: dB.
  double line_x4_db;
  // True if `line_x8_db` holds a value.
  bool has_line_x8_db;
  // The eighth power's line over the median. Unit: dB.
  double line_x8_db;
  // True if `note` holds a value.
  bool has_note;
  // What the analyser said when it found no PSK signal. A value of `sdrkit_psk_demod_fields_note`.
  int32_t note;
} sdrkit_psk_demod_fields;

// A rate candidate of the analyser. The row key is its place ("0").
typedef struct sdrkit_psk_demod_rate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A rate candidate. A frequency in Hz.
  double rate_hz;
  // A line's strength over the median. Unit: dB.
  double strength_db;
} sdrkit_psk_demod_rate_row;

// The options of `order` of `sdrkit_psk_demod_trial_row`.
typedef enum sdrkit_psk_demod_trial_row_order {
  // Found by the analyser
  SDRKIT_PSK_DEMOD_TRIAL_ROW_ORDER_AUTO = 0,
  // BPSK
  SDRKIT_PSK_DEMOD_TRIAL_ROW_ORDER_BPSK = 1,
  // QPSK
  SDRKIT_PSK_DEMOD_TRIAL_ROW_ORDER_QPSK = 2,
  // OQPSK
  SDRKIT_PSK_DEMOD_TRIAL_ROW_ORDER_OQPSK = 3,
  // 8PSK
  SDRKIT_PSK_DEMOD_TRIAL_ROW_ORDER_8PSK = 4,
} sdrkit_psk_demod_trial_row_order;

// A trial demodulation of the analyser. The row key is its place ("00").
typedef struct sdrkit_psk_demod_trial_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The modulation order the demodulator runs. A value of `sdrkit_psk_demod_trial_row_order`.
  int32_t order;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // The error vector of a trial, unit power.
  double evm;
} sdrkit_psk_demod_trial_row;

// The bit index after a recent sync word, oldest first. The row key is its place ("00").
typedef struct sdrkit_psk_demod_sync_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The bit index after a sync word.
  int64_t bit;
} sdrkit_psk_demod_sync_row;

// A line of the log of analyses. The row key is the sequence number of the line.
typedef struct sdrkit_psk_demod_log_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a line of the log. A length of time in seconds.
  double at;
  // The line.
  sdrkit_string text;
} sdrkit_psk_demod_log_row;

// The options of `beacon_lock` of `sdrkit_qo100_fields`.
typedef enum sdrkit_qo100_fields_beacon_lock {
  // Outside the band
  SDRKIT_QO100_FIELDS_BEACON_LOCK_OUTSIDE = 0,
  // Searching
  SDRKIT_QO100_FIELDS_BEACON_LOCK_SEARCHING = 1,
  // Locked
  SDRKIT_QO100_FIELDS_BEACON_LOCK_LOCKED = 2,
  // Held
  SDRKIT_QO100_FIELDS_BEACON_LOCK_HELD = 3,
} sdrkit_qo100_fields_beacon_lock;

// The result fields of QO-100.
typedef struct sdrkit_qo100_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The LNB's LO error (actual minus nominal). A frequency in Hz.
  double lnb_error_hz;
  // The LNB error in parts per million of the nominal LO. Unit: ppm.
  double lnb_error_ppm;
  // The drift. Unit: Hz/s.
  double drift_hz_s;
  // The phase of the beacon lock. A value of `sdrkit_qo100_fields_beacon_lock`.
  int32_t beacon_lock;
  // Whether the LNB error is applied to the frequencies shown.
  bool correction_applied;
  // Fine estimates that found a CW beacon.
  int64_t measurements;
  // The locked for. A length of time in seconds.
  double locked_for;
  // The USB dial for listening (RF). A frequency in Hz.
  double dial_hz;
  // True if `listening` holds a value.
  bool has_listening;
  // The conversation followed by the dial.
  int64_t listening;
  // Whether the wideband transponder is outside the radio's band.
  bool wideband_outside;
  // True if `narrow_floor_dbfs` holds a value.
  bool has_narrow_floor_dbfs;
  // The noise floor of the narrowband spectrum. Unit: dBFS.
  double narrow_floor_dbfs;
  // True if `own_to_beacon_db` holds a value.
  bool has_own_to_beacon_db;
  // The level of the strongest signal in the 2.7 kHz above the dial less the level of the PSK beacon, both as the peak of a spectrum bin. While the stack sends, that signal is the user's own downlink: it must stay below 0 dB. With nothing on the dial it is the level of the noise peaks there. No value while the beacon is not found. Unit: dB.
  double own_to_beacon_db;
} sdrkit_qo100_fields;

// The result fields of the voice sender of QO-100. The measurements have no value while the stack does not send.
typedef struct sdrkit_qo100_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the stack transmits now.
  bool transmitting;
  // The uplink frequency of the dial: the dial less the translation of the transponder. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_time_s` holds a value.
  bool has_tx_time_s;
  // The time on air of the transmission. A length of time in seconds.
  double tx_time_s;
  // True if `tx_mic_dbfs` holds a value.
  bool has_tx_mic_dbfs;
  // The peak of the microphone audio over the last report. Unit: dBFS.
  double tx_mic_dbfs;
  // True if `tx_peak_db` holds a value.
  bool has_tx_peak_db;
  // The peak of the modulation over the last report, against the peak level. Unit: dB.
  double tx_peak_db;
  // True if `tx_level_db` holds a value.
  bool has_tx_level_db;
  // The gain of the level control of the speech processor. Unit: dB.
  double tx_level_db;
  // True if `tx_limit_db` holds a value.
  bool has_tx_limit_db;
  // The most gain reduction of the limiter over the last report: 0 when it did not act. Unit: dB.
  double tx_limit_db;
} sdrkit_qo100_tx_fields;

// A beacon of the narrowband transponder. The row key is its place ("000").
typedef struct sdrkit_qo100_beacon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of a beacon.
  sdrkit_string name;
  // A nominal RF. A frequency in Hz.
  double nominal_hz;
  // Whether a beacon was found.
  bool found;
  // The RF where a beacon shows with no correction. A frequency in Hz.
  double apparent_hz;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
} sdrkit_qo100_beacon_row;

// The options of `class_` of `sdrkit_qo100_signal_row`.
typedef enum sdrkit_qo100_signal_row_class_ {
  // Unknown
  SDRKIT_QO100_SIGNAL_ROW_CLASS__UNKNOWN = 0,
  // Beacon
  SDRKIT_QO100_SIGNAL_ROW_CLASS__BEACON = 1,
  // Carrier
  SDRKIT_QO100_SIGNAL_ROW_CLASS__CARRIER = 2,
  // CW
  SDRKIT_QO100_SIGNAL_ROW_CLASS__CW = 3,
  // PSK
  SDRKIT_QO100_SIGNAL_ROW_CLASS__PSK = 4,
  // SSB
  SDRKIT_QO100_SIGNAL_ROW_CLASS__SSB = 5,
  // SSTV
  SDRKIT_QO100_SIGNAL_ROW_CLASS__SSTV = 6,
  // Wide digital
  SDRKIT_QO100_SIGNAL_ROW_CLASS__WIDE_DIGITAL = 7,
} sdrkit_qo100_signal_row_class_;

// A signal of the spectrum now. The row key is its place ("000").
typedef struct sdrkit_qo100_signal_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The RF of a signal. A frequency in Hz.
  double rf_hz;
  // The width of a signal. A frequency in Hz.
  double width_hz;
  // The level of a signal. Unit: dBFS.
  double level_dbfs;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The tracker's class of a signal. A value of `sdrkit_qo100_signal_row_class_`.
  int32_t class_;
  // True if `beacon` holds a value.
  bool has_beacon;
  // The beacon a signal is.
  sdrkit_string beacon;
  // True if `track` holds a value.
  bool has_track;
  // The id of a track.
  int64_t track;
} sdrkit_qo100_signal_row;

// The options of `class_` of `sdrkit_qo100_track_row`.
typedef enum sdrkit_qo100_track_row_class_ {
  // Unknown
  SDRKIT_QO100_TRACK_ROW_CLASS__UNKNOWN = 0,
  // Beacon
  SDRKIT_QO100_TRACK_ROW_CLASS__BEACON = 1,
  // Carrier
  SDRKIT_QO100_TRACK_ROW_CLASS__CARRIER = 2,
  // CW
  SDRKIT_QO100_TRACK_ROW_CLASS__CW = 3,
  // PSK
  SDRKIT_QO100_TRACK_ROW_CLASS__PSK = 4,
  // SSB
  SDRKIT_QO100_TRACK_ROW_CLASS__SSB = 5,
  // SSTV
  SDRKIT_QO100_TRACK_ROW_CLASS__SSTV = 6,
  // Wide digital
  SDRKIT_QO100_TRACK_ROW_CLASS__WIDE_DIGITAL = 7,
} sdrkit_qo100_track_row_class_;

// A signal followed over time. The row key is the track number.
typedef struct sdrkit_qo100_track_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // The RF of a signal. A frequency in Hz.
  double rf_hz;
  // The width of a signal. A frequency in Hz.
  double width_hz;
  // The level of a signal. Unit: dBFS.
  double level_dbfs;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // Whether a track or a conversation is active now.
  bool active;
  // The share of the last 30 s a track was on. Unit: %.
  double duty_pct;
  // The tracker's class of a signal. A value of `sdrkit_qo100_track_row_class_`.
  int32_t class_;
  // True if `conversation` holds a value.
  bool has_conversation;
  // The conversation of a track.
  int64_t conversation;
} sdrkit_qo100_track_row;

// A conversation: SSB tracks grouped by frequency and time. The row key is the conversation number.
typedef struct sdrkit_qo100_conversation_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // Whether a track or a conversation is active now.
  bool active;
  // The USB dial for listening (RF). A frequency in Hz.
  double dial_hz;
  // The lower edge of a conversation. A frequency in Hz.
  double lower_edge_hz;
  // The level of a signal. Unit: dBFS.
  double level_dbfs;
  // The peak level of a conversation. Unit: dBFS.
  double peak_dbfs;
  // The stations estimated in a conversation.
  int64_t stations;
  // The talk time of a conversation. A length of time in seconds.
  double talk;
  // How long a conversation has lasted. A length of time in seconds.
  double length;
} sdrkit_qo100_conversation_row;

// A callsign read in a conversation. The row key is the conversation and the callsign ("3/G4ABC").
typedef struct sdrkit_qo100_callsign_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The conversation of a track.
  int64_t conversation;
  // The callsign.
  sdrkit_string callsign;
} sdrkit_qo100_callsign_row;

// A DATV carrier of the wideband transponder. The row key is its place ("000").
typedef struct sdrkit_qo100_datv_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The RF of a signal. A frequency in Hz.
  double rf_hz;
  // The centre of a DATV carrier in the input. A frequency in Hz.
  double if_hz;
  // The width of a signal. A frequency in Hz.
  double width_hz;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // Whether a DATV carrier has a flat top.
  bool flat_topped;
  // Whether a DATV carrier is the beacon.
  bool is_beacon;
} sdrkit_qo100_datv_carrier_row;

// The result fields of the CubeSat telemetry stack.
typedef struct sdrkit_cubesat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that passed their check.
  int64_t frames_ok;
  // Frames the Reed-Solomon code repaired.
  int64_t rs_repaired;
  // Frames sent to the KISS output.
  int64_t kiss_sent;
  // Which parts of the framings received are not confirmed by a public document.
  sdrkit_string note;
} sdrkit_cubesat_fields;

// The result fields of the AX.25 sender of the CubeSat stack.
typedef struct sdrkit_cubesat_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is on air.
  bool tx_sending;
  // The frames of the transmission on air.
  int64_t tx_sent;
  // The frames waiting for the next transmission.
  int64_t tx_waiting;
} sdrkit_cubesat_tx_fields;

// The options of `framing` of `sdrkit_cubesat_frame_row`.
typedef enum sdrkit_cubesat_frame_row_framing {
  // AX.25 AFSK
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_AX25_AFSK = 0,
  // AX.25 G3RUH
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_AX25_G3RUH = 1,
  // AX.25 NRZI
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_AX25_NRZI = 2,
  // AX100 Mode 5
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_AX100_MODE5 = 3,
  // AX100 Mode 6
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_AX100_MODE6 = 4,
  // CCSDS RS
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_CCSDS_RS = 5,
  // Geoscan
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_GEOSCAN = 6,
  // Lucky-7
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_LUCKY7 = 7,
  // Spino
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_SPINO = 8,
  // Erminaz
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_ERMINAZ = 9,
  // USP
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_USP = 10,
  // NGHam
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_NGHAM = 11,
  // U482C
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_U482C = 12,
  // Mobitex
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_MOBITEX = 13,
  // Mobitex-NX
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_MOBITEX_NX = 14,
  // BEESAT-1
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_BEESAT1 = 15,
  // BEESAT-9
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_BEESAT9 = 16,
  // CCSDS concatenated
  SDRKIT_CUBESAT_FRAME_ROW_FRAMING_CCSDS_CONCAT = 17,
} sdrkit_cubesat_frame_row_framing;

// A frame of the log. The row key is the sequence number of the frame.
typedef struct sdrkit_cubesat_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `heard_at` holds a value.
  bool has_heard_at;
  // The wall-clock time a frame was heard (live radios only). Seconds since 1970 (UTC).
  double heard_at;
  // The framing of a frame. A value of `sdrkit_cubesat_frame_row_framing`.
  int32_t framing;
  // The bit rate of a frame.
  int64_t baud;
  // True if `rs_corrected` holds a value.
  bool has_rs_corrected;
  // Reed-Solomon symbols corrected (none for a framing without Reed-Solomon).
  int64_t rs_corrected;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The satellite of a frame.
  sdrkit_string satellite;
  // Its NORAD number.
  int64_t norad;
  // The name comes from the dial frequency.
  bool guess;
  // The frequency of the frame. A frequency in Hz.
  double freq_hz;
  // True if `from` holds a value.
  bool has_from;
  // AX.25 source.
  sdrkit_string from;
  // True if `to` holds a value.
  bool has_to;
  // AX.25 destination.
  sdrkit_string to;
  // True if `path` holds a value.
  bool has_path;
  // AX.25 repeaters, separated by commas.
  sdrkit_string path;
  // True if `frame_type` holds a value.
  bool has_frame_type;
  // AX.25 frame type.
  sdrkit_string frame_type;
  // True if `pid` holds a value.
  bool has_pid;
  // AX.25 PID.
  int64_t pid;
  // True if `csp` holds a value.
  bool has_csp;
  // The CSP header in words.
  sdrkit_string csp;
  // The frame's data.
  sdrkit_bytes data;
  // The payload.
  sdrkit_bytes payload;
  // True if `text` holds a value.
  bool has_text;
  // The text beacon.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // True if `validated` holds a value.
  bool has_validated;
  // What passed the checks of a frame.
  sdrkit_string validated;
  // True if `kind` holds a value.
  bool has_kind;
  // The Lucky-7 kind of a frame.
  sdrkit_string kind;
  // The sync word was found complemented.
  bool inverted;
  // True if `spacecraft` holds a value.
  bool has_spacecraft;
  // The spacecraft id of a CCSDS header.
  int64_t spacecraft;
  // True if `vc` holds a value.
  bool has_vc;
  // The virtual channel of a CCSDS header.
  int64_t vc;
  // True if `flags` holds a value.
  bool has_flags;
  // The NGHam flags.
  int64_t flags;
} sdrkit_cubesat_frame_row;

// The options of `framing` of `sdrkit_cubesat_framing_row`.
typedef enum sdrkit_cubesat_framing_row_framing {
  // AX.25 AFSK
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_AX25_AFSK = 0,
  // AX.25 G3RUH
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_AX25_G3RUH = 1,
  // AX.25 NRZI
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_AX25_NRZI = 2,
  // AX100 Mode 5
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_AX100_MODE5 = 3,
  // AX100 Mode 6
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_AX100_MODE6 = 4,
  // CCSDS RS
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_CCSDS_RS = 5,
  // Geoscan
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_GEOSCAN = 6,
  // Lucky-7
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_LUCKY7 = 7,
  // Spino
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_SPINO = 8,
  // Erminaz
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_ERMINAZ = 9,
  // USP
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_USP = 10,
  // NGHam
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_NGHAM = 11,
  // U482C
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_U482C = 12,
  // Mobitex
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_MOBITEX = 13,
  // Mobitex-NX
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_MOBITEX_NX = 14,
  // BEESAT-1
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_BEESAT1 = 15,
  // BEESAT-9
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_BEESAT9 = 16,
  // CCSDS concatenated
  SDRKIT_CUBESAT_FRAMING_ROW_FRAMING_CCSDS_CONCAT = 17,
} sdrkit_cubesat_framing_row_framing;

// The frames of one framing. The row key is the key of the framing.
typedef struct sdrkit_cubesat_framing_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The framing of a frame. A value of `sdrkit_cubesat_framing_row_framing`.
  int32_t framing;
  // Frames by framing.
  int64_t frames;
  // True if `sync_hits` holds a value.
  bool has_sync_hits;
  // Sync word matches of a framing.
  int64_t sync_hits;
  // True if `decode_failed` holds a value.
  bool has_decode_failed;
  // Candidates of a framing that failed their checks.
  int64_t decode_failed;
} sdrkit_cubesat_framing_row;

// A carrier found now. The row key is its place ("000").
typedef struct sdrkit_cubesat_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A carrier's offset from the dial. A frequency in Hz.
  double offset_hz;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
} sdrkit_cubesat_carrier_row;

// The result fields of Fleet Broadcast.
typedef struct sdrkit_fleet_broadcast_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The state: 0 nothing, 1 carrier, 2 bits, 3 framed.
  int64_t state;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // The line of the carrier search over the floor. Unit: dB.
  double line_db;
  // Sample-clock or symbol-clock error, parts per million. Unit: ppm.
  double clock_ppm;
  // The eye: 0 clean, towards 1 noisy.
  double eye;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Symbols so far.
  int64_t symbols;
  // The frames are aligned.
  bool framed;
  // Several columns repeat.
  bool ambiguous;
  // True if `sync_period` holds a value.
  bool has_sync_period;
  // The repeat of the sync channel, frames. Unit: frames.
  int64_t sync_period;
  // True if `sync_agreement` holds a value.
  bool has_sync_agreement;
  // The agreement of the sync channel with itself one period later. Unit: %.
  double sync_agreement;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // The class letters, channel 0 first.
  sdrkit_string classes;
  // What is not confirmed by a public document.
  sdrkit_string note;
} sdrkit_fleet_broadcast_fields;

// One of the 16 channels. The row key is its place, `000` first.
typedef struct sdrkit_fleet_broadcast_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel (or column, when not framed).
  int64_t channel;
  // The class of a channel.
  sdrkit_string class_;
  // The share of bit changes. Unit: %.
  double activity_pct;
  // The share of ones. Unit: %.
  double ones_pct;
  // True if `sync_period` holds a value.
  bool has_sync_period;
  // The repeat of the sync channel, frames. Unit: frames.
  int64_t sync_period;
  // The last 64 bits as hex, oldest first.
  sdrkit_string last_bits;
} sdrkit_fleet_broadcast_channel_row;

// A frame of the frame log. The row key is its place, `000` first.
typedef struct sdrkit_fleet_broadcast_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A frame as 4 hex digits, channel 0 the most significant bit.
  sdrkit_string word;
  // A frame as 16 bits, channel 0 first.
  sdrkit_string bits;
} sdrkit_fleet_broadcast_frame_row;

// The result fields of BGAN.
typedef struct sdrkit_bgan_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Carriers found.
  int64_t carriers;
  // Carriers in frame sync.
  int64_t in_sync;
  // Carriers whose signalling decoded.
  int64_t decoded;
  // GMR-2 class carriers.
  int64_t gmr2;
  // Networks heard (MCC-MNC).
  sdrkit_string networks;
  // Satellites heard.
  sdrkit_string satellites_heard;
  // FEC blocks decoded.
  int64_t blocks_ok;
  // FEC blocks that failed.
  int64_t blocks_bad;
  // Connection PDUs counted, never read.
  int64_t connection_pdus;
  // PDUs addressed to a terminal counted, never read.
  int64_t addressed_pdus;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // What is not confirmed by a public document.
  sdrkit_string note;
} sdrkit_bgan_fields;

// A carrier. The row key is its place, `000` first.
typedef struct sdrkit_bgan_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // Width of a carrier. A frequency in Hz.
  double width_hz;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // What the carrier is.
  sdrkit_string kind;
  // The demodulator state.
  sdrkit_string state;
  // True if `bearer` holds a value.
  bool has_bearer;
  // Bearer.
  sdrkit_string bearer;
  // True if `subtype` holds a value.
  bool has_subtype;
  // Subtype of block 0.
  sdrkit_string subtype;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // Symbol rate found, Hz. A frequency in Hz.
  double symbol_rate_hz;
  // True if `cfo_hz` holds a value.
  bool has_cfo_hz;
  // Frequency left after the channel filter. A frequency in Hz.
  double cfo_hz;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // C/N0 from the pilots. Unit: dB-Hz.
  double cn0_dbhz;
  // Frames in sync.
  int64_t frames;
  // FEC blocks decoded.
  int64_t blocks_ok;
  // FEC blocks that failed.
  int64_t blocks_bad;
  // Share of blocks with PDUs. Unit: %.
  double load_pct;
  // Frame sync without decodable tables.
  bool structure_only;
  // True if `spot_beam` holds a value.
  bool has_spot_beam;
  // Spot beam id.
  int64_t spot_beam;
  // True if `rnc` holds a value.
  bool has_rnc;
  // rnc-id.
  int64_t rnc;
  // True if `bct_id` holds a value.
  bool has_bct_id;
  // bct-id.
  int64_t bct_id;
  // True if `f_bearer` holds a value.
  bool has_f_bearer;
  // f-bearer.
  int64_t f_bearer;
  // True if `beam_type` holds a value.
  bool has_beam_type;
  // Beam type.
  sdrkit_string beam_type;
  // True if `network` holds a value.
  bool has_network;
  // MCC-MNC.
  sdrkit_string network;
  // True if `satellite` holds a value.
  bool has_satellite;
  // The carrying satellite.
  sdrkit_string satellite;
  // Seconds since last heard. A length of time in seconds.
  double last_seen;
} sdrkit_bgan_carrier_row;

// A spot beam. The row key is its place, `000` first.
typedef struct sdrkit_bgan_beam_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Spot beam id.
  int64_t spot_beam;
  // True if `beam_type` holds a value.
  bool has_beam_type;
  // Beam type.
  sdrkit_string beam_type;
  // True if `primary_hz` holds a value.
  bool has_primary_hz;
  // The forward frequency of a primary bearer. A frequency in Hz.
  double primary_hz;
  // True if `bearer_type` holds a value.
  bool has_bearer_type;
  // The FBearerType name of a primary bearer.
  sdrkit_string bearer_type;
} sdrkit_bgan_beam_row;

// A line of the signalling log. The row key is its place, `000` first.
typedef struct sdrkit_bgan_log_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Seconds into the capture. A length of time in seconds.
  double at;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_bgan_log_row;

// The result fields of the satellite carrier survey.
typedef struct sdrkit_sat_survey_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message.
  sdrkit_string text;
  // The method of the submode.
  sdrkit_string method;
  // The threshold in force. Unit: dB.
  double threshold_in_force_db;
  // True if `bin_hz` holds a value.
  bool has_bin_hz;
  // The bin of the transform. Unit: Hz.
  double bin_hz;
  // Element sets read.
  int64_t catalogue;
  // True if `lines` holds a value.
  bool has_lines;
  // Carrier lines shown.
  int64_t lines;
  // True if `channels_active` holds a value.
  bool has_channels_active;
  // Channels active now.
  int64_t channels_active;
  // True if `channels_seen` holds a value.
  bool has_channels_seen;
  // Channels that have been active.
  int64_t channels_seen;
  // True if `frame_rate_hz` holds a value.
  bool has_frame_rate_hz;
  // The frame rate searched. Unit: Hz.
  double frame_rate_hz;
  // True if `measured_rate_hz` holds a value.
  bool has_measured_rate_hz;
  // The frame rate that fitted best. Unit: Hz.
  double measured_rate_hz;
  // True if `frame_level_db` holds a value.
  bool has_frame_level_db;
  // The level of the frame fit. Unit: dB.
  double frame_level_db;
  // True if `detected` holds a value.
  bool has_detected;
  // The frame structure is detected.
  bool detected;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_sat_survey_fields;

// A carrier line. The row key is the number of the line.
typedef struct sdrkit_sat_survey_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of a carrier line.
  int64_t line;
  // Offset from the centre of the samples. A frequency in Hz.
  double offset_hz;
  // The frequency, MHz. Unit: MHz.
  double mhz;
  // The level over the background or floor. Unit: dB.
  double snr_db;
  // The length of a line. A length of time in seconds.
  double duration_s;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The drift of a line. Unit: Hz/s.
  double drift_hz_s;
  // The line or channel is active.
  bool active;
  // The channel name of a line.
  sdrkit_string channel;
  // The satellite that the Doppler curve names.
  sdrkit_string satellite;
  // True if `norad` holds a value.
  bool has_norad;
  // Its catalogue number.
  int64_t norad;
  // True if `rms_hz` holds a value.
  bool has_rms_hz;
  // The rms of the Doppler match. Unit: Hz.
  double rms_hz;
  // Another satellite fits about as well.
  bool ambiguous;
} sdrkit_sat_survey_carrier_row;

// A channel that has been active. The row key is its grid frequency in whole Hz.
typedef struct sdrkit_sat_survey_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Offset from the centre of the samples. A frequency in Hz.
  double offset_hz;
  // The frequency of a channel. A frequency in Hz.
  double frequency_hz;
  // The width of a channel. Unit: Hz.
  double width_hz;
  // The level over the background or floor. Unit: dB.
  double snr_db;
  // The share of time a channel is on.
  double duty;
  // Times a channel turned active.
  int64_t bursts;
  // The signal time a channel was last on. Unit: s.
  double last_seen_s;
  // The line or channel is active.
  bool active;
  // The name of a channel.
  sdrkit_string name;
} sdrkit_sat_survey_channel_row;

// The items that each decoder component of the stack "inmarsatc" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_inmarsatc_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `demod` holds a value.
  bool has_demod;
  // The items that the component "demod" dropped.
  int64_t demod;
} sdrkit_inmarsatc_dropped;

// The items that each decoder component of the stack "aero" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_aero_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
  // True if `su` holds a value.
  bool has_su;
  // The items that the component "su" dropped.
  int64_t su;
} sdrkit_aero_dropped;

// The items that each decoder component of the stack "inmarsat" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_inmarsat_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
  // True if `aero_display` holds a value.
  bool has_aero_display;
  // The items that the component "aero_display" dropped.
  int64_t aero_display;
  // True if `egc` holds a value.
  bool has_egc;
  // The items that the component "egc" dropped.
  int64_t egc;
} sdrkit_inmarsat_dropped;

// The items that each decoder component of the stack "globalstar" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_globalstar_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `channels` holds a value.
  bool has_channels;
  // The items that the component "channels" dropped.
  int64_t channels;
} sdrkit_globalstar_dropped;

// The items that each decoder component of the stack "goes_dcs" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_goes_dcs_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_goes_dcs_dropped;

// The items that each decoder component of the stack "argos" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_argos_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_argos_dropped;

// The items that each decoder component of the stack "hrdcp" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_hrdcp_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_hrdcp_dropped;

// The items that each decoder component of the stack "sarsat" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_sarsat_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rx` holds a value.
  bool has_rx;
  // The items that the component "rx" dropped.
  int64_t rx;
} sdrkit_sarsat_dropped;

// The items that each decoder component of the stack "lora_sat" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_lora_sat_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
} sdrkit_lora_sat_dropped;

// The items that each decoder component of the stack "orbcomm" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_orbcomm_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
} sdrkit_orbcomm_dropped;

// The items that each decoder component of the stack "psk_demod" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_psk_demod_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
  // True if `bits` holds a value.
  bool has_bits;
  // The items that the component "bits" dropped.
  int64_t bits;
} sdrkit_psk_demod_dropped;

// The items that each decoder component of the stack "qo100" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_qo100_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
} sdrkit_qo100_dropped;

// The items that each decoder component of the stack "cubesat" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_cubesat_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `receiver` holds a value.
  bool has_receiver;
  // The items that the component "receiver" dropped.
  int64_t receiver;
  // True if `naming` holds a value.
  bool has_naming;
  // The items that the component "naming" dropped.
  int64_t naming;
} sdrkit_cubesat_dropped;

// The options of `family` of `sdrkit_inmarsatc_egc_event`.
typedef enum sdrkit_inmarsatc_egc_event_family {
  // Aircraft
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_INMARSATC_EGC_EVENT_FAMILY_OTHER = 12,
} sdrkit_inmarsatc_egc_event_family;

// The options of `urgency` of `sdrkit_inmarsatc_egc_event`.
typedef enum sdrkit_inmarsatc_egc_event_urgency {
  // None
  SDRKIT_INMARSATC_EGC_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_INMARSATC_EGC_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_INMARSATC_EGC_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_INMARSATC_EGC_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_inmarsatc_egc_event_urgency;

// A complete EGC message. The source is the message number and the service, the text the message.
typedef struct sdrkit_inmarsatc_egc_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The message number and the service.
  sdrkit_string source;
  // The text of the message.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_inmarsatc_egc_event_family`.
  int32_t family;
  // How urgent the message is, from its priority. A value of `sdrkit_inmarsatc_egc_event_urgency`.
  int32_t urgency;
  // The message number.
  int64_t message_number;
  // The EGC service code.
  int64_t service_code;
  // The EGC priority code (0 routine, 1 safety, 2 urgency, 3 distress).
  int64_t priority_code;
  // The subject of a message.
  sdrkit_string subject;
  // The area of a message.
  sdrkit_string area;
} sdrkit_inmarsatc_egc_event;

// The options of `family` of `sdrkit_aero_message_event`.
typedef enum sdrkit_aero_message_event_family {
  // Aircraft
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AERO_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_aero_message_event_family;

// An ACARS message.
typedef struct sdrkit_aero_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The registration. Absent when the message names none.
  sdrkit_string source;
  // The text.
  sdrkit_string text;
  // What the event is about: an aircraft. A value of `sdrkit_aero_message_event_family`.
  int32_t family;
  // True if `position` holds a value.
  bool has_position;
  // The position in the message.
  sdrkit_position position;
  // The ACARS label.
  sdrkit_string label;
  // The AES id, six hexadecimal digits.
  sdrkit_string aes;
  // The flight id.
  sdrkit_string flight;
  // The block check is good.
  bool check_ok;
} sdrkit_aero_message_event;

// The options of `family` of `sdrkit_aero_logon_event`.
typedef enum sdrkit_aero_logon_event_family {
  // Aircraft
  SDRKIT_AERO_LOGON_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AERO_LOGON_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AERO_LOGON_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AERO_LOGON_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AERO_LOGON_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AERO_LOGON_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AERO_LOGON_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AERO_LOGON_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AERO_LOGON_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AERO_LOGON_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AERO_LOGON_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AERO_LOGON_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AERO_LOGON_EVENT_FAMILY_OTHER = 12,
} sdrkit_aero_logon_event_family;

// A log-on or log-off.
typedef struct sdrkit_aero_logon_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The AES id, six hexadecimal digits.
  sdrkit_string source;
  // `log-on` or `log-off`.
  sdrkit_string text;
  // What the event is about: an aircraft. A value of `sdrkit_aero_logon_event_family`.
  int32_t family;
  // The GES id, two hexadecimal digits.
  sdrkit_string ges;
  // A log-on (else a log-off).
  bool logon;
} sdrkit_aero_logon_event;

// The options of `family` of `sdrkit_iridium_raw_event`.
typedef enum sdrkit_iridium_raw_event_family {
  // Aircraft
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_IRIDIUM_RAW_EVENT_FAMILY_OTHER = 12,
} sdrkit_iridium_raw_event_family;

// A RAW line of one burst.
typedef struct sdrkit_iridium_raw_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_iridium_raw_event_family`.
  int32_t family;
  // Confidence. Unit: %.
  double confidence_pct;
} sdrkit_iridium_raw_event;

// The options of `family` of `sdrkit_globalstar_activity_event`.
typedef enum sdrkit_globalstar_activity_event_family {
  // Aircraft
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_GLOBALSTAR_ACTIVITY_EVENT_FAMILY_OTHER = 12,
} sdrkit_globalstar_activity_event_family;

// A stretch of activity on a channel. The source is the channel ("channel 7").
typedef struct sdrkit_globalstar_activity_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The channel ("channel 7").
  sdrkit_string source;
  // The length and the Doppler shift of the stretch.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The peak signal to noise ratio of the stretch. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_globalstar_activity_event_family`.
  int32_t family;
  // A channel number, 1 to 13.
  int64_t channel;
  // How long a stretch of activity lasted. A length of time in seconds.
  double duration;
} sdrkit_globalstar_activity_event;

// The options of `family` of `sdrkit_goes_dcs_report_event`.
typedef enum sdrkit_goes_dcs_report_event_family {
  // Aircraft
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_GOES_DCS_REPORT_EVENT_FAMILY_OTHER = 12,
} sdrkit_goes_dcs_report_event_family;

// The options of `format` of `sdrkit_goes_dcs_report_event`.
typedef enum sdrkit_goes_dcs_report_event_format {
  // ASCII
  SDRKIT_GOES_DCS_REPORT_EVENT_FORMAT_ASCII = 0,
  // Pseudo-binary B
  SDRKIT_GOES_DCS_REPORT_EVENT_FORMAT_PSEUDO_BINARY = 1,
  // Other
  SDRKIT_GOES_DCS_REPORT_EVENT_FORMAT_OTHER = 2,
} sdrkit_goes_dcs_report_event_format;

// A platform report. The source is the address, the text the data.
typedef struct sdrkit_goes_dcs_report_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address of the platform.
  sdrkit_string source;
  // The data of the report.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_goes_dcs_report_event_family`.
  int32_t family;
  // The address word (8 hex digits).
  sdrkit_string address;
  // The channel number.
  int64_t channel;
  // The bit rate. A frequency in Hz.
  double bit_rate_hz;
  // Manchester coded.
  bool manchester;
  // The flag byte, or -1.
  int64_t flags;
  // The data format. A value of `sdrkit_goes_dcs_report_event_format`.
  int32_t format;
  // The report ended with an EOT.
  bool eot;
  // Characters whose parity failed.
  int64_t parity_errors;
  // The address bits corrected.
  int64_t bits_corrected;
} sdrkit_goes_dcs_report_event;

// The options of `family` of `sdrkit_argos_ptt_event`.
typedef enum sdrkit_argos_ptt_event_family {
  // Aircraft
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ARGOS_PTT_EVENT_FAMILY_OTHER = 12,
} sdrkit_argos_ptt_event_family;

// A platform message. The source is the platform id, the text the data in hexadecimal.
typedef struct sdrkit_argos_ptt_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The platform id.
  sdrkit_string source;
  // The data in hexadecimal.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_argos_ptt_event_family`.
  int32_t family;
  // The platform id (7 hex digits).
  sdrkit_string platform;
  // The number of user data bits.
  int64_t data_bits;
  // The carrier from the dial. A frequency in Hz.
  double offset_hz;
  // The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
  double quality;
  // The user data, the first bit the high bit of the first byte.
  sdrkit_bytes data;
} sdrkit_argos_ptt_event;

// The options of `family` of `sdrkit_argos_downlink_frame_event`.
typedef enum sdrkit_argos_downlink_frame_event_family {
  // Aircraft
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_argos_downlink_frame_event_family;

// The options of `coding` of `sdrkit_argos_downlink_frame_event`.
typedef enum sdrkit_argos_downlink_frame_event_coding {
  // NRZ
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_CODING_NRZ = 0,
  // Inverted NRZ
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_CODING_INVERTED_NRZ = 1,
  // NRZI
  SDRKIT_ARGOS_DOWNLINK_FRAME_EVENT_CODING_NRZI = 2,
} sdrkit_argos_downlink_frame_event_coding;

// A downlink frame with a good FCS.
typedef struct sdrkit_argos_downlink_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The kind of thing that the event is about. A value of `sdrkit_argos_downlink_frame_event_family`.
  int32_t family;
  // The downlink coding in use. A value of `sdrkit_argos_downlink_frame_event_coding`.
  int32_t coding;
  // The length of a frame without its FCS.
  int64_t bytes;
  // A frame's bytes.
  sdrkit_bytes frame;
} sdrkit_argos_downlink_frame_event;

// The options of `family` of `sdrkit_hrdcp_report_event`.
typedef enum sdrkit_hrdcp_report_event_family {
  // Aircraft
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HRDCP_REPORT_EVENT_FAMILY_OTHER = 12,
} sdrkit_hrdcp_report_event_family;

// A message received whole with a good CRC. The source is the address, the text the data.
typedef struct sdrkit_hrdcp_report_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address of the platform.
  sdrkit_string source;
  // The data of the message.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_hrdcp_report_event_family`.
  int32_t family;
  // The address word (8 hex digits).
  sdrkit_string address;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel number of the plan.
  int64_t channel;
  // The sequence counter.
  int64_t sequence;
  // Self-timed or alert.
  sdrkit_string type;
  // The 10 health bits.
  sdrkit_string health;
  // The compression of the data.
  sdrkit_string compression;
  // Platform data bytes.
  int64_t bytes;
  // Reed-Solomon bytes corrected.
  int64_t rs_corrected;
  // The CRC passed.
  bool crc_ok;
} sdrkit_hrdcp_report_event;

// The options of `family` of `sdrkit_sarsat_beacon_event`.
typedef enum sdrkit_sarsat_beacon_event_family {
  // Aircraft
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SARSAT_BEACON_EVENT_FAMILY_OTHER = 12,
} sdrkit_sarsat_beacon_event_family;

// The options of `urgency` of `sdrkit_sarsat_beacon_event`.
typedef enum sdrkit_sarsat_beacon_event_urgency {
  // None
  SDRKIT_SARSAT_BEACON_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_SARSAT_BEACON_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_SARSAT_BEACON_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_SARSAT_BEACON_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_sarsat_beacon_event_urgency;

// A beacon decoded. The source is the hex identity. A distress beacon writes the urgency emergency, a test beacon none.
typedef struct sdrkit_sarsat_beacon_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The hex identity.
  sdrkit_string source;
  // What the beacon says.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_sarsat_beacon_event_family`.
  int32_t family;
  // How urgent the beacon is: emergency for a distress beacon. A value of `sdrkit_sarsat_beacon_event_urgency`.
  int32_t urgency;
  // True if `position` holds a value.
  bool has_position;
  // The position that the beacon sends.
  sdrkit_position position;
  // The beacon generation, 1 or 2.
  int64_t generation;
  // The beacon's hex identity (15 digits, or 23 for the second generation).
  sdrkit_string hex_id;
  // The beacon type (EPIRB, ELT, PLB ...).
  sdrkit_string beacon_type;
  // The country of the beacon's MID.
  sdrkit_string country;
  // The country code (MID).
  int64_t mid;
  // The protocol in words.
  sdrkit_string protocol;
  // The identification in words.
  sdrkit_string identification;
  // The 121.5 MHz homing flag.
  bool homing;
  // The self-test or test-protocol flag.
  bool self_test;
  // The bits corrected.
  int64_t bits_corrected;
  // The offset of the carrier from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // The message bits as hexadecimal digits (as received after the repair).
  sdrkit_string message_hex;
} sdrkit_sarsat_beacon_event;

// The options of `family` of `sdrkit_lora_sat_frame_event`.
typedef enum sdrkit_lora_sat_frame_event_family {
  // Aircraft
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_LORA_SAT_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_lora_sat_frame_event_family;

// A frame with a good CRC. The source is the satellite overhead, the text the payload as printable text.
typedef struct sdrkit_lora_sat_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The satellite overhead.
  sdrkit_string source;
  // The payload as printable text.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_lora_sat_frame_event_family`.
  int32_t family;
  // The spreading factor of a frame.
  int64_t sf;
  // The payload of a frame.
  sdrkit_bytes payload;
  // The carrier offset from the dial at the first data symbol. A frequency in Hz.
  double doppler_hz;
  // True if `doppler_rate_hz_s` holds a value.
  bool has_doppler_rate_hz_s;
  // The carrier rate during a frame. Unit: Hz/s.
  double doppler_rate_hz_s;
} sdrkit_lora_sat_frame_event;

// The options of `family` of `sdrkit_orbcomm_message_event`.
typedef enum sdrkit_orbcomm_message_event_family {
  // Aircraft
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ORBCOMM_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_orbcomm_message_event_family;

// A message joined from its parts. The source is the satellite ("satellite 12").
typedef struct sdrkit_orbcomm_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The satellite ("satellite 12").
  sdrkit_string source;
  // The text of the message.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_orbcomm_message_event_family`.
  int32_t family;
  // The message number of a joined message.
  int64_t message_id;
  // Whether every part of a joined message came.
  bool complete;
} sdrkit_orbcomm_message_event;

// The options of `family` of `sdrkit_psk_demod_lock_event`.
typedef enum sdrkit_psk_demod_lock_event_family {
  // Aircraft
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PSK_DEMOD_LOCK_EVENT_FAMILY_OTHER = 12,
} sdrkit_psk_demod_lock_event_family;

// The options of `order` of `sdrkit_psk_demod_lock_event`.
typedef enum sdrkit_psk_demod_lock_event_order {
  // Found by the analyser
  SDRKIT_PSK_DEMOD_LOCK_EVENT_ORDER_AUTO = 0,
  // BPSK
  SDRKIT_PSK_DEMOD_LOCK_EVENT_ORDER_BPSK = 1,
  // QPSK
  SDRKIT_PSK_DEMOD_LOCK_EVENT_ORDER_QPSK = 2,
  // OQPSK
  SDRKIT_PSK_DEMOD_LOCK_EVENT_ORDER_OQPSK = 3,
  // 8PSK
  SDRKIT_PSK_DEMOD_LOCK_EVENT_ORDER_8PSK = 4,
} sdrkit_psk_demod_lock_event_order;

// A signal was locked.
typedef struct sdrkit_psk_demod_lock_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The order and the symbol rate.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_psk_demod_lock_event_family`.
  int32_t family;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `order` holds a value.
  bool has_order;
  // The modulation order the demodulator runs. A value of `sdrkit_psk_demod_lock_event_order`.
  int32_t order;
} sdrkit_psk_demod_lock_event;

// The options of `family` of `sdrkit_psk_demod_bytes_event`.
typedef enum sdrkit_psk_demod_bytes_event_family {
  // Aircraft
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_PSK_DEMOD_BYTES_EVENT_FAMILY_OTHER = 12,
} sdrkit_psk_demod_bytes_event_family;

// Bits packed into bytes for the host. The text is the bytes in hexadecimal.
typedef struct sdrkit_psk_demod_bytes_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The bytes in hexadecimal.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_psk_demod_bytes_event_family`.
  int32_t family;
  // The bytes of a bytes event.
  sdrkit_bytes bytes;
} sdrkit_psk_demod_bytes_event;

// The options of `family` of `sdrkit_qo100_lock_event`.
typedef enum sdrkit_qo100_lock_event_family {
  // Aircraft
  SDRKIT_QO100_LOCK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_QO100_LOCK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_QO100_LOCK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_QO100_LOCK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_QO100_LOCK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_QO100_LOCK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_QO100_LOCK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_QO100_LOCK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_QO100_LOCK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_QO100_LOCK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_QO100_LOCK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_QO100_LOCK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_QO100_LOCK_EVENT_FAMILY_OTHER = 12,
} sdrkit_qo100_lock_event_family;

// The beacons were locked.
typedef struct sdrkit_qo100_lock_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The LNB error at the lock.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_qo100_lock_event_family`.
  int32_t family;
  // The LNB's LO error (actual minus nominal). A frequency in Hz.
  double lnb_error_hz;
  // The LNB error in parts per million of the nominal LO. Unit: ppm.
  double lnb_error_ppm;
} sdrkit_qo100_lock_event;

// The options of `family` of `sdrkit_qo100_conversation_event`.
typedef enum sdrkit_qo100_conversation_event_family {
  // Aircraft
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_QO100_CONVERSATION_EVENT_FAMILY_OTHER = 12,
} sdrkit_qo100_conversation_event_family;

// A conversation started. The channel is the conversation number.
typedef struct sdrkit_qo100_conversation_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // "conversation started".
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_qo100_conversation_event_family`.
  int32_t family;
  // The channel.
  sdrkit_string channel;
  // The conversation of a track.
  int64_t conversation;
  // The USB dial for listening (RF). A frequency in Hz.
  double dial_hz;
} sdrkit_qo100_conversation_event;

// The options of `family` of `sdrkit_cubesat_frame_event`.
typedef enum sdrkit_cubesat_frame_event_family {
  // Aircraft
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CUBESAT_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_cubesat_frame_event_family;

// The options of `framing` of `sdrkit_cubesat_frame_event`.
typedef enum sdrkit_cubesat_frame_event_framing {
  // AX.25 AFSK
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_AX25_AFSK = 0,
  // AX.25 G3RUH
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_AX25_G3RUH = 1,
  // AX.25 NRZI
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_AX25_NRZI = 2,
  // AX100 Mode 5
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_AX100_MODE5 = 3,
  // AX100 Mode 6
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_AX100_MODE6 = 4,
  // CCSDS RS
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_CCSDS_RS = 5,
  // Geoscan
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_GEOSCAN = 6,
  // Lucky-7
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_LUCKY7 = 7,
  // Spino
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_SPINO = 8,
  // Erminaz
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_ERMINAZ = 9,
  // USP
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_USP = 10,
  // NGHam
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_NGHAM = 11,
  // U482C
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_U482C = 12,
  // Mobitex
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_MOBITEX = 13,
  // Mobitex-NX
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_MOBITEX_NX = 14,
  // BEESAT-1
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_BEESAT1 = 15,
  // BEESAT-9
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_BEESAT9 = 16,
  // CCSDS concatenated
  SDRKIT_CUBESAT_FRAME_EVENT_FRAMING_CCSDS_CONCAT = 17,
} sdrkit_cubesat_frame_event_framing;

// A frame. The source is the satellite, the text the text beacon or nothing.
typedef struct sdrkit_cubesat_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The satellite.
  sdrkit_string source;
  // True if `text` holds a value.
  bool has_text;
  // The text beacon.
  sdrkit_string text;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The kind of thing that the event is about. A value of `sdrkit_cubesat_frame_event_family`.
  int32_t family;
  // The framing of a frame. A value of `sdrkit_cubesat_frame_event_framing`.
  int32_t framing;
  // Its NORAD number.
  int64_t norad;
  // The frame's data.
  sdrkit_bytes data;
} sdrkit_cubesat_frame_event;

// The options of `family` of `sdrkit_cubesat_kiss_event`.
typedef enum sdrkit_cubesat_kiss_event_family {
  // Aircraft
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CUBESAT_KISS_EVENT_FAMILY_OTHER = 12,
} sdrkit_cubesat_kiss_event_family;

// A frame in KISS form for a packet program. The source is the satellite.
typedef struct sdrkit_cubesat_kiss_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The satellite.
  sdrkit_string source;
  // The kind of thing that the event is about. A value of `sdrkit_cubesat_kiss_event_family`.
  int32_t family;
  // The KISS frame.
  sdrkit_bytes kiss_frame;
} sdrkit_cubesat_kiss_event;

// The options of `family` of `sdrkit_cubesat_tx_event`.
typedef enum sdrkit_cubesat_tx_event_family {
  // Aircraft
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CUBESAT_TX_EVENT_FAMILY_OTHER = 12,
} sdrkit_cubesat_tx_event_family;

// The fields of a change of an AX.25 transmission.
typedef struct sdrkit_cubesat_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The state: started, queued, finished or refused.
  sdrkit_string state;
  // Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
  sdrkit_string reason;
  // The destination and the digipeaters of the frames, separated by commas.
  sdrkit_string target;
  // The text or the hexadecimal bytes of the frame for started, queued and refused; empty for finished.
  sdrkit_string text;
  // The family of the source: a satellite. A value of `sdrkit_cubesat_tx_event_family`.
  int32_t family;
} sdrkit_cubesat_tx_event;

// A change of the state of the receiver.
typedef struct sdrkit_fleet_broadcast_state_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The state: 0 nothing, 1 carrier, 2 bits, 3 framed.
  int64_t state;
} sdrkit_fleet_broadcast_state_event;

// A line of decoded signalling.
typedef struct sdrkit_bgan_signalling_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_bgan_signalling_event;

// A finished carrier line.
typedef struct sdrkit_sat_survey_carrier_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The number of a carrier line.
  int64_t line;
  // Offset from the centre of the samples. A frequency in Hz.
  double offset_hz;
  // The frequency, MHz. Unit: MHz.
  double mhz;
  // The level over the background or floor. Unit: dB.
  double snr_db;
  // The length of a line. A length of time in seconds.
  double duration_s;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The drift of a line. Unit: Hz/s.
  double drift_hz_s;
  // The channel name of a line.
  sdrkit_string channel;
  // The satellite that the Doppler curve names.
  sdrkit_string satellite;
  // True if `norad` holds a value.
  bool has_norad;
  // Its catalogue number.
  int64_t norad;
  // True if `rms_hz` holds a value.
  bool has_rms_hz;
  // The rms of the Doppler match. Unit: Hz.
  double rms_hz;
  // Another satellite fits about as well.
  bool ambiguous;
} sdrkit_sat_survey_carrier_event;

// The arguments of `remove` of the Aero stack.
typedef struct sdrkit_aero_aircraft_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the aircraft.
  sdrkit_string row;
} sdrkit_aero_aircraft_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_AERO_AIRCRAFT_ARGS_INIT {sizeof(sdrkit_aero_aircraft_args), {0}}

// The arguments of `carrier.focus`.
typedef struct sdrkit_inmarsat_carrier_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of a carrier, or empty for none.
  sdrkit_string row;
} sdrkit_inmarsat_carrier_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_INMARSAT_CARRIER_ARGS_INIT {sizeof(sdrkit_inmarsat_carrier_args), {0}}

// The arguments of `remove` of GOES DCS.
typedef struct sdrkit_goes_dcs_platform_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the platform.
  sdrkit_string row;
} sdrkit_goes_dcs_platform_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_GOES_DCS_PLATFORM_ARGS_INIT {sizeof(sdrkit_goes_dcs_platform_args), {0}}

// The arguments of `remove` of Argos.
typedef struct sdrkit_argos_platform_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the platform.
  sdrkit_string row;
} sdrkit_argos_platform_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ARGOS_PLATFORM_ARGS_INIT {sizeof(sdrkit_argos_platform_args), {0}}

// The arguments of `remove` of COSPAS-SARSAT.
typedef struct sdrkit_sarsat_beacon_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the beacon.
  sdrkit_string row;
} sdrkit_sarsat_beacon_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SARSAT_BEACON_ARGS_INIT {sizeof(sdrkit_sarsat_beacon_args), {0}}

// The arguments of `preset.apply` of COSPAS-SARSAT.
typedef struct sdrkit_sarsat_preset_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `preset` holds a value.
  bool has_preset;
  // The dial preset. The index of an option.
  int32_t preset;
} sdrkit_sarsat_preset_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SARSAT_PRESET_ARGS_INIT {sizeof(sdrkit_sarsat_preset_args), 0, 0}

// The arguments of `preset.apply` of Orbcomm.
typedef struct sdrkit_orbcomm_channel_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel preset. The index of an option.
  int32_t channel;
} sdrkit_orbcomm_channel_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ORBCOMM_CHANNEL_ARGS_INIT {sizeof(sdrkit_orbcomm_channel_args), 0, 0}

// The arguments of `remove` of Orbcomm.
typedef struct sdrkit_orbcomm_satellite_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the satellite.
  sdrkit_string row;
} sdrkit_orbcomm_satellite_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ORBCOMM_SATELLITE_ARGS_INIT {sizeof(sdrkit_orbcomm_satellite_args), {0}}

// The arguments of `speech.text` of QO-100.
typedef struct sdrkit_qo100_speech_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text that the host's speech-to-text heard.
  sdrkit_string text;
} sdrkit_qo100_speech_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_QO100_SPEECH_ARGS_INIT {sizeof(sdrkit_qo100_speech_args), {0}}

// The arguments of `conversation.listen` of QO-100.
typedef struct sdrkit_qo100_listen_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of a conversation, or empty for the dial.
  sdrkit_string row;
} sdrkit_qo100_listen_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_QO100_LISTEN_ARGS_INIT {sizeof(sdrkit_qo100_listen_args), {0}}

// The arguments of `remove` of QO-100.
typedef struct sdrkit_qo100_conversation_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the conversation.
  sdrkit_string row;
} sdrkit_qo100_conversation_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_QO100_CONVERSATION_ARGS_INIT {sizeof(sdrkit_qo100_conversation_args), {0}}

// The arguments of `ptt`.
typedef struct sdrkit_qo100_ptt_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Push-to-talk pressed (on) or released (off).
  bool on;
} sdrkit_qo100_ptt_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_QO100_PTT_ARGS_INIT {sizeof(sdrkit_qo100_ptt_args), 0}

// The arguments of the cubesat `send`.
typedef struct sdrkit_cubesat_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `destination` holds a value.
  bool has_destination;
  // The destination address.
  sdrkit_string destination;
  // True if `path` holds a value.
  bool has_path;
  // The digipeaters.
  sdrkit_string path;
  // True if `text` holds a value.
  bool has_text;
  // The information as text.
  sdrkit_string text;
  // True if `hex` holds a value.
  bool has_hex;
  // The information as bytes.
  sdrkit_string hex;
  // True if `pid` holds a value.
  bool has_pid;
  // The protocol identifier.
  int64_t pid;
} sdrkit_cubesat_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_CUBESAT_SEND_ARGS_INIT {sizeof(sdrkit_cubesat_send_args), 0, {0}, 0, {0}, 0, {0}, 0, {0}, 0, 0}

// Fills `rows` with the changed rows of the table "messages" of the stack "inmarsatc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsatc_messages_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_inmarsatc_message_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatcMessageRow", "messages", rows, sizeof(sdrkit_inmarsatc_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "inmarsatc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsatc_channels_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_inmarsatc_channel_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatcChannelRow", "channels", rows, sizeof(sdrkit_inmarsatc_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packet_counts" of the stack "inmarsatc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsatc_packet_counts_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_inmarsatc_packet_count_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatcPacketCountRow", "packet_counts", rows, sizeof(sdrkit_inmarsatc_packet_count_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "inmarsatc.egc"
// (EGC message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_inmarsatc_egc_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_inmarsatc_egc_event *event) {
  return sdrkit_typed_event(handle, n, "inmarsatc.egc", "satcom.InmarsatcEgcEvent", event, sizeof(sdrkit_inmarsatc_egc_event));
}

// Runs the command "clear" (Clear) of the stack "inmarsatc" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_inmarsatc_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "inmarsatc" are the sdrkit_inmarsatc_gen_*() functions
// after the settings of its receiver.

// The setting "channel_search" (Channel search) of the stack "inmarsatc". Look for up to 3 more channels in the capture.
static inline int32_t sdrkit_inmarsatc_get_channel_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "channel_search", value);
}
static inline int32_t sdrkit_inmarsatc_set_channel_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "channel_search", value);
}

// The setting "search_span_hz" (Search span) of the stack "inmarsatc". A frequency in Hz. Limit of the channel search around 0 Hz (0: the whole input).
static inline int32_t sdrkit_inmarsatc_get_search_span_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_span_hz", value);
}
static inline int32_t sdrkit_inmarsatc_set_search_span_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_span_hz", value);
}

// The setting "satellite" (Satellite) of the stack "inmarsatc". A value of `sdrkit_inmarsatc_satellite`. Asks the host to tune the dial to the NCS common channel of this satellite.
static inline int32_t sdrkit_inmarsatc_get_satellite(SDRKIT_HANDLE handle, sdrkit_inmarsatc_satellite *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "satellite", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_inmarsatc_satellite)index;
  return code;
}
static inline int32_t sdrkit_inmarsatc_set_satellite(SDRKIT_HANDLE handle, sdrkit_inmarsatc_satellite value) {
  return sdrkit_typed_setting_set_enum(handle, "satellite", (int32_t)value);
}

// The setting "min_priority" (Priority) of the stack "inmarsatc". A value of `sdrkit_inmarsatc_min_priority`. Shows the messages of this priority and higher.
static inline int32_t sdrkit_inmarsatc_get_min_priority(SDRKIT_HANDLE handle, sdrkit_inmarsatc_min_priority *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "min_priority", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_inmarsatc_min_priority)index;
  return code;
}
static inline int32_t sdrkit_inmarsatc_set_min_priority(SDRKIT_HANDLE handle, sdrkit_inmarsatc_min_priority value) {
  return sdrkit_typed_setting_set_enum(handle, "min_priority", (int32_t)value);
}

// The setting "service_filter" (Service) of the stack "inmarsatc". A value of `sdrkit_inmarsatc_service_filter`. Shows the messages of this service only.
static inline int32_t sdrkit_inmarsatc_get_service_filter(SDRKIT_HANDLE handle, sdrkit_inmarsatc_service_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "service_filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_inmarsatc_service_filter)index;
  return code;
}
static inline int32_t sdrkit_inmarsatc_set_service_filter(SDRKIT_HANDLE handle, sdrkit_inmarsatc_service_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "service_filter", (int32_t)value);
}

// The setting "gen_invert" (Inverted polarity) of the test signal generator of the stack "inmarsatc".
static inline int32_t sdrkit_inmarsatc_gen_get_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert", value);
}
static inline int32_t sdrkit_inmarsatc_gen_set_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert", value);
}

// The setting "gen_first_frame" (First frame number) of the test signal generator of the stack "inmarsatc".
static inline int32_t sdrkit_inmarsatc_gen_get_first_frame(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_first_frame", value);
}
static inline int32_t sdrkit_inmarsatc_gen_set_first_frame(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_first_frame", value);
}

// The setting "gen_extra" (Extra channels) of the test signal generator of the stack "inmarsatc".
static inline int32_t sdrkit_inmarsatc_gen_get_extra(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_extra", value);
}
static inline int32_t sdrkit_inmarsatc_gen_set_extra(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_extra", value);
}

// The setting "gen_start_symbols" (Start into the first frame) of the test signal generator of the stack "inmarsatc". Unit: symbols.
static inline int32_t sdrkit_inmarsatc_gen_get_start_symbols(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_start_symbols", value);
}
static inline int32_t sdrkit_inmarsatc_gen_set_start_symbols(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_start_symbols", value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "aero" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aero_channels_rows(SDRKIT_HANDLE handle,
                                               sdrkit_aero_channel_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroChannelRow", "channels", rows, sizeof(sdrkit_aero_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "su_types" of the stack "aero" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aero_su_types_rows(SDRKIT_HANDLE handle,
                                               sdrkit_aero_su_type_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroSuTypeRow", "su_types", rows, sizeof(sdrkit_aero_su_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "logons" of the stack "aero" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aero_logons_rows(SDRKIT_HANDLE handle,
                                             sdrkit_aero_logon_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroLogonRow", "logons", rows, sizeof(sdrkit_aero_logon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "aero" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aero_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_aero_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroMessageRow", "messages", rows, sizeof(sdrkit_aero_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "aero" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aero_aircraft_rows(SDRKIT_HANDLE handle,
                                               sdrkit_aero_aircraft_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroAircraftRow", "aircraft", rows, sizeof(sdrkit_aero_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "facts" of the stack "aero" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_aero_facts_rows(SDRKIT_HANDLE handle,
                                            sdrkit_aero_fact_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroFactRow", "facts", rows, sizeof(sdrkit_aero_fact_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "aero.message"
// (ACARS message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_aero_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_aero_message_event *event) {
  return sdrkit_typed_event(handle, n, "aero.message", "satcom.AeroMessageEvent", event, sizeof(sdrkit_aero_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "aero.logon"
// (Log-on). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_aero_logon_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_aero_logon_event *event) {
  return sdrkit_typed_event(handle, n, "aero.logon", "satcom.AeroLogonEvent", event, sizeof(sdrkit_aero_logon_event));
}

// Runs the command "clear" (Clear) of the stack "aero" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_aero_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "aero" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_aero_remove(SDRKIT_HANDLE handle, const sdrkit_aero_aircraft_args *args) {
  return sdrkit_typed_command(handle, "remove", "satcom.AeroAircraftArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "aero" are the sdrkit_aero_gen_*() functions
// after the settings of its receiver.

// The setting "search_limit_hz" (Search limit) of the stack "aero". A frequency in Hz. The band search looks only within +-n Hz (0: the whole input).
static inline int32_t sdrkit_aero_get_search_limit_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_limit_hz", value);
}
static inline int32_t sdrkit_aero_set_search_limit_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_limit_hz", value);
}

// The setting "satellite" (Satellite) of the stack "aero". A value of `sdrkit_aero_satellite`. Asks the host to tune the dial to the middle of the satellite's two P channel groups.
static inline int32_t sdrkit_aero_get_satellite(SDRKIT_HANDLE handle, sdrkit_aero_satellite *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "satellite", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_aero_satellite)index;
  return code;
}
static inline int32_t sdrkit_aero_set_satellite(SDRKIT_HANDLE handle, sdrkit_aero_satellite value) {
  return sdrkit_typed_setting_set_enum(handle, "satellite", (int32_t)value);
}

// The setting "filter_label" (Label filter) of the stack "aero". Shows the messages whose label holds this text.
static inline int32_t sdrkit_aero_get_filter_label(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "filter_label", value);
}
static inline int32_t sdrkit_aero_set_filter_label(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "filter_label", value);
}

// The setting "filter_registration" (Registration filter) of the stack "aero". Shows the messages whose registration holds this text.
static inline int32_t sdrkit_aero_get_filter_registration(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "filter_registration", value);
}
static inline int32_t sdrkit_aero_set_filter_registration(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "filter_registration", value);
}

// The setting "show_failed" (Show failed messages) of the stack "aero". Shows messages whose block check failed too.
static inline int32_t sdrkit_aero_get_show_failed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "show_failed", value);
}
static inline int32_t sdrkit_aero_set_show_failed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "show_failed", value);
}

// The setting "gen_channels" (Channels (1 = 600, 2 = 1200, 4 = 10500)) of the test signal generator of the stack "aero".
static inline int32_t sdrkit_aero_gen_get_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_channels", value);
}
static inline int32_t sdrkit_aero_gen_set_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_channels", value);
}

// The setting "gen_plain" (No built-in offsets and drift) of the test signal generator of the stack "aero".
static inline int32_t sdrkit_aero_gen_get_plain(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_plain", value);
}
static inline int32_t sdrkit_aero_gen_set_plain(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_plain", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_carriers_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_inmarsat_carrier_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatCarrierRow", "carriers", rows, sizeof(sdrkit_inmarsat_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "candidates" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_candidates_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_inmarsat_candidate_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatCandidateRow", "candidates", rows, sizeof(sdrkit_inmarsat_candidate_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "candidate_reasons" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_candidate_reasons_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_inmarsat_candidate_reason_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatCandidateReasonRow", "candidate_reasons", rows, sizeof(sdrkit_inmarsat_candidate_reason_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aero_display.channels" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_aero_display_channels_rows(SDRKIT_HANDLE handle,
                                                                sdrkit_aero_channel_row *rows,
                                                                uint32_t capacity,
                                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroChannelRow", "aero_display.channels", rows, sizeof(sdrkit_aero_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aero_display.su_types" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_aero_display_su_types_rows(SDRKIT_HANDLE handle,
                                                                sdrkit_aero_su_type_row *rows,
                                                                uint32_t capacity,
                                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroSuTypeRow", "aero_display.su_types", rows, sizeof(sdrkit_aero_su_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aero_display.logons" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_aero_display_logons_rows(SDRKIT_HANDLE handle,
                                                              sdrkit_aero_logon_row *rows,
                                                              uint32_t capacity,
                                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroLogonRow", "aero_display.logons", rows, sizeof(sdrkit_aero_logon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aero_display.messages" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_aero_display_messages_rows(SDRKIT_HANDLE handle,
                                                                sdrkit_aero_message_row *rows,
                                                                uint32_t capacity,
                                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroMessageRow", "aero_display.messages", rows, sizeof(sdrkit_aero_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aero_display.aircraft" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_aero_display_aircraft_rows(SDRKIT_HANDLE handle,
                                                                sdrkit_aero_aircraft_row *rows,
                                                                uint32_t capacity,
                                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroAircraftRow", "aero_display.aircraft", rows, sizeof(sdrkit_aero_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aero_display.facts" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_aero_display_facts_rows(SDRKIT_HANDLE handle,
                                                             sdrkit_aero_fact_row *rows,
                                                             uint32_t capacity,
                                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.AeroFactRow", "aero_display.facts", rows, sizeof(sdrkit_aero_fact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "egc.messages" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_egc_messages_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_inmarsatc_message_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatcMessageRow", "egc.messages", rows, sizeof(sdrkit_inmarsatc_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "egc.channels" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_egc_channels_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_inmarsatc_channel_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatcChannelRow", "egc.channels", rows, sizeof(sdrkit_inmarsatc_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "egc.packet_counts" of the stack "inmarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_inmarsat_egc_packet_counts_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_inmarsatc_packet_count_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.InmarsatcPacketCountRow", "egc.packet_counts", rows, sizeof(sdrkit_inmarsatc_packet_count_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "inmarsat" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_inmarsat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "carrier.focus" (Focus on carrier) of the stack "inmarsat" on the session of `handle`. Never parks the window of this carrier. An empty row ends the focus.
static inline int32_t sdrkit_inmarsat_carrier_focus(SDRKIT_HANDLE handle, const sdrkit_inmarsat_carrier_args *args) {
  return sdrkit_typed_command(handle, "carrier.focus", "satcom.InmarsatCarrierArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "inmarsat" are the sdrkit_inmarsat_gen_*() functions
// after the settings of its receiver.

// The setting "inmarsat_c" (Inmarsat-C) of the stack "inmarsat". Runs Inmarsat-C decoders on medium carriers.
static inline int32_t sdrkit_inmarsat_get_inmarsat_c(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "inmarsat_c", value);
}
static inline int32_t sdrkit_inmarsat_set_inmarsat_c(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "inmarsat_c", value);
}

// The setting "budget" (CPU budget) of the stack "inmarsat". Unit: units. Aero windows of 160 kHz cost 1, an Inmarsat-C decoder in one 0.6. Windows over the budget are parked.
static inline int32_t sdrkit_inmarsat_get_budget(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "budget", value);
}
static inline int32_t sdrkit_inmarsat_set_budget(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "budget", value);
}

// The setting "satellite" (Satellite) of the stack "inmarsat". A value of `sdrkit_inmarsat_satellite`. Names the satellite instead of the identification.
static inline int32_t sdrkit_inmarsat_get_satellite(SDRKIT_HANDLE handle, sdrkit_inmarsat_satellite *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "satellite", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_inmarsat_satellite)index;
  return code;
}
static inline int32_t sdrkit_inmarsat_set_satellite(SDRKIT_HANDLE handle, sdrkit_inmarsat_satellite value) {
  return sdrkit_typed_setting_set_enum(handle, "satellite", (int32_t)value);
}

// The setting "gen_channels" (Channels (1 = 600, 2 = 1200, 4 = 10500)) of the test signal generator of the stack "inmarsat".
static inline int32_t sdrkit_inmarsat_gen_get_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_channels", value);
}
static inline int32_t sdrkit_inmarsat_gen_set_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_channels", value);
}

// The setting "gen_plain" (No built-in offsets and drift) of the test signal generator of the stack "inmarsat".
static inline int32_t sdrkit_inmarsat_gen_get_plain(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_plain", value);
}
static inline int32_t sdrkit_inmarsat_gen_set_plain(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_plain", value);
}

// Fills `rows` with the changed rows of the table "raw" of the stack "iridium" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_iridium_raw_rows(SDRKIT_HANDLE handle,
                                             sdrkit_iridium_raw_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.IridiumRawRow", "raw", rows, sizeof(sdrkit_iridium_raw_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "iridium.raw"
// (RAW line). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_iridium_raw_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_iridium_raw_event *event) {
  return sdrkit_typed_event(handle, n, "iridium.raw", "satcom.IridiumRawEvent", event, sizeof(sdrkit_iridium_raw_event));
}

// Runs the command "clear" (Clear) of the stack "iridium" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_iridium_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "iridium" are the sdrkit_iridium_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Burst threshold) of the stack "iridium". Unit: dB. Burst level over the noise floor needed to demodulate a burst.
static inline int32_t sdrkit_iridium_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_iridium_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "raw_output" (RAW output) of the stack "iridium". Publish each burst as a RAW line (events iridium.raw and the table raw). The host writes them where it wants.
static inline int32_t sdrkit_iridium_get_raw_output(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw_output", value);
}
static inline int32_t sdrkit_iridium_set_raw_output(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw_output", value);
}

// The setting "gen_burst_doppler" (Doppler) of the test signal generator of the stack "iridium". Each burst gets a Doppler shift of up to +-35 kHz.
static inline int32_t sdrkit_iridium_gen_get_burst_doppler(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_burst_doppler", value);
}
static inline int32_t sdrkit_iridium_gen_set_burst_doppler(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_burst_doppler", value);
}

// The setting "gen_bursts" (Bursts per frame) of the test signal generator of the stack "iridium".
static inline int32_t sdrkit_iridium_gen_get_bursts(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_bursts", value);
}
static inline int32_t sdrkit_iridium_gen_set_bursts(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_bursts", value);
}

// The setting "gen_centre_hz" (Radio centre) of the test signal generator of the stack "iridium". A frequency in Hz. Where the radio is tuned, so the generator puts the channels in the right place.
static inline int32_t sdrkit_iridium_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_iridium_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "globalstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_globalstar_channels_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_globalstar_channel_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.GlobalstarChannelRow", "channels", rows, sizeof(sdrkit_globalstar_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "activity" of the stack "globalstar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_globalstar_activity_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_globalstar_activity_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.GlobalstarActivityRow", "activity", rows, sizeof(sdrkit_globalstar_activity_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "globalstar.activity"
// (Activity). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_globalstar_activity_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_globalstar_activity_event *event) {
  return sdrkit_typed_event(handle, n, "globalstar.activity", "satcom.GlobalstarActivityEvent", event, sizeof(sdrkit_globalstar_activity_event));
}

// Runs the command "clear" (Clear) of the stack "globalstar" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_globalstar_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "globalstar" are the sdrkit_globalstar_gen_*() functions
// after the settings of its receiver.

// The setting "window_s" (Doppler plot span) of the stack "globalstar". A length of time in seconds.
static inline int32_t sdrkit_globalstar_get_window_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "window_s", value);
}
static inline int32_t sdrkit_globalstar_set_window_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "window_s", value);
}

// The setting "pass_overlay" (Pass overlay) of the stack "globalstar". Draw the pass planner's predicted Doppler over the track.
static inline int32_t sdrkit_globalstar_get_pass_overlay(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "pass_overlay", value);
}
static inline int32_t sdrkit_globalstar_set_pass_overlay(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "pass_overlay", value);
}

// The setting "gen_scenario" (Scenario) of the test signal generator of the stack "globalstar". A value of `sdrkit_globalstar_gen_scenario`. A pass: one channel sweeps from +30 to -30 kHz in 40 s, and the next channel sends 300 ms bursts every 800 ms. Steady: one channel at a constant frequency. Silence: no terminal.
static inline int32_t sdrkit_globalstar_gen_get_scenario(SDRKIT_HANDLE handle, sdrkit_globalstar_gen_scenario *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_globalstar_gen_scenario)index;
  return code;
}
static inline int32_t sdrkit_globalstar_gen_set_scenario(SDRKIT_HANDLE handle, sdrkit_globalstar_gen_scenario value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scenario", (int32_t)value);
}

// The setting "gen_channel" (Channel) of the test signal generator of the stack "globalstar". 0: the channel nearest the centre.
static inline int32_t sdrkit_globalstar_gen_get_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_channel", value);
}
static inline int32_t sdrkit_globalstar_gen_set_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_channel", value);
}

// The setting "gen_time_scale" (Time scale) of the test signal generator of the stack "globalstar". 10 plays a pass in a tenth of the time.
static inline int32_t sdrkit_globalstar_gen_get_time_scale(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_time_scale", value);
}
static inline int32_t sdrkit_globalstar_gen_set_time_scale(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_time_scale", value);
}

// The setting "gen_centre_hz" (Radio centre) of the test signal generator of the stack "globalstar". A frequency in Hz. Where the radio is tuned, so the generator puts the channels in the right place.
static inline int32_t sdrkit_globalstar_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_globalstar_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "reports" of the stack "goes_dcs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_dcs_reports_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_goes_dcs_report_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.GoesDcsReportRow", "reports", rows, sizeof(sdrkit_goes_dcs_report_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "platforms" of the stack "goes_dcs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_dcs_platforms_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_goes_dcs_platform_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.GoesDcsPlatformRow", "platforms", rows, sizeof(sdrkit_goes_dcs_platform_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "goes_dcs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_goes_dcs_carriers_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_goes_dcs_carrier_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.GoesDcsCarrierRow", "carriers", rows, sizeof(sdrkit_goes_dcs_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "goes_dcs.report"
// (Platform report). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_goes_dcs_report_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_goes_dcs_report_event *event) {
  return sdrkit_typed_event(handle, n, "goes_dcs.report", "satcom.GoesDcsReportEvent", event, sizeof(sdrkit_goes_dcs_report_event));
}

// Runs the command "clear" (Clear) of the stack "goes_dcs" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_goes_dcs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "goes_dcs" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_goes_dcs_remove(SDRKIT_HANDLE handle, const sdrkit_goes_dcs_platform_args *args) {
  return sdrkit_typed_command(handle, "remove", "satcom.GoesDcsPlatformArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "goes_dcs" are the sdrkit_goes_dcs_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Carrier threshold) of the stack "goes_dcs". Unit: dB. A carrier line must stand this far over the local noise floor.
static inline int32_t sdrkit_goes_dcs_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_goes_dcs_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "gen_scenario" (Scenario) of the test signal generator of the stack "goes_dcs". A value of `sdrkit_goes_dcs_gen_scenario`.
static inline int32_t sdrkit_goes_dcs_gen_get_scenario(SDRKIT_HANDLE handle, sdrkit_goes_dcs_gen_scenario *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_goes_dcs_gen_scenario)index;
  return code;
}
static inline int32_t sdrkit_goes_dcs_gen_set_scenario(SDRKIT_HANDLE handle, sdrkit_goes_dcs_gen_scenario value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scenario", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "argos" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_argos_bursts_rows(SDRKIT_HANDLE handle,
                                              sdrkit_argos_burst_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.ArgosBurstRow", "bursts", rows, sizeof(sdrkit_argos_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "platforms" of the stack "argos" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_argos_platforms_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_argos_platform_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.ArgosPlatformRow", "platforms", rows, sizeof(sdrkit_argos_platform_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "argos" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_argos_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_argos_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.ArgosFrameRow", "frames", rows, sizeof(sdrkit_argos_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "argos.ptt"
// (Platform message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_argos_ptt_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_argos_ptt_event *event) {
  return sdrkit_typed_event(handle, n, "argos.ptt", "satcom.ArgosPttEvent", event, sizeof(sdrkit_argos_ptt_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "argos.downlink_frame"
// (Downlink frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_argos_downlink_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_argos_downlink_frame_event *event) {
  return sdrkit_typed_event(handle, n, "argos.downlink_frame", "satcom.ArgosDownlinkFrameEvent", event, sizeof(sdrkit_argos_downlink_frame_event));
}

// Runs the command "clear" (Clear) of the stack "argos" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_argos_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "argos" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_argos_remove(SDRKIT_HANDLE handle, const sdrkit_argos_platform_args *args) {
  return sdrkit_typed_command(handle, "remove", "satcom.ArgosPlatformArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "argos" are the sdrkit_argos_gen_*() functions
// after the settings of its receiver.

// The setting "link" (Link) of the stack "argos". A value of `sdrkit_argos_link`. Automatic: the downlink above 430 MHz.
static inline int32_t sdrkit_argos_get_link(SDRKIT_HANDLE handle, sdrkit_argos_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_argos_link)index;
  return code;
}
static inline int32_t sdrkit_argos_set_link(SDRKIT_HANDLE handle, sdrkit_argos_link value) {
  return sdrkit_typed_setting_set_enum(handle, "link", (int32_t)value);
}

// The setting "gen_downlink" (Downlink) of the test signal generator of the stack "argos".
static inline int32_t sdrkit_argos_gen_get_downlink(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_downlink", value);
}
static inline int32_t sdrkit_argos_gen_set_downlink(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_downlink", value);
}

// The setting "gen_gap_s" (Seconds between bursts) of the test signal generator of the stack "argos". A length of time in seconds.
static inline int32_t sdrkit_argos_gen_get_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_s", value);
}
static inline int32_t sdrkit_argos_gen_set_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value);
}

// The setting "gen_step_hz" (Burst-to-burst step) of the test signal generator of the stack "argos". A frequency in Hz.
static inline int32_t sdrkit_argos_gen_get_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_step_hz", value);
}
static inline int32_t sdrkit_argos_gen_set_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_step_hz", value);
}

// The setting "gen_other" (Another modulation each cycle) of the test signal generator of the stack "argos".
static inline int32_t sdrkit_argos_gen_get_other(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_other", value);
}
static inline int32_t sdrkit_argos_gen_set_other(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_other", value);
}

// The setting "gen_pass_doppler_hz" (Largest Doppler of the pass) of the test signal generator of the stack "argos". A frequency in Hz.
static inline int32_t sdrkit_argos_gen_get_pass_doppler_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pass_doppler_hz", value);
}
static inline int32_t sdrkit_argos_gen_set_pass_doppler_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pass_doppler_hz", value);
}

// The setting "gen_pass_tau_s" (Doppler time constant) of the test signal generator of the stack "argos". A length of time in seconds.
static inline int32_t sdrkit_argos_gen_get_pass_tau_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pass_tau_s", value);
}
static inline int32_t sdrkit_argos_gen_set_pass_tau_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pass_tau_s", value);
}

// The setting "gen_pass_tca_s" (Closest approach) of the test signal generator of the stack "argos". A length of time in seconds.
static inline int32_t sdrkit_argos_gen_get_pass_tca_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pass_tca_s", value);
}
static inline int32_t sdrkit_argos_gen_set_pass_tca_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pass_tca_s", value);
}

// The setting "gen_bpsk" (BPSK) of the test signal generator of the stack "argos".
static inline int32_t sdrkit_argos_gen_get_bpsk(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_bpsk", value);
}
static inline int32_t sdrkit_argos_gen_set_bpsk(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_bpsk", value);
}

// The setting "gen_nrzi" (NRZI) of the test signal generator of the stack "argos".
static inline int32_t sdrkit_argos_gen_get_nrzi(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_nrzi", value);
}
static inline int32_t sdrkit_argos_gen_set_nrzi(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_nrzi", value);
}

// Fills `rows` with the changed rows of the table "reports" of the stack "hrdcp" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hrdcp_reports_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hrdcp_report_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.HrdcpReportRow", "reports", rows, sizeof(sdrkit_hrdcp_report_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "platforms" of the stack "hrdcp" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hrdcp_platforms_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_hrdcp_platform_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.HrdcpPlatformRow", "platforms", rows, sizeof(sdrkit_hrdcp_platform_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "hrdcp" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hrdcp_carriers_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hrdcp_carrier_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.HrdcpCarrierRow", "carriers", rows, sizeof(sdrkit_hrdcp_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hrdcp.report"
// (HRDCP message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hrdcp_report_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hrdcp_report_event *event) {
  return sdrkit_typed_event(handle, n, "hrdcp.report", "satcom.HrdcpReportEvent", event, sizeof(sdrkit_hrdcp_report_event));
}

// Runs the command "clear" (Clear) of the stack "hrdcp" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_hrdcp_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hrdcp" are the sdrkit_hrdcp_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Carrier threshold) of the stack "hrdcp". Unit: dB. A carrier line must stand this far over the local noise floor.
static inline int32_t sdrkit_hrdcp_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_hrdcp_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "channel_plan" (Channel plan) of the stack "hrdcp". The path of a plan file in the host's storage whose numbered bands give the channel numbers (docs/schema/plans.json). Empty for none.
static inline int32_t sdrkit_hrdcp_get_channel_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channel_plan", value);
}
static inline int32_t sdrkit_hrdcp_set_channel_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channel_plan", value);
}

// The setting "gen_scene" (Scene) of the test signal generator of the stack "hrdcp". A value of `sdrkit_hrdcp_gen_scene`.
static inline int32_t sdrkit_hrdcp_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_hrdcp_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hrdcp_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_hrdcp_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_hrdcp_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "sarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sarsat_bursts_rows(SDRKIT_HANDLE handle,
                                               sdrkit_sarsat_burst_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.SarsatBurstRow", "bursts", rows, sizeof(sdrkit_sarsat_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "beacons" of the stack "sarsat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sarsat_beacons_rows(SDRKIT_HANDLE handle,
                                                sdrkit_sarsat_beacon_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.SarsatBeaconRow", "beacons", rows, sizeof(sdrkit_sarsat_beacon_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sarsat.beacon"
// (Beacon decoded). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sarsat_beacon_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sarsat_beacon_event *event) {
  return sdrkit_typed_event(handle, n, "sarsat.beacon", "satcom.SarsatBeaconEvent", event, sizeof(sdrkit_sarsat_beacon_event));
}

// Runs the command "clear" (Clear) of the stack "sarsat" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_sarsat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "sarsat" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_sarsat_remove(SDRKIT_HANDLE handle, const sdrkit_sarsat_beacon_args *args) {
  return sdrkit_typed_command(handle, "remove", "satcom.SarsatBeaconArgs", args, NULL, NULL, 0);
}

// Runs the command "preset.apply" (Apply preset) of the stack "sarsat" on the session of `handle`. Asks the host to tune the dial to the preset.
static inline int32_t sdrkit_sarsat_preset_apply(SDRKIT_HANDLE handle, const sdrkit_sarsat_preset_args *args) {
  return sdrkit_typed_command(handle, "preset.apply", "satcom.SarsatPresetArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sarsat" are the sdrkit_sarsat_gen_*() functions
// after the settings of its receiver.

// The setting "wide_search" (Wide search) of the stack "sarsat". A bank of burst finders 90 kHz apart across up to 1.1 MHz of the input. It is on by itself while the dial is between 1543.5 and 1546.0 MHz.
static inline int32_t sdrkit_sarsat_get_wide_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide_search", value);
}
static inline int32_t sdrkit_sarsat_set_wide_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide_search", value);
}

// The setting "repeat_combining" (Repeat combining) of the stack "sarsat". Sums the soft bits of a first-generation burst that does not decode with those of the same beacon's earlier bursts on the same carrier.
static inline int32_t sdrkit_sarsat_get_repeat_combining(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "repeat_combining", value);
}
static inline int32_t sdrkit_sarsat_set_repeat_combining(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "repeat_combining", value);
}

// The setting "repeat_min_s" (Shortest repeat interval) of the stack "sarsat". A length of time in seconds. C/S T.001: a beacon repeats every 50 s +-5 %.
static inline int32_t sdrkit_sarsat_get_repeat_min_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "repeat_min_s", value);
}
static inline int32_t sdrkit_sarsat_set_repeat_min_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "repeat_min_s", value);
}

// The setting "repeat_max_s" (Longest repeat interval) of the stack "sarsat". A length of time in seconds. C/S T.001: a beacon repeats every 50 s +-5 %.
static inline int32_t sdrkit_sarsat_get_repeat_max_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "repeat_max_s", value);
}
static inline int32_t sdrkit_sarsat_set_repeat_max_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "repeat_max_s", value);
}

// The setting "gen_second_generation" (Second generation) of the test signal generator of the stack "sarsat".
static inline int32_t sdrkit_sarsat_gen_get_second_generation(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_second_generation", value);
}
static inline int32_t sdrkit_sarsat_gen_set_second_generation(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_second_generation", value);
}

// The setting "gen_beacon" (Test beacon) of the test signal generator of the stack "sarsat". 0 sends every test beacon in turn.
static inline int32_t sdrkit_sarsat_gen_get_beacon(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_beacon", value);
}
static inline int32_t sdrkit_sarsat_gen_set_beacon(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_beacon", value);
}

// The setting "gen_reverse" (Reversed phase polarity) of the test signal generator of the stack "sarsat".
static inline int32_t sdrkit_sarsat_gen_get_reverse(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_reverse", value);
}
static inline int32_t sdrkit_sarsat_gen_set_reverse(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_reverse", value);
}

// The setting "gen_bitrate_pct" (Bit rate error) of the test signal generator of the stack "sarsat". Unit: %. The first-generation bit rate away from 400 bit/s (T.001 allows 1 %).
static inline int32_t sdrkit_sarsat_gen_get_bitrate_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bitrate_pct", value);
}
static inline int32_t sdrkit_sarsat_gen_set_bitrate_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bitrate_pct", value);
}

// The setting "gen_gap_s" (Seconds between bursts) of the test signal generator of the stack "sarsat". A length of time in seconds.
static inline int32_t sdrkit_sarsat_gen_get_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_s", value);
}
static inline int32_t sdrkit_sarsat_gen_set_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "lora_sat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_lora_sat_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_lora_sat_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.LoraSatFrameRow", "frames", rows, sizeof(sdrkit_lora_sat_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "demodulators" of the stack "lora_sat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_lora_sat_demodulators_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_lora_sat_demodulator_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.LoraSatDemodulatorRow", "demodulators", rows, sizeof(sdrkit_lora_sat_demodulator_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "demodulator_syncs" of the stack "lora_sat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_lora_sat_demodulator_syncs_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_lora_sat_demodulator_sync_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.LoraSatDemodulatorSyncRow", "demodulator_syncs", rows, sizeof(sdrkit_lora_sat_demodulator_sync_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames_per_sf" of the stack "lora_sat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_lora_sat_frames_per_sf_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_lora_sat_sf_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.LoraSatSfRow", "frames_per_sf", rows, sizeof(sdrkit_lora_sat_sf_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "lora_sat.frame"
// (LoRa frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_lora_sat_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_lora_sat_frame_event *event) {
  return sdrkit_typed_event(handle, n, "lora_sat.frame", "satcom.LoraSatFrameEvent", event, sizeof(sdrkit_lora_sat_frame_event));
}

// Runs the command "clear" (Clear) of the stack "lora_sat" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_lora_sat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "lora_sat" are the sdrkit_lora_sat_gen_*() functions
// after the settings of its receiver.

// The setting "follow_dial" (Follow the dial) of the stack "lora_sat". When the dial is within 15 kHz of a preset, run its SF, bandwidth and sync.
static inline int32_t sdrkit_lora_sat_get_follow_dial(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_dial", value);
}
static inline int32_t sdrkit_lora_sat_set_follow_dial(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_dial", value);
}

// The setting "auto_sf" (Search SF) of the stack "lora_sat". SF7 to SF12 (a preset's SF overrides while following the dial).
static inline int32_t sdrkit_lora_sat_get_auto_sf(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_sf", value);
}
static inline int32_t sdrkit_lora_sat_set_auto_sf(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_sf", value);
}

// The setting "sf" (SF) of the stack "lora_sat". A value of `sdrkit_lora_sat_sf`.
static inline int32_t sdrkit_lora_sat_get_sf(SDRKIT_HANDLE handle, sdrkit_lora_sat_sf *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sf", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lora_sat_sf)index;
  return code;
}
static inline int32_t sdrkit_lora_sat_set_sf(SDRKIT_HANDLE handle, sdrkit_lora_sat_sf value) {
  return sdrkit_typed_setting_set_enum(handle, "sf", (int32_t)value);
}

// The setting "auto_bw" (Search bandwidth) of the stack "lora_sat". 125, 250 and 500 kHz.
static inline int32_t sdrkit_lora_sat_get_auto_bw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_bw", value);
}
static inline int32_t sdrkit_lora_sat_set_auto_bw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_bw", value);
}

// The setting "bw_khz" (Bandwidth, kHz) of the stack "lora_sat". A value of `sdrkit_lora_sat_bw_khz`.
static inline int32_t sdrkit_lora_sat_get_bw_khz(SDRKIT_HANDLE handle, sdrkit_lora_sat_bw_khz *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bw_khz", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lora_sat_bw_khz)index;
  return code;
}
static inline int32_t sdrkit_lora_sat_set_bw_khz(SDRKIT_HANDLE handle, sdrkit_lora_sat_bw_khz value) {
  return sdrkit_typed_setting_set_enum(handle, "bw_khz", (int32_t)value);
}

// The setting "sync" (Sync word) of the stack "lora_sat". A value of `sdrkit_lora_sat_sync`.
static inline int32_t sdrkit_lora_sat_get_sync(SDRKIT_HANDLE handle, sdrkit_lora_sat_sync *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sync", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lora_sat_sync)index;
  return code;
}
static inline int32_t sdrkit_lora_sat_set_sync(SDRKIT_HANDLE handle, sdrkit_lora_sat_sync value) {
  return sdrkit_typed_setting_set_enum(handle, "sync", (int32_t)value);
}

// The setting "sync_other" (Other sync word) of the stack "lora_sat".
static inline int32_t sdrkit_lora_sat_get_sync_other(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sync_other", value);
}
static inline int32_t sdrkit_lora_sat_set_sync_other(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sync_other", value);
}

// The setting "preamble" (Preamble) of the stack "lora_sat". Unit: symbols.
static inline int32_t sdrkit_lora_sat_get_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "preamble", value);
}
static inline int32_t sdrkit_lora_sat_set_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "preamble", value);
}

// The setting "doppler_max_hz" (Doppler allowance) of the stack "lora_sat". A frequency in Hz. 0: from the dial.
static inline int32_t sdrkit_lora_sat_get_doppler_max_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "doppler_max_hz", value);
}
static inline int32_t sdrkit_lora_sat_set_doppler_max_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "doppler_max_hz", value);
}

// The setting "track" (Track the carrier) of the stack "lora_sat". Follow the carrier through each frame.
static inline int32_t sdrkit_lora_sat_get_track(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "track", value);
}
static inline int32_t sdrkit_lora_sat_set_track(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "track", value);
}

// The setting "offset_hz" (Channel offset) of the stack "lora_sat". A frequency in Hz.
static inline int32_t sdrkit_lora_sat_get_offset_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "offset_hz", value);
}
static inline int32_t sdrkit_lora_sat_set_offset_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "offset_hz", value);
}

// The setting "pass_satellite" (Satellite overhead) of the stack "lora_sat". The pass planner's (the host's) name of the satellite overhead.
static inline int32_t sdrkit_lora_sat_get_pass_satellite(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "pass_satellite", value);
}
static inline int32_t sdrkit_lora_sat_set_pass_satellite(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "pass_satellite", value);
}

// The setting "gen_band" (Pass) of the test signal generator of the stack "lora_sat". A value of `sdrkit_lora_sat_gen_band`.
static inline int32_t sdrkit_lora_sat_gen_get_band(SDRKIT_HANDLE handle, sdrkit_lora_sat_gen_band *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_band", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lora_sat_gen_band)index;
  return code;
}
static inline int32_t sdrkit_lora_sat_gen_set_band(SDRKIT_HANDLE handle, sdrkit_lora_sat_gen_band value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_band", (int32_t)value);
}

// The setting "gen_sf" (SF) of the test signal generator of the stack "lora_sat". A value of `sdrkit_lora_sat_gen_sf`.
static inline int32_t sdrkit_lora_sat_gen_get_sf(SDRKIT_HANDLE handle, sdrkit_lora_sat_gen_sf *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_sf", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lora_sat_gen_sf)index;
  return code;
}
static inline int32_t sdrkit_lora_sat_gen_set_sf(SDRKIT_HANDLE handle, sdrkit_lora_sat_gen_sf value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_sf", (int32_t)value);
}

// The setting "gen_bw_khz" (Bandwidth, kHz) of the test signal generator of the stack "lora_sat". A value of `sdrkit_lora_sat_gen_bw_khz`.
static inline int32_t sdrkit_lora_sat_gen_get_bw_khz(SDRKIT_HANDLE handle, sdrkit_lora_sat_gen_bw_khz *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bw_khz", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lora_sat_gen_bw_khz)index;
  return code;
}
static inline int32_t sdrkit_lora_sat_gen_set_bw_khz(SDRKIT_HANDLE handle, sdrkit_lora_sat_gen_bw_khz value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bw_khz", (int32_t)value);
}

// The setting "gen_pass_doppler" (Doppler of the pass) of the test signal generator of the stack "lora_sat".
static inline int32_t sdrkit_lora_sat_gen_get_pass_doppler(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_pass_doppler", value);
}
static inline int32_t sdrkit_lora_sat_gen_set_pass_doppler(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_pass_doppler", value);
}

// The setting "gen_start_s" (Seconds into the pass) of the test signal generator of the stack "lora_sat". A length of time in seconds.
static inline int32_t sdrkit_lora_sat_gen_get_start_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_start_s", value);
}
static inline int32_t sdrkit_lora_sat_gen_set_start_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_start_s", value);
}

// The setting "gen_sync" (Sync word) of the test signal generator of the stack "lora_sat".
static inline int32_t sdrkit_lora_sat_gen_get_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_sync", value);
}
static inline int32_t sdrkit_lora_sat_gen_set_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_sync", value);
}

// Fills `rows` with the changed rows of the table "satellites" of the stack "orbcomm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_orbcomm_satellites_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_orbcomm_satellite_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.OrbcommSatelliteRow", "satellites", rows, sizeof(sdrkit_orbcomm_satellite_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "satellite_channels" of the stack "orbcomm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_orbcomm_satellite_channels_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_orbcomm_satellite_channel_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.OrbcommSatelliteChannelRow", "satellite_channels", rows, sizeof(sdrkit_orbcomm_satellite_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "orbcomm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_orbcomm_decoders_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_orbcomm_decoder_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.OrbcommDecoderRow", "decoders", rows, sizeof(sdrkit_orbcomm_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "orbcomm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_orbcomm_packets_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_orbcomm_packet_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.OrbcommPacketRow", "packets", rows, sizeof(sdrkit_orbcomm_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packet_types" of the stack "orbcomm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_orbcomm_packet_types_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_orbcomm_packet_type_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.OrbcommPacketTypeRow", "packet_types", rows, sizeof(sdrkit_orbcomm_packet_type_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "orbcomm.message"
// (Message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_orbcomm_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_orbcomm_message_event *event) {
  return sdrkit_typed_event(handle, n, "orbcomm.message", "satcom.OrbcommMessageEvent", event, sizeof(sdrkit_orbcomm_message_event));
}

// Runs the command "clear" (Clear) of the stack "orbcomm" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_orbcomm_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "preset.apply" (Tune to channel) of the stack "orbcomm" on the session of `handle`. Asks the host to tune the dial to the channel.
static inline int32_t sdrkit_orbcomm_preset_apply(SDRKIT_HANDLE handle, const sdrkit_orbcomm_channel_args *args) {
  return sdrkit_typed_command(handle, "preset.apply", "satcom.OrbcommChannelArgs", args, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "orbcomm" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_orbcomm_remove(SDRKIT_HANDLE handle, const sdrkit_orbcomm_satellite_args *args) {
  return sdrkit_typed_command(handle, "remove", "satcom.OrbcommSatelliteArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "orbcomm" are the sdrkit_orbcomm_gen_*() functions
// after the settings of its receiver.

// The setting "decoders" (Decoders) of the stack "orbcomm". A value of `sdrkit_orbcomm_decoders`. Downlinks decoded at once.
static inline int32_t sdrkit_orbcomm_get_decoders(SDRKIT_HANDLE handle, sdrkit_orbcomm_decoders *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "decoders", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_orbcomm_decoders)index;
  return code;
}
static inline int32_t sdrkit_orbcomm_set_decoders(SDRKIT_HANDLE handle, sdrkit_orbcomm_decoders value) {
  return sdrkit_typed_setting_set_enum(handle, "decoders", (int32_t)value);
}

// The setting "follow" (Follow satellite) of the stack "orbcomm". The map follows the selected satellite.
static inline int32_t sdrkit_orbcomm_get_follow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow", value);
}
static inline int32_t sdrkit_orbcomm_set_follow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow", value);
}

// The setting "gen_downlinks" (Downlinks) of the test signal generator of the stack "orbcomm".
static inline int32_t sdrkit_orbcomm_gen_get_downlinks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_downlinks", value);
}
static inline int32_t sdrkit_orbcomm_gen_set_downlinks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_downlinks", value);
}

// The setting "gen_satellite_doppler_hz" (Satellite Doppler) of the test signal generator of the stack "orbcomm". A frequency in Hz. The first downlink's Doppler. The second has the opposite sign.
static inline int32_t sdrkit_orbcomm_gen_get_satellite_doppler_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_satellite_doppler_hz", value);
}
static inline int32_t sdrkit_orbcomm_gen_set_satellite_doppler_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_satellite_doppler_hz", value);
}

// The setting "gen_ramp_hz" (Doppler ramp (+-)) of the test signal generator of the stack "orbcomm". A frequency in Hz. 0: the fixed Doppler. Else a triangular ramp between + and - this.
static inline int32_t sdrkit_orbcomm_gen_get_ramp_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ramp_hz", value);
}
static inline int32_t sdrkit_orbcomm_gen_set_ramp_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ramp_hz", value);
}

// The setting "gen_ramp_s" (Ramp leg) of the test signal generator of the stack "orbcomm". A length of time in seconds.
static inline int32_t sdrkit_orbcomm_gen_get_ramp_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ramp_s", value);
}
static inline int32_t sdrkit_orbcomm_gen_set_ramp_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ramp_s", value);
}

// The setting "gen_broken" (Broken checks) of the test signal generator of the stack "orbcomm". The share of packets sent with a broken Fletcher check.
static inline int32_t sdrkit_orbcomm_gen_get_broken(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_broken", value);
}
static inline int32_t sdrkit_orbcomm_gen_set_broken(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_broken", value);
}

// The setting "gen_centre_hz" (Radio centre) of the test signal generator of the stack "orbcomm". A frequency in Hz. Where the radio is tuned, so the generator puts the downlinks in the right place.
static inline int32_t sdrkit_orbcomm_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_orbcomm_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "rates" of the stack "psk_demod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_psk_demod_rates_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_psk_demod_rate_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.PskDemodRateRow", "rates", rows, sizeof(sdrkit_psk_demod_rate_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trials" of the stack "psk_demod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_psk_demod_trials_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_psk_demod_trial_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.PskDemodTrialRow", "trials", rows, sizeof(sdrkit_psk_demod_trial_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "syncs" of the stack "psk_demod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_psk_demod_syncs_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_psk_demod_sync_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.PskDemodSyncRow", "syncs", rows, sizeof(sdrkit_psk_demod_sync_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "log" of the stack "psk_demod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_psk_demod_log_rows(SDRKIT_HANDLE handle,
                                               sdrkit_psk_demod_log_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.PskDemodLogRow", "log", rows, sizeof(sdrkit_psk_demod_log_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "psk_demod.lock"
// (Signal locked). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_psk_demod_lock_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_psk_demod_lock_event *event) {
  return sdrkit_typed_event(handle, n, "psk_demod.lock", "satcom.PskDemodLockEvent", event, sizeof(sdrkit_psk_demod_lock_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "psk_demod.bytes"
// (Bits as bytes). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_psk_demod_bytes_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_psk_demod_bytes_event *event) {
  return sdrkit_typed_event(handle, n, "psk_demod.bytes", "satcom.PskDemodBytesEvent", event, sizeof(sdrkit_psk_demod_bytes_event));
}

// Runs the command "clear" (Clear) of the stack "psk_demod" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_psk_demod_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "psk_demod" are the sdrkit_psk_demod_gen_*() functions
// after the settings of its receiver.

// The setting "order" (Order) of the stack "psk_demod". A value of `sdrkit_psk_demod_order`.
static inline int32_t sdrkit_psk_demod_get_order(SDRKIT_HANDLE handle, sdrkit_psk_demod_order *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "order", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_demod_order)index;
  return code;
}
static inline int32_t sdrkit_psk_demod_set_order(SDRKIT_HANDLE handle, sdrkit_psk_demod_order value) {
  return sdrkit_typed_setting_set_enum(handle, "order", (int32_t)value);
}

// The setting "baud" (Symbol rate) of the stack "psk_demod". A frequency in Hz. 0: found by the analyser.
static inline int32_t sdrkit_psk_demod_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "baud", value);
}
static inline int32_t sdrkit_psk_demod_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "baud", value);
}

// The setting "shape" (Pulse shape) of the stack "psk_demod". A value of `sdrkit_psk_demod_shape`.
static inline int32_t sdrkit_psk_demod_get_shape(SDRKIT_HANDLE handle, sdrkit_psk_demod_shape *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "shape", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_demod_shape)index;
  return code;
}
static inline int32_t sdrkit_psk_demod_set_shape(SDRKIT_HANDLE handle, sdrkit_psk_demod_shape value) {
  return sdrkit_typed_setting_set_enum(handle, "shape", (int32_t)value);
}

// The setting "span" (Search span) of the stack "psk_demod". auto (200 kHz or what the radio gives), a number of Hz (2500 to 200000), 500k, 2M or whole.
static inline int32_t sdrkit_psk_demod_get_span(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "span", value);
}
static inline int32_t sdrkit_psk_demod_set_span(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "span", value);
}

// The setting "snap_standard" (Snap to standard rates) of the stack "psk_demod".
static inline int32_t sdrkit_psk_demod_get_snap_standard(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "snap_standard", value);
}
static inline int32_t sdrkit_psk_demod_set_snap_standard(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "snap_standard", value);
}

// The setting "follow_doppler" (Follow Doppler) of the stack "psk_demod".
static inline int32_t sdrkit_psk_demod_get_follow_doppler(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "follow_doppler", value);
}
static inline int32_t sdrkit_psk_demod_set_follow_doppler(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "follow_doppler", value);
}

// The setting "click_hz" (Clicked frequency) of the stack "psk_demod". Unit: Hz. Hz from the dial; empty: the strongest signal.
static inline int32_t sdrkit_psk_demod_get_click_hz(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "click_hz", value);
}
static inline int32_t sdrkit_psk_demod_set_click_hz(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "click_hz", value);
}

// The setting "differential" (Differential) of the stack "psk_demod". A value of `sdrkit_psk_demod_differential`.
static inline int32_t sdrkit_psk_demod_get_differential(SDRKIT_HANDLE handle, sdrkit_psk_demod_differential *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "differential", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_demod_differential)index;
  return code;
}
static inline int32_t sdrkit_psk_demod_set_differential(SDRKIT_HANDLE handle, sdrkit_psk_demod_differential value) {
  return sdrkit_typed_setting_set_enum(handle, "differential", (int32_t)value);
}

// The setting "descrambler" (Descrambler) of the stack "psk_demod". A value of `sdrkit_psk_demod_descrambler`.
static inline int32_t sdrkit_psk_demod_get_descrambler(SDRKIT_HANDLE handle, sdrkit_psk_demod_descrambler *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "descrambler", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_demod_descrambler)index;
  return code;
}
static inline int32_t sdrkit_psk_demod_set_descrambler(SDRKIT_HANDLE handle, sdrkit_psk_demod_descrambler value) {
  return sdrkit_typed_setting_set_enum(handle, "descrambler", (int32_t)value);
}

// The setting "sync" (Sync word) of the stack "psk_demod". Bits or hex ("1ACFFC1D"); every rotation and both spectral senses are tried.
static inline int32_t sdrkit_psk_demod_get_sync(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "sync", value);
}
static inline int32_t sdrkit_psk_demod_set_sync(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "sync", value);
}

// The setting "invert" (Invert the bits) of the stack "psk_demod".
static inline int32_t sdrkit_psk_demod_get_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "invert", value);
}
static inline int32_t sdrkit_psk_demod_set_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "invert", value);
}

// The setting "save_bits" (Save bits) of the stack "psk_demod". Hands the bits, packed into bytes, to the host (event psk_demod.bytes).
static inline int32_t sdrkit_psk_demod_get_save_bits(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "save_bits", value);
}
static inline int32_t sdrkit_psk_demod_set_save_bits(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "save_bits", value);
}

// The setting "gen_order" (Order) of the test signal generator of the stack "psk_demod". A value of `sdrkit_psk_demod_gen_order`.
static inline int32_t sdrkit_psk_demod_gen_get_order(SDRKIT_HANDLE handle, sdrkit_psk_demod_gen_order *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_order", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_demod_gen_order)index;
  return code;
}
static inline int32_t sdrkit_psk_demod_gen_set_order(SDRKIT_HANDLE handle, sdrkit_psk_demod_gen_order value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_order", (int32_t)value);
}

// The setting "gen_baud" (Symbol rate) of the test signal generator of the stack "psk_demod". A frequency in Hz.
static inline int32_t sdrkit_psk_demod_gen_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud", value);
}
static inline int32_t sdrkit_psk_demod_gen_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud", value);
}

// The setting "gen_rolloff" (Roll-off (0: rectangular)) of the test signal generator of the stack "psk_demod".
static inline int32_t sdrkit_psk_demod_gen_get_rolloff(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rolloff", value);
}
static inline int32_t sdrkit_psk_demod_gen_set_rolloff(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value);
}

// The setting "gen_coding" (Coding) of the test signal generator of the stack "psk_demod". A value of `sdrkit_psk_demod_gen_coding`.
static inline int32_t sdrkit_psk_demod_gen_get_coding(SDRKIT_HANDLE handle, sdrkit_psk_demod_gen_coding *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_coding", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_psk_demod_gen_coding)index;
  return code;
}
static inline int32_t sdrkit_psk_demod_gen_set_coding(SDRKIT_HANDLE handle, sdrkit_psk_demod_gen_coding value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_coding", (int32_t)value);
}

// The setting "gen_sync" (Sync word) of the test signal generator of the stack "psk_demod".
static inline int32_t sdrkit_psk_demod_gen_get_sync(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_sync", value);
}
static inline int32_t sdrkit_psk_demod_gen_set_sync(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_sync", value);
}

// The setting "gen_conjugate" (Conjugated symbols) of the test signal generator of the stack "psk_demod". The transmitter swaps I and Q (the decoder must find the spectral inversion).
static inline int32_t sdrkit_psk_demod_gen_get_conjugate(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_conjugate", value);
}
static inline int32_t sdrkit_psk_demod_gen_set_conjugate(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_conjugate", value);
}

// Fills `rows` with the changed rows of the table "beacons" of the stack "qo100" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qo100_beacons_rows(SDRKIT_HANDLE handle,
                                               sdrkit_qo100_beacon_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.Qo100BeaconRow", "beacons", rows, sizeof(sdrkit_qo100_beacon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "signals" of the stack "qo100" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qo100_signals_rows(SDRKIT_HANDLE handle,
                                               sdrkit_qo100_signal_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.Qo100SignalRow", "signals", rows, sizeof(sdrkit_qo100_signal_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "qo100" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qo100_tracks_rows(SDRKIT_HANDLE handle,
                                              sdrkit_qo100_track_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.Qo100TrackRow", "tracks", rows, sizeof(sdrkit_qo100_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "conversations" of the stack "qo100" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qo100_conversations_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_qo100_conversation_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.Qo100ConversationRow", "conversations", rows, sizeof(sdrkit_qo100_conversation_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "conversation_callsigns" of the stack "qo100" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qo100_conversation_callsigns_rows(SDRKIT_HANDLE handle,
                                                              sdrkit_qo100_callsign_row *rows,
                                                              uint32_t capacity,
                                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.Qo100CallsignRow", "conversation_callsigns", rows, sizeof(sdrkit_qo100_callsign_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "datv" of the stack "qo100" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qo100_datv_rows(SDRKIT_HANDLE handle,
                                            sdrkit_qo100_datv_carrier_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.Qo100DatvCarrierRow", "datv", rows, sizeof(sdrkit_qo100_datv_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "qo100.lock"
// (Beacon lock). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_qo100_lock_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_qo100_lock_event *event) {
  return sdrkit_typed_event(handle, n, "qo100.lock", "satcom.Qo100LockEvent", event, sizeof(sdrkit_qo100_lock_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "qo100.conversation"
// (Conversation started). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_qo100_conversation_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_qo100_conversation_event *event) {
  return sdrkit_typed_event(handle, n, "qo100.conversation", "satcom.Qo100ConversationEvent", event, sizeof(sdrkit_qo100_conversation_event));
}

// Runs the command "clear" (Clear) of the stack "qo100" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_qo100_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "speech.text" (Speech-to-text) of the stack "qo100" on the session of `handle`. Gives the text that the host's speech-to-text heard. Callsigns are read from it.
static inline int32_t sdrkit_qo100_speech_text(SDRKIT_HANDLE handle, const sdrkit_qo100_speech_args *args) {
  return sdrkit_typed_command(handle, "speech.text", "satcom.Qo100SpeechArgs", args, NULL, NULL, 0);
}

// Runs the command "conversation.listen" (Listen to conversation) of the stack "qo100" on the session of `handle`. Makes the dial follow a conversation. An empty row returns to the dial.
static inline int32_t sdrkit_qo100_conversation_listen(SDRKIT_HANDLE handle, const sdrkit_qo100_listen_args *args) {
  return sdrkit_typed_command(handle, "conversation.listen", "satcom.Qo100ListenArgs", args, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "qo100" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_qo100_remove(SDRKIT_HANDLE handle, const sdrkit_qo100_conversation_args *args) {
  return sdrkit_typed_command(handle, "remove", "satcom.Qo100ConversationArgs", args, NULL, NULL, 0);
}

// Runs the command "ptt" (Push to talk) of the stack "qo100" on the session of `handle`. With on true, sends the speech of the microphone input in upper sideband on the uplink of the dial until ptt with on false or the time-out. Transmit must be on. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
static inline int32_t sdrkit_qo100_ptt(SDRKIT_HANDLE handle, const sdrkit_qo100_ptt_args *args) {
  return sdrkit_typed_command(handle, "ptt", "satcom.PttArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "qo100" are the sdrkit_qo100_gen_*() functions
// after the settings of its receiver.

// The setting "lo" (LNB oscillator) of the stack "qo100". A value of `sdrkit_qo100_lo`.
static inline int32_t sdrkit_qo100_get_lo(SDRKIT_HANDLE handle, sdrkit_qo100_lo *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "lo", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qo100_lo)index;
  return code;
}
static inline int32_t sdrkit_qo100_set_lo(SDRKIT_HANDLE handle, sdrkit_qo100_lo value) {
  return sdrkit_typed_setting_set_enum(handle, "lo", (int32_t)value);
}

// The setting "lo_custom_mhz" (Custom LO) of the stack "qo100". Unit: MHz.
static inline int32_t sdrkit_qo100_get_lo_custom_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "lo_custom_mhz", value);
}
static inline int32_t sdrkit_qo100_set_lo_custom_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "lo_custom_mhz", value);
}

// The setting "auto_correction" (Automatic correction) of the stack "qo100". Apply the LNB error measured on the beacons.
static inline int32_t sdrkit_qo100_get_auto_correction(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_correction", value);
}
static inline int32_t sdrkit_qo100_set_auto_correction(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_correction", value);
}

// The setting "lnb_ppm" (LNB calibration) of the stack "qo100". Unit: ppm.
static inline int32_t sdrkit_qo100_get_lnb_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "lnb_ppm", value);
}
static inline int32_t sdrkit_qo100_set_lnb_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "lnb_ppm", value);
}

// The setting "dial_hz" (Dial (RF)) of the stack "qo100". A frequency in Hz. The USB dial for listening.
static inline int32_t sdrkit_qo100_get_dial_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_hz", value);
}
static inline int32_t sdrkit_qo100_set_dial_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_hz", value);
}

// The setting "auto_listen" (Auto-listen) of the stack "qo100". A value of `sdrkit_qo100_auto_listen`.
static inline int32_t sdrkit_qo100_get_auto_listen(SDRKIT_HANDLE handle, sdrkit_qo100_auto_listen *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "auto_listen", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qo100_auto_listen)index;
  return code;
}
static inline int32_t sdrkit_qo100_set_auto_listen(SDRKIT_HANDLE handle, sdrkit_qo100_auto_listen value) {
  return sdrkit_typed_setting_set_enum(handle, "auto_listen", (int32_t)value);
}

// The setting "conversation_gap_s" (Conversation gap) of the stack "qo100". A length of time in seconds.
static inline int32_t sdrkit_qo100_get_conversation_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "conversation_gap_s", value);
}
static inline int32_t sdrkit_qo100_set_conversation_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "conversation_gap_s", value);
}

// The setting "tx_level" (Peak level) of the stack "qo100". Unit: of full scale. The peak magnitude of the signal sent, of full scale. Set it so that your downlink stays below the beacon.
static inline int32_t sdrkit_qo100_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_qo100_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "tx_timeout" (Time-out) of the stack "qo100". A length of time in seconds. Push-to-talk ends after this time on air.
static inline int32_t sdrkit_qo100_get_tx_timeout(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_timeout", value);
}
static inline int32_t sdrkit_qo100_set_tx_timeout(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_timeout", value);
}

// The setting "gen_empty" (No signals) of the test signal generator of the stack "qo100". Send nothing: only the noise of the impairments.
static inline int32_t sdrkit_qo100_gen_get_empty(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_empty", value);
}
static inline int32_t sdrkit_qo100_gen_set_empty(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_empty", value);
}

// The setting "gen_centre_mhz" (Radio centre (RF, 0: automatic)) of the test signal generator of the stack "qo100". Unit: MHz.
static inline int32_t sdrkit_qo100_gen_get_centre_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_mhz", value);
}
static inline int32_t sdrkit_qo100_gen_set_centre_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_mhz", value);
}

// The setting "gen_lo" (Nominal LO) of the test signal generator of the stack "qo100". A value of `sdrkit_qo100_gen_lo`.
static inline int32_t sdrkit_qo100_gen_get_lo(SDRKIT_HANDLE handle, sdrkit_qo100_gen_lo *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_lo", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qo100_gen_lo)index;
  return code;
}
static inline int32_t sdrkit_qo100_gen_set_lo(SDRKIT_HANDLE handle, sdrkit_qo100_gen_lo value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_lo", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "cubesat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cubesat_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_cubesat_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.CubesatFrameRow", "frames", rows, sizeof(sdrkit_cubesat_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames_per_framing" of the stack "cubesat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cubesat_frames_per_framing_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_cubesat_framing_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.CubesatFramingRow", "frames_per_framing", rows, sizeof(sdrkit_cubesat_framing_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "cubesat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cubesat_carriers_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_cubesat_carrier_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.CubesatCarrierRow", "carriers", rows, sizeof(sdrkit_cubesat_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cubesat.frame"
// (CubeSat frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cubesat_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cubesat_frame_event *event) {
  return sdrkit_typed_event(handle, n, "cubesat.frame", "satcom.CubesatFrameEvent", event, sizeof(sdrkit_cubesat_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cubesat.kiss"
// (KISS frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cubesat_kiss_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cubesat_kiss_event *event) {
  return sdrkit_typed_event(handle, n, "cubesat.kiss", "satcom.CubesatKissEvent", event, sizeof(sdrkit_cubesat_kiss_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cubesat.tx"
// (AX.25 transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cubesat_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cubesat_tx_event *event) {
  return sdrkit_typed_event(handle, n, "cubesat.tx", "satcom.CubesatTxEvent", event, sizeof(sdrkit_cubesat_tx_event));
}

// Runs the command "clear" (Clear) of the stack "cubesat" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cubesat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "cubesat" on the session of `handle`. Builds a UI frame from the station callsign to the destination and queues it. A transmission starts when none is on air; frames sent during a transmission go in the next one. Transmit must be on and the station callsign set. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_cubesat_send(SDRKIT_HANDLE handle, const sdrkit_cubesat_send_args *args) {
  return sdrkit_typed_command(handle, "send", "satcom.CubesatSendArgs", args, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "cubesat" on the session of `handle`. Drops the frames waiting and ends the transmission after the current bit with a 5 ms fall.
static inline int32_t sdrkit_cubesat_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cubesat" are the sdrkit_cubesat_gen_*() functions
// after the settings of its receiver.

// The setting "search" (Search) of the stack "cubesat". A value of `sdrkit_cubesat_search`.
static inline int32_t sdrkit_cubesat_get_search(SDRKIT_HANDLE handle, sdrkit_cubesat_search *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "search", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_search)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_search(SDRKIT_HANDLE handle, sdrkit_cubesat_search value) {
  return sdrkit_typed_setting_set_enum(handle, "search", (int32_t)value);
}

// The setting "ccsds_length" (CCSDS code length) of the stack "cubesat". Unit: bytes.
static inline int32_t sdrkit_cubesat_get_ccsds_length(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "ccsds_length", value);
}
static inline int32_t sdrkit_cubesat_set_ccsds_length(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "ccsds_length", value);
}

// The setting "smart.max_channels" (Carriers followed) of the stack "cubesat".
static inline int32_t sdrkit_cubesat_get_smart_max_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "smart.max_channels", value);
}
static inline int32_t sdrkit_cubesat_set_smart_max_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "smart.max_channels", value);
}

// The setting "smart.min_snr_db" (Carrier SNR) of the stack "cubesat". Unit: dB.
static inline int32_t sdrkit_cubesat_get_smart_min_snr_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "smart.min_snr_db", value);
}
static inline int32_t sdrkit_cubesat_set_smart_min_snr_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "smart.min_snr_db", value);
}

// The setting "extra_bitrates" (Extra bit rates) of the stack "cubesat". Unit: bit/s. Up to four bit rates of 300 to 20000 bit/s, separated by commas: one more FSK slicer each, for every framing.
static inline int32_t sdrkit_cubesat_get_extra_bitrates(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "extra_bitrates", value);
}
static inline int32_t sdrkit_cubesat_set_extra_bitrates(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "extra_bitrates", value);
}

// The setting "extra_ax25" (AX.25 on the extra slicers) of the stack "cubesat". A value of `sdrkit_cubesat_extra_ax25`.
static inline int32_t sdrkit_cubesat_get_extra_ax25(SDRKIT_HANDLE handle, sdrkit_cubesat_extra_ax25 *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "extra_ax25", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_extra_ax25)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_extra_ax25(SDRKIT_HANDLE handle, sdrkit_cubesat_extra_ax25 value) {
  return sdrkit_typed_setting_set_enum(handle, "extra_ax25", (int32_t)value);
}

// The setting "afsk_carrier_hz" (U482C audio carrier) of the stack "cubesat". Unit: Hz. 0 turns the audio FSK path off; else 500 to 8000 Hz.
static inline int32_t sdrkit_cubesat_get_afsk_carrier_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "afsk_carrier_hz", value);
}
static inline int32_t sdrkit_cubesat_set_afsk_carrier_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "afsk_carrier_hz", value);
}

// The setting "afsk_bitrate" (U482C audio bit rate) of the stack "cubesat". A value of `sdrkit_cubesat_afsk_bitrate`.
static inline int32_t sdrkit_cubesat_get_afsk_bitrate(SDRKIT_HANDLE handle, sdrkit_cubesat_afsk_bitrate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "afsk_bitrate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_afsk_bitrate)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_afsk_bitrate(SDRKIT_HANDLE handle, sdrkit_cubesat_afsk_bitrate value) {
  return sdrkit_typed_setting_set_enum(handle, "afsk_bitrate", (int32_t)value);
}

// The setting "geoscan_length" (Geoscan frame bytes) of the stack "cubesat". A value of `sdrkit_cubesat_geoscan_length`.
static inline int32_t sdrkit_cubesat_get_geoscan_length(SDRKIT_HANDLE handle, sdrkit_cubesat_geoscan_length *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "geoscan_length", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_geoscan_length)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_geoscan_length(SDRKIT_HANDLE handle, sdrkit_cubesat_geoscan_length value) {
  return sdrkit_typed_setting_set_enum(handle, "geoscan_length", (int32_t)value);
}

// The setting "ccsds_basis" (CCSDS RS basis) of the stack "cubesat". A value of `sdrkit_cubesat_ccsds_basis`.
static inline int32_t sdrkit_cubesat_get_ccsds_basis(SDRKIT_HANDLE handle, sdrkit_cubesat_ccsds_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "ccsds_basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_ccsds_basis)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_ccsds_basis(SDRKIT_HANDLE handle, sdrkit_cubesat_ccsds_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "ccsds_basis", (int32_t)value);
}

// The setting "concat.frame_bytes" (CCSDS concatenated frame size) of the stack "cubesat". Unit: bytes. Data bytes, check bytes not included: a multiple of the interleave, at most 223 for each code word.
static inline int32_t sdrkit_cubesat_get_concat_frame_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "concat.frame_bytes", value);
}
static inline int32_t sdrkit_cubesat_set_concat_frame_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "concat.frame_bytes", value);
}

// The setting "concat.interleave" (Interleave) of the stack "cubesat".
static inline int32_t sdrkit_cubesat_get_concat_interleave(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "concat.interleave", value);
}
static inline int32_t sdrkit_cubesat_set_concat_interleave(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "concat.interleave", value);
}

// The setting "concat.basis" (Basis) of the stack "cubesat". A value of `sdrkit_cubesat_concat_basis`.
static inline int32_t sdrkit_cubesat_get_concat_basis(SDRKIT_HANDLE handle, sdrkit_cubesat_concat_basis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "concat.basis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_concat_basis)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_concat_basis(SDRKIT_HANDLE handle, sdrkit_cubesat_concat_basis value) {
  return sdrkit_typed_setting_set_enum(handle, "concat.basis", (int32_t)value);
}

// The setting "concat.code" (Code) of the stack "cubesat". A value of `sdrkit_cubesat_concat_code`.
static inline int32_t sdrkit_cubesat_get_concat_code(SDRKIT_HANDLE handle, sdrkit_cubesat_concat_code *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "concat.code", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_concat_code)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_concat_code(SDRKIT_HANDLE handle, sdrkit_cubesat_concat_code value) {
  return sdrkit_typed_setting_set_enum(handle, "concat.code", (int32_t)value);
}

// The setting "concat.nrzm" (NRZ-M) of the stack "cubesat".
static inline int32_t sdrkit_cubesat_get_concat_nrzm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "concat.nrzm", value);
}
static inline int32_t sdrkit_cubesat_set_concat_nrzm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "concat.nrzm", value);
}

// The setting "concat.randomiser" (Randomiser) of the stack "cubesat".
static inline int32_t sdrkit_cubesat_get_concat_randomiser(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "concat.randomiser", value);
}
static inline int32_t sdrkit_cubesat_set_concat_randomiser(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "concat.randomiser", value);
}

// The setting "mobitex_variant" (Mobitex variant) of the stack "cubesat". A value of `sdrkit_cubesat_mobitex_variant`.
static inline int32_t sdrkit_cubesat_get_mobitex_variant(SDRKIT_HANDLE handle, sdrkit_cubesat_mobitex_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mobitex_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_mobitex_variant)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_mobitex_variant(SDRKIT_HANDLE handle, sdrkit_cubesat_mobitex_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "mobitex_variant", (int32_t)value);
}

// The setting "satellites" (Satellite table) of the stack "cubesat". name|norad|MHz|framing|baud|call;...
static inline int32_t sdrkit_cubesat_get_satellites(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "satellites", value);
}
static inline int32_t sdrkit_cubesat_set_satellites(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "satellites", value);
}

// The setting "tracked" (Satellite followed) of the stack "cubesat". name|norad of the satellite the pass planner follows (from the host).
static inline int32_t sdrkit_cubesat_get_tracked(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "tracked", value);
}
static inline int32_t sdrkit_cubesat_set_tracked(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "tracked", value);
}

// The setting "kiss" (KISS output) of the stack "cubesat".
static inline int32_t sdrkit_cubesat_get_kiss(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "kiss", value);
}
static inline int32_t sdrkit_cubesat_set_kiss(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "kiss", value);
}

// The setting "framing_filter" (Framing) of the stack "cubesat". A value of `sdrkit_cubesat_framing_filter`.
static inline int32_t sdrkit_cubesat_get_framing_filter(SDRKIT_HANDLE handle, sdrkit_cubesat_framing_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "framing_filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_framing_filter)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_framing_filter(SDRKIT_HANDLE handle, sdrkit_cubesat_framing_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "framing_filter", (int32_t)value);
}

// The setting "tx_rate" (Speed) of the stack "cubesat". A value of `sdrkit_cubesat_tx_rate`. 1200 bit/s Bell 202 AFSK or 9600 bit/s G3RUH FSK, both on FM with 3 kHz deviation.
static inline int32_t sdrkit_cubesat_get_tx_rate(SDRKIT_HANDLE handle, sdrkit_cubesat_tx_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_tx_rate)index;
  return code;
}
static inline int32_t sdrkit_cubesat_set_tx_rate(SDRKIT_HANDLE handle, sdrkit_cubesat_tx_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_rate", (int32_t)value);
}

// The setting "tx_delay" (Flags before) of the stack "cubesat". Unit: ms. The time of HDLC flags before the first frame, for the receiver to lock.
static inline int32_t sdrkit_cubesat_get_tx_delay(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_delay", value);
}
static inline int32_t sdrkit_cubesat_set_tx_delay(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_delay", value);
}

// The setting "tx_level" (Level) of the stack "cubesat". Unit: of full scale. The magnitude of the signal sent, of full scale.
static inline int32_t sdrkit_cubesat_get_tx_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_level", value);
}
static inline int32_t sdrkit_cubesat_set_tx_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_level", value);
}

// The setting "gen_framing" (Framing) of the test signal generator of the stack "cubesat". A value of `sdrkit_cubesat_gen_framing`.
static inline int32_t sdrkit_cubesat_gen_get_framing(SDRKIT_HANDLE handle, sdrkit_cubesat_gen_framing *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_framing", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cubesat_gen_framing)index;
  return code;
}
static inline int32_t sdrkit_cubesat_gen_set_framing(SDRKIT_HANDLE handle, sdrkit_cubesat_gen_framing value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_framing", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "fleet_broadcast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fleet_broadcast_channels_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_fleet_broadcast_channel_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.FleetBroadcastChannelRow", "channels", rows, sizeof(sdrkit_fleet_broadcast_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frame_log" of the stack "fleet_broadcast" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fleet_broadcast_frame_log_rows(SDRKIT_HANDLE handle,
                                                           sdrkit_fleet_broadcast_frame_row *rows,
                                                           uint32_t capacity,
                                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.FleetBroadcastFrameRow", "frame_log", rows, sizeof(sdrkit_fleet_broadcast_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fleet_broadcast.state"
// (Fleet Broadcast state). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fleet_broadcast_state_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fleet_broadcast_state_event *event) {
  return sdrkit_typed_event(handle, n, "fleet_broadcast.state", "satcom.FleetBroadcastStateEvent", event, sizeof(sdrkit_fleet_broadcast_state_event));
}

// Runs the command "clear" (Clear) of the stack "fleet_broadcast" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fleet_broadcast_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fleet_broadcast" are the sdrkit_fleet_broadcast_gen_*() functions
// after the settings of its receiver.

// The setting "invert" (Invert bits) of the stack "fleet_broadcast".
static inline int32_t sdrkit_fleet_broadcast_get_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "invert", value);
}
static inline int32_t sdrkit_fleet_broadcast_set_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "invert", value);
}

// The setting "gen_scenario" (Scenario) of the test signal generator of the stack "fleet_broadcast". A value of `sdrkit_fleet_broadcast_gen_scenario`.
static inline int32_t sdrkit_fleet_broadcast_gen_get_scenario(SDRKIT_HANDLE handle, sdrkit_fleet_broadcast_gen_scenario *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fleet_broadcast_gen_scenario)index;
  return code;
}
static inline int32_t sdrkit_fleet_broadcast_gen_set_scenario(SDRKIT_HANDLE handle, sdrkit_fleet_broadcast_gen_scenario value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scenario", (int32_t)value);
}

// The setting "gen_carrier_hz" (Carrier from the dial) of the test signal generator of the stack "fleet_broadcast". Unit: Hz.
static inline int32_t sdrkit_fleet_broadcast_gen_get_carrier_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_hz", value);
}
static inline int32_t sdrkit_fleet_broadcast_gen_set_carrier_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_hz", value);
}

// The setting "gen_fade_db" (Fading depth) of the test signal generator of the stack "fleet_broadcast". Unit: dB. 0 for none; else the depth of a flat fading at 0.5 Hz.
static inline int32_t sdrkit_fleet_broadcast_gen_get_fade_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fade_db", value);
}
static inline int32_t sdrkit_fleet_broadcast_gen_set_fade_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fade_db", value);
}

// The setting "gen_invert" (Invert bits) of the test signal generator of the stack "fleet_broadcast".
static inline int32_t sdrkit_fleet_broadcast_gen_get_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert", value);
}
static inline int32_t sdrkit_fleet_broadcast_gen_set_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert", value);
}

// Fills `rows` with the changed rows of the table "carrier_table" of the stack "bgan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_bgan_carrier_table_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_bgan_carrier_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.BganCarrierRow", "carrier_table", rows, sizeof(sdrkit_bgan_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "beam_table" of the stack "bgan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_bgan_beam_table_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_bgan_beam_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.BganBeamRow", "beam_table", rows, sizeof(sdrkit_bgan_beam_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "log" of the stack "bgan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_bgan_log_rows(SDRKIT_HANDLE handle,
                                          sdrkit_bgan_log_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.BganLogRow", "log", rows, sizeof(sdrkit_bgan_log_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "bgan.signalling"
// (BGAN signalling). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_bgan_signalling_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_bgan_signalling_event *event) {
  return sdrkit_typed_event(handle, n, "bgan.signalling", "satcom.BganSignallingEvent", event, sizeof(sdrkit_bgan_signalling_event));
}

// Runs the command "clear" (Clear) of the stack "bgan" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_bgan_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "bgan" are the sdrkit_bgan_gen_*() functions
// after the settings of its receiver.

// The setting "decode_all" (Decode every FEC block) of the stack "bgan". Off: block 0 of each frame only.
static inline int32_t sdrkit_bgan_get_decode_all(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_all", value);
}
static inline int32_t sdrkit_bgan_set_decode_all(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_all", value);
}

// The setting "bearer" (Bearer) of the stack "bgan". A value of `sdrkit_bgan_bearer`.
static inline int32_t sdrkit_bgan_get_bearer(SDRKIT_HANDLE handle, sdrkit_bgan_bearer *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bearer", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bgan_bearer)index;
  return code;
}
static inline int32_t sdrkit_bgan_set_bearer(SDRKIT_HANDLE handle, sdrkit_bgan_bearer value) {
  return sdrkit_typed_setting_set_enum(handle, "bearer", (int32_t)value);
}

// The setting "uw_order" (Unique word bit order) of the stack "bgan". A value of `sdrkit_bgan_uw_order`. No public document states which end of the printed hex is sent first.
static inline int32_t sdrkit_bgan_get_uw_order(SDRKIT_HANDLE handle, sdrkit_bgan_uw_order *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "uw_order", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bgan_uw_order)index;
  return code;
}
static inline int32_t sdrkit_bgan_set_uw_order(SDRKIT_HANDLE handle, sdrkit_bgan_uw_order value) {
  return sdrkit_typed_setting_set_enum(handle, "uw_order", (int32_t)value);
}

// The setting "iq_labels" (I and Q columns) of the stack "bgan". A value of `sdrkit_bgan_iq_labels`. How the columns of the 38 tables labelled with I and Q map to the bits of a symbol; no public document states it.
static inline int32_t sdrkit_bgan_get_iq_labels(SDRKIT_HANDLE handle, sdrkit_bgan_iq_labels *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "iq_labels", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bgan_iq_labels)index;
  return code;
}
static inline int32_t sdrkit_bgan_set_iq_labels(SDRKIT_HANDLE handle, sdrkit_bgan_iq_labels value) {
  return sdrkit_typed_setting_set_enum(handle, "iq_labels", (int32_t)value);
}

// The setting "satellites" (Satellite names) of the stack "bgan". One a line (or separated by ;): longitude in degrees east, name. A SatelliteLocation within 1.5 degrees takes the name; without one the longitude is shown.
static inline int32_t sdrkit_bgan_get_satellites(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "satellites", value);
}
static inline int32_t sdrkit_bgan_set_satellites(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "satellites", value);
}

// The setting "scan" (Scan the band) of the stack "bgan". Asks the host to visit the band from scan_from_hz to scan_to_hz in steps of the sample rate, 7 s on each. The carrier rows stay with their last state.
static inline int32_t sdrkit_bgan_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_bgan_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "scan_from_hz" (Scan from) of the stack "bgan". A frequency in Hz.
static inline int32_t sdrkit_bgan_get_scan_from_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_from_hz", value);
}
static inline int32_t sdrkit_bgan_set_scan_from_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_from_hz", value);
}

// The setting "scan_to_hz" (Scan to) of the stack "bgan". A frequency in Hz.
static inline int32_t sdrkit_bgan_get_scan_to_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_to_hz", value);
}
static inline int32_t sdrkit_bgan_set_scan_to_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_to_hz", value);
}

// The setting "gen_bearer" (Bearers) of the test signal generator of the stack "bgan". A value of `sdrkit_bgan_gen_bearer`.
static inline int32_t sdrkit_bgan_gen_get_bearer(SDRKIT_HANDLE handle, sdrkit_bgan_gen_bearer *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bearer", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bgan_gen_bearer)index;
  return code;
}
static inline int32_t sdrkit_bgan_gen_set_bearer(SDRKIT_HANDLE handle, sdrkit_bgan_gen_bearer value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bearer", (int32_t)value);
}

// The setting "gen_subtype" (Subtype) of the test signal generator of the stack "bgan". A value of `sdrkit_bgan_gen_subtype`.
static inline int32_t sdrkit_bgan_gen_get_subtype(SDRKIT_HANDLE handle, sdrkit_bgan_gen_subtype *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_subtype", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_bgan_gen_subtype)index;
  return code;
}
static inline int32_t sdrkit_bgan_gen_set_subtype(SDRKIT_HANDLE handle, sdrkit_bgan_gen_subtype value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_subtype", (int32_t)value);
}

// The setting "gen_carrier_cn0" (Carrier C/N0) of the test signal generator of the stack "bgan". Unit: dB-Hz.
static inline int32_t sdrkit_bgan_gen_get_carrier_cn0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_cn0", value);
}
static inline int32_t sdrkit_bgan_gen_set_carrier_cn0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_cn0", value);
}

// The setting "gen_carrier_offset_hz" (Carrier from the dial) of the test signal generator of the stack "bgan". Unit: Hz.
static inline int32_t sdrkit_bgan_gen_get_carrier_offset_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_offset_hz", value);
}
static inline int32_t sdrkit_bgan_gen_set_carrier_offset_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_offset_hz", value);
}

// The setting "gen_carrier_clock_ppm" (Symbol clock error) of the test signal generator of the stack "bgan". Unit: ppm.
static inline int32_t sdrkit_bgan_gen_get_carrier_clock_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_clock_ppm", value);
}
static inline int32_t sdrkit_bgan_gen_set_carrier_clock_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_clock_ppm", value);
}

// The setting "gen_carrier_drift_hz_s" (Carrier drift) of the test signal generator of the stack "bgan". Unit: Hz/s.
static inline int32_t sdrkit_bgan_gen_get_carrier_drift_hz_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_drift_hz_s", value);
}
static inline int32_t sdrkit_bgan_gen_set_carrier_drift_hz_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_drift_hz_s", value);
}

// The setting "gen_fade_hz" (Fading rate) of the test signal generator of the stack "bgan". Unit: Hz. 0 for none.
static inline int32_t sdrkit_bgan_gen_get_fade_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fade_hz", value);
}
static inline int32_t sdrkit_bgan_gen_set_fade_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fade_hz", value);
}

// The setting "gen_fade_depth" (Fading depth) of the test signal generator of the stack "bgan". The share of the amplitude that the fading takes away at its deepest.
static inline int32_t sdrkit_bgan_gen_get_fade_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fade_depth", value);
}
static inline int32_t sdrkit_bgan_gen_set_fade_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fade_depth", value);
}

// The setting "gen_spot" (Spot beam) of the test signal generator of the stack "bgan".
static inline int32_t sdrkit_bgan_gen_get_spot(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_spot", value);
}
static inline int32_t sdrkit_bgan_gen_set_spot(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_spot", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "sat_survey" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sat_survey_carriers_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_sat_survey_carrier_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.SatSurveyCarrierRow", "carriers", rows, sizeof(sdrkit_sat_survey_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "sat_survey" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sat_survey_channels_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_sat_survey_channel_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "satcom.SatSurveyChannelRow", "channels", rows, sizeof(sdrkit_sat_survey_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "survey.carrier"
// (Carrier line). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sat_survey_carrier_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sat_survey_carrier_event *event) {
  return sdrkit_typed_event(handle, n, "survey.carrier", "satcom.SatSurveyCarrierEvent", event, sizeof(sdrkit_sat_survey_carrier_event));
}

// Runs the command "clear" (Clear) of the stack "sat_survey" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_sat_survey_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sat_survey" are the sdrkit_sat_survey_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Threshold) of the stack "sat_survey". Unit: dB. The detection threshold: 3 to 30 dB, 0 for the submode's own.
static inline int32_t sdrkit_sat_survey_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_sat_survey_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "channels" (Channel table) of the stack "sat_survey". Channels as text, one a line (or separated by ;): frequency_hz,name[,norad]. Lines of the data file (submode,frequency_hz,name,norad) are taken for this submode only. A measured frequency takes the nearest name within max(3.5 kHz, 30 ppm).
static inline int32_t sdrkit_sat_survey_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_sat_survey_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "satellites" (Satellite elements) of the stack "sat_survey". Two-line element sets (each with or without a name line, checksums checked). With the station position they name the finished carrier lines of the submodes with a Doppler match.
static inline int32_t sdrkit_sat_survey_get_satellites(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "satellites", value);
}
static inline int32_t sdrkit_sat_survey_set_satellites(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "satellites", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "sat_survey". Unit: s.
static inline int32_t sdrkit_sat_survey_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_sat_survey_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_signal" (Test signal) of the test signal generator of the stack "sat_survey". The test signal holds the signal of the submode's method; off, noise only.
static inline int32_t sdrkit_sat_survey_gen_get_signal(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_signal", value);
}
static inline int32_t sdrkit_sat_survey_gen_set_signal(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_signal", value);
}

// The setting "gen_level_db" (Test signal S/N) of the test signal generator of the stack "sat_survey". Unit: dB. Carrier and channel methods: S/N in 1 kHz (0 gives 18 and 30 dB). Frame methods: S/N over the whole rate (0 gives 0 dB).
static inline int32_t sdrkit_sat_survey_gen_get_level_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_level_db", value);
}
static inline int32_t sdrkit_sat_survey_gen_set_level_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_level_db", value);
}

// The setting "gen_carrier_hz" (Test signal carrier offset) of the test signal generator of the stack "sat_survey". A frequency in Hz. Where the carrier of the carrier method sits, from the centre, before its slow Doppler curve.
static inline int32_t sdrkit_sat_survey_gen_get_carrier_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_hz", value);
}
static inline int32_t sdrkit_sat_survey_gen_set_carrier_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_hz", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_inmarsatc_fields_read(SDRKIT_HANDLE handle, sdrkit_inmarsatc_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.InmarsatcFields", fields, sizeof(sdrkit_inmarsatc_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_satcom_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_satcom_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.DecoderFields", fields, sizeof(sdrkit_satcom_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_aero_fields_read(SDRKIT_HANDLE handle, sdrkit_aero_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.AeroFields", fields, sizeof(sdrkit_aero_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_inmarsat_fields_read(SDRKIT_HANDLE handle, sdrkit_inmarsat_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.InmarsatFields", fields, sizeof(sdrkit_inmarsat_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_iridium_fields_read(SDRKIT_HANDLE handle, sdrkit_iridium_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.IridiumFields", fields, sizeof(sdrkit_iridium_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_globalstar_fields_read(SDRKIT_HANDLE handle, sdrkit_globalstar_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.GlobalstarFields", fields, sizeof(sdrkit_globalstar_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_goes_dcs_fields_read(SDRKIT_HANDLE handle, sdrkit_goes_dcs_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.GoesDcsFields", fields, sizeof(sdrkit_goes_dcs_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_argos_fields_read(SDRKIT_HANDLE handle, sdrkit_argos_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.ArgosFields", fields, sizeof(sdrkit_argos_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hrdcp_fields_read(SDRKIT_HANDLE handle, sdrkit_hrdcp_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.HrdcpFields", fields, sizeof(sdrkit_hrdcp_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sarsat_fields_read(SDRKIT_HANDLE handle, sdrkit_sarsat_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.SarsatFields", fields, sizeof(sdrkit_sarsat_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_lora_sat_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_sat_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.LoraSatFields", fields, sizeof(sdrkit_lora_sat_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_orbcomm_fields_read(SDRKIT_HANDLE handle, sdrkit_orbcomm_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.OrbcommFields", fields, sizeof(sdrkit_orbcomm_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_psk_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_psk_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.PskDemodFields", fields, sizeof(sdrkit_psk_demod_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_qo100_fields_read(SDRKIT_HANDLE handle, sdrkit_qo100_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.Qo100Fields", fields, sizeof(sdrkit_qo100_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_qo100_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_qo100_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.Qo100TxFields", fields, sizeof(sdrkit_qo100_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cubesat_fields_read(SDRKIT_HANDLE handle, sdrkit_cubesat_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.CubesatFields", fields, sizeof(sdrkit_cubesat_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cubesat_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_cubesat_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.CubesatTxFields", fields, sizeof(sdrkit_cubesat_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fleet_broadcast_fields_read(SDRKIT_HANDLE handle, sdrkit_fleet_broadcast_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.FleetBroadcastFields", fields, sizeof(sdrkit_fleet_broadcast_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_bgan_fields_read(SDRKIT_HANDLE handle, sdrkit_bgan_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.BganFields", fields, sizeof(sdrkit_bgan_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sat_survey_fields_read(SDRKIT_HANDLE handle, sdrkit_sat_survey_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.SatSurveyFields", fields, sizeof(sdrkit_sat_survey_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "inmarsatc" in the
// snapshot of `handle`.
static inline int32_t sdrkit_inmarsatc_dropped_read(SDRKIT_HANDLE handle, sdrkit_inmarsatc_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.inmarsatc.dropped", dropped, sizeof(sdrkit_inmarsatc_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "aero" in the
// snapshot of `handle`.
static inline int32_t sdrkit_aero_dropped_read(SDRKIT_HANDLE handle, sdrkit_aero_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.aero.dropped", dropped, sizeof(sdrkit_aero_dropped));
}

// Fills `fields` with the result fields of the component "aero_display" of the stack "inmarsat" in the snapshot of
// `handle`. A generic consumer reads them under the keys "aero_display.<key>".
static inline int32_t sdrkit_inmarsat_aero_display_aero_fields_read(SDRKIT_HANDLE handle, sdrkit_aero_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.AeroFields@aero_display", fields, sizeof(sdrkit_aero_fields));
}

// Fills `fields` with the result fields of the component "egc" of the stack "inmarsat" in the snapshot of
// `handle`. A generic consumer reads them under the keys "egc.<key>".
static inline int32_t sdrkit_inmarsat_egc_inmarsatc_fields_read(SDRKIT_HANDLE handle, sdrkit_inmarsatc_fields *fields) {
  return sdrkit_typed_fields(handle, "satcom.InmarsatcFields@egc", fields, sizeof(sdrkit_inmarsatc_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "inmarsat" in the
// snapshot of `handle`.
static inline int32_t sdrkit_inmarsat_dropped_read(SDRKIT_HANDLE handle, sdrkit_inmarsat_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.inmarsat.dropped", dropped, sizeof(sdrkit_inmarsat_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "globalstar" in the
// snapshot of `handle`.
static inline int32_t sdrkit_globalstar_dropped_read(SDRKIT_HANDLE handle, sdrkit_globalstar_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.globalstar.dropped", dropped, sizeof(sdrkit_globalstar_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "goes_dcs" in the
// snapshot of `handle`.
static inline int32_t sdrkit_goes_dcs_dropped_read(SDRKIT_HANDLE handle, sdrkit_goes_dcs_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.goes_dcs.dropped", dropped, sizeof(sdrkit_goes_dcs_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "argos" in the
// snapshot of `handle`.
static inline int32_t sdrkit_argos_dropped_read(SDRKIT_HANDLE handle, sdrkit_argos_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.argos.dropped", dropped, sizeof(sdrkit_argos_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "hrdcp" in the
// snapshot of `handle`.
static inline int32_t sdrkit_hrdcp_dropped_read(SDRKIT_HANDLE handle, sdrkit_hrdcp_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.hrdcp.dropped", dropped, sizeof(sdrkit_hrdcp_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "sarsat" in the
// snapshot of `handle`.
static inline int32_t sdrkit_sarsat_dropped_read(SDRKIT_HANDLE handle, sdrkit_sarsat_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.sarsat.dropped", dropped, sizeof(sdrkit_sarsat_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "lora_sat" in the
// snapshot of `handle`.
static inline int32_t sdrkit_lora_sat_dropped_read(SDRKIT_HANDLE handle, sdrkit_lora_sat_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.lora_sat.dropped", dropped, sizeof(sdrkit_lora_sat_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "orbcomm" in the
// snapshot of `handle`.
static inline int32_t sdrkit_orbcomm_dropped_read(SDRKIT_HANDLE handle, sdrkit_orbcomm_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.orbcomm.dropped", dropped, sizeof(sdrkit_orbcomm_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "psk_demod" in the
// snapshot of `handle`.
static inline int32_t sdrkit_psk_demod_dropped_read(SDRKIT_HANDLE handle, sdrkit_psk_demod_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.psk_demod.dropped", dropped, sizeof(sdrkit_psk_demod_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "qo100" in the
// snapshot of `handle`.
static inline int32_t sdrkit_qo100_dropped_read(SDRKIT_HANDLE handle, sdrkit_qo100_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.qo100.dropped", dropped, sizeof(sdrkit_qo100_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "cubesat" in the
// snapshot of `handle`.
static inline int32_t sdrkit_cubesat_dropped_read(SDRKIT_HANDLE handle, sdrkit_cubesat_dropped *dropped) {
  return sdrkit_typed_fields(handle, "satcom.cubesat.dropped", dropped, sizeof(sdrkit_cubesat_dropped));
}

#endif
