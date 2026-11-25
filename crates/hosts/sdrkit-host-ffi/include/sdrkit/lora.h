/*
 * sdrkit: the typed interface of the stack family "lora", version 4.0. MIT licence.
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

#ifndef SDRKIT_LORA_H
#define SDRKIT_LORA_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_LORA_VERSION_MAJOR 4
#define SDRKIT_LORA_VERSION_MINOR 0

// The stack "mesh": its id, and the keys of its event types.
#define SDRKIT_STACK_MESH "mesh"
#define SDRKIT_EVENT_LORA_PACKET "lora.packet"
#define SDRKIT_EVENT_MESH_TEXT "mesh.text"
#define SDRKIT_EVENT_MESH_NODE "mesh.node"
#define SDRKIT_EVENT_TX_REQUEST "tx.request"

// The stack "meshtastic": its id, and the keys of its event types.
#define SDRKIT_STACK_MESHTASTIC "meshtastic"

// The stack "meshcore": its id, and the keys of its event types.
#define SDRKIT_STACK_MESHCORE "meshcore"

// The stack "lorawan": its id, and the keys of its event types.
#define SDRKIT_STACK_LORAWAN "lorawan"

// The stack "loraaprs": its id, and the keys of its event types.
#define SDRKIT_STACK_LORAAPRS "loraaprs"

// The stack "meshcom": its id, and the keys of its event types.
#define SDRKIT_STACK_MESHCOM "meshcom"

// The stack "lorawanwide": its id, and the keys of its event types.
#define SDRKIT_STACK_LORAWANWIDE "lorawanwide"

// The stack "loraraw": its id, and the keys of its event types.
#define SDRKIT_STACK_LORARAW "loraraw"

// The stack "aprs": its id, and the keys of its event types.
#define SDRKIT_STACK_APRS "aprs"
#define SDRKIT_EVENT_APRS_PACKET "aprs.packet"

// The stack "fanet": its id, and the keys of its event types.
#define SDRKIT_STACK_FANET "fanet"
#define SDRKIT_EVENT_FANET_STATION "fanet.station"
#define SDRKIT_EVENT_FANET_MESSAGE "fanet.message"
#define SDRKIT_EVENT_FANET_DISTRESS "fanet.distress"

// The stack "reticulum": its id, and the keys of its event types.
#define SDRKIT_STACK_RETICULUM "reticulum"
#define SDRKIT_EVENT_RETICULUM_DESTINATION "reticulum.destination"
#define SDRKIT_EVENT_RETICULUM_ANNOUNCE "reticulum.announce"

// The stack "elrs": its id, and the keys of its event types.
#define SDRKIT_STACK_ELRS "elrs"
#define SDRKIT_EVENT_ELRS_LINK "elrs.link"

// The options of the setting "mt_preset" of the stack "mesh".
typedef enum sdrkit_mesh_mt_preset {
  // LongFast
  SDRKIT_MESH_MT_PRESET_LONG_FAST = 0,
  // LongSlow
  SDRKIT_MESH_MT_PRESET_LONG_SLOW = 1,
  // MediumSlow
  SDRKIT_MESH_MT_PRESET_MEDIUM_SLOW = 2,
  // MediumFast
  SDRKIT_MESH_MT_PRESET_MEDIUM_FAST = 3,
  // ShortSlow
  SDRKIT_MESH_MT_PRESET_SHORT_SLOW = 4,
  // ShortFast
  SDRKIT_MESH_MT_PRESET_SHORT_FAST = 5,
  // LongModerate
  SDRKIT_MESH_MT_PRESET_LONG_MODERATE = 6,
  // ShortTurbo
  SDRKIT_MESH_MT_PRESET_SHORT_TURBO = 7,
  // LongTurbo
  SDRKIT_MESH_MT_PRESET_LONG_TURBO = 8,
  // EdgeFastLow
  SDRKIT_MESH_MT_PRESET_EDGE_FAST_LOW = 9,
} sdrkit_mesh_mt_preset;

// The options of the setting "custom_bw" of the stack "mesh".
typedef enum sdrkit_mesh_custom_bw {
  // 7.8 kHz
  SDRKIT_MESH_CUSTOM_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_MESH_CUSTOM_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_MESH_CUSTOM_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_MESH_CUSTOM_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_MESH_CUSTOM_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_MESH_CUSTOM_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_MESH_CUSTOM_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_MESH_CUSTOM_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_MESH_CUSTOM_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_MESH_CUSTOM_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_MESH_CUSTOM_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_MESH_CUSTOM_BW_1625_KHZ = 11,
} sdrkit_mesh_custom_bw;

// The options of the setting "custom_cr" of the stack "mesh".
typedef enum sdrkit_mesh_custom_cr {
  // 4/5
  SDRKIT_MESH_CUSTOM_CR_4_5 = 0,
  // 4/6
  SDRKIT_MESH_CUSTOM_CR_4_6 = 1,
  // 4/7
  SDRKIT_MESH_CUSTOM_CR_4_7 = 2,
  // 4/8
  SDRKIT_MESH_CUSTOM_CR_4_8 = 3,
} sdrkit_mesh_custom_cr;

// The options of the setting "mt_role" of the stack "mesh".
typedef enum sdrkit_mesh_mt_role {
  // Client
  SDRKIT_MESH_MT_ROLE_C_L_I_E_N_T = 0,
  // Client (mute)
  SDRKIT_MESH_MT_ROLE_C_L_I_E_N_T_M_U_T_E = 1,
  // Client (hidden)
  SDRKIT_MESH_MT_ROLE_C_L_I_E_N_T_H_I_D_D_E_N = 2,
} sdrkit_mesh_mt_role;

// The options of the setting "mt_position_mode" of the stack "mesh".
typedef enum sdrkit_mesh_mt_position_mode {
  // Off
  SDRKIT_MESH_MT_POSITION_MODE_OFF = 0,
  // The station's
  SDRKIT_MESH_MT_POSITION_MODE_STATION = 1,
} sdrkit_mesh_mt_position_mode;

// The options of the setting "gen_scene" of the stack "mesh".
typedef enum sdrkit_mesh_gen_scene {
  // The stack's
  SDRKIT_MESH_GEN_SCENE_AUTO = 0,
  // Meshtastic and MeshCore
  SDRKIT_MESH_GEN_SCENE_TEXTING = 1,
  // LoRa APRS and MeshCom
  SDRKIT_MESH_GEN_SCENE_AMATEUR = 2,
  // LoRaWAN
  SDRKIT_MESH_GEN_SCENE_LORAWAN = 3,
} sdrkit_mesh_gen_scene;

// The options of the setting "mt_preset" of the stack "meshtastic".
typedef enum sdrkit_meshtastic_mt_preset {
  // LongFast
  SDRKIT_MESHTASTIC_MT_PRESET_LONG_FAST = 0,
  // LongSlow
  SDRKIT_MESHTASTIC_MT_PRESET_LONG_SLOW = 1,
  // MediumSlow
  SDRKIT_MESHTASTIC_MT_PRESET_MEDIUM_SLOW = 2,
  // MediumFast
  SDRKIT_MESHTASTIC_MT_PRESET_MEDIUM_FAST = 3,
  // ShortSlow
  SDRKIT_MESHTASTIC_MT_PRESET_SHORT_SLOW = 4,
  // ShortFast
  SDRKIT_MESHTASTIC_MT_PRESET_SHORT_FAST = 5,
  // LongModerate
  SDRKIT_MESHTASTIC_MT_PRESET_LONG_MODERATE = 6,
  // ShortTurbo
  SDRKIT_MESHTASTIC_MT_PRESET_SHORT_TURBO = 7,
  // LongTurbo
  SDRKIT_MESHTASTIC_MT_PRESET_LONG_TURBO = 8,
  // EdgeFastLow
  SDRKIT_MESHTASTIC_MT_PRESET_EDGE_FAST_LOW = 9,
} sdrkit_meshtastic_mt_preset;

// The options of the setting "custom_bw" of the stack "meshtastic".
typedef enum sdrkit_meshtastic_custom_bw {
  // 7.8 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_MESHTASTIC_CUSTOM_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_MESHTASTIC_CUSTOM_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_MESHTASTIC_CUSTOM_BW_1625_KHZ = 11,
} sdrkit_meshtastic_custom_bw;

// The options of the setting "custom_cr" of the stack "meshtastic".
typedef enum sdrkit_meshtastic_custom_cr {
  // 4/5
  SDRKIT_MESHTASTIC_CUSTOM_CR_4_5 = 0,
  // 4/6
  SDRKIT_MESHTASTIC_CUSTOM_CR_4_6 = 1,
  // 4/7
  SDRKIT_MESHTASTIC_CUSTOM_CR_4_7 = 2,
  // 4/8
  SDRKIT_MESHTASTIC_CUSTOM_CR_4_8 = 3,
} sdrkit_meshtastic_custom_cr;

// The options of the setting "mt_role" of the stack "meshtastic".
typedef enum sdrkit_meshtastic_mt_role {
  // Client
  SDRKIT_MESHTASTIC_MT_ROLE_C_L_I_E_N_T = 0,
  // Client (mute)
  SDRKIT_MESHTASTIC_MT_ROLE_C_L_I_E_N_T_M_U_T_E = 1,
  // Client (hidden)
  SDRKIT_MESHTASTIC_MT_ROLE_C_L_I_E_N_T_H_I_D_D_E_N = 2,
} sdrkit_meshtastic_mt_role;

// The options of the setting "mt_position_mode" of the stack "meshtastic".
typedef enum sdrkit_meshtastic_mt_position_mode {
  // Off
  SDRKIT_MESHTASTIC_MT_POSITION_MODE_OFF = 0,
  // The station's
  SDRKIT_MESHTASTIC_MT_POSITION_MODE_STATION = 1,
} sdrkit_meshtastic_mt_position_mode;

// The options of the setting "gen_scene" of the stack "meshtastic".
typedef enum sdrkit_meshtastic_gen_scene {
  // The stack's
  SDRKIT_MESHTASTIC_GEN_SCENE_AUTO = 0,
  // Meshtastic and MeshCore
  SDRKIT_MESHTASTIC_GEN_SCENE_TEXTING = 1,
  // LoRa APRS and MeshCom
  SDRKIT_MESHTASTIC_GEN_SCENE_AMATEUR = 2,
  // LoRaWAN
  SDRKIT_MESHTASTIC_GEN_SCENE_LORAWAN = 3,
} sdrkit_meshtastic_gen_scene;

// The options of the setting "custom_bw" of the stack "meshcore".
typedef enum sdrkit_meshcore_custom_bw {
  // 7.8 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_MESHCORE_CUSTOM_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_MESHCORE_CUSTOM_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_MESHCORE_CUSTOM_BW_1625_KHZ = 11,
} sdrkit_meshcore_custom_bw;

// The options of the setting "custom_cr" of the stack "meshcore".
typedef enum sdrkit_meshcore_custom_cr {
  // 4/5
  SDRKIT_MESHCORE_CUSTOM_CR_4_5 = 0,
  // 4/6
  SDRKIT_MESHCORE_CUSTOM_CR_4_6 = 1,
  // 4/7
  SDRKIT_MESHCORE_CUSTOM_CR_4_7 = 2,
  // 4/8
  SDRKIT_MESHCORE_CUSTOM_CR_4_8 = 3,
} sdrkit_meshcore_custom_cr;

// The options of the setting "gen_scene" of the stack "meshcore".
typedef enum sdrkit_meshcore_gen_scene {
  // The stack's
  SDRKIT_MESHCORE_GEN_SCENE_AUTO = 0,
  // Meshtastic and MeshCore
  SDRKIT_MESHCORE_GEN_SCENE_TEXTING = 1,
  // LoRa APRS and MeshCom
  SDRKIT_MESHCORE_GEN_SCENE_AMATEUR = 2,
  // LoRaWAN
  SDRKIT_MESHCORE_GEN_SCENE_LORAWAN = 3,
} sdrkit_meshcore_gen_scene;

// The options of the setting "custom_bw" of the stack "lorawan".
typedef enum sdrkit_lorawan_custom_bw {
  // 7.8 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_LORAWAN_CUSTOM_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_LORAWAN_CUSTOM_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_LORAWAN_CUSTOM_BW_1625_KHZ = 11,
} sdrkit_lorawan_custom_bw;

// The options of the setting "custom_cr" of the stack "lorawan".
typedef enum sdrkit_lorawan_custom_cr {
  // 4/5
  SDRKIT_LORAWAN_CUSTOM_CR_4_5 = 0,
  // 4/6
  SDRKIT_LORAWAN_CUSTOM_CR_4_6 = 1,
  // 4/7
  SDRKIT_LORAWAN_CUSTOM_CR_4_7 = 2,
  // 4/8
  SDRKIT_LORAWAN_CUSTOM_CR_4_8 = 3,
} sdrkit_lorawan_custom_cr;

// The options of the setting "gen_scene" of the stack "lorawan".
typedef enum sdrkit_lorawan_gen_scene {
  // The stack's
  SDRKIT_LORAWAN_GEN_SCENE_AUTO = 0,
  // Meshtastic and MeshCore
  SDRKIT_LORAWAN_GEN_SCENE_TEXTING = 1,
  // LoRa APRS and MeshCom
  SDRKIT_LORAWAN_GEN_SCENE_AMATEUR = 2,
  // LoRaWAN
  SDRKIT_LORAWAN_GEN_SCENE_LORAWAN = 3,
} sdrkit_lorawan_gen_scene;

// The options of the setting "custom_bw" of the stack "loraaprs".
typedef enum sdrkit_loraaprs_custom_bw {
  // 7.8 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_LORAAPRS_CUSTOM_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_LORAAPRS_CUSTOM_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_LORAAPRS_CUSTOM_BW_1625_KHZ = 11,
} sdrkit_loraaprs_custom_bw;

// The options of the setting "custom_cr" of the stack "loraaprs".
typedef enum sdrkit_loraaprs_custom_cr {
  // 4/5
  SDRKIT_LORAAPRS_CUSTOM_CR_4_5 = 0,
  // 4/6
  SDRKIT_LORAAPRS_CUSTOM_CR_4_6 = 1,
  // 4/7
  SDRKIT_LORAAPRS_CUSTOM_CR_4_7 = 2,
  // 4/8
  SDRKIT_LORAAPRS_CUSTOM_CR_4_8 = 3,
} sdrkit_loraaprs_custom_cr;

// The options of the setting "gen_scene" of the stack "loraaprs".
typedef enum sdrkit_loraaprs_gen_scene {
  // The stack's
  SDRKIT_LORAAPRS_GEN_SCENE_AUTO = 0,
  // Meshtastic and MeshCore
  SDRKIT_LORAAPRS_GEN_SCENE_TEXTING = 1,
  // LoRa APRS and MeshCom
  SDRKIT_LORAAPRS_GEN_SCENE_AMATEUR = 2,
  // LoRaWAN
  SDRKIT_LORAAPRS_GEN_SCENE_LORAWAN = 3,
} sdrkit_loraaprs_gen_scene;

// The options of the setting "custom_bw" of the stack "meshcom".
typedef enum sdrkit_meshcom_custom_bw {
  // 7.8 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_MESHCOM_CUSTOM_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_MESHCOM_CUSTOM_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_MESHCOM_CUSTOM_BW_1625_KHZ = 11,
} sdrkit_meshcom_custom_bw;

// The options of the setting "custom_cr" of the stack "meshcom".
typedef enum sdrkit_meshcom_custom_cr {
  // 4/5
  SDRKIT_MESHCOM_CUSTOM_CR_4_5 = 0,
  // 4/6
  SDRKIT_MESHCOM_CUSTOM_CR_4_6 = 1,
  // 4/7
  SDRKIT_MESHCOM_CUSTOM_CR_4_7 = 2,
  // 4/8
  SDRKIT_MESHCOM_CUSTOM_CR_4_8 = 3,
} sdrkit_meshcom_custom_cr;

// The options of the setting "gen_scene" of the stack "meshcom".
typedef enum sdrkit_meshcom_gen_scene {
  // The stack's
  SDRKIT_MESHCOM_GEN_SCENE_AUTO = 0,
  // Meshtastic and MeshCore
  SDRKIT_MESHCOM_GEN_SCENE_TEXTING = 1,
  // LoRa APRS and MeshCom
  SDRKIT_MESHCOM_GEN_SCENE_AMATEUR = 2,
  // LoRaWAN
  SDRKIT_MESHCOM_GEN_SCENE_LORAWAN = 3,
} sdrkit_meshcom_gen_scene;

// The options of the setting "bw" of the stack "loraraw".
typedef enum sdrkit_loraraw_bw {
  // 7.8 kHz
  SDRKIT_LORARAW_BW_7812_5 = 0,
  // 10.4 kHz
  SDRKIT_LORARAW_BW_10_4_KHZ = 1,
  // 15.6 kHz
  SDRKIT_LORARAW_BW_15_6_KHZ = 2,
  // 20.8 kHz
  SDRKIT_LORARAW_BW_20_8_KHZ = 3,
  // 31.25 kHz
  SDRKIT_LORARAW_BW_31_25_KHZ = 4,
  // 41.7 kHz
  SDRKIT_LORARAW_BW_41_7_KHZ = 5,
  // 62.5 kHz
  SDRKIT_LORARAW_BW_62_5_KHZ = 6,
  // 125 kHz
  SDRKIT_LORARAW_BW_125_KHZ = 7,
  // 250 kHz
  SDRKIT_LORARAW_BW_250_KHZ = 8,
  // 500 kHz
  SDRKIT_LORARAW_BW_500_KHZ = 9,
  // 812.5 kHz (2.4 GHz)
  SDRKIT_LORARAW_BW_812_5_KHZ = 10,
  // 1625 kHz (2.4 GHz)
  SDRKIT_LORARAW_BW_1625_KHZ = 11,
} sdrkit_loraraw_bw;

// The options of the setting "sync" of the stack "loraraw".
typedef enum sdrkit_loraraw_sync {
  // 0x12 private
  SDRKIT_LORARAW_SYNC_0X12 = 0,
  // 0x34 LoRaWAN
  SDRKIT_LORARAW_SYNC_0X34 = 1,
  // 0x2B Meshtastic
  SDRKIT_LORARAW_SYNC_0X2_B = 2,
  // Any of these
  SDRKIT_LORARAW_SYNC_ANY = 3,
  // Custom
  SDRKIT_LORARAW_SYNC_CUSTOM = 4,
} sdrkit_loraraw_sync;

// The options of the setting "ldro" of the stack "loraraw".
typedef enum sdrkit_loraraw_ldro {
  // Automatic
  SDRKIT_LORARAW_LDRO_AUTO = 0,
  // Off
  SDRKIT_LORARAW_LDRO_OFF = 1,
  // On
  SDRKIT_LORARAW_LDRO_ON = 2,
} sdrkit_loraraw_ldro;

// The options of the setting "emphasis" of the stack "aprs".
typedef enum sdrkit_aprs_emphasis {
  // None
  SDRKIT_APRS_EMPHASIS_AUTO = 0,
  // De-emphasis
  SDRKIT_APRS_EMPHASIS_DE = 1,
  // Pre-emphasis
  SDRKIT_APRS_EMPHASIS_PRE = 2,
} sdrkit_aprs_emphasis;

// The options of the setting "tx_baud" of the stack "aprs".
typedef enum sdrkit_aprs_tx_baud {
  // 1200 bit/s
  SDRKIT_APRS_TX_BAUD_1200_BIT_S = 0,
  // 9600 bit/s
  SDRKIT_APRS_TX_BAUD_9600_BIT_S = 1,
} sdrkit_aprs_tx_baud;

// The options of the setting "gen_option" of the stack "aprs".
typedef enum sdrkit_aprs_gen_option {
  // 1200 and 9600 bit/s
  SDRKIT_APRS_GEN_OPTION_BOTH = 0,
  // Only 1200 bit/s
  SDRKIT_APRS_GEN_OPTION_ONLY_1200_BIT_S = 1,
  // Only 9600 bit/s
  SDRKIT_APRS_GEN_OPTION_ONLY_9600_BIT_S = 2,
} sdrkit_aprs_gen_option;

// The options of the setting "bandwidth" of the stack "fanet".
typedef enum sdrkit_fanet_bandwidth {
  // 125 kHz
  SDRKIT_FANET_BANDWIDTH_125_KHZ = 0,
  // 250 kHz
  SDRKIT_FANET_BANDWIDTH_250_KHZ = 1,
  // 500 kHz
  SDRKIT_FANET_BANDWIDTH_500_KHZ = 2,
} sdrkit_fanet_bandwidth;

// The options of the setting "bw" of the stack "reticulum".
typedef enum sdrkit_reticulum_bw {
  // 62.5 kHz
  SDRKIT_RETICULUM_BW_62_5_KHZ = 0,
  // 125 kHz
  SDRKIT_RETICULUM_BW_125_KHZ = 1,
  // 250 kHz
  SDRKIT_RETICULUM_BW_250_KHZ = 2,
  // 500 kHz
  SDRKIT_RETICULUM_BW_500_KHZ = 3,
} sdrkit_reticulum_bw;

// The options of the setting "packet_rate" of the stack "elrs".
typedef enum sdrkit_elrs_packet_rate {
  // All rates
  SDRKIT_ELRS_PACKET_RATE_ALL = 0,
  // 200 Hz
  SDRKIT_ELRS_PACKET_RATE_200_HZ = 1,
  // 100 Hz
  SDRKIT_ELRS_PACKET_RATE_100_HZ = 2,
  // 50 Hz
  SDRKIT_ELRS_PACKET_RATE_50_HZ = 3,
  // 25 Hz
  SDRKIT_ELRS_PACKET_RATE_25_HZ = 4,
  // 100 Hz Full
  SDRKIT_ELRS_PACKET_RATE_100FULL = 5,
} sdrkit_elrs_packet_rate;

// The result fields of the mesh decoder.
typedef struct sdrkit_mesh_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a good CRC since the last reset.
  int64_t frames_ok;
  // Frames whose CRC failed since the last reset.
  int64_t frames_bad;
  // Frame preambles seen.
  int64_t preambles;
  // Headers that failed their checksum.
  int64_t header_failed;
  // Meshtastic nodes.
  int64_t mt_nodes;
  // MeshCore nodes.
  int64_t mc_nodes;
  // Messages in the table.
  int64_t messages;
  // APRS stations.
  int64_t stations;
  // LoRaWAN devices.
  int64_t devices;
  // The band plan.
  sdrkit_string plan;
  // What is searched.
  sdrkit_string searching;
  // The dial. A frequency in Hz.
  double dial;
  // The protocols searched, in words.
  sdrkit_string protocols;
  // The custom LoRa setting is searched.
  bool custom;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last good frame, while frames come. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last good frame, while frames come. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the bank reports. Unit: dBFS.
  double level_dbfs;
} sdrkit_mesh_fields;

// The own Meshtastic node, once the Meshtastic client reports.
typedef struct sdrkit_mesh_mt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `mt_node_id` holds a value.
  bool has_mt_node_id;
  // The own node id.
  sdrkit_string mt_node_id;
  // True if `mt_public_key` holds a value.
  bool has_mt_public_key;
  // The own public key, base64.
  sdrkit_string mt_public_key;
  // True if `mt_long_name` holds a value.
  bool has_mt_long_name;
  // The own long name.
  sdrkit_string mt_long_name;
  // True if `mt_short_name` holds a value.
  bool has_mt_short_name;
  // The own short name.
  sdrkit_string mt_short_name;
  // True if `mt_in_flight` holds a value.
  bool has_mt_in_flight;
  // Own packets waiting for an acknowledgement.
  int64_t mt_in_flight;
  // True if `mt_rebroadcasts` holds a value.
  bool has_mt_rebroadcasts;
  // Packets of other nodes sent again.
  int64_t mt_rebroadcasts;
  // True if `mt_warnings` holds a value.
  bool has_mt_warnings;
  // Warnings about the settings of the own node.
  sdrkit_string mt_warnings;
  // True if `mt_freq` holds a value.
  bool has_mt_freq;
  // The channel frequency that the client sends on, while the session listens to a channel of the network. A frequency in Hz.
  double mt_freq;
} sdrkit_mesh_mt_fields;

// The own MeshCore node, once the MeshCore client reports.
typedef struct sdrkit_mesh_mc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `mc_public_key` holds a value.
  bool has_mc_public_key;
  // The own public key, hex.
  sdrkit_string mc_public_key;
  // True if `mc_name` holds a value.
  bool has_mc_name;
  // The own name.
  sdrkit_string mc_name;
  // True if `mc_contacts` holds a value.
  bool has_mc_contacts;
  // The contacts.
  int64_t mc_contacts;
  // True if `mc_freq` holds a value.
  bool has_mc_freq;
  // The channel frequency that the client sends on, while the session listens to a channel of the network. A frequency in Hz.
  double mc_freq;
} sdrkit_mesh_mc_fields;

// The own APRS station, once the amateur client reports.
typedef struct sdrkit_lora_ham_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `aprs_call` holds a value.
  bool has_aprs_call;
  // The source call of own APRS frames: the callsign of the station with the SSID. Empty without a callsign.
  sdrkit_string aprs_call;
  // True if `aprs_freq` holds a value.
  bool has_aprs_freq;
  // The frequency that the client sends on, while the session listens to a channel of its link. A frequency in Hz.
  double aprs_freq;
} sdrkit_lora_ham_fields;

// The state of the transmit scheduler.
typedef struct sdrkit_mesh_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames waiting to be sent.
  int64_t tx_pending;
  // Frames sent.
  int64_t tx_sent;
} sdrkit_mesh_tx_fields;

// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.
typedef struct sdrkit_lora_guard_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Input samples replaced because they were not finite or far beyond full scale.
  int64_t bad_samples;
} sdrkit_lora_guard_fields;

// The counts of the LoRa modulator.
typedef struct sdrkit_lora_modulate_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts written to the output.
  int64_t tx_bursts;
  // Samples written to the output.
  int64_t tx_samples;
  // Frames that could not be modulated: a bad LoRa setting, a payload that does not fit, or a carrier outside the band.
  int64_t tx_dropped;
} sdrkit_lora_modulate_fields;

// The options of `net` of `sdrkit_mesh_packet_row`.
typedef enum sdrkit_mesh_packet_row_net {
  // Meshtastic
  SDRKIT_MESH_PACKET_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_PACKET_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_PACKET_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_PACKET_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_PACKET_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_PACKET_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_PACKET_ROW_NET_APRS = 6,
} sdrkit_mesh_packet_row_net;

// A row of the packet list. The row key is the number of the packet.
typedef struct sdrkit_mesh_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The protocol. A value of `sdrkit_mesh_packet_row_net`.
  int32_t net;
  // True if `type` holds a value.
  bool has_type;
  // The packet type, as the protocol names it.
  sdrkit_string type;
  // True if `source` holds a value.
  bool has_source;
  // The sender.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The addressee.
  sdrkit_string target;
  // The payload length.
  int64_t bytes;
  // True if `content` holds a value.
  bool has_content;
  // The content, decoded or raw.
  sdrkit_string content;
  // True if `hops` holds a value.
  bool has_hops;
  // The hops taken.
  int64_t hops;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
  // The spreading factor.
  int64_t sf;
  // The bandwidth. A frequency in Hz.
  double bw;
  // True if `summary` holds a value.
  bool has_summary;
  // More about the packet.
  sdrkit_string summary;
} sdrkit_mesh_packet_row;

// The options of `net` of `sdrkit_mesh_node_row`.
typedef enum sdrkit_mesh_node_row_net {
  // Meshtastic
  SDRKIT_MESH_NODE_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_NODE_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_NODE_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_NODE_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_NODE_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_NODE_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_NODE_ROW_NET_APRS = 6,
} sdrkit_mesh_node_row_net;

// A row of the node list. The row key is `<net key>/<id>`.
typedef struct sdrkit_mesh_node_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol. A value of `sdrkit_mesh_node_row_net`.
  int32_t net;
  // The node name.
  sdrkit_string name;
  // True if `short_` holds a value.
  bool has_short_;
  // The short name.
  sdrkit_string short_;
  // The node id.
  sdrkit_string id;
  // True if `hw` holds a value.
  bool has_hw;
  // The hardware model.
  sdrkit_string hw;
  // True if `role` holds a value.
  bool has_role;
  // The role.
  sdrkit_string role;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // The packets of the node.
  int64_t heard;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last packet. Unit: dB.
  double snr_db;
  // True if `hops` holds a value.
  bool has_hops;
  // The hops of the last packet.
  int64_t hops;
  // True if `battery_pct` holds a value.
  bool has_battery_pct;
  // The battery charge; above 100 the node is powered from outside. Unit: %.
  double battery_pct;
  // True if `voltage_v` holds a value.
  bool has_voltage_v;
  // The battery voltage. Unit: V.
  double voltage_v;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
} sdrkit_mesh_node_row;

// The options of `net` of `sdrkit_mesh_message_row`.
typedef enum sdrkit_mesh_message_row_net {
  // Meshtastic
  SDRKIT_MESH_MESSAGE_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_MESSAGE_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_MESSAGE_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_MESSAGE_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_MESSAGE_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_MESSAGE_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_MESSAGE_ROW_NET_APRS = 6,
} sdrkit_mesh_message_row_net;

// The options of `state` of `sdrkit_mesh_message_row`.
typedef enum sdrkit_mesh_message_row_state {
  // Queued
  SDRKIT_MESH_MESSAGE_ROW_STATE_QUEUED = 0,
  // Waiting for the duty cycle
  SDRKIT_MESH_MESSAGE_ROW_STATE_WAITING = 1,
  // Sent
  SDRKIT_MESH_MESSAGE_ROW_STATE_SENT = 2,
  // Acknowledged by the mesh
  SDRKIT_MESH_MESSAGE_ROW_STATE_ACKED_BY_MESH = 3,
  // Delivered
  SDRKIT_MESH_MESSAGE_ROW_STATE_DELIVERED = 4,
  // Relayed, not confirmed
  SDRKIT_MESH_MESSAGE_ROW_STATE_RELAYED = 5,
  // Rejected
  SDRKIT_MESH_MESSAGE_ROW_STATE_REJECTED = 6,
  // Failed
  SDRKIT_MESH_MESSAGE_ROW_STATE_FAILED = 7,
} sdrkit_mesh_message_row_state;

// A row of the messages. The row key is the place of the message in the log.
typedef struct sdrkit_mesh_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The protocol. A value of `sdrkit_mesh_message_row_net`.
  int32_t net;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel.
  sdrkit_string channel;
  // True if `source` holds a value.
  bool has_source;
  // The sender.
  sdrkit_string source;
  // True if `sender` holds a value.
  bool has_sender;
  // The name of the sender.
  sdrkit_string sender;
  // True if `target` holds a value.
  bool has_target;
  // The addressee.
  sdrkit_string target;
  // The text.
  sdrkit_string text;
  // True if `hops` holds a value.
  bool has_hops;
  // The hops taken.
  int64_t hops;
  // True if `state` holds a value.
  bool has_state;
  // The delivery state of an own message. A value of `sdrkit_mesh_message_row_state`.
  int32_t state;
  // True if `failure` holds a value.
  bool has_failure;
  // Why the delivery failed.
  sdrkit_string failure;
  // True if `relays` holds a value.
  bool has_relays;
  // The distinct relays heard for an own broadcast.
  int64_t relays;
  // True if `packet_id` holds a value.
  bool has_packet_id;
  // The packet id: the reply of `mt.send` for an own Meshtastic text, the message number of an APRS message (the reply of `aprs.send` for an own one). None for a protocol without packet ids.
  int64_t packet_id;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC when the text was received or sent, if the clock of the session is known. Seconds since 1970 (UTC).
  double utc;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the frame of a received text. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the frame of a received text, relative to the full scale of the input (not an absolute power). Unit: dBFS.
  double level_dbfs;
  // True if `crc_ok` holds a value.
  bool has_crc_ok;
  // The text comes from a copy with a good CRC. False for a damaged copy that was shown after the wait without a good copy; a later good copy replaces the text and sets it. None for an own message.
  bool crc_ok;
  // True if `copies` holds a value.
  bool has_copies;
  // The copies of the packet heard while the message is remembered (10 minutes), damaged ones included. None for an own message.
  int64_t copies;
} sdrkit_mesh_message_row;

// A row of the telemetry and position readings. The row key is the number of the reading.
typedef struct sdrkit_mesh_reading_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The node of the reading.
  sdrkit_string node;
  // True if `battery_pct` holds a value.
  bool has_battery_pct;
  // The battery charge. Unit: %.
  double battery_pct;
  // True if `voltage_v` holds a value.
  bool has_voltage_v;
  // The battery voltage. Unit: V.
  double voltage_v;
  // True if `ch_util_pct` holds a value.
  bool has_ch_util_pct;
  // The channel use. Unit: %.
  double ch_util_pct;
  // True if `air_tx_pct` holds a value.
  bool has_air_tx_pct;
  // The own air time. Unit: %.
  double air_tx_pct;
  // True if `temp_c` holds a value.
  bool has_temp_c;
  // The temperature. Unit: °C.
  double temp_c;
  // True if `humidity_pct` holds a value.
  bool has_humidity_pct;
  // The relative humidity. Unit: %.
  double humidity_pct;
  // True if `pressure_hpa` holds a value.
  bool has_pressure_hpa;
  // The barometric pressure. Unit: hPa.
  double pressure_hpa;
  // True if `position` holds a value.
  bool has_position;
  // The position.
  sdrkit_position position;
  // True if `alt_m` holds a value.
  bool has_alt_m;
  // The altitude. Unit: m.
  double alt_m;
} sdrkit_mesh_reading_row;

// The options of `net` of `sdrkit_mesh_station_row`.
typedef enum sdrkit_mesh_station_row_net {
  // Meshtastic
  SDRKIT_MESH_STATION_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_STATION_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_STATION_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_STATION_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_STATION_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_STATION_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_STATION_ROW_NET_APRS = 6,
} sdrkit_mesh_station_row_net;

// A row of the APRS stations of LoRa APRS and MeshCom. The row key is `<net key>/<call>`.
typedef struct sdrkit_mesh_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol. A value of `sdrkit_mesh_station_row_net`.
  int32_t net;
  // The call or the object name.
  sdrkit_string call;
  // True if `symbol` holds a value.
  bool has_symbol;
  // The map symbol.
  sdrkit_string symbol;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `type` holds a value.
  bool has_type;
  // The type of the last packet.
  sdrkit_string type;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // The packets of the station.
  int64_t heard;
  // The signal to noise ratio of the last packet. Unit: dB.
  double snr_db;
  // True if `path` holds a value.
  bool has_path;
  // The relay path.
  sdrkit_string path;
  // True if `comment` holds a value.
  bool has_comment;
  // The comment.
  sdrkit_string comment;
  // True if `battery_pct` holds a value.
  bool has_battery_pct;
  // The battery charge. Unit: %.
  double battery_pct;
} sdrkit_mesh_station_row;

// A row of the LoRaWAN devices. The row key is the device id.
typedef struct sdrkit_mesh_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The DevAddr or the DevEUI.
  sdrkit_string id;
  // True if `network` holds a value.
  bool has_network;
  // The network operator.
  sdrkit_string network;
  // True if `vendor` holds a value.
  bool has_vendor;
  // The maker, or the JoinEUI.
  sdrkit_string vendor;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // The frames of the device.
  int64_t heard;
  // The level of the last frame. Unit: dBFS.
  double level_dbfs;
  // The signal to noise ratio of the last frame. Unit: dB.
  double snr_db;
  // True if `fcnt` holds a value.
  bool has_fcnt;
  // The last frame counter.
  int64_t fcnt;
  // The frames that the counter missed.
  int64_t missed;
  // True if `sf` holds a value.
  bool has_sf;
  // The spreading factor of the last frame, none for FSK.
  int64_t sf;
  // The frequency of the last frame. A frequency in Hz.
  double freq;
  // Uplinks.
  int64_t uplinks;
  // Downlinks.
  int64_t downlinks;
  // Join requests.
  int64_t joins;
  // True if `type` holds a value.
  bool has_type;
  // The type of the last frame.
  sdrkit_string type;
  // True if `fport` holds a value.
  bool has_fport;
  // The last FPort.
  int64_t fport;
  // True if `mac` holds a value.
  bool has_mac;
  // The last MAC commands or status.
  sdrkit_string mac;
} sdrkit_mesh_device_row;

// The options of `net` of `sdrkit_mesh_type_row`.
typedef enum sdrkit_mesh_type_row_net {
  // Meshtastic
  SDRKIT_MESH_TYPE_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_TYPE_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_TYPE_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_TYPE_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_TYPE_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_TYPE_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_TYPE_ROW_NET_APRS = 6,
} sdrkit_mesh_type_row_net;

// A row of the packet counts by protocol and type. The row key is `<net key>/<type>`.
typedef struct sdrkit_mesh_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol. A value of `sdrkit_mesh_type_row_net`.
  int32_t net;
  // The packet type.
  sdrkit_string type;
  // The packets of the type.
  int64_t count;
} sdrkit_mesh_type_row;

// The options of `net` of `sdrkit_mesh_decoder_row`.
typedef enum sdrkit_mesh_decoder_row_net {
  // Meshtastic
  SDRKIT_MESH_DECODER_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_DECODER_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_DECODER_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_DECODER_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_DECODER_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_DECODER_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_DECODER_ROW_NET_APRS = 6,
} sdrkit_mesh_decoder_row_net;

// A row of the decoders: one searched LoRa setting. The row key is the entry number of the plan.
typedef struct sdrkit_mesh_decoder_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol. A value of `sdrkit_mesh_decoder_row_net`.
  int32_t net;
  // The preset.
  sdrkit_string preset;
  // The channel frequency. A frequency in Hz.
  double freq;
  // The channel offset from the dial. A frequency in Hz.
  double offset;
  // The spreading factor.
  int64_t sf;
  // The bandwidth. A frequency in Hz.
  double bw;
  // The coding rate denominator: 5 is 4/5, 8 is 4/8.
  int64_t cr;
  // The sync word.
  int64_t sync;
  // The preamble length.
  int64_t preamble;
  // Low data rate optimisation.
  bool ldro;
  // The channel lies inside the captured band.
  bool in_band;
  // Frames decoded.
  int64_t frames;
  // Frames lost to errors.
  int64_t errors;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last good frame. Unit: dB.
  double snr_db;
  // True if `cfo` holds a value.
  bool has_cfo;
  // The carrier offset of the last good frame. A frequency in Hz.
  double cfo;
  // True if `last_s` holds a value.
  bool has_last_s;
  // The signal time of the last frame. Unit: s.
  double last_s;
} sdrkit_mesh_decoder_row;

// A row of the MeshCore contacts. The row key is the hex of the public key.
typedef struct sdrkit_mesh_contact_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name.
  sdrkit_string name;
  // The first 12 hex digits of the public key.
  sdrkit_string key_;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet, when the clock is known. Seconds since 1970 (UTC).
  double last_heard;
  // The signal to noise ratio of the last packet. Unit: dB.
  double snr_db;
  // The learned path, or "flood".
  sdrkit_string path;
} sdrkit_mesh_contact_row;

// The options of `net` of `sdrkit_mesh_channel_row`.
typedef enum sdrkit_mesh_channel_row_net {
  // Meshtastic
  SDRKIT_MESH_CHANNEL_ROW_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_CHANNEL_ROW_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_CHANNEL_ROW_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_CHANNEL_ROW_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_CHANNEL_ROW_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_CHANNEL_ROW_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_CHANNEL_ROW_NET_APRS = 6,
} sdrkit_mesh_channel_row_net;

// A row of the channels with keys. The row key is `<net key>/<name>`. The key itself is never shown.
typedef struct sdrkit_mesh_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol. A value of `sdrkit_mesh_channel_row_net`.
  int32_t net;
  // The name of the channel.
  sdrkit_string channel;
  // The channel holds a key.
  bool key_set;
} sdrkit_mesh_channel_row;

// A row of the Meshtastic frequency slots of the primary preset in the slot band. The row key is the slot number with four digits.
typedef struct sdrkit_mesh_slot_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The slot number, from 1, as the setting `mt_channel_num` counts.
  int64_t slot;
  // The centre of the slot plus the setting `mt_freq_offset`. A frequency in Hz.
  double freq_hz;
  // The slot that the stack listens on: that of `mt_channel_num`, else of the hash of `mt_channel_name`; with `mt_override_freq` the slot whose frequency it is, and no row for a frequency between slots.
  bool current;
  // The slot of the default channel name of the primary preset.
  bool preset_default;
} sdrkit_mesh_slot_row;

// The reply of `mt.send`.
typedef struct sdrkit_lora_mt_send_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The packet id of the text: the column `packet_id` of its row in the messages and the field `packet_id` of its `mesh.text` events. The column `state` of the row says whether it was acknowledged.
  int64_t packet_id;
} sdrkit_lora_mt_send_reply;

// The reply of `aprs.send`.
typedef struct sdrkit_aprs_send_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The message number: the column `packet_id` of its row in the messages. The column `state` of the row says whether the addressee acknowledged it.
  int64_t packet_id;
} sdrkit_aprs_send_reply;

// The result fields of the LoRaWAN stack of every channel.
typedef struct sdrkit_lorawanwide_wide_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the bank reports. Unit: dBFS.
  double level_dbfs;
  // Frames with a good CRC.
  int64_t frames_ok;
  // Frames that are not LoRaWAN or whose CRC failed.
  int64_t frames_bad;
  // Frame preambles seen.
  int64_t preambles;
  // Headers that failed their checksum.
  int64_t header_failed;
  // FSK sync words found.
  int64_t fsk_syncs;
  // FSK frames.
  int64_t fsk_frames;
  // FSK frames whose CRC failed.
  int64_t fsk_crc_errors;
  // LoRa demodulators running.
  int64_t demodulators;
  // Channels in the plan.
  int64_t channels_listed;
  // Channels decoded.
  int64_t channels_decoded;
  // What is decoded, in words.
  sdrkit_string note;
  // The plan in words: its count of channels and their span.
  sdrkit_string plan;
  // LoRaWAN devices.
  int64_t devices;
} sdrkit_lorawanwide_wide_fields;

// The options of `modulation` of `sdrkit_lorawanwide_wide_frame_row`.
typedef enum sdrkit_lorawanwide_wide_frame_row_modulation {
  // LoRa
  SDRKIT_LORAWANWIDE_WIDE_FRAME_ROW_MODULATION_LORA = 0,
  // FSK
  SDRKIT_LORAWANWIDE_WIDE_FRAME_ROW_MODULATION_FSK = 1,
} sdrkit_lorawanwide_wide_frame_row_modulation;

// A row of the LoRaWAN frame log. The row key is the place of the frame in the log.
typedef struct sdrkit_lorawanwide_wide_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // True if `type` holds a value.
  bool has_type;
  // The frame type.
  sdrkit_string type;
  // True if `device` holds a value.
  bool has_device;
  // The DevAddr or the DevEUI.
  sdrkit_string device;
  // True if `network` holds a value.
  bool has_network;
  // The network operator.
  sdrkit_string network;
  // True if `fcnt` holds a value.
  bool has_fcnt;
  // The frame counter.
  int64_t fcnt;
  // True if `fport` holds a value.
  bool has_fport;
  // The FPort.
  int64_t fport;
  // True if `payload_bytes` holds a value.
  bool has_payload_bytes;
  // The encrypted payload length of a LoRaWAN frame. Unit: B.
  int64_t payload_bytes;
  // True if `fctrl` holds a value.
  bool has_fctrl;
  // The FCtrl flags.
  sdrkit_string fctrl;
  // True if `mac_commands` holds a value.
  bool has_mac_commands;
  // The MAC commands.
  sdrkit_string mac_commands;
  // True if `mic` holds a value.
  bool has_mic;
  // The message integrity code, hex.
  sdrkit_string mic;
  // How the frame was modulated. A value of `sdrkit_lorawanwide_wide_frame_row_modulation`.
  int32_t modulation;
  // True if `sf` holds a value.
  bool has_sf;
  // The spreading factor, none for the FSK channel.
  int64_t sf;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
  // The frequency of the frame. A frequency in Hz.
  double freq;
  // More about the frame.
  sdrkit_string summary;
} sdrkit_lorawanwide_wide_frame_row;

// A row of the LoRaWAN channels. The row key is the place of the channel in the plan.
typedef struct sdrkit_lorawanwide_wide_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel frequency. A frequency in Hz.
  double freq;
  // The bandwidth. A frequency in Hz.
  double bw;
  // The least spreading factor searched.
  int64_t sf_min;
  // The greatest spreading factor searched.
  int64_t sf_max;
  // The channel lies inside the captured band.
  bool in_band;
  // Preambles.
  int64_t preambles;
  // Frames decoded.
  int64_t frames;
  // CRC errors.
  int64_t crc_errors;
} sdrkit_lorawanwide_wide_channel_row;

// The result fields of the raw LoRa stack.
typedef struct sdrkit_loraraw_raw_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the bank reports. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame, while frames come. A frequency in Hz.
  double offset_hz;
  // Frames with a good CRC.
  int64_t frames_ok;
  // Frames whose CRC failed.
  int64_t frames_bad;
  // Frame preambles seen.
  int64_t preambles;
  // Headers that failed their checksum.
  int64_t header_failed;
  // What is searched.
  sdrkit_string searching;
  // The dial. A frequency in Hz.
  double dial;
  // Frames sent.
  int64_t tx_sent;
  // Frames at SF5.
  int64_t sf5;
  // Frames at SF6.
  int64_t sf6;
  // Frames at SF7.
  int64_t sf7;
  // Frames at SF8.
  int64_t sf8;
  // Frames at SF9.
  int64_t sf9;
  // Frames at SF10.
  int64_t sf10;
  // Frames at SF11.
  int64_t sf11;
  // Frames at SF12.
  int64_t sf12;
  // Frames at 125 kHz.
  int64_t bw125;
  // Frames at 250 kHz.
  int64_t bw250;
  // Frames at 500 kHz.
  int64_t bw500;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last frame, while frames come. Unit: dB.
  double snr_db;
} sdrkit_loraraw_raw_fields;

// The options of `crc` of `sdrkit_loraraw_raw_frame_row`.
typedef enum sdrkit_loraraw_raw_frame_row_crc {
  // Right
  SDRKIT_LORARAW_RAW_FRAME_ROW_CRC_OK = 0,
  // Wrong
  SDRKIT_LORARAW_RAW_FRAME_ROW_CRC_BAD = 1,
  // None
  SDRKIT_LORARAW_RAW_FRAME_ROW_CRC_NONE = 2,
} sdrkit_loraraw_raw_frame_row_crc;

// A row of the raw frame log. The row key is the place of the frame in the log.
typedef struct sdrkit_loraraw_raw_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The spreading factor.
  int64_t sf;
  // The bandwidth. A frequency in Hz.
  double bw;
  // The coding rate denominator.
  int64_t cr;
  // The payload length. Unit: B.
  int64_t len;
  // What the CRC says. A value of `sdrkit_loraraw_raw_frame_row_crc`.
  int32_t crc;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The carrier offset. A frequency in Hz.
  double cfo;
  // The payload.
  sdrkit_bytes payload;
  // The payload as text for people.
  sdrkit_string text;
} sdrkit_loraraw_raw_frame_row;

// A row of the raw decoders. The row key is the tag of the detector.
typedef struct sdrkit_loraraw_raw_decoder_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The spreading factor.
  int64_t sf;
  // The bandwidth. A frequency in Hz.
  double bw;
  // The sync words searched, in words.
  sdrkit_string syncs;
  // The channel lies inside the captured band.
  bool in_band;
  // Preambles.
  int64_t preambles;
  // Frames decoded.
  int64_t frames;
  // CRC errors.
  int64_t crc_errors;
  // Headers failed.
  int64_t header_failed;
  // Sync words failed.
  int64_t sync_failed;
} sdrkit_loraraw_raw_decoder_row;

// The result fields of the APRS stack.
typedef struct sdrkit_aprs_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a good CRC.
  int64_t frames_ok;
  // Frames whose CRC failed.
  int64_t frames_bad;
  // Frames at 1200 bit/s.
  int64_t frames_1200;
  // Frames at 9600 bit/s.
  int64_t frames_9600;
  // APRS frames, without the digipeated copies.
  int64_t aprs_frames;
  // Digipeated copies.
  int64_t copies;
  // Frames that are not AX.25.
  int64_t not_ax25;
  // Stations, objects and items in the table.
  int64_t stations;
  // A carrier is present.
  bool carrier;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio, once the demodulator reports. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the demodulator reports. Unit: dBFS.
  double level_dbfs;
} sdrkit_aprs_fields;

// A row of the APRS frame log. The row key is the place of the frame in the log.
typedef struct sdrkit_aprs_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The sender.
  sdrkit_string source;
  // The destination.
  sdrkit_string target;
  // The digipeater path.
  sdrkit_string path;
  // The APRS kind or the AX.25 frame type.
  sdrkit_string type;
  // The frame in one line.
  sdrkit_string text;
  // The bit rate. A frequency in Hz.
  double bit_rate;
} sdrkit_aprs_frame_row;

// A row of the APRS stations. The row key is the callsign with its SSID, or the name of an object or item.
typedef struct sdrkit_aprs_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign with its SSID, or the name of an object or item.
  sdrkit_string call;
  // The map symbol.
  sdrkit_string symbol;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // The frames of the station.
  int64_t heard;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course. Unit: °.
  double course_deg;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed. Unit: kt.
  double speed_kt;
  // True if `alt_m` holds a value.
  bool has_alt_m;
  // The altitude. Unit: m.
  double alt_m;
  // The comment or status.
  sdrkit_string comment;
  // The station that sent an object, an item or a third-party packet.
  sdrkit_string via;
} sdrkit_aprs_station_row;

// The result fields of the FANET stack.
typedef struct sdrkit_fanet_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames with a good CRC.
  int64_t frames_ok;
  // Frames whose CRC failed.
  int64_t frames_bad;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last frame, while frames come. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame, while frames come. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the bank reports. Unit: dBFS.
  double level_dbfs;
  // The channel of the plan.
  sdrkit_string listening;
  // The stations in the table.
  int64_t stations_heard;
} sdrkit_fanet_fields;

// The options of `kind` of `sdrkit_fanet_frame_row`.
typedef enum sdrkit_fanet_frame_row_kind {
  // Tracking
  SDRKIT_FANET_FRAME_ROW_KIND_TRACKING = 0,
  // Ground
  SDRKIT_FANET_FRAME_ROW_KIND_GROUND = 1,
  // Name
  SDRKIT_FANET_FRAME_ROW_KIND_NAME = 2,
  // Message
  SDRKIT_FANET_FRAME_ROW_KIND_MESSAGE = 3,
  // Service
  SDRKIT_FANET_FRAME_ROW_KIND_SERVICE = 4,
  // Thermal
  SDRKIT_FANET_FRAME_ROW_KIND_THERMAL = 5,
  // Other
  SDRKIT_FANET_FRAME_ROW_KIND_OTHER = 6,
} sdrkit_fanet_frame_row_kind;

// A row of the FANET frame log. The row key is the place of the frame in the log.
typedef struct sdrkit_fanet_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the frame. Unit: s.
  double time_s;
  // What the frame is. A value of `sdrkit_fanet_frame_row_kind`.
  int32_t kind;
  // The sender.
  sdrkit_string source;
  // The addressee of a unicast frame, else empty.
  sdrkit_string target;
  // The frame in one line.
  sdrkit_string decoded;
  // True if `position` holds a value.
  bool has_position;
  // The position that the frame gives, if it gives one.
  sdrkit_position position;
  // The level of the frame. Unit: dB.
  double level_db;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
} sdrkit_fanet_frame_row;

// The options of `kind` of `sdrkit_fanet_station_row`.
typedef enum sdrkit_fanet_station_row_kind {
  // Aircraft
  SDRKIT_FANET_STATION_ROW_KIND_AIRCRAFT = 0,
  // Person
  SDRKIT_FANET_STATION_ROW_KIND_PERSON = 1,
  // Weather station
  SDRKIT_FANET_STATION_ROW_KIND_WEATHER = 2,
  // Other
  SDRKIT_FANET_STATION_ROW_KIND_OTHER = 3,
} sdrkit_fanet_station_row_kind;

// A row of the FANET stations. The row key and `source` are the station id "MM:DDDD".
typedef struct sdrkit_fanet_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station id.
  sdrkit_string source;
  // The name that the station sent.
  sdrkit_string name;
  // What the station is. A value of `sdrkit_fanet_station_row_kind`.
  int32_t kind;
  // True if `aircraft` holds a value.
  bool has_aircraft;
  // The aircraft type, from a tracking frame.
  sdrkit_string aircraft;
  // True if `position` holds a value.
  bool has_position;
  // The last position.
  sdrkit_position position;
  // True if `alt_m` holds a value.
  bool has_alt_m;
  // The altitude, from a tracking frame. Unit: m.
  double alt_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed, from a tracking frame. Unit: kt.
  double speed_kt;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // The climb rate, from a tracking frame. Unit: m/s.
  double climb_m_s;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The heading, from a tracking frame. Unit: °.
  double course_deg;
  // The last service frame in one line.
  sdrkit_string weather;
  // True if `heard` holds a value.
  bool has_heard;
  // The frames of the station.
  int64_t heard;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_fanet_station_row;

// A track point of a FANET station. The row key is the station id and the signal time of the point.
typedef struct sdrkit_fanet_track_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station that the point belongs to.
  sdrkit_string station;
  // The position.
  sdrkit_position position;
  // The signal time of the point. Unit: s.
  double time_s;
} sdrkit_fanet_track_row;

// The result fields of the Reticulum stack.
typedef struct sdrkit_reticulum_rns_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that gave a packet.
  int64_t frames_ok;
  // Frames that gave no packet.
  int64_t frames_bad;
  // Preambles whose sync word followed.
  int64_t preambles;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last frame, while frames come. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the bank reports. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame, while frames come. A frequency in Hz.
  double offset_hz;
  // The LoRa settings that the plan searches.
  sdrkit_string searching;
  // The destinations in the table.
  int64_t destinations_heard;
} sdrkit_reticulum_rns_fields;

// The options of `type` of `sdrkit_reticulum_rns_packet_row`.
typedef enum sdrkit_reticulum_rns_packet_row_type {
  // DATA
  SDRKIT_RETICULUM_RNS_PACKET_ROW_TYPE_DATA = 0,
  // ANNOUNCE
  SDRKIT_RETICULUM_RNS_PACKET_ROW_TYPE_ANNOUNCE = 1,
  // LINKREQUEST
  SDRKIT_RETICULUM_RNS_PACKET_ROW_TYPE_LINKREQUEST = 2,
  // PROOF
  SDRKIT_RETICULUM_RNS_PACKET_ROW_TYPE_PROOF = 3,
} sdrkit_reticulum_rns_packet_row_type;

// A row of the Reticulum packet log. The row key is the place of the packet in the log.
typedef struct sdrkit_reticulum_rns_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the packet, when known. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the packet. Unit: s.
  double time_s;
  // The destination hash as hex.
  sdrkit_string destination;
  // The type of the packet. A value of `sdrkit_reticulum_rns_packet_row_type`.
  int32_t type;
  // The packet in one line.
  sdrkit_string content;
  // The channel plus the carrier offset. A frequency in Hz.
  double freq_hz;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
  // The payload is encrypted.
  bool encrypted;
  // The first bytes of the packet.
  sdrkit_bytes hex;
  // The LoRa setting of the frame.
  sdrkit_string setting;
} sdrkit_reticulum_rns_packet_row;

// The options of `type` of `sdrkit_reticulum_rns_destination_row`.
typedef enum sdrkit_reticulum_rns_destination_row_type {
  // DATA
  SDRKIT_RETICULUM_RNS_DESTINATION_ROW_TYPE_DATA = 0,
  // ANNOUNCE
  SDRKIT_RETICULUM_RNS_DESTINATION_ROW_TYPE_ANNOUNCE = 1,
  // LINKREQUEST
  SDRKIT_RETICULUM_RNS_DESTINATION_ROW_TYPE_LINKREQUEST = 2,
  // PROOF
  SDRKIT_RETICULUM_RNS_DESTINATION_ROW_TYPE_PROOF = 3,
} sdrkit_reticulum_rns_destination_row_type;

// A row of the Reticulum destinations. The row key is the destination hash as hex.
typedef struct sdrkit_reticulum_rns_destination_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The destination hash as hex.
  sdrkit_string destination;
  // The type of the last packet. A value of `sdrkit_reticulum_rns_destination_row_type`.
  int32_t type;
  // The last packet in one line.
  sdrkit_string content;
  // True if `packets` holds a value.
  bool has_packets;
  // The packets to the destination.
  int64_t packets;
  // The identity that an announce gave, as hex.
  sdrkit_string identity;
  // The text that an announce gave.
  sdrkit_string app_text;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first packet. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last packet. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_reticulum_rns_destination_row;

// The result fields of the ExpressLRS stack.
typedef struct sdrkit_elrs_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames whose CRC fits a link.
  int64_t frames_ok;
  // Frames whose CRC fits no link.
  int64_t frames_bad;
  // Preambles whose sync word followed.
  int64_t preambles;
  // The binding phrase of the user's own link is set.
  bool phrase_set;
  // The hop channels inside the capture.
  int64_t hop_channels_inside;
  // The hop channels of the plan.
  int64_t hop_channels;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the last frame, while frames come. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the input, once the bank reports. Unit: dBFS.
  double level_dbfs;
  // The links in the table.
  int64_t links_heard;
  // What the decoder cannot confirm, and what it needs.
  sdrkit_string note;
} sdrkit_elrs_fields;

// The options of `kind` of `sdrkit_elrs_frame_row`.
typedef enum sdrkit_elrs_frame_row_kind {
  // SYNC
  SDRKIT_ELRS_FRAME_ROW_KIND_SYNC = 0,
  // DATA
  SDRKIT_ELRS_FRAME_ROW_KIND_DATA = 1,
  // RC or link stats
  SDRKIT_ELRS_FRAME_ROW_KIND_RC = 2,
} sdrkit_elrs_frame_row_kind;

// A row of the frame log. The row key is the place of the frame in the log.
typedef struct sdrkit_elrs_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the frame, when known. Seconds since 1970 (UTC).
  double utc;
  // The signal time of the frame. Unit: s.
  double time_s;
  // The link of the frame.
  sdrkit_string link;
  // What the frame is. A value of `sdrkit_elrs_frame_row_kind`.
  int32_t kind;
  // The frame in one line.
  sdrkit_string content;
  // The hop channel plus the carrier offset. A frequency in Hz.
  double freq_hz;
  // The index of the hop channel.
  int64_t hop_channel;
  // The spreading factor.
  int64_t sf;
  // The length of the frame.
  int64_t bytes;
  // The IQ was inverted.
  bool inverted;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
  // The payload is encrypted.
  bool encrypted;
  // The frame bytes as hex.
  sdrkit_string hex;
} sdrkit_elrs_frame_row;

// The options of `kind` of `sdrkit_elrs_link_row`.
typedef enum sdrkit_elrs_link_row_kind {
  // SYNC
  SDRKIT_ELRS_LINK_ROW_KIND_SYNC = 0,
  // DATA
  SDRKIT_ELRS_LINK_ROW_KIND_DATA = 1,
  // RC or link stats
  SDRKIT_ELRS_LINK_ROW_KIND_RC = 2,
} sdrkit_elrs_link_row_kind;

// A row of the links. The row key and `source` are the link, 4 hex digits.
typedef struct sdrkit_elrs_link_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The link.
  sdrkit_string source;
  // What the last frame of the link was. A value of `sdrkit_elrs_link_row_kind`.
  int32_t kind;
  // The last frame of the link in one line.
  sdrkit_string content;
  // The frames of the link.
  int64_t frames;
  // The sync frames of the link.
  int64_t sync_frames;
  // The rate of the last frame.
  sdrkit_string rate;
  // True if `hop_index` holds a value.
  bool has_hop_index;
  // The hop index of the last sync frame, once one came.
  int64_t hop_index;
  // The hop channels on which the link was heard, in rising order.
  sdrkit_string heard_on;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // The UTC of the first frame. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // The UTC of the last frame. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_elrs_link_row;

// The options of `net` of `sdrkit_lora_packet_event`.
typedef enum sdrkit_lora_packet_event_net {
  // Meshtastic
  SDRKIT_LORA_PACKET_EVENT_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_LORA_PACKET_EVENT_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_LORA_PACKET_EVENT_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_LORA_PACKET_EVENT_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_LORA_PACKET_EVENT_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_LORA_PACKET_EVENT_NET_LORA = 5,
  // APRS
  SDRKIT_LORA_PACKET_EVENT_NET_APRS = 6,
} sdrkit_lora_packet_event_net;

// The options of `modulation` of `sdrkit_lora_packet_event`.
typedef enum sdrkit_lora_packet_event_modulation {
  // LoRa
  SDRKIT_LORA_PACKET_EVENT_MODULATION_LORA = 0,
  // FSK
  SDRKIT_LORA_PACKET_EVENT_MODULATION_FSK = 1,
} sdrkit_lora_packet_event_modulation;

// The options of `family` of `sdrkit_lora_packet_event`.
typedef enum sdrkit_lora_packet_event_family {
  // Aircraft
  SDRKIT_LORA_PACKET_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_LORA_PACKET_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_LORA_PACKET_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_LORA_PACKET_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_LORA_PACKET_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_LORA_PACKET_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_LORA_PACKET_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_LORA_PACKET_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_LORA_PACKET_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_LORA_PACKET_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_LORA_PACKET_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_LORA_PACKET_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_LORA_PACKET_EVENT_FAMILY_OTHER = 12,
} sdrkit_lora_packet_event_family;

// The fields of one LoRa or FSK frame.
typedef struct sdrkit_lora_packet_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The protocol of the frame. A value of `sdrkit_lora_packet_event_net`.
  int32_t net;
  // How the frame was modulated. A value of `sdrkit_lora_packet_event_modulation`.
  int32_t modulation;
  // The signal to noise ratio of the frame. Unit: dB.
  double snr_db;
  // True if `sf` holds a value.
  bool has_sf;
  // The spreading factor, none for FSK.
  int64_t sf;
  // The payload.
  sdrkit_bytes payload;
  // The CRC is right, or the frame has none.
  bool crc_ok;
  // True if `source` holds a value.
  bool has_source;
  // The sender, if the protocol names one.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The addressee, if the protocol names one.
  sdrkit_string target;
  // True if `devaddr` holds a value.
  bool has_devaddr;
  // LoRaWAN: the device address, eight hex digits.
  sdrkit_string devaddr;
  // True if `iq_inverted` holds a value.
  bool has_iq_inverted;
  // LoRaWAN: the IQ is inverted (a downlink).
  bool iq_inverted;
  // True if `crc` holds a value.
  bool has_crc;
  // The frame has a CRC.
  bool crc;
  // True if `bw` holds a value.
  bool has_bw;
  // The bandwidth. A frequency in Hz.
  double bw;
  // True if `cr` holds a value.
  bool has_cr;
  // The coding rate denominator.
  int64_t cr;
  // True if `implicit` holds a value.
  bool has_implicit;
  // The header is implicit.
  bool implicit;
  // True if `sync` holds a value.
  bool has_sync;
  // The sync word.
  int64_t sync;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the frame. Unit: dBFS.
  double level_dbfs;
  // The family of the event. A value of `sdrkit_lora_packet_event_family`.
  int32_t family;
} sdrkit_lora_packet_event;

// The options of `net` of `sdrkit_mesh_text_event`.
typedef enum sdrkit_mesh_text_event_net {
  // Meshtastic
  SDRKIT_MESH_TEXT_EVENT_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_TEXT_EVENT_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_TEXT_EVENT_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_TEXT_EVENT_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_TEXT_EVENT_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_TEXT_EVENT_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_TEXT_EVENT_NET_APRS = 6,
} sdrkit_mesh_text_event_net;

// The options of `state` of `sdrkit_mesh_text_event`.
typedef enum sdrkit_mesh_text_event_state {
  // Queued
  SDRKIT_MESH_TEXT_EVENT_STATE_QUEUED = 0,
  // Waiting for the duty cycle
  SDRKIT_MESH_TEXT_EVENT_STATE_WAITING = 1,
  // Sent
  SDRKIT_MESH_TEXT_EVENT_STATE_SENT = 2,
  // Acknowledged by the mesh
  SDRKIT_MESH_TEXT_EVENT_STATE_ACKED_BY_MESH = 3,
  // Delivered
  SDRKIT_MESH_TEXT_EVENT_STATE_DELIVERED = 4,
  // Relayed, not confirmed
  SDRKIT_MESH_TEXT_EVENT_STATE_RELAYED = 5,
  // Rejected
  SDRKIT_MESH_TEXT_EVENT_STATE_REJECTED = 6,
  // Failed
  SDRKIT_MESH_TEXT_EVENT_STATE_FAILED = 7,
} sdrkit_mesh_text_event_state;

// The options of `family` of `sdrkit_mesh_text_event`.
typedef enum sdrkit_mesh_text_event_family {
  // Aircraft
  SDRKIT_MESH_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MESH_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MESH_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MESH_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MESH_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MESH_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MESH_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MESH_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MESH_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MESH_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MESH_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MESH_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MESH_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_mesh_text_event_family;

// The fields of a text message, sent or received.
typedef struct sdrkit_mesh_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender.
  sdrkit_string source;
  // The addressee.
  sdrkit_string target;
  // The channel.
  sdrkit_string channel;
  // The text.
  sdrkit_string text;
  // The protocol. A value of `sdrkit_mesh_text_event_net`.
  int32_t net;
  // True if `sender` holds a value.
  bool has_sender;
  // The name of the sender, if known.
  sdrkit_string sender;
  // True if `hops` holds a value.
  bool has_hops;
  // The hops taken, if known.
  int64_t hops;
  // The packet id.
  int64_t packet_id;
  // The message that the text replies to.
  int64_t reply_id;
  // A reaction, as a Unicode code point.
  int64_t emoji;
  // Sent by the own node.
  bool own;
  // True if `state` holds a value.
  bool has_state;
  // The delivery state of an own message. A value of `sdrkit_mesh_text_event_state`.
  int32_t state;
  // The family of the event. A value of `sdrkit_mesh_text_event_family`.
  int32_t family;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio of the frame of a received text. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the frame of a received text, relative to the full scale of the input (not an absolute power). Unit: dBFS.
  double level_dbfs;
  // True if `crc_ok` holds a value.
  bool has_crc_ok;
  // The text comes from a copy with a good CRC. False for a damaged copy shown after the wait; a later good copy sends the text again with true. None for an own message.
  bool crc_ok;
} sdrkit_mesh_text_event;

// The options of `net` of `sdrkit_mesh_node_event`.
typedef enum sdrkit_mesh_node_event_net {
  // Meshtastic
  SDRKIT_MESH_NODE_EVENT_NET_MESHTASTIC = 0,
  // MeshCore
  SDRKIT_MESH_NODE_EVENT_NET_MESHCORE = 1,
  // LoRa APRS
  SDRKIT_MESH_NODE_EVENT_NET_LORAAPRS = 2,
  // MeshCom
  SDRKIT_MESH_NODE_EVENT_NET_MESHCOM = 3,
  // LoRaWAN
  SDRKIT_MESH_NODE_EVENT_NET_LORAWAN = 4,
  // LoRa
  SDRKIT_MESH_NODE_EVENT_NET_LORA = 5,
  // APRS
  SDRKIT_MESH_NODE_EVENT_NET_APRS = 6,
} sdrkit_mesh_node_event_net;

// The options of `family` of `sdrkit_mesh_node_event`.
typedef enum sdrkit_mesh_node_event_family {
  // Aircraft
  SDRKIT_MESH_NODE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MESH_NODE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MESH_NODE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MESH_NODE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MESH_NODE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MESH_NODE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MESH_NODE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MESH_NODE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MESH_NODE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MESH_NODE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MESH_NODE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MESH_NODE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MESH_NODE_EVENT_FAMILY_OTHER = 12,
} sdrkit_mesh_node_event_family;

// The fields of a node heard for the first time or with news.
typedef struct sdrkit_mesh_node_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The node id.
  sdrkit_string source;
  // True if `name` holds a value.
  bool has_name;
  // The node name, if known.
  sdrkit_string name;
  // True if `position` holds a value.
  bool has_position;
  // The position, if known.
  sdrkit_position position;
  // The protocol. A value of `sdrkit_mesh_node_event_net`.
  int32_t net;
  // The family of the event. A value of `sdrkit_mesh_node_event_family`.
  int32_t family;
} sdrkit_mesh_node_event;

// The options of `what` of `sdrkit_lora_tx_request_event`.
typedef enum sdrkit_lora_tx_request_event_what {
  // Text
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_TEXT = 0,
  // Retransmission
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_RETRANSMISSION = 1,
  // Acknowledgement
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_ACK = 2,
  // Node information
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_NODEINFO = 3,
  // Position
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_POSITION = 4,
  // Route trace
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_TRACEROUTE = 5,
  // Telemetry
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_TELEMETRY = 6,
  // Rebroadcast
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_REBROADCAST = 7,
  // Advert
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_ADVERT = 8,
  // Local advert
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_LOCAL_ADVERT = 9,
  // Beacon
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_BEACON = 10,
  // Raw
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_RAW = 11,
  // Cancel
  SDRKIT_LORA_TX_REQUEST_EVENT_WHAT_CANCEL = 12,
} sdrkit_lora_tx_request_event_what;

// The fields of a frame that the stack requests the host to send.
typedef struct sdrkit_lora_tx_request_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // A number for the reply.
  int64_t id;
  // What the frame is. A value of `sdrkit_lora_tx_request_event_what`.
  int32_t what;
  // The payload.
  sdrkit_bytes payload;
  // The frequency to send on. A frequency in Hz.
  double freq_hz_;
  // The earliest send time, signal seconds. Unit: s.
  double not_before_s;
  // True if `air_time` holds a value.
  bool has_air_time;
  // The time on air of a LoRa frame. A length of time in seconds.
  double air_time;
  // True if `message_key` holds a value.
  bool has_message_key;
  // The own message that the frame carries.
  int64_t message_key;
  // True if `setting` holds a value.
  bool has_setting;
  // The LoRa setting in words.
  sdrkit_string setting;
  // True if `afsk_baud` holds a value.
  bool has_afsk_baud;
  // The packet speed of an AX.25 frame. Unit: bit/s.
  int64_t afsk_baud;
} sdrkit_lora_tx_request_event;

// The options of `family` of `sdrkit_aprs_packet_event`.
typedef enum sdrkit_aprs_packet_event_family {
  // Aircraft
  SDRKIT_APRS_PACKET_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_APRS_PACKET_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_APRS_PACKET_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_APRS_PACKET_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_APRS_PACKET_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_APRS_PACKET_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_APRS_PACKET_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_APRS_PACKET_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_APRS_PACKET_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_APRS_PACKET_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_APRS_PACKET_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_APRS_PACKET_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_APRS_PACKET_EVENT_FAMILY_OTHER = 12,
} sdrkit_aprs_packet_event_family;

// The fields of one AX.25 frame.
typedef struct sdrkit_aprs_packet_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender.
  sdrkit_string source;
  // The destination.
  sdrkit_string target;
  // The digipeater path.
  sdrkit_string path;
  // The information field as received.
  sdrkit_string info;
  // The APRS kind or the AX.25 frame type.
  sdrkit_string type;
  // The frame in one line.
  sdrkit_string text;
  // True if `position` holds a value.
  bool has_position;
  // The position that the frame gives, if it gives one.
  sdrkit_position position;
  // The frame in TNC2 text.
  sdrkit_string tnc2;
  // The family of the event. A value of `sdrkit_aprs_packet_event_family`.
  int32_t family;
} sdrkit_aprs_packet_event;

// The options of `family` of `sdrkit_fanet_station_event`.
typedef enum sdrkit_fanet_station_event_family {
  // Aircraft
  SDRKIT_FANET_STATION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FANET_STATION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FANET_STATION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FANET_STATION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FANET_STATION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FANET_STATION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FANET_STATION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FANET_STATION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FANET_STATION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FANET_STATION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FANET_STATION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FANET_STATION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FANET_STATION_EVENT_FAMILY_OTHER = 12,
} sdrkit_fanet_station_event_family;

// The fields of a station heard for the first time.
typedef struct sdrkit_fanet_station_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The station id.
  sdrkit_string source;
  // The family of the event. A value of `sdrkit_fanet_station_event_family`.
  int32_t family;
} sdrkit_fanet_station_event;

// The options of `family` of `sdrkit_fanet_message_event`.
typedef enum sdrkit_fanet_message_event_family {
  // Aircraft
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FANET_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_fanet_message_event_family;

// The fields of a message.
typedef struct sdrkit_fanet_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender.
  sdrkit_string source;
  // The addressee of a unicast message, else empty.
  sdrkit_string target;
  // The message.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_fanet_message_event_family`.
  int32_t family;
} sdrkit_fanet_message_event;

// The options of `urgency` of `sdrkit_fanet_distress_event`.
typedef enum sdrkit_fanet_distress_event_urgency {
  // None
  SDRKIT_FANET_DISTRESS_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_FANET_DISTRESS_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_FANET_DISTRESS_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_FANET_DISTRESS_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_fanet_distress_event_urgency;

// The options of `family` of `sdrkit_fanet_distress_event`.
typedef enum sdrkit_fanet_distress_event_family {
  // Aircraft
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FANET_DISTRESS_EVENT_FAMILY_OTHER = 12,
} sdrkit_fanet_distress_event_family;

// The fields of a ground tracker that requests help.
typedef struct sdrkit_fanet_distress_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The station id.
  sdrkit_string source;
  // The state of the tracker in words.
  sdrkit_string text;
  // True if `position` holds a value.
  bool has_position;
  // The position of the tracker, if the frame gives one.
  sdrkit_position position;
  // How urgent the request is. A value of `sdrkit_fanet_distress_event_urgency`.
  int32_t urgency;
  // The family of the event. A value of `sdrkit_fanet_distress_event_family`.
  int32_t family;
} sdrkit_fanet_distress_event;

// The options of `family` of `sdrkit_reticulum_destination_event`.
typedef enum sdrkit_reticulum_destination_event_family {
  // Aircraft
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RETICULUM_DESTINATION_EVENT_FAMILY_OTHER = 12,
} sdrkit_reticulum_destination_event_family;

// The fields of a destination heard for the first time.
typedef struct sdrkit_reticulum_destination_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The destination hash as hex.
  sdrkit_string source;
  // The line of the event.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_reticulum_destination_event_family`.
  int32_t family;
} sdrkit_reticulum_destination_event;

// The options of `family` of `sdrkit_reticulum_announce_event`.
typedef enum sdrkit_reticulum_announce_event_family {
  // Aircraft
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RETICULUM_ANNOUNCE_EVENT_FAMILY_OTHER = 12,
} sdrkit_reticulum_announce_event_family;

// The fields of an announce.
typedef struct sdrkit_reticulum_announce_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The destination hash as hex.
  sdrkit_string source;
  // The identity as hex.
  sdrkit_string identity;
  // The announced text.
  sdrkit_string text;
  // The family of the event. A value of `sdrkit_reticulum_announce_event_family`.
  int32_t family;
} sdrkit_reticulum_announce_event;

// The options of `family` of `sdrkit_elrs_link_event`.
typedef enum sdrkit_elrs_link_event_family {
  // Aircraft
  SDRKIT_ELRS_LINK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ELRS_LINK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ELRS_LINK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ELRS_LINK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ELRS_LINK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ELRS_LINK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ELRS_LINK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ELRS_LINK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ELRS_LINK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ELRS_LINK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ELRS_LINK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ELRS_LINK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ELRS_LINK_EVENT_FAMILY_OTHER = 12,
} sdrkit_elrs_link_event_family;

// The fields of a link heard for the first time.
typedef struct sdrkit_elrs_link_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The link, 4 hex digits.
  sdrkit_string source;
  // The family of the event. A value of `sdrkit_elrs_link_event_family`.
  int32_t family;
} sdrkit_elrs_link_event;

// The arguments of `mt.send`.
typedef struct sdrkit_lora_mt_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text to send.
  sdrkit_string text;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel of the text.
  sdrkit_string channel;
  // True if `to` holds a value.
  bool has_to;
  // The addressee.
  sdrkit_string to;
  // True if `reply_id` holds a value.
  bool has_reply_id;
  // The packet id that the text replies to.
  int64_t reply_id;
  // True if `emoji` holds a value.
  bool has_emoji;
  // A reaction.
  int64_t emoji;
  // True if `want_ack` holds a value.
  bool has_want_ack;
  // Whether the text asks for an acknowledgement. The index of an option.
  int32_t want_ack;
  // True if `hop_limit` holds a value.
  bool has_hop_limit;
  // The hop limit of the text.
  int64_t hop_limit;
} sdrkit_lora_mt_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MT_SEND_ARGS_INIT {sizeof(sdrkit_lora_mt_send_args), {0}, 0, {0}, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0}

// The arguments of `mc.send`.
typedef struct sdrkit_lora_mc_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text to send.
  sdrkit_string text;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel of the text.
  sdrkit_string channel;
  // True if `to` holds a value.
  bool has_to;
  // The addressee of a direct text.
  sdrkit_string to;
} sdrkit_lora_mc_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MC_SEND_ARGS_INIT {sizeof(sdrkit_lora_mc_send_args), {0}, 0, {0}, 0, {0}}

// The arguments of `aprs.send`.
typedef struct sdrkit_aprs_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The addressee.
  sdrkit_string to;
  // The text of the message.
  sdrkit_string text;
} sdrkit_aprs_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_APRS_SEND_ARGS_INIT {sizeof(sdrkit_aprs_send_args), {0}, {0}}

// The arguments of `mt.channel.add`.
typedef struct sdrkit_lora_mt_channel_add_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The name of the channel.
  sdrkit_string name;
  // True if `key` holds a value.
  bool has_key;
  // The key of the channel.
  sdrkit_string key;
} sdrkit_lora_mt_channel_add_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MT_CHANNEL_ADD_ARGS_INIT {sizeof(sdrkit_lora_mt_channel_add_args), {0}, 0, {0}}

// The arguments of `mc.channel.add`.
typedef struct sdrkit_lora_mc_channel_add_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The name of the channel.
  sdrkit_string name;
  // True if `key` holds a value.
  bool has_key;
  // The secret of the channel.
  sdrkit_string key;
} sdrkit_lora_mc_channel_add_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MC_CHANNEL_ADD_ARGS_INIT {sizeof(sdrkit_lora_mc_channel_add_args), {0}, 0, {0}}

// The arguments of `channel.remove`.
typedef struct sdrkit_lora_channel_remove_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the channel.
  sdrkit_string row;
} sdrkit_lora_channel_remove_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_CHANNEL_REMOVE_ARGS_INIT {sizeof(sdrkit_lora_channel_remove_args), {0}}

// The arguments of `mc.advert`.
typedef struct sdrkit_lora_mc_advert_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `scope` holds a value.
  bool has_scope;
  // Where the advert goes. The index of an option.
  int32_t scope;
} sdrkit_lora_mc_advert_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MC_ADVERT_ARGS_INIT {sizeof(sdrkit_lora_mc_advert_args), 0, 0}

// The arguments of `mc.contact.add`.
typedef struct sdrkit_lora_mc_contact_add_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The public key of the contact.
  sdrkit_string key;
  // True if `name` holds a value.
  bool has_name;
  // The name of the contact.
  sdrkit_string name;
} sdrkit_lora_mc_contact_add_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MC_CONTACT_ADD_ARGS_INIT {sizeof(sdrkit_lora_mc_contact_add_args), {0}, 0, {0}}

// The arguments of `mt.request`.
typedef struct sdrkit_lora_mt_request_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The node that the request goes to.
  sdrkit_string node;
  // True if `what` holds a value.
  bool has_what;
  // What the request is for. The index of an option.
  int32_t what;
} sdrkit_lora_mt_request_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MT_REQUEST_ARGS_INIT {sizeof(sdrkit_lora_mt_request_args), {0}, 0, 0}

// The arguments of `remove` of the mesh stacks.
typedef struct sdrkit_mesh_remove_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the node.
  sdrkit_string row;
} sdrkit_mesh_remove_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_MESH_REMOVE_ARGS_INIT {sizeof(sdrkit_mesh_remove_args), {0}}

// The arguments of `mt.channel`.
typedef struct sdrkit_lora_mt_channel_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `index` holds a value.
  bool has_index;
  // The slot: 0 is the primary channel.
  int64_t index;
  // The name of the channel.
  sdrkit_string name;
  // True if `key` holds a value.
  bool has_key;
  // The key of the channel.
  sdrkit_string key;
  // True if `precision` holds a value.
  bool has_precision;
  // The position precision. Unit: bits.
  int64_t precision;
} sdrkit_lora_mt_channel_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_MT_CHANNEL_ARGS_INIT {sizeof(sdrkit_lora_mt_channel_args), 0, 0, {0}, 0, {0}, 0, 0}

// The arguments of `mt.identity` and `mc.identity`: a private key or an identity seed.
typedef struct sdrkit_lora_identity_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The key in base64 or hex.
  sdrkit_string key;
} sdrkit_lora_identity_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORA_IDENTITY_ARGS_INIT {sizeof(sdrkit_lora_identity_args), {0}}

// The arguments of `send` of the raw stack.
typedef struct sdrkit_loraraw_raw_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The payload of the frame.
  sdrkit_bytes payload;
} sdrkit_loraraw_raw_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LORARAW_RAW_SEND_ARGS_INIT {sizeof(sdrkit_loraraw_raw_send_args), {0}}

// The arguments of `remove` of the APRS stack.
typedef struct sdrkit_aprs_remove_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_aprs_remove_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_APRS_REMOVE_ARGS_INIT {sizeof(sdrkit_aprs_remove_args), {0}}

// Fills `rows` with the changed rows of the table "packets" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_mesh_packets_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mesh_packet_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshPacketRow", "packets", rows, sizeof(sdrkit_mesh_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `last_heard`.
static inline int32_t sdrkit_mesh_nodes_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mesh_node_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshNodeRow", "nodes", rows, sizeof(sdrkit_mesh_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_mesh_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "readings" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `node` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`.
// Time: `time_s`.
static inline int32_t sdrkit_mesh_readings_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_reading_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshReadingRow", "readings", rows, sizeof(sdrkit_mesh_reading_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_mesh_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshStationRow", "stations", rows, sizeof(sdrkit_mesh_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_mesh_devices_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mesh_device_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_mesh_types_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mesh_type_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `preset` names a row.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_mesh_decoders_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_decoder_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDecoderRow", "decoders", rows, sizeof(sdrkit_mesh_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "contacts" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_mesh_contacts_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_contact_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshContactRow", "contacts", rows, sizeof(sdrkit_mesh_contact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
static inline int32_t sdrkit_mesh_channels_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_channel_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshChannelRow", "channels", rows, sizeof(sdrkit_mesh_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slots" of the stack "mesh" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `slot` names a row.
// Default order: the column `slot`, lowest first.
static inline int32_t sdrkit_mesh_slots_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mesh_slot_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshSlotRow", "slots", rows, sizeof(sdrkit_mesh_slot_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "lora.packet"
// (LoRa packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_lora_packet_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_lora_packet_event *event) {
  return sdrkit_typed_event(handle, n, "lora.packet", "lora.LoraPacketEvent", event, sizeof(sdrkit_lora_packet_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mesh.text"
// (Mesh text message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mesh_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mesh_text_event *event) {
  return sdrkit_typed_event(handle, n, "mesh.text", "lora.MeshTextEvent", event, sizeof(sdrkit_mesh_text_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mesh.node"
// (Mesh node heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mesh_node_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mesh_node_event *event) {
  return sdrkit_typed_event(handle, n, "mesh.node", "lora.MeshNodeEvent", event, sizeof(sdrkit_mesh_node_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tx.request"
// (Frame to send). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_lora_tx_request_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_lora_tx_request_event *event) {
  return sdrkit_typed_event(handle, n, "tx.request", "lora.TxRequestEvent", event, sizeof(sdrkit_lora_tx_request_event));
}

// Runs the command "mt.send" (Send a Meshtastic text) of the stack "mesh" on the session of `handle`. To a node ("!xxxxxxxx") or to the channel ("^all"). The reply holds the packet id of the text; the column `state` of its row in the messages says whether it was delivered. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_mesh_mt_send(SDRKIT_HANDLE handle, const sdrkit_lora_mt_send_args *args, sdrkit_lora_mt_send_reply *reply) {
  return sdrkit_typed_command(handle, "mt.send", "lora.MtSendArgs", args, "lora.MtSendReply", reply, sizeof(sdrkit_lora_mt_send_reply));
}

// Runs the command "mc.send" (Send a MeshCore text) of the stack "mesh" on the session of `handle`. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_mesh_mc_send(SDRKIT_HANDLE handle, const sdrkit_lora_mc_send_args *args) {
  return sdrkit_typed_command(handle, "mc.send", "lora.McSendArgs", args, NULL, NULL, 0);
}

// Runs the command "aprs.send" (Send an APRS message) of the stack "mesh" on the session of `handle`. Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_mesh_aprs_send(SDRKIT_HANDLE handle, const sdrkit_aprs_send_args *args, sdrkit_aprs_send_reply *reply) {
  return sdrkit_typed_command(handle, "aprs.send", "lora.AprsSendArgs", args, "lora.AprsSendReply", reply, sizeof(sdrkit_aprs_send_reply));
}

// Runs the command "mt.channel.add" (Add a Meshtastic channel) of the stack "mesh" on the session of `handle`. The session reads the texts of the channel and can send on it. The key is kept and never shown.
static inline int32_t sdrkit_mesh_mt_channel_add(SDRKIT_HANDLE handle, const sdrkit_lora_mt_channel_add_args *args) {
  return sdrkit_typed_command(handle, "mt.channel.add", "lora.MtChannelAddArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.channel.add" (Add a MeshCore channel) of the stack "mesh" on the session of `handle`. The session reads the texts of the channel and can send on it. The key is kept and never shown.
static inline int32_t sdrkit_mesh_mc_channel_add(SDRKIT_HANDLE handle, const sdrkit_lora_mc_channel_add_args *args) {
  return sdrkit_typed_command(handle, "mc.channel.add", "lora.McChannelAddArgs", args, NULL, NULL, 0);
}

// Runs the command "channel.remove" (Remove a channel) of the stack "mesh" on the session of `handle`.
static inline int32_t sdrkit_mesh_channel_remove(SDRKIT_HANDLE handle, const sdrkit_lora_channel_remove_args *args) {
  return sdrkit_typed_command(handle, "channel.remove", "lora.ChannelRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.advert" (Send a MeshCore advert) of the stack "mesh" on the session of `handle`. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_mesh_mc_advert(SDRKIT_HANDLE handle, const sdrkit_lora_mc_advert_args *args) {
  return sdrkit_typed_command(handle, "mc.advert", "lora.McAdvertArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.contact.add" (Add a MeshCore contact) of the stack "mesh" on the session of `handle`.
static inline int32_t sdrkit_mesh_mc_contact_add(SDRKIT_HANDLE handle, const sdrkit_lora_mc_contact_add_args *args) {
  return sdrkit_typed_command(handle, "mc.contact.add", "lora.McContactAddArgs", args, NULL, NULL, 0);
}

// Runs the command "aprs.beacon" (Send an APRS beacon) of the stack "mesh" on the session of `handle`. The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_mesh_aprs_beacon(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "aprs.beacon", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "mt.request" (Ask a Meshtastic node) of the stack "mesh" on the session of `handle`. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_mesh_mt_request(SDRKIT_HANDLE handle, const sdrkit_lora_mt_request_args *args) {
  return sdrkit_typed_command(handle, "mt.request", "lora.MtRequestArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "mesh" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_mesh_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "mesh" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_mesh_remove(SDRKIT_HANDLE handle, const sdrkit_mesh_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.MeshRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "mesh" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_mesh_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "mt.channel" (Set a Meshtastic channel slot) of the stack "mesh" on the session of `handle`. Sets the key and the position precision of one of the 8 channel slots. Slot 0 is the primary channel: its name comes from the setting `mt_channel_name`.
static inline int32_t sdrkit_mesh_mt_channel(SDRKIT_HANDLE handle, const sdrkit_lora_mt_channel_args *args) {
  return sdrkit_typed_command(handle, "mt.channel", "lora.MtChannelArgs", args, NULL, NULL, 0);
}

// Runs the command "mt.identity" (Set the Meshtastic key) of the stack "mesh" on the session of `handle`.
static inline int32_t sdrkit_mesh_mt_identity(SDRKIT_HANDLE handle, const sdrkit_lora_identity_args *args) {
  return sdrkit_typed_command(handle, "mt.identity", "lora.IdentityArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.identity" (Set the MeshCore identity) of the stack "mesh" on the session of `handle`.
static inline int32_t sdrkit_mesh_mc_identity(SDRKIT_HANDLE handle, const sdrkit_lora_identity_args *args) {
  return sdrkit_typed_command(handle, "mc.identity", "lora.IdentityArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "mesh" are the sdrkit_mesh_gen_*() functions
// after the settings of its receiver.

// The setting "dial_freq" (Dial) of the stack "mesh". A frequency in Hz. The frequency of the stack's primary channel. A change after the start asks the host to tune there.
static inline int32_t sdrkit_mesh_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_mesh_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "protocols" (Protocols) of the stack "mesh". Bits: 1 Meshtastic, 2 MeshCore, 4 LoRa APRS, 8 MeshCom, 16 LoRaWAN. Only the mesh stack reads it.
static inline int32_t sdrkit_mesh_get_protocols(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "protocols", value);
}
static inline int32_t sdrkit_mesh_set_protocols(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "protocols", value);
}

// The setting "preset_search" (Every Meshtastic preset) of the stack "mesh". Also every other preset of the band (more work).
static inline int32_t sdrkit_mesh_get_preset_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "preset_search", value);
}
static inline int32_t sdrkit_mesh_set_preset_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "preset_search", value);
}

// The setting "mt_slot_start" (Slot band start) of the stack "mesh". A frequency in Hz. The lower edge of the band in which the name of the primary channel chooses the frequency slot. With both edges 0 each preset lies at the dial. From 2.4 GHz the presets take the wide bandwidths.
static inline int32_t sdrkit_mesh_get_mt_slot_start(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_slot_start", value);
}
static inline int32_t sdrkit_mesh_set_mt_slot_start(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_slot_start", value);
}

// The setting "mt_slot_end" (Slot band end) of the stack "mesh". A frequency in Hz. The upper edge of the band of the frequency slots.
static inline int32_t sdrkit_mesh_get_mt_slot_end(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_slot_end", value);
}
static inline int32_t sdrkit_mesh_set_mt_slot_end(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_slot_end", value);
}

// The setting "channels" (Channels) of the stack "mesh". MeshCore, LoRa APRS and MeshCom networks: entries separated by ";", each a label, "=", the protocol (meshcore, loraaprs or meshcom) and, each after "/", the frequency in Hz, the bandwidth in Hz, the spreading factor, the coding rate and, when they differ from the protocol's, the preamble and the sync word. For example "Net=meshcore/100000000/62500/8/8". At most 256 entries. A protocol without a channel listens at the dial.
static inline int32_t sdrkit_mesh_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_mesh_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "lw_channels" (LoRaWAN channels) of the stack "mesh". Entries separated by ";", each the frequency in Hz, the bandwidth in Hz, the spreading factors and the flags, separated by "/": u devices send there, d gateways answer there, j a default channel. For example "100100000/125000/7-12/udj". At most 256 entries. Empty: one channel at the dial.
static inline int32_t sdrkit_mesh_get_lw_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "lw_channels", value);
}
static inline int32_t sdrkit_mesh_set_lw_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "lw_channels", value);
}

// The setting "mt_preset" (Meshtastic preset) of the stack "mesh". A value of `sdrkit_mesh_mt_preset`.
static inline int32_t sdrkit_mesh_get_mt_preset(SDRKIT_HANDLE handle, sdrkit_mesh_mt_preset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mt_preset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mesh_mt_preset)index;
  return code;
}
static inline int32_t sdrkit_mesh_set_mt_preset(SDRKIT_HANDLE handle, sdrkit_mesh_mt_preset value) {
  return sdrkit_typed_setting_set_enum(handle, "mt_preset", (int32_t)value);
}

// The setting "mt_channel_name" (Primary channel name) of the stack "mesh". The name decides the frequency slot. Empty takes the name of the preset.
static inline int32_t sdrkit_mesh_get_mt_channel_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_channel_name", value);
}
static inline int32_t sdrkit_mesh_set_mt_channel_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_channel_name", value);
}

// The setting "mt_channel_num" (Channel number) of the stack "mesh". The frequency slot, from 1. 0 takes it from the primary channel's name. With a slot band, a number above the slots of the preset in the band is refused when it is set; after a later change of the band or the preset it counts on from slot 1 again (number minus 1, modulo the slots), as Meshtastic devices count.
static inline int32_t sdrkit_mesh_get_mt_channel_num(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "mt_channel_num", value);
}
static inline int32_t sdrkit_mesh_set_mt_channel_num(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "mt_channel_num", value);
}

// The setting "mt_override_freq" (Frequency override) of the stack "mesh". A frequency in Hz.
static inline int32_t sdrkit_mesh_get_mt_override_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_override_freq", value);
}
static inline int32_t sdrkit_mesh_set_mt_override_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_override_freq", value);
}

// The setting "mt_freq_offset" (Frequency offset) of the stack "mesh". A frequency in Hz.
static inline int32_t sdrkit_mesh_get_mt_freq_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_freq_offset", value);
}
static inline int32_t sdrkit_mesh_set_mt_freq_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_freq_offset", value);
}

// The setting "custom" (Custom LoRa setting) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_custom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "custom", value);
}
static inline int32_t sdrkit_mesh_set_custom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "custom", value);
}

// The setting "custom_freq" (Frequency) of the stack "mesh". A frequency in Hz.
static inline int32_t sdrkit_mesh_get_custom_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "custom_freq", value);
}
static inline int32_t sdrkit_mesh_set_custom_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "custom_freq", value);
}

// The setting "custom_bw" (Bandwidth) of the stack "mesh". A value of `sdrkit_mesh_custom_bw`.
static inline int32_t sdrkit_mesh_get_custom_bw(SDRKIT_HANDLE handle, sdrkit_mesh_custom_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mesh_custom_bw)index;
  return code;
}
static inline int32_t sdrkit_mesh_set_custom_bw(SDRKIT_HANDLE handle, sdrkit_mesh_custom_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_bw", (int32_t)value);
}

// The setting "custom_sf" (Spreading factor) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_custom_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sf", value);
}
static inline int32_t sdrkit_mesh_set_custom_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sf", value);
}

// The setting "custom_cr" (Coding rate) of the stack "mesh". A value of `sdrkit_mesh_custom_cr`.
static inline int32_t sdrkit_mesh_get_custom_cr(SDRKIT_HANDLE handle, sdrkit_mesh_custom_cr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mesh_custom_cr)index;
  return code;
}
static inline int32_t sdrkit_mesh_set_custom_cr(SDRKIT_HANDLE handle, sdrkit_mesh_custom_cr value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_cr", (int32_t)value);
}

// The setting "custom_sync" (Sync word) of the stack "mesh". 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.
static inline int32_t sdrkit_mesh_get_custom_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sync", value);
}
static inline int32_t sdrkit_mesh_set_custom_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sync", value);
}

// The setting "custom_preamble" (Preamble) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_custom_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_preamble", value);
}
static inline int32_t sdrkit_mesh_set_custom_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_preamble", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_mesh_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "jump_chips" (Expected jump) of the stack "mesh". Unit: chips.
static inline int32_t sdrkit_mesh_get_jump_chips(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jump_chips", value);
}
static inline int32_t sdrkit_mesh_set_jump_chips(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jump_chips", value);
}

// The setting "raw" (Raw payload) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_raw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw", value);
}
static inline int32_t sdrkit_mesh_set_raw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "mesh". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_mesh_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_mesh_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "mt_node_id" (Node ID) of the stack "mesh". "!xxxxxxxx". Empty takes one from the callsign of the station, or a random one.
static inline int32_t sdrkit_mesh_get_mt_node_id(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_node_id", value);
}
static inline int32_t sdrkit_mesh_set_mt_node_id(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_node_id", value);
}

// The setting "mt_long_name" (Long name) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mt_long_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_long_name", value);
}
static inline int32_t sdrkit_mesh_set_mt_long_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_long_name", value);
}

// The setting "mt_short_name" (Short name) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mt_short_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_short_name", value);
}
static inline int32_t sdrkit_mesh_set_mt_short_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_short_name", value);
}

// The setting "mt_role" (Role) of the stack "mesh". A value of `sdrkit_mesh_mt_role`.
static inline int32_t sdrkit_mesh_get_mt_role(SDRKIT_HANDLE handle, sdrkit_mesh_mt_role *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mt_role", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mesh_mt_role)index;
  return code;
}
static inline int32_t sdrkit_mesh_set_mt_role(SDRKIT_HANDLE handle, sdrkit_mesh_mt_role value) {
  return sdrkit_typed_setting_set_enum(handle, "mt_role", (int32_t)value);
}

// The setting "mt_licensed" (Licensed (ham) mode) of the stack "mesh". No encryption, and the callsign of the station is the long name.
static inline int32_t sdrkit_mesh_get_mt_licensed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mt_licensed", value);
}
static inline int32_t sdrkit_mesh_set_mt_licensed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mt_licensed", value);
}

// The setting "mt_callsign_prefix" (Callsign prefix) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mt_callsign_prefix(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mt_callsign_prefix", value);
}
static inline int32_t sdrkit_mesh_set_mt_callsign_prefix(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mt_callsign_prefix", value);
}

// The setting "mt_hop_limit" (Hop limit) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mt_hop_limit(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "mt_hop_limit", value);
}
static inline int32_t sdrkit_mesh_set_mt_hop_limit(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "mt_hop_limit", value);
}

// The setting "mt_nodeinfo_interval" (NodeInfo interval) of the stack "mesh". Unit: s. 0 sends it only when asked.
static inline int32_t sdrkit_mesh_get_mt_nodeinfo_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_nodeinfo_interval", value);
}
static inline int32_t sdrkit_mesh_set_mt_nodeinfo_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_nodeinfo_interval", value);
}

// The setting "mt_position_mode" (Position) of the stack "mesh". A value of `sdrkit_mesh_mt_position_mode`.
static inline int32_t sdrkit_mesh_get_mt_position_mode(SDRKIT_HANDLE handle, sdrkit_mesh_mt_position_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mt_position_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mesh_mt_position_mode)index;
  return code;
}
static inline int32_t sdrkit_mesh_set_mt_position_mode(SDRKIT_HANDLE handle, sdrkit_mesh_mt_position_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "mt_position_mode", (int32_t)value);
}

// The setting "mt_position_interval" (Position interval) of the stack "mesh". Unit: s.
static inline int32_t sdrkit_mesh_get_mt_position_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_position_interval", value);
}
static inline int32_t sdrkit_mesh_set_mt_position_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_position_interval", value);
}

// The setting "mt_ack_broadcast" (Want-ack on channel texts) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mt_ack_broadcast(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mt_ack_broadcast", value);
}
static inline int32_t sdrkit_mesh_set_mt_ack_broadcast(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mt_ack_broadcast", value);
}

// The setting "mc_name" (Name) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mc_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mc_name", value);
}
static inline int32_t sdrkit_mesh_set_mc_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mc_name", value);
}

// The setting "mc_share_location" (Send the station's position) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_mc_share_location(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mc_share_location", value);
}
static inline int32_t sdrkit_mesh_set_mc_share_location(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mc_share_location", value);
}

// The setting "mc_advert_interval" (Advert interval) of the stack "mesh". Unit: s. 0 sends an advert only when asked.
static inline int32_t sdrkit_mesh_get_mc_advert_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mc_advert_interval", value);
}
static inline int32_t sdrkit_mesh_set_mc_advert_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mc_advert_interval", value);
}

// The setting "aprs_ssid" (SSID) of the stack "mesh". The SSID after the callsign of the station in own frames. On AX.25 the callsign must then carry no SSID of its own.
static inline int32_t sdrkit_mesh_get_aprs_ssid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "aprs_ssid", value);
}
static inline int32_t sdrkit_mesh_set_aprs_ssid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "aprs_ssid", value);
}

// The setting "aprs_tocall" (Destination) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_aprs_tocall(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_tocall", value);
}
static inline int32_t sdrkit_mesh_set_aprs_tocall(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_tocall", value);
}

// The setting "aprs_path" (Path) of the stack "mesh". Calls separated by commas, at most eight. Empty sends no path.
static inline int32_t sdrkit_mesh_get_aprs_path(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_path", value);
}
static inline int32_t sdrkit_mesh_set_aprs_path(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_path", value);
}

// The setting "aprs_symbol" (Symbol) of the stack "mesh". Two characters: the symbol table, then the symbol code.
static inline int32_t sdrkit_mesh_get_aprs_symbol(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_symbol", value);
}
static inline int32_t sdrkit_mesh_set_aprs_symbol(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_symbol", value);
}

// The setting "aprs_comment" (Comment) of the stack "mesh". The comment of the beacon. Without a position of the station the beacon sends this text as its status.
static inline int32_t sdrkit_mesh_get_aprs_comment(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_comment", value);
}
static inline int32_t sdrkit_mesh_set_aprs_comment(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_comment", value);
}

// The setting "duty_limit" (Duty-cycle limit) of the stack "mesh".
static inline int32_t sdrkit_mesh_get_duty_limit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "duty_limit", value);
}
static inline int32_t sdrkit_mesh_set_duty_limit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "duty_limit", value);
}

// The setting "duty_bands" (Duty-cycle bands) of the stack "mesh". Entries separated by ";", each the lower and the upper edge in Hz separated by "-", "=" and the limit in percent of the air time of the last hour, for example "100000000-100250000=10". Empty: no band has a limit.
static inline int32_t sdrkit_mesh_get_duty_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "duty_bands", value);
}
static inline int32_t sdrkit_mesh_set_duty_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "duty_bands", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "mesh". A value of `sdrkit_mesh_gen_scene`.
static inline int32_t sdrkit_mesh_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_mesh_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mesh_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_mesh_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_mesh_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_damaged_copy" (Damaged copy first) of the test signal generator of the stack "mesh". The texting scene sends one more Meshtastic text: first with one bit of its payload flipped, so that its checksum fails, and 6 s later as a good copy. The text shows with crc_ok false, and the good copy corrects it.
static inline int32_t sdrkit_mesh_gen_get_damaged_copy(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_damaged_copy", value);
}
static inline int32_t sdrkit_mesh_gen_set_damaged_copy(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_damaged_copy", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_meshtastic_packets_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mesh_packet_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshPacketRow", "packets", rows, sizeof(sdrkit_mesh_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `last_heard`.
static inline int32_t sdrkit_meshtastic_nodes_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_node_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshNodeRow", "nodes", rows, sizeof(sdrkit_mesh_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_meshtastic_messages_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_message_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "readings" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `node` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`.
// Time: `time_s`.
static inline int32_t sdrkit_meshtastic_readings_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_reading_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshReadingRow", "readings", rows, sizeof(sdrkit_mesh_reading_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_meshtastic_stations_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_station_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshStationRow", "stations", rows, sizeof(sdrkit_mesh_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_meshtastic_devices_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mesh_device_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_meshtastic_types_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_type_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `preset` names a row.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_meshtastic_decoders_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_decoder_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDecoderRow", "decoders", rows, sizeof(sdrkit_mesh_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "contacts" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_meshtastic_contacts_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_contact_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshContactRow", "contacts", rows, sizeof(sdrkit_mesh_contact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
static inline int32_t sdrkit_meshtastic_channels_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_channel_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshChannelRow", "channels", rows, sizeof(sdrkit_mesh_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slots" of the stack "meshtastic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `slot` names a row.
// Default order: the column `slot`, lowest first.
static inline int32_t sdrkit_meshtastic_slots_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_slot_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshSlotRow", "slots", rows, sizeof(sdrkit_mesh_slot_row), capacity, info);
}

// Runs the command "mt.send" (Send a Meshtastic text) of the stack "meshtastic" on the session of `handle`. To a node ("!xxxxxxxx") or to the channel ("^all"). The reply holds the packet id of the text; the column `state` of its row in the messages says whether it was delivered. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_meshtastic_mt_send(SDRKIT_HANDLE handle, const sdrkit_lora_mt_send_args *args, sdrkit_lora_mt_send_reply *reply) {
  return sdrkit_typed_command(handle, "mt.send", "lora.MtSendArgs", args, "lora.MtSendReply", reply, sizeof(sdrkit_lora_mt_send_reply));
}

// Runs the command "mt.channel.add" (Add a Meshtastic channel) of the stack "meshtastic" on the session of `handle`. The session reads the texts of the channel and can send on it. The key is kept and never shown.
static inline int32_t sdrkit_meshtastic_mt_channel_add(SDRKIT_HANDLE handle, const sdrkit_lora_mt_channel_add_args *args) {
  return sdrkit_typed_command(handle, "mt.channel.add", "lora.MtChannelAddArgs", args, NULL, NULL, 0);
}

// Runs the command "channel.remove" (Remove a channel) of the stack "meshtastic" on the session of `handle`.
static inline int32_t sdrkit_meshtastic_channel_remove(SDRKIT_HANDLE handle, const sdrkit_lora_channel_remove_args *args) {
  return sdrkit_typed_command(handle, "channel.remove", "lora.ChannelRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "mt.request" (Ask a Meshtastic node) of the stack "meshtastic" on the session of `handle`. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_meshtastic_mt_request(SDRKIT_HANDLE handle, const sdrkit_lora_mt_request_args *args) {
  return sdrkit_typed_command(handle, "mt.request", "lora.MtRequestArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "meshtastic" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_meshtastic_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "meshtastic" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_meshtastic_remove(SDRKIT_HANDLE handle, const sdrkit_mesh_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.MeshRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "meshtastic" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_meshtastic_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "mt.channel" (Set a Meshtastic channel slot) of the stack "meshtastic" on the session of `handle`. Sets the key and the position precision of one of the 8 channel slots. Slot 0 is the primary channel: its name comes from the setting `mt_channel_name`.
static inline int32_t sdrkit_meshtastic_mt_channel(SDRKIT_HANDLE handle, const sdrkit_lora_mt_channel_args *args) {
  return sdrkit_typed_command(handle, "mt.channel", "lora.MtChannelArgs", args, NULL, NULL, 0);
}

// Runs the command "mt.identity" (Set the Meshtastic key) of the stack "meshtastic" on the session of `handle`.
static inline int32_t sdrkit_meshtastic_mt_identity(SDRKIT_HANDLE handle, const sdrkit_lora_identity_args *args) {
  return sdrkit_typed_command(handle, "mt.identity", "lora.IdentityArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "meshtastic" are the sdrkit_meshtastic_gen_*() functions
// after the settings of its receiver.

// The setting "dial_freq" (Dial) of the stack "meshtastic". A frequency in Hz. The frequency of the stack's primary channel. A change after the start asks the host to tune there.
static inline int32_t sdrkit_meshtastic_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_meshtastic_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "preset_search" (Every Meshtastic preset) of the stack "meshtastic". Also every other preset of the band (more work).
static inline int32_t sdrkit_meshtastic_get_preset_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "preset_search", value);
}
static inline int32_t sdrkit_meshtastic_set_preset_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "preset_search", value);
}

// The setting "mt_slot_start" (Slot band start) of the stack "meshtastic". A frequency in Hz. The lower edge of the band in which the name of the primary channel chooses the frequency slot. With both edges 0 each preset lies at the dial. From 2.4 GHz the presets take the wide bandwidths.
static inline int32_t sdrkit_meshtastic_get_mt_slot_start(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_slot_start", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_slot_start(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_slot_start", value);
}

// The setting "mt_slot_end" (Slot band end) of the stack "meshtastic". A frequency in Hz. The upper edge of the band of the frequency slots.
static inline int32_t sdrkit_meshtastic_get_mt_slot_end(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_slot_end", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_slot_end(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_slot_end", value);
}

// The setting "mt_preset" (Meshtastic preset) of the stack "meshtastic". A value of `sdrkit_meshtastic_mt_preset`.
static inline int32_t sdrkit_meshtastic_get_mt_preset(SDRKIT_HANDLE handle, sdrkit_meshtastic_mt_preset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mt_preset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshtastic_mt_preset)index;
  return code;
}
static inline int32_t sdrkit_meshtastic_set_mt_preset(SDRKIT_HANDLE handle, sdrkit_meshtastic_mt_preset value) {
  return sdrkit_typed_setting_set_enum(handle, "mt_preset", (int32_t)value);
}

// The setting "mt_channel_name" (Primary channel name) of the stack "meshtastic". The name decides the frequency slot. Empty takes the name of the preset.
static inline int32_t sdrkit_meshtastic_get_mt_channel_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_channel_name", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_channel_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_channel_name", value);
}

// The setting "mt_channel_num" (Channel number) of the stack "meshtastic". The frequency slot, from 1. 0 takes it from the primary channel's name. With a slot band, a number above the slots of the preset in the band is refused when it is set; after a later change of the band or the preset it counts on from slot 1 again (number minus 1, modulo the slots), as Meshtastic devices count.
static inline int32_t sdrkit_meshtastic_get_mt_channel_num(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "mt_channel_num", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_channel_num(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "mt_channel_num", value);
}

// The setting "mt_override_freq" (Frequency override) of the stack "meshtastic". A frequency in Hz.
static inline int32_t sdrkit_meshtastic_get_mt_override_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_override_freq", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_override_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_override_freq", value);
}

// The setting "mt_freq_offset" (Frequency offset) of the stack "meshtastic". A frequency in Hz.
static inline int32_t sdrkit_meshtastic_get_mt_freq_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_freq_offset", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_freq_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_freq_offset", value);
}

// The setting "custom" (Custom LoRa setting) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_custom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "custom", value);
}
static inline int32_t sdrkit_meshtastic_set_custom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "custom", value);
}

// The setting "custom_freq" (Frequency) of the stack "meshtastic". A frequency in Hz.
static inline int32_t sdrkit_meshtastic_get_custom_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "custom_freq", value);
}
static inline int32_t sdrkit_meshtastic_set_custom_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "custom_freq", value);
}

// The setting "custom_bw" (Bandwidth) of the stack "meshtastic". A value of `sdrkit_meshtastic_custom_bw`.
static inline int32_t sdrkit_meshtastic_get_custom_bw(SDRKIT_HANDLE handle, sdrkit_meshtastic_custom_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshtastic_custom_bw)index;
  return code;
}
static inline int32_t sdrkit_meshtastic_set_custom_bw(SDRKIT_HANDLE handle, sdrkit_meshtastic_custom_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_bw", (int32_t)value);
}

// The setting "custom_sf" (Spreading factor) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_custom_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sf", value);
}
static inline int32_t sdrkit_meshtastic_set_custom_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sf", value);
}

// The setting "custom_cr" (Coding rate) of the stack "meshtastic". A value of `sdrkit_meshtastic_custom_cr`.
static inline int32_t sdrkit_meshtastic_get_custom_cr(SDRKIT_HANDLE handle, sdrkit_meshtastic_custom_cr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshtastic_custom_cr)index;
  return code;
}
static inline int32_t sdrkit_meshtastic_set_custom_cr(SDRKIT_HANDLE handle, sdrkit_meshtastic_custom_cr value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_cr", (int32_t)value);
}

// The setting "custom_sync" (Sync word) of the stack "meshtastic". 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.
static inline int32_t sdrkit_meshtastic_get_custom_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sync", value);
}
static inline int32_t sdrkit_meshtastic_set_custom_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sync", value);
}

// The setting "custom_preamble" (Preamble) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_custom_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_preamble", value);
}
static inline int32_t sdrkit_meshtastic_set_custom_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_preamble", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_meshtastic_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "jump_chips" (Expected jump) of the stack "meshtastic". Unit: chips.
static inline int32_t sdrkit_meshtastic_get_jump_chips(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jump_chips", value);
}
static inline int32_t sdrkit_meshtastic_set_jump_chips(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jump_chips", value);
}

// The setting "raw" (Raw payload) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_raw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw", value);
}
static inline int32_t sdrkit_meshtastic_set_raw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "meshtastic". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_meshtastic_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_meshtastic_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "mt_node_id" (Node ID) of the stack "meshtastic". "!xxxxxxxx". Empty takes one from the callsign of the station, or a random one.
static inline int32_t sdrkit_meshtastic_get_mt_node_id(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_node_id", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_node_id(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_node_id", value);
}

// The setting "mt_long_name" (Long name) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_mt_long_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_long_name", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_long_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_long_name", value);
}

// The setting "mt_short_name" (Short name) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_mt_short_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mt_short_name", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_short_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mt_short_name", value);
}

// The setting "mt_role" (Role) of the stack "meshtastic". A value of `sdrkit_meshtastic_mt_role`.
static inline int32_t sdrkit_meshtastic_get_mt_role(SDRKIT_HANDLE handle, sdrkit_meshtastic_mt_role *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mt_role", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshtastic_mt_role)index;
  return code;
}
static inline int32_t sdrkit_meshtastic_set_mt_role(SDRKIT_HANDLE handle, sdrkit_meshtastic_mt_role value) {
  return sdrkit_typed_setting_set_enum(handle, "mt_role", (int32_t)value);
}

// The setting "mt_licensed" (Licensed (ham) mode) of the stack "meshtastic". No encryption, and the callsign of the station is the long name.
static inline int32_t sdrkit_meshtastic_get_mt_licensed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mt_licensed", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_licensed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mt_licensed", value);
}

// The setting "mt_callsign_prefix" (Callsign prefix) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_mt_callsign_prefix(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mt_callsign_prefix", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_callsign_prefix(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mt_callsign_prefix", value);
}

// The setting "mt_hop_limit" (Hop limit) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_mt_hop_limit(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "mt_hop_limit", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_hop_limit(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "mt_hop_limit", value);
}

// The setting "mt_nodeinfo_interval" (NodeInfo interval) of the stack "meshtastic". Unit: s. 0 sends it only when asked.
static inline int32_t sdrkit_meshtastic_get_mt_nodeinfo_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_nodeinfo_interval", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_nodeinfo_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_nodeinfo_interval", value);
}

// The setting "mt_position_mode" (Position) of the stack "meshtastic". A value of `sdrkit_meshtastic_mt_position_mode`.
static inline int32_t sdrkit_meshtastic_get_mt_position_mode(SDRKIT_HANDLE handle, sdrkit_meshtastic_mt_position_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mt_position_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshtastic_mt_position_mode)index;
  return code;
}
static inline int32_t sdrkit_meshtastic_set_mt_position_mode(SDRKIT_HANDLE handle, sdrkit_meshtastic_mt_position_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "mt_position_mode", (int32_t)value);
}

// The setting "mt_position_interval" (Position interval) of the stack "meshtastic". Unit: s.
static inline int32_t sdrkit_meshtastic_get_mt_position_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_position_interval", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_position_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_position_interval", value);
}

// The setting "mt_ack_broadcast" (Want-ack on channel texts) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_mt_ack_broadcast(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mt_ack_broadcast", value);
}
static inline int32_t sdrkit_meshtastic_set_mt_ack_broadcast(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mt_ack_broadcast", value);
}

// The setting "duty_limit" (Duty-cycle limit) of the stack "meshtastic".
static inline int32_t sdrkit_meshtastic_get_duty_limit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "duty_limit", value);
}
static inline int32_t sdrkit_meshtastic_set_duty_limit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "duty_limit", value);
}

// The setting "duty_bands" (Duty-cycle bands) of the stack "meshtastic". Entries separated by ";", each the lower and the upper edge in Hz separated by "-", "=" and the limit in percent of the air time of the last hour, for example "100000000-100250000=10". Empty: no band has a limit.
static inline int32_t sdrkit_meshtastic_get_duty_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "duty_bands", value);
}
static inline int32_t sdrkit_meshtastic_set_duty_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "duty_bands", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "meshtastic". A value of `sdrkit_meshtastic_gen_scene`.
static inline int32_t sdrkit_meshtastic_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_meshtastic_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshtastic_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_meshtastic_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_meshtastic_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_damaged_copy" (Damaged copy first) of the test signal generator of the stack "meshtastic". The texting scene sends one more Meshtastic text: first with one bit of its payload flipped, so that its checksum fails, and 6 s later as a good copy. The text shows with crc_ok false, and the good copy corrects it.
static inline int32_t sdrkit_meshtastic_gen_get_damaged_copy(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_damaged_copy", value);
}
static inline int32_t sdrkit_meshtastic_gen_set_damaged_copy(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_damaged_copy", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_meshcore_packets_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_packet_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshPacketRow", "packets", rows, sizeof(sdrkit_mesh_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `last_heard`.
static inline int32_t sdrkit_meshcore_nodes_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mesh_node_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshNodeRow", "nodes", rows, sizeof(sdrkit_mesh_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_meshcore_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "readings" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `node` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`.
// Time: `time_s`.
static inline int32_t sdrkit_meshcore_readings_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_reading_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshReadingRow", "readings", rows, sizeof(sdrkit_mesh_reading_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_meshcore_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshStationRow", "stations", rows, sizeof(sdrkit_mesh_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_meshcore_devices_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_device_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_meshcore_types_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mesh_type_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `preset` names a row.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_meshcore_decoders_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_decoder_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDecoderRow", "decoders", rows, sizeof(sdrkit_mesh_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "contacts" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_meshcore_contacts_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_contact_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshContactRow", "contacts", rows, sizeof(sdrkit_mesh_contact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
static inline int32_t sdrkit_meshcore_channels_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_channel_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshChannelRow", "channels", rows, sizeof(sdrkit_mesh_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slots" of the stack "meshcore" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `slot` names a row.
// Default order: the column `slot`, lowest first.
static inline int32_t sdrkit_meshcore_slots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mesh_slot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshSlotRow", "slots", rows, sizeof(sdrkit_mesh_slot_row), capacity, info);
}

// Runs the command "mc.send" (Send a MeshCore text) of the stack "meshcore" on the session of `handle`. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_meshcore_mc_send(SDRKIT_HANDLE handle, const sdrkit_lora_mc_send_args *args) {
  return sdrkit_typed_command(handle, "mc.send", "lora.McSendArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.channel.add" (Add a MeshCore channel) of the stack "meshcore" on the session of `handle`. The session reads the texts of the channel and can send on it. The key is kept and never shown.
static inline int32_t sdrkit_meshcore_mc_channel_add(SDRKIT_HANDLE handle, const sdrkit_lora_mc_channel_add_args *args) {
  return sdrkit_typed_command(handle, "mc.channel.add", "lora.McChannelAddArgs", args, NULL, NULL, 0);
}

// Runs the command "channel.remove" (Remove a channel) of the stack "meshcore" on the session of `handle`.
static inline int32_t sdrkit_meshcore_channel_remove(SDRKIT_HANDLE handle, const sdrkit_lora_channel_remove_args *args) {
  return sdrkit_typed_command(handle, "channel.remove", "lora.ChannelRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.advert" (Send a MeshCore advert) of the stack "meshcore" on the session of `handle`. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_meshcore_mc_advert(SDRKIT_HANDLE handle, const sdrkit_lora_mc_advert_args *args) {
  return sdrkit_typed_command(handle, "mc.advert", "lora.McAdvertArgs", args, NULL, NULL, 0);
}

// Runs the command "mc.contact.add" (Add a MeshCore contact) of the stack "meshcore" on the session of `handle`.
static inline int32_t sdrkit_meshcore_mc_contact_add(SDRKIT_HANDLE handle, const sdrkit_lora_mc_contact_add_args *args) {
  return sdrkit_typed_command(handle, "mc.contact.add", "lora.McContactAddArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "meshcore" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_meshcore_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "meshcore" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_meshcore_remove(SDRKIT_HANDLE handle, const sdrkit_mesh_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.MeshRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "meshcore" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_meshcore_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "mc.identity" (Set the MeshCore identity) of the stack "meshcore" on the session of `handle`.
static inline int32_t sdrkit_meshcore_mc_identity(SDRKIT_HANDLE handle, const sdrkit_lora_identity_args *args) {
  return sdrkit_typed_command(handle, "mc.identity", "lora.IdentityArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "meshcore" are the sdrkit_meshcore_gen_*() functions
// after the settings of its receiver.

// The setting "dial_freq" (Dial) of the stack "meshcore". A frequency in Hz. The frequency of the stack's primary channel. A change after the start asks the host to tune there.
static inline int32_t sdrkit_meshcore_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_meshcore_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "mt_slot_start" (Slot band start) of the stack "meshcore". A frequency in Hz. The lower edge of the band in which the name of the primary channel chooses the frequency slot. With both edges 0 each preset lies at the dial. From 2.4 GHz the presets take the wide bandwidths.
static inline int32_t sdrkit_meshcore_get_mt_slot_start(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_slot_start", value);
}
static inline int32_t sdrkit_meshcore_set_mt_slot_start(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_slot_start", value);
}

// The setting "mt_slot_end" (Slot band end) of the stack "meshcore". A frequency in Hz. The upper edge of the band of the frequency slots.
static inline int32_t sdrkit_meshcore_get_mt_slot_end(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mt_slot_end", value);
}
static inline int32_t sdrkit_meshcore_set_mt_slot_end(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mt_slot_end", value);
}

// The setting "channels" (Channels) of the stack "meshcore". MeshCore, LoRa APRS and MeshCom networks: entries separated by ";", each a label, "=", the protocol (meshcore, loraaprs or meshcom) and, each after "/", the frequency in Hz, the bandwidth in Hz, the spreading factor, the coding rate and, when they differ from the protocol's, the preamble and the sync word. For example "Net=meshcore/100000000/62500/8/8". At most 256 entries. A protocol without a channel listens at the dial.
static inline int32_t sdrkit_meshcore_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_meshcore_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "custom" (Custom LoRa setting) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_custom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "custom", value);
}
static inline int32_t sdrkit_meshcore_set_custom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "custom", value);
}

// The setting "custom_freq" (Frequency) of the stack "meshcore". A frequency in Hz.
static inline int32_t sdrkit_meshcore_get_custom_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "custom_freq", value);
}
static inline int32_t sdrkit_meshcore_set_custom_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "custom_freq", value);
}

// The setting "custom_bw" (Bandwidth) of the stack "meshcore". A value of `sdrkit_meshcore_custom_bw`.
static inline int32_t sdrkit_meshcore_get_custom_bw(SDRKIT_HANDLE handle, sdrkit_meshcore_custom_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshcore_custom_bw)index;
  return code;
}
static inline int32_t sdrkit_meshcore_set_custom_bw(SDRKIT_HANDLE handle, sdrkit_meshcore_custom_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_bw", (int32_t)value);
}

// The setting "custom_sf" (Spreading factor) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_custom_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sf", value);
}
static inline int32_t sdrkit_meshcore_set_custom_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sf", value);
}

// The setting "custom_cr" (Coding rate) of the stack "meshcore". A value of `sdrkit_meshcore_custom_cr`.
static inline int32_t sdrkit_meshcore_get_custom_cr(SDRKIT_HANDLE handle, sdrkit_meshcore_custom_cr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshcore_custom_cr)index;
  return code;
}
static inline int32_t sdrkit_meshcore_set_custom_cr(SDRKIT_HANDLE handle, sdrkit_meshcore_custom_cr value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_cr", (int32_t)value);
}

// The setting "custom_sync" (Sync word) of the stack "meshcore". 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.
static inline int32_t sdrkit_meshcore_get_custom_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sync", value);
}
static inline int32_t sdrkit_meshcore_set_custom_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sync", value);
}

// The setting "custom_preamble" (Preamble) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_custom_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_preamble", value);
}
static inline int32_t sdrkit_meshcore_set_custom_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_preamble", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_meshcore_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "jump_chips" (Expected jump) of the stack "meshcore". Unit: chips.
static inline int32_t sdrkit_meshcore_get_jump_chips(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jump_chips", value);
}
static inline int32_t sdrkit_meshcore_set_jump_chips(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jump_chips", value);
}

// The setting "raw" (Raw payload) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_raw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw", value);
}
static inline int32_t sdrkit_meshcore_set_raw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "meshcore". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_meshcore_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_meshcore_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "mc_name" (Name) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_mc_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mc_name", value);
}
static inline int32_t sdrkit_meshcore_set_mc_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mc_name", value);
}

// The setting "mc_share_location" (Send the station's position) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_mc_share_location(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mc_share_location", value);
}
static inline int32_t sdrkit_meshcore_set_mc_share_location(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mc_share_location", value);
}

// The setting "mc_advert_interval" (Advert interval) of the stack "meshcore". Unit: s. 0 sends an advert only when asked.
static inline int32_t sdrkit_meshcore_get_mc_advert_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mc_advert_interval", value);
}
static inline int32_t sdrkit_meshcore_set_mc_advert_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mc_advert_interval", value);
}

// The setting "duty_limit" (Duty-cycle limit) of the stack "meshcore".
static inline int32_t sdrkit_meshcore_get_duty_limit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "duty_limit", value);
}
static inline int32_t sdrkit_meshcore_set_duty_limit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "duty_limit", value);
}

// The setting "duty_bands" (Duty-cycle bands) of the stack "meshcore". Entries separated by ";", each the lower and the upper edge in Hz separated by "-", "=" and the limit in percent of the air time of the last hour, for example "100000000-100250000=10". Empty: no band has a limit.
static inline int32_t sdrkit_meshcore_get_duty_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "duty_bands", value);
}
static inline int32_t sdrkit_meshcore_set_duty_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "duty_bands", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "meshcore". A value of `sdrkit_meshcore_gen_scene`.
static inline int32_t sdrkit_meshcore_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_meshcore_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshcore_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_meshcore_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_meshcore_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_lorawan_packets_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_mesh_packet_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshPacketRow", "packets", rows, sizeof(sdrkit_mesh_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `last_heard`.
static inline int32_t sdrkit_lorawan_nodes_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_node_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshNodeRow", "nodes", rows, sizeof(sdrkit_mesh_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_lorawan_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "readings" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `node` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`.
// Time: `time_s`.
static inline int32_t sdrkit_lorawan_readings_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_reading_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshReadingRow", "readings", rows, sizeof(sdrkit_mesh_reading_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_lorawan_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshStationRow", "stations", rows, sizeof(sdrkit_mesh_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_lorawan_devices_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_mesh_device_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_lorawan_types_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_type_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `preset` names a row.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_lorawan_decoders_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_decoder_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDecoderRow", "decoders", rows, sizeof(sdrkit_mesh_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "contacts" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_lorawan_contacts_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_contact_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshContactRow", "contacts", rows, sizeof(sdrkit_mesh_contact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
static inline int32_t sdrkit_lorawan_channels_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_channel_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshChannelRow", "channels", rows, sizeof(sdrkit_mesh_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slots" of the stack "lorawan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `slot` names a row.
// Default order: the column `slot`, lowest first.
static inline int32_t sdrkit_lorawan_slots_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_slot_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshSlotRow", "slots", rows, sizeof(sdrkit_mesh_slot_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "lorawan" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_lorawan_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "lorawan" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_lorawan_remove(SDRKIT_HANDLE handle, const sdrkit_mesh_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.MeshRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "lorawan" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_lorawan_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "lorawan" are the sdrkit_lorawan_gen_*() functions
// after the settings of its receiver.

// The setting "dial_freq" (Dial) of the stack "lorawan". A frequency in Hz. The frequency of the stack's primary channel. A change after the start asks the host to tune there.
static inline int32_t sdrkit_lorawan_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_lorawan_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "lw_channels" (LoRaWAN channels) of the stack "lorawan". Entries separated by ";", each the frequency in Hz, the bandwidth in Hz, the spreading factors and the flags, separated by "/": u devices send there, d gateways answer there, j a default channel. For example "100100000/125000/7-12/udj". At most 256 entries. Empty: one channel at the dial.
static inline int32_t sdrkit_lorawan_get_lw_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "lw_channels", value);
}
static inline int32_t sdrkit_lorawan_set_lw_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "lw_channels", value);
}

// The setting "custom" (Custom LoRa setting) of the stack "lorawan".
static inline int32_t sdrkit_lorawan_get_custom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "custom", value);
}
static inline int32_t sdrkit_lorawan_set_custom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "custom", value);
}

// The setting "custom_freq" (Frequency) of the stack "lorawan". A frequency in Hz.
static inline int32_t sdrkit_lorawan_get_custom_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "custom_freq", value);
}
static inline int32_t sdrkit_lorawan_set_custom_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "custom_freq", value);
}

// The setting "custom_bw" (Bandwidth) of the stack "lorawan". A value of `sdrkit_lorawan_custom_bw`.
static inline int32_t sdrkit_lorawan_get_custom_bw(SDRKIT_HANDLE handle, sdrkit_lorawan_custom_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lorawan_custom_bw)index;
  return code;
}
static inline int32_t sdrkit_lorawan_set_custom_bw(SDRKIT_HANDLE handle, sdrkit_lorawan_custom_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_bw", (int32_t)value);
}

// The setting "custom_sf" (Spreading factor) of the stack "lorawan".
static inline int32_t sdrkit_lorawan_get_custom_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sf", value);
}
static inline int32_t sdrkit_lorawan_set_custom_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sf", value);
}

// The setting "custom_cr" (Coding rate) of the stack "lorawan". A value of `sdrkit_lorawan_custom_cr`.
static inline int32_t sdrkit_lorawan_get_custom_cr(SDRKIT_HANDLE handle, sdrkit_lorawan_custom_cr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lorawan_custom_cr)index;
  return code;
}
static inline int32_t sdrkit_lorawan_set_custom_cr(SDRKIT_HANDLE handle, sdrkit_lorawan_custom_cr value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_cr", (int32_t)value);
}

// The setting "custom_sync" (Sync word) of the stack "lorawan". 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.
static inline int32_t sdrkit_lorawan_get_custom_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sync", value);
}
static inline int32_t sdrkit_lorawan_set_custom_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sync", value);
}

// The setting "custom_preamble" (Preamble) of the stack "lorawan".
static inline int32_t sdrkit_lorawan_get_custom_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_preamble", value);
}
static inline int32_t sdrkit_lorawan_set_custom_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_preamble", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "lorawan".
static inline int32_t sdrkit_lorawan_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_lorawan_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "jump_chips" (Expected jump) of the stack "lorawan". Unit: chips.
static inline int32_t sdrkit_lorawan_get_jump_chips(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jump_chips", value);
}
static inline int32_t sdrkit_lorawan_set_jump_chips(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jump_chips", value);
}

// The setting "raw" (Raw payload) of the stack "lorawan".
static inline int32_t sdrkit_lorawan_get_raw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw", value);
}
static inline int32_t sdrkit_lorawan_set_raw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "lorawan". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_lorawan_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_lorawan_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "lorawan". A value of `sdrkit_lorawan_gen_scene`.
static inline int32_t sdrkit_lorawan_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_lorawan_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lorawan_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_lorawan_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_lorawan_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_loraaprs_packets_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_packet_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshPacketRow", "packets", rows, sizeof(sdrkit_mesh_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `last_heard`.
static inline int32_t sdrkit_loraaprs_nodes_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mesh_node_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshNodeRow", "nodes", rows, sizeof(sdrkit_mesh_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_loraaprs_messages_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_message_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "readings" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `node` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`.
// Time: `time_s`.
static inline int32_t sdrkit_loraaprs_readings_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_reading_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshReadingRow", "readings", rows, sizeof(sdrkit_mesh_reading_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_loraaprs_stations_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_station_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshStationRow", "stations", rows, sizeof(sdrkit_mesh_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_loraaprs_devices_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_device_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_loraaprs_types_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mesh_type_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `preset` names a row.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_loraaprs_decoders_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_decoder_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDecoderRow", "decoders", rows, sizeof(sdrkit_mesh_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "contacts" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_loraaprs_contacts_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_contact_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshContactRow", "contacts", rows, sizeof(sdrkit_mesh_contact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
static inline int32_t sdrkit_loraaprs_channels_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_channel_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshChannelRow", "channels", rows, sizeof(sdrkit_mesh_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slots" of the stack "loraaprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `slot` names a row.
// Default order: the column `slot`, lowest first.
static inline int32_t sdrkit_loraaprs_slots_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mesh_slot_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshSlotRow", "slots", rows, sizeof(sdrkit_mesh_slot_row), capacity, info);
}

// Runs the command "aprs.send" (Send an APRS message) of the stack "loraaprs" on the session of `handle`. Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_loraaprs_aprs_send(SDRKIT_HANDLE handle, const sdrkit_aprs_send_args *args, sdrkit_aprs_send_reply *reply) {
  return sdrkit_typed_command(handle, "aprs.send", "lora.AprsSendArgs", args, "lora.AprsSendReply", reply, sizeof(sdrkit_aprs_send_reply));
}

// Runs the command "aprs.beacon" (Send an APRS beacon) of the stack "loraaprs" on the session of `handle`. The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_loraaprs_aprs_beacon(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "aprs.beacon", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "loraaprs" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_loraaprs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "loraaprs" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_loraaprs_remove(SDRKIT_HANDLE handle, const sdrkit_mesh_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.MeshRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "loraaprs" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_loraaprs_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "loraaprs" are the sdrkit_loraaprs_gen_*() functions
// after the settings of its receiver.

// The setting "dial_freq" (Dial) of the stack "loraaprs". A frequency in Hz. The frequency of the stack's primary channel. A change after the start asks the host to tune there.
static inline int32_t sdrkit_loraaprs_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_loraaprs_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "channels" (Channels) of the stack "loraaprs". MeshCore, LoRa APRS and MeshCom networks: entries separated by ";", each a label, "=", the protocol (meshcore, loraaprs or meshcom) and, each after "/", the frequency in Hz, the bandwidth in Hz, the spreading factor, the coding rate and, when they differ from the protocol's, the preamble and the sync word. For example "Net=meshcore/100000000/62500/8/8". At most 256 entries. A protocol without a channel listens at the dial.
static inline int32_t sdrkit_loraaprs_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_loraaprs_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "custom" (Custom LoRa setting) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_custom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "custom", value);
}
static inline int32_t sdrkit_loraaprs_set_custom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "custom", value);
}

// The setting "custom_freq" (Frequency) of the stack "loraaprs". A frequency in Hz.
static inline int32_t sdrkit_loraaprs_get_custom_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "custom_freq", value);
}
static inline int32_t sdrkit_loraaprs_set_custom_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "custom_freq", value);
}

// The setting "custom_bw" (Bandwidth) of the stack "loraaprs". A value of `sdrkit_loraaprs_custom_bw`.
static inline int32_t sdrkit_loraaprs_get_custom_bw(SDRKIT_HANDLE handle, sdrkit_loraaprs_custom_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_loraaprs_custom_bw)index;
  return code;
}
static inline int32_t sdrkit_loraaprs_set_custom_bw(SDRKIT_HANDLE handle, sdrkit_loraaprs_custom_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_bw", (int32_t)value);
}

// The setting "custom_sf" (Spreading factor) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_custom_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sf", value);
}
static inline int32_t sdrkit_loraaprs_set_custom_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sf", value);
}

// The setting "custom_cr" (Coding rate) of the stack "loraaprs". A value of `sdrkit_loraaprs_custom_cr`.
static inline int32_t sdrkit_loraaprs_get_custom_cr(SDRKIT_HANDLE handle, sdrkit_loraaprs_custom_cr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_loraaprs_custom_cr)index;
  return code;
}
static inline int32_t sdrkit_loraaprs_set_custom_cr(SDRKIT_HANDLE handle, sdrkit_loraaprs_custom_cr value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_cr", (int32_t)value);
}

// The setting "custom_sync" (Sync word) of the stack "loraaprs". 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.
static inline int32_t sdrkit_loraaprs_get_custom_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sync", value);
}
static inline int32_t sdrkit_loraaprs_set_custom_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sync", value);
}

// The setting "custom_preamble" (Preamble) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_custom_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_preamble", value);
}
static inline int32_t sdrkit_loraaprs_set_custom_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_preamble", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_loraaprs_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "jump_chips" (Expected jump) of the stack "loraaprs". Unit: chips.
static inline int32_t sdrkit_loraaprs_get_jump_chips(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jump_chips", value);
}
static inline int32_t sdrkit_loraaprs_set_jump_chips(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jump_chips", value);
}

// The setting "raw" (Raw payload) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_raw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw", value);
}
static inline int32_t sdrkit_loraaprs_set_raw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "loraaprs". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_loraaprs_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_loraaprs_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "aprs_ssid" (SSID) of the stack "loraaprs". The SSID after the callsign of the station in own frames. On AX.25 the callsign must then carry no SSID of its own.
static inline int32_t sdrkit_loraaprs_get_aprs_ssid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "aprs_ssid", value);
}
static inline int32_t sdrkit_loraaprs_set_aprs_ssid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "aprs_ssid", value);
}

// The setting "aprs_tocall" (Destination) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_aprs_tocall(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_tocall", value);
}
static inline int32_t sdrkit_loraaprs_set_aprs_tocall(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_tocall", value);
}

// The setting "aprs_path" (Path) of the stack "loraaprs". Calls separated by commas, at most eight. Empty sends no path.
static inline int32_t sdrkit_loraaprs_get_aprs_path(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_path", value);
}
static inline int32_t sdrkit_loraaprs_set_aprs_path(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_path", value);
}

// The setting "aprs_symbol" (Symbol) of the stack "loraaprs". Two characters: the symbol table, then the symbol code.
static inline int32_t sdrkit_loraaprs_get_aprs_symbol(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_symbol", value);
}
static inline int32_t sdrkit_loraaprs_set_aprs_symbol(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_symbol", value);
}

// The setting "aprs_comment" (Comment) of the stack "loraaprs". The comment of the beacon. Without a position of the station the beacon sends this text as its status.
static inline int32_t sdrkit_loraaprs_get_aprs_comment(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_comment", value);
}
static inline int32_t sdrkit_loraaprs_set_aprs_comment(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_comment", value);
}

// The setting "duty_limit" (Duty-cycle limit) of the stack "loraaprs".
static inline int32_t sdrkit_loraaprs_get_duty_limit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "duty_limit", value);
}
static inline int32_t sdrkit_loraaprs_set_duty_limit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "duty_limit", value);
}

// The setting "duty_bands" (Duty-cycle bands) of the stack "loraaprs". Entries separated by ";", each the lower and the upper edge in Hz separated by "-", "=" and the limit in percent of the air time of the last hour, for example "100000000-100250000=10". Empty: no band has a limit.
static inline int32_t sdrkit_loraaprs_get_duty_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "duty_bands", value);
}
static inline int32_t sdrkit_loraaprs_set_duty_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "duty_bands", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "loraaprs". A value of `sdrkit_loraaprs_gen_scene`.
static inline int32_t sdrkit_loraaprs_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_loraaprs_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_loraaprs_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_loraaprs_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_loraaprs_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_meshcom_packets_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_mesh_packet_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshPacketRow", "packets", rows, sizeof(sdrkit_mesh_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "nodes" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `last_heard`.
static inline int32_t sdrkit_meshcom_nodes_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_node_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshNodeRow", "nodes", rows, sizeof(sdrkit_mesh_node_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_meshcom_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "readings" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `node` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`.
// Time: `time_s`.
static inline int32_t sdrkit_meshcom_readings_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_reading_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshReadingRow", "readings", rows, sizeof(sdrkit_mesh_reading_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_meshcom_stations_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_station_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshStationRow", "stations", rows, sizeof(sdrkit_mesh_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_meshcom_devices_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_mesh_device_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_meshcom_types_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_type_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `preset` names a row.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_meshcom_decoders_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_decoder_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDecoderRow", "decoders", rows, sizeof(sdrkit_mesh_decoder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "contacts" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_meshcom_contacts_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_contact_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshContactRow", "contacts", rows, sizeof(sdrkit_mesh_contact_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
static inline int32_t sdrkit_meshcom_channels_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mesh_channel_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshChannelRow", "channels", rows, sizeof(sdrkit_mesh_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slots" of the stack "meshcom" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `slot` names a row.
// Default order: the column `slot`, lowest first.
static inline int32_t sdrkit_meshcom_slots_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_slot_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshSlotRow", "slots", rows, sizeof(sdrkit_mesh_slot_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "meshcom" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_meshcom_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "meshcom" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_meshcom_remove(SDRKIT_HANDLE handle, const sdrkit_mesh_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.MeshRemoveArgs", args, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "meshcom" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_meshcom_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "meshcom" are the sdrkit_meshcom_gen_*() functions
// after the settings of its receiver.

// The setting "dial_freq" (Dial) of the stack "meshcom". A frequency in Hz. The frequency of the stack's primary channel. A change after the start asks the host to tune there.
static inline int32_t sdrkit_meshcom_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_meshcom_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "channels" (Channels) of the stack "meshcom". MeshCore, LoRa APRS and MeshCom networks: entries separated by ";", each a label, "=", the protocol (meshcore, loraaprs or meshcom) and, each after "/", the frequency in Hz, the bandwidth in Hz, the spreading factor, the coding rate and, when they differ from the protocol's, the preamble and the sync word. For example "Net=meshcore/100000000/62500/8/8". At most 256 entries. A protocol without a channel listens at the dial.
static inline int32_t sdrkit_meshcom_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_meshcom_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "custom" (Custom LoRa setting) of the stack "meshcom".
static inline int32_t sdrkit_meshcom_get_custom(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "custom", value);
}
static inline int32_t sdrkit_meshcom_set_custom(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "custom", value);
}

// The setting "custom_freq" (Frequency) of the stack "meshcom". A frequency in Hz.
static inline int32_t sdrkit_meshcom_get_custom_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "custom_freq", value);
}
static inline int32_t sdrkit_meshcom_set_custom_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "custom_freq", value);
}

// The setting "custom_bw" (Bandwidth) of the stack "meshcom". A value of `sdrkit_meshcom_custom_bw`.
static inline int32_t sdrkit_meshcom_get_custom_bw(SDRKIT_HANDLE handle, sdrkit_meshcom_custom_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshcom_custom_bw)index;
  return code;
}
static inline int32_t sdrkit_meshcom_set_custom_bw(SDRKIT_HANDLE handle, sdrkit_meshcom_custom_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_bw", (int32_t)value);
}

// The setting "custom_sf" (Spreading factor) of the stack "meshcom".
static inline int32_t sdrkit_meshcom_get_custom_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sf", value);
}
static inline int32_t sdrkit_meshcom_set_custom_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sf", value);
}

// The setting "custom_cr" (Coding rate) of the stack "meshcom". A value of `sdrkit_meshcom_custom_cr`.
static inline int32_t sdrkit_meshcom_get_custom_cr(SDRKIT_HANDLE handle, sdrkit_meshcom_custom_cr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshcom_custom_cr)index;
  return code;
}
static inline int32_t sdrkit_meshcom_set_custom_cr(SDRKIT_HANDLE handle, sdrkit_meshcom_custom_cr value) {
  return sdrkit_typed_setting_set_enum(handle, "custom_cr", (int32_t)value);
}

// The setting "custom_sync" (Sync word) of the stack "meshcom". 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.
static inline int32_t sdrkit_meshcom_get_custom_sync(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_sync", value);
}
static inline int32_t sdrkit_meshcom_set_custom_sync(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_sync", value);
}

// The setting "custom_preamble" (Preamble) of the stack "meshcom".
static inline int32_t sdrkit_meshcom_get_custom_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "custom_preamble", value);
}
static inline int32_t sdrkit_meshcom_set_custom_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "custom_preamble", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "meshcom".
static inline int32_t sdrkit_meshcom_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_meshcom_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "jump_chips" (Expected jump) of the stack "meshcom". Unit: chips.
static inline int32_t sdrkit_meshcom_get_jump_chips(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jump_chips", value);
}
static inline int32_t sdrkit_meshcom_set_jump_chips(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jump_chips", value);
}

// The setting "raw" (Raw payload) of the stack "meshcom".
static inline int32_t sdrkit_meshcom_get_raw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "raw", value);
}
static inline int32_t sdrkit_meshcom_set_raw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "raw", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "meshcom". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_meshcom_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_meshcom_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "meshcom". A value of `sdrkit_meshcom_gen_scene`.
static inline int32_t sdrkit_meshcom_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_meshcom_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_meshcom_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_meshcom_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_meshcom_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "lorawanwide" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `device` names a row.
// Time: `time_s`.
static inline int32_t sdrkit_lorawanwide_frames_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_lorawanwide_wide_frame_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.WideFrameRow", "frames", rows, sizeof(sdrkit_lorawanwide_wide_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "lorawanwide" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_lorawanwide_channels_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_lorawanwide_wide_channel_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.WideChannelRow", "channels", rows, sizeof(sdrkit_lorawanwide_wide_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "lorawanwide" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_lorawanwide_devices_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mesh_device_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshDeviceRow", "devices", rows, sizeof(sdrkit_mesh_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "lorawanwide" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_lorawanwide_types_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_mesh_type_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshTypeRow", "types", rows, sizeof(sdrkit_mesh_type_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "lorawanwide" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_lorawanwide_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "lorawanwide" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_lorawanwide_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The setting "dial_freq" (Dial) of the stack "lorawanwide". A frequency in Hz. The middle of the channels: the centre of the capture when the input does not state it, and the dial of the test signal.
static inline int32_t sdrkit_lorawanwide_get_dial_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dial_freq", value);
}
static inline int32_t sdrkit_lorawanwide_set_dial_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dial_freq", value);
}

// The setting "lw_channels" (LoRaWAN channels) of the stack "lorawanwide". Entries separated by ";", each the frequency in Hz, the bandwidth in Hz, the spreading factors and the flags, separated by "/": u devices send there, d gateways answer there, j a default channel. For example "100100000/125000/7-12/udj". At most 256 entries. Empty: one channel at the dial.
static inline int32_t sdrkit_lorawanwide_get_lw_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "lw_channels", value);
}
static inline int32_t sdrkit_lorawanwide_set_lw_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "lw_channels", value);
}

// The setting "downlinks" (Downlinks) of the stack "lorawanwide".
static inline int32_t sdrkit_lorawanwide_get_downlinks(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "downlinks", value);
}
static inline int32_t sdrkit_lorawanwide_set_downlinks(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "downlinks", value);
}

// The setting "defaults_only" (Default channels only) of the stack "lorawanwide".
static inline int32_t sdrkit_lorawanwide_get_defaults_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "defaults_only", value);
}
static inline int32_t sdrkit_lorawanwide_set_defaults_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "defaults_only", value);
}

// The setting "private_sync" (Private networks (0x12)) of the stack "lorawanwide".
static inline int32_t sdrkit_lorawanwide_get_private_sync(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "private_sync", value);
}
static inline int32_t sdrkit_lorawanwide_set_private_sync(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "private_sync", value);
}

// The setting "max_channels" (Channels at most) of the stack "lorawanwide".
static inline int32_t sdrkit_lorawanwide_get_max_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_channels", value);
}
static inline int32_t sdrkit_lorawanwide_set_max_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_channels", value);
}

// The setting "wide_channels" (250 and 500 kHz channels) of the stack "lorawanwide".
static inline int32_t sdrkit_lorawanwide_get_wide_channels(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide_channels", value);
}
static inline int32_t sdrkit_lorawanwide_set_wide_channels(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide_channels", value);
}

// The setting "fsk" (FSK channel) of the stack "lorawanwide".
static inline int32_t sdrkit_lorawanwide_get_fsk(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "fsk", value);
}
static inline int32_t sdrkit_lorawanwide_set_fsk(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "fsk", value);
}

// The setting "fsk_freq" (FSK frequency) of the stack "lorawanwide". A frequency in Hz.
static inline int32_t sdrkit_lorawanwide_get_fsk_freq(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fsk_freq", value);
}
static inline int32_t sdrkit_lorawanwide_set_fsk_freq(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fsk_freq", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "lorawanwide". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_lorawanwide_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_lorawanwide_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "loraraw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_loraraw_frames_rows(SDRKIT_HANDLE handle,
                                                sdrkit_loraraw_raw_frame_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.RawFrameRow", "frames", rows, sizeof(sdrkit_loraraw_raw_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "decoders" of the stack "loraraw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_loraraw_decoders_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_loraraw_raw_decoder_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.RawDecoderRow", "decoders", rows, sizeof(sdrkit_loraraw_raw_decoder_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "loraraw" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_loraraw_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "loraraw" on the session of `handle`. Forgets the signal and makes the bank again.
static inline int32_t sdrkit_loraraw_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send a frame) of the stack "loraraw" on the session of `handle`. Sends the bytes with the LoRa setting of the stack. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_loraraw_send(SDRKIT_HANDLE handle, const sdrkit_loraraw_raw_send_args *args) {
  return sdrkit_typed_command(handle, "send", "lora.RawSendArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "loraraw" are the sdrkit_loraraw_gen_*() functions
// after the settings of its receiver.

// The setting "auto_sf" (Search SF7 to SF12) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_auto_sf(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_sf", value);
}
static inline int32_t sdrkit_loraraw_set_auto_sf(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_sf", value);
}

// The setting "sf" (Spreading factor) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sf", value);
}
static inline int32_t sdrkit_loraraw_set_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sf", value);
}

// The setting "auto_bw" (Search 125, 250 and 500 kHz) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_auto_bw(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_bw", value);
}
static inline int32_t sdrkit_loraraw_set_auto_bw(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_bw", value);
}

// The setting "bw" (Bandwidth) of the stack "loraraw". A value of `sdrkit_loraraw_bw`.
static inline int32_t sdrkit_loraraw_get_bw(SDRKIT_HANDLE handle, sdrkit_loraraw_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_loraraw_bw)index;
  return code;
}
static inline int32_t sdrkit_loraraw_set_bw(SDRKIT_HANDLE handle, sdrkit_loraraw_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "bw", (int32_t)value);
}

// The setting "implicit" (Implicit header) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_implicit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "implicit", value);
}
static inline int32_t sdrkit_loraraw_set_implicit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "implicit", value);
}

// The setting "cr" (Coding rate) of the stack "loraraw". 5 is 4/5, 8 is 4/8.
static inline int32_t sdrkit_loraraw_get_cr(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "cr", value);
}
static inline int32_t sdrkit_loraraw_set_cr(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "cr", value);
}

// The setting "crc" (CRC) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_crc(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "crc", value);
}
static inline int32_t sdrkit_loraraw_set_crc(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "crc", value);
}

// The setting "length" (Length) of the stack "loraraw". Unit: B.
static inline int32_t sdrkit_loraraw_get_length(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "length", value);
}
static inline int32_t sdrkit_loraraw_set_length(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "length", value);
}

// The setting "sync" (Sync word) of the stack "loraraw". A value of `sdrkit_loraraw_sync`.
static inline int32_t sdrkit_loraraw_get_sync(SDRKIT_HANDLE handle, sdrkit_loraraw_sync *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sync", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_loraraw_sync)index;
  return code;
}
static inline int32_t sdrkit_loraraw_set_sync(SDRKIT_HANDLE handle, sdrkit_loraraw_sync value) {
  return sdrkit_typed_setting_set_enum(handle, "sync", (int32_t)value);
}

// The setting "sync_custom" (Custom sync word) of the stack "loraraw". Used when the sync word is custom.
static inline int32_t sdrkit_loraraw_get_sync_custom(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sync_custom", value);
}
static inline int32_t sdrkit_loraraw_set_sync_custom(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sync_custom", value);
}

// The setting "preamble" (Preamble) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "preamble", value);
}
static inline int32_t sdrkit_loraraw_set_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "preamble", value);
}

// The setting "ldro" (LDRO) of the stack "loraraw". A value of `sdrkit_loraraw_ldro`.
static inline int32_t sdrkit_loraraw_get_ldro(SDRKIT_HANDLE handle, sdrkit_loraraw_ldro *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "ldro", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_loraraw_ldro)index;
  return code;
}
static inline int32_t sdrkit_loraraw_set_ldro(SDRKIT_HANDLE handle, sdrkit_loraraw_ldro value) {
  return sdrkit_typed_setting_set_enum(handle, "ldro", (int32_t)value);
}

// The setting "offset" (Offset from the dial) of the stack "loraraw". A frequency in Hz.
static inline int32_t sdrkit_loraraw_get_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "offset", value);
}
static inline int32_t sdrkit_loraraw_set_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "offset", value);
}

// The setting "jump_search" (Lost-sample search) of the stack "loraraw".
static inline int32_t sdrkit_loraraw_get_jump_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "jump_search", value);
}
static inline int32_t sdrkit_loraraw_set_jump_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "jump_search", value);
}

// The setting "tx_repeat_s" (Repeat every) of the stack "loraraw". A length of time in seconds. 0 sends each frame once.
static inline int32_t sdrkit_loraraw_get_tx_repeat_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_repeat_s", value);
}
static inline int32_t sdrkit_loraraw_set_tx_repeat_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_repeat_s", value);
}

// The setting "clock0" (Computer clock at signal 0) of the stack "loraraw". Unit: s. Seconds since 1970 at the first sample. 0 when the times follow the signal.
static inline int32_t sdrkit_loraraw_get_clock0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock0", value);
}
static inline int32_t sdrkit_loraraw_set_clock0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock0", value);
}

// The setting "gen_implicit" (Implicit scene) of the test signal generator of the stack "loraraw".
static inline int32_t sdrkit_loraraw_gen_get_implicit(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_implicit", value);
}
static inline int32_t sdrkit_loraraw_gen_set_implicit(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_implicit", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "aprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Sender: `source`. Addressee: `target`. Text: `text`. Time: `time_s`.
static inline int32_t sdrkit_aprs_frames_rows(SDRKIT_HANDLE handle,
                                             sdrkit_aprs_frame_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.AprsFrameRow", "frames", rows, sizeof(sdrkit_aprs_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "aprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `call` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
// Time: `last_heard`.
static inline int32_t sdrkit_aprs_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_aprs_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.AprsStationRow", "stations", rows, sizeof(sdrkit_aprs_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "aprs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `sender` names a row.
// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
// Delivery state: `state`. Failure reason: `failure`.
static inline int32_t sdrkit_aprs_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mesh_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.MeshMessageRow", "messages", rows, sizeof(sdrkit_mesh_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "aprs.packet"
// (APRS packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_aprs_packet_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_aprs_packet_event *event) {
  return sdrkit_typed_event(handle, n, "aprs.packet", "lora.AprsPacketEvent", event, sizeof(sdrkit_aprs_packet_event));
}

// Runs the command "aprs.send" (Send an APRS message) of the stack "aprs" on the session of `handle`. Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_aprs_send(SDRKIT_HANDLE handle, const sdrkit_aprs_send_args *args, sdrkit_aprs_send_reply *reply) {
  return sdrkit_typed_command(handle, "aprs.send", "lora.AprsSendArgs", args, "lora.AprsSendReply", reply, sizeof(sdrkit_aprs_send_reply));
}

// Runs the command "aprs.beacon" (Send an APRS beacon) of the stack "aprs" on the session of `handle`. The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
static inline int32_t sdrkit_aprs_beacon(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "aprs.beacon", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "aprs" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_aprs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "aprs" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_aprs_remove(SDRKIT_HANDLE handle, const sdrkit_aprs_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "lora.AprsRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "aprs" are the sdrkit_aprs_gen_*() functions
// after the settings of its receiver.

// The setting "narrow" (12.5 kHz channel) of the stack "aprs".
static inline int32_t sdrkit_aprs_get_narrow(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "narrow", value);
}
static inline int32_t sdrkit_aprs_set_narrow(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "narrow", value);
}

// The setting "emphasis" (Emphasis) of the stack "aprs". A value of `sdrkit_aprs_emphasis`.
static inline int32_t sdrkit_aprs_get_emphasis(SDRKIT_HANDLE handle, sdrkit_aprs_emphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "emphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_aprs_emphasis)index;
  return code;
}
static inline int32_t sdrkit_aprs_set_emphasis(SDRKIT_HANDLE handle, sdrkit_aprs_emphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "emphasis", (int32_t)value);
}

// The setting "audio_input" (Audio input) of the stack "aprs". The input is FM-demodulated audio: no minimum rate.
static inline int32_t sdrkit_aprs_get_audio_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_input", value);
}
static inline int32_t sdrkit_aprs_set_audio_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_input", value);
}

// The setting "listen" (Listen) of the stack "aprs".
static inline int32_t sdrkit_aprs_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_aprs_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "aprs_ssid" (SSID) of the stack "aprs". The SSID after the callsign of the station in own frames. On AX.25 the callsign must then carry no SSID of its own.
static inline int32_t sdrkit_aprs_get_aprs_ssid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "aprs_ssid", value);
}
static inline int32_t sdrkit_aprs_set_aprs_ssid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "aprs_ssid", value);
}

// The setting "aprs_tocall" (Destination) of the stack "aprs".
static inline int32_t sdrkit_aprs_get_aprs_tocall(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_tocall", value);
}
static inline int32_t sdrkit_aprs_set_aprs_tocall(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_tocall", value);
}

// The setting "aprs_path" (Path) of the stack "aprs". Calls separated by commas, at most eight. Empty sends no path.
static inline int32_t sdrkit_aprs_get_aprs_path(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_path", value);
}
static inline int32_t sdrkit_aprs_set_aprs_path(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_path", value);
}

// The setting "aprs_symbol" (Symbol) of the stack "aprs". Two characters: the symbol table, then the symbol code.
static inline int32_t sdrkit_aprs_get_aprs_symbol(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_symbol", value);
}
static inline int32_t sdrkit_aprs_set_aprs_symbol(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_symbol", value);
}

// The setting "aprs_comment" (Comment) of the stack "aprs". The comment of the beacon. Without a position of the station the beacon sends this text as its status.
static inline int32_t sdrkit_aprs_get_aprs_comment(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "aprs_comment", value);
}
static inline int32_t sdrkit_aprs_set_aprs_comment(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "aprs_comment", value);
}

// The setting "tx_baud" (Speed) of the stack "aprs". A value of `sdrkit_aprs_tx_baud`.
static inline int32_t sdrkit_aprs_get_tx_baud(SDRKIT_HANDLE handle, sdrkit_aprs_tx_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_aprs_tx_baud)index;
  return code;
}
static inline int32_t sdrkit_aprs_set_tx_baud(SDRKIT_HANDLE handle, sdrkit_aprs_tx_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_baud", (int32_t)value);
}

// The setting "tx_delay" (TX delay) of the stack "aprs". Unit: s. Flags before each frame, so that the receivers open their squelch and find the bit clock.
static inline int32_t sdrkit_aprs_get_tx_delay(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_delay", value);
}
static inline int32_t sdrkit_aprs_set_tx_delay(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_delay", value);
}

// The setting "tx_pre_emphasis" (Pre-emphasis) of the stack "aprs". Lifts the 2200 Hz tone over the 1200 Hz tone (6 dB an octave above 300 Hz), as the microphone path of an FM radio does, for receivers with de-emphasis. Off sends both tones at the same level.
static inline int32_t sdrkit_aprs_get_tx_pre_emphasis(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_pre_emphasis", value);
}
static inline int32_t sdrkit_aprs_set_tx_pre_emphasis(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_pre_emphasis", value);
}

// The setting "beacon_interval" (Beacon interval) of the stack "aprs". Unit: s. At least 60 s.
static inline int32_t sdrkit_aprs_get_beacon_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "beacon_interval", value);
}
static inline int32_t sdrkit_aprs_set_beacon_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "beacon_interval", value);
}

// The setting "match_ssid" (Match the SSID) of the stack "aprs". Off: a message to the callsign with any SSID is for the station.
static inline int32_t sdrkit_aprs_get_match_ssid(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "match_ssid", value);
}
static inline int32_t sdrkit_aprs_set_match_ssid(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "match_ssid", value);
}

// The setting "wide_input" (Wide input) of the stack "aprs". One carrier at 192 ksps from a channel bank: no minimum rate.
static inline int32_t sdrkit_aprs_get_wide_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wide_input", value);
}
static inline int32_t sdrkit_aprs_set_wide_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wide_input", value);
}

// The setting "gen_option" (Test speeds) of the test signal generator of the stack "aprs". A value of `sdrkit_aprs_gen_option`.
static inline int32_t sdrkit_aprs_gen_get_option(SDRKIT_HANDLE handle, sdrkit_aprs_gen_option *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_option", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_aprs_gen_option)index;
  return code;
}
static inline int32_t sdrkit_aprs_gen_set_option(SDRKIT_HANDLE handle, sdrkit_aprs_gen_option value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_option", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "fanet" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Place of a row: the column `position`.
// Time: `time_s`.
static inline int32_t sdrkit_fanet_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_fanet_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.FanetFrameRow", "frames", rows, sizeof(sdrkit_fanet_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "fanet" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Place of a row: the column `position`.
// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_fanet_stations_rows(SDRKIT_HANDLE handle,
                                                sdrkit_fanet_station_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.FanetStationRow", "stations", rows, sizeof(sdrkit_fanet_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "station_tracks" of the stack "fanet" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `station` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_fanet_station_tracks_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_fanet_track_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.FanetTrackRow", "station_tracks", rows, sizeof(sdrkit_fanet_track_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fanet.station"
// (FANET station). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fanet_station_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fanet_station_event *event) {
  return sdrkit_typed_event(handle, n, "fanet.station", "lora.FanetStationEvent", event, sizeof(sdrkit_fanet_station_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fanet.message"
// (FANET message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fanet_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fanet_message_event *event) {
  return sdrkit_typed_event(handle, n, "fanet.message", "lora.FanetMessageEvent", event, sizeof(sdrkit_fanet_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fanet.distress"
// (FANET distress). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fanet_distress_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fanet_distress_event *event) {
  return sdrkit_typed_event(handle, n, "fanet.distress", "lora.FanetDistressEvent", event, sizeof(sdrkit_fanet_distress_event));
}

// Runs the command "clear" (Clear) of the stack "fanet" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_fanet_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fanet" are the sdrkit_fanet_gen_*() functions
// after the settings of its receiver.

// The setting "bandwidth" (Bandwidth) of the stack "fanet". A value of `sdrkit_fanet_bandwidth`. The bandwidth that the stations of your area use. The default is the one of the test signal.
static inline int32_t sdrkit_fanet_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_fanet_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fanet_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_fanet_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_fanet_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "expiry_s" (Forget stations after) of the stack "fanet". Unit: s.
static inline int32_t sdrkit_fanet_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_fanet_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "gen_centre" (Test signal centre) of the test signal generator of the stack "fanet".
static inline int32_t sdrkit_fanet_gen_get_centre(SDRKIT_HANDLE handle, sdrkit_position *value) {
  return sdrkit_typed_setting_get_position(handle, "gen_centre", value);
}
static inline int32_t sdrkit_fanet_gen_set_centre(SDRKIT_HANDLE handle, sdrkit_position value) {
  return sdrkit_typed_setting_set_position(handle, "gen_centre", value);
}

// The setting "gen_stations" (Test signal stations) of the test signal generator of the stack "fanet".
static inline int32_t sdrkit_fanet_gen_get_stations(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_stations", value);
}
static inline int32_t sdrkit_fanet_gen_set_stations(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_stations", value);
}

// The setting "gen_bad_crc" (Wrong CRC in every n-th frame) of the test signal generator of the stack "fanet".
static inline int32_t sdrkit_fanet_gen_get_bad_crc(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_bad_crc", value);
}
static inline int32_t sdrkit_fanet_gen_set_bad_crc(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_bad_crc", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "reticulum" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `destination` names a row.
// Addressee: `destination`. Text: `content`. Time: `utc`.
static inline int32_t sdrkit_reticulum_packets_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_reticulum_rns_packet_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.RnsPacketRow", "packets", rows, sizeof(sdrkit_reticulum_rns_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "destinations" of the stack "reticulum" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `destination` names a row.
// Time: `first_heard`.
static inline int32_t sdrkit_reticulum_destinations_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_reticulum_rns_destination_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.RnsDestinationRow", "destinations", rows, sizeof(sdrkit_reticulum_rns_destination_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "reticulum.destination"
// (Reticulum destination). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_reticulum_destination_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_reticulum_destination_event *event) {
  return sdrkit_typed_event(handle, n, "reticulum.destination", "lora.RnsDestinationEvent", event, sizeof(sdrkit_reticulum_destination_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "reticulum.announce"
// (Reticulum announce). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_reticulum_announce_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_reticulum_announce_event *event) {
  return sdrkit_typed_event(handle, n, "reticulum.announce", "lora.RnsAnnounceEvent", event, sizeof(sdrkit_reticulum_announce_event));
}

// Runs the command "clear" (Clear) of the stack "reticulum" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_reticulum_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "reticulum" are the sdrkit_reticulum_gen_*() functions
// after the settings of its receiver.

// The setting "auto_plan" (Search bandwidth and spreading factor) of the stack "reticulum".
static inline int32_t sdrkit_reticulum_get_auto_plan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_plan", value);
}
static inline int32_t sdrkit_reticulum_set_auto_plan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_plan", value);
}

// The setting "bw" (Bandwidth) of the stack "reticulum". A value of `sdrkit_reticulum_bw`.
static inline int32_t sdrkit_reticulum_get_bw(SDRKIT_HANDLE handle, sdrkit_reticulum_bw *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bw", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_reticulum_bw)index;
  return code;
}
static inline int32_t sdrkit_reticulum_set_bw(SDRKIT_HANDLE handle, sdrkit_reticulum_bw value) {
  return sdrkit_typed_setting_set_enum(handle, "bw", (int32_t)value);
}

// The setting "sf" (Spreading factor) of the stack "reticulum".
static inline int32_t sdrkit_reticulum_get_sf(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sf", value);
}
static inline int32_t sdrkit_reticulum_set_sf(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sf", value);
}

// The setting "gen_split" (Test signal: a split packet) of the test signal generator of the stack "reticulum".
static inline int32_t sdrkit_reticulum_gen_get_split(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_split", value);
}
static inline int32_t sdrkit_reticulum_gen_set_split(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_split", value);
}

// The setting "gen_tampered" (Test signal: an announce with a wrong signature) of the test signal generator of the stack "reticulum".
static inline int32_t sdrkit_reticulum_gen_get_tampered(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_tampered", value);
}
static inline int32_t sdrkit_reticulum_gen_set_tampered(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_tampered", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "elrs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `link` names a row.
// Time: `utc`.
static inline int32_t sdrkit_elrs_frames_rows(SDRKIT_HANDLE handle,
                                             sdrkit_elrs_frame_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.ElrsFrameRow", "frames", rows, sizeof(sdrkit_elrs_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "links" of the stack "elrs" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `source` names a row.
static inline int32_t sdrkit_elrs_links_rows(SDRKIT_HANDLE handle,
                                            sdrkit_elrs_link_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "lora.ElrsLinkRow", "links", rows, sizeof(sdrkit_elrs_link_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "elrs.link"
// (ExpressLRS link). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_elrs_link_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_elrs_link_event *event) {
  return sdrkit_typed_event(handle, n, "elrs.link", "lora.ElrsLinkEvent", event, sizeof(sdrkit_elrs_link_event));
}

// Runs the command "clear" (Clear) of the stack "elrs" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_elrs_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "elrs" are the sdrkit_elrs_gen_*() functions
// after the settings of its receiver.

// The setting "hop_channels" (Hop channels) of the stack "elrs". The count of channels of the hop plan of your area. The tuned frequency is the middle of the plan. The default is the plan of the test signal.
static inline int32_t sdrkit_elrs_get_hop_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "hop_channels", value);
}
static inline int32_t sdrkit_elrs_set_hop_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "hop_channels", value);
}

// The setting "hop_step_hz" (Hop channel spacing) of the stack "elrs". A frequency in Hz. The distance between two hop channels. The default is the plan of the test signal.
static inline int32_t sdrkit_elrs_get_hop_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hop_step_hz", value);
}
static inline int32_t sdrkit_elrs_set_hop_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hop_step_hz", value);
}

// The setting "packet_rate" (Packet rate) of the stack "elrs". A value of `sdrkit_elrs_packet_rate`. The packet rate of the links. All rates needs five times the processor time. The default is the rate of the test signal.
static inline int32_t sdrkit_elrs_get_packet_rate(SDRKIT_HANDLE handle, sdrkit_elrs_packet_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "packet_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_elrs_packet_rate)index;
  return code;
}
static inline int32_t sdrkit_elrs_set_packet_rate(SDRKIT_HANDLE handle, sdrkit_elrs_packet_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "packet_rate", (int32_t)value);
}

// The setting "binding_phrase" (Binding phrase (your own link)) of the stack "elrs". Secret. With it the stick values of your own link are shown. The stack keeps it in memory only: it is not in the saved state, the log or the results. Empty forgets it.
static inline int32_t sdrkit_elrs_get_binding_phrase(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "binding_phrase", value);
}
static inline int32_t sdrkit_elrs_set_binding_phrase(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "binding_phrase", value);
}

// The setting "gen_phrase" (Test signal binding phrase) of the test signal generator of the stack "elrs".
static inline int32_t sdrkit_elrs_gen_get_phrase(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_phrase", value);
}
static inline int32_t sdrkit_elrs_gen_set_phrase(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_phrase", value);
}

// The setting "gen_row" (Test signal rate row) of the test signal generator of the stack "elrs".
static inline int32_t sdrkit_elrs_gen_get_row(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_row", value);
}
static inline int32_t sdrkit_elrs_gen_set_row(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_row", value);
}

// The setting "gen_inverted_iq" (Test signal with inverted IQ) of the test signal generator of the stack "elrs".
static inline int32_t sdrkit_elrs_gen_get_inverted_iq(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_inverted_iq", value);
}
static inline int32_t sdrkit_elrs_gen_set_inverted_iq(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_inverted_iq", value);
}

// The setting "gen_second_link" (Test signal with a second link without sync frames) of the test signal generator of the stack "elrs".
static inline int32_t sdrkit_elrs_gen_get_second_link(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_second_link", value);
}
static inline int32_t sdrkit_elrs_gen_set_second_link(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_second_link", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mesh_fields_read(SDRKIT_HANDLE handle, sdrkit_mesh_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.MeshFields", fields, sizeof(sdrkit_mesh_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mesh_mt_fields_read(SDRKIT_HANDLE handle, sdrkit_mesh_mt_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.MeshMtFields", fields, sizeof(sdrkit_mesh_mt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mesh_mc_fields_read(SDRKIT_HANDLE handle, sdrkit_mesh_mc_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.MeshMcFields", fields, sizeof(sdrkit_mesh_mc_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_lora_ham_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_ham_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.HamFields", fields, sizeof(sdrkit_lora_ham_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mesh_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_mesh_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.MeshTxFields", fields, sizeof(sdrkit_mesh_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_lora_guard_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_guard_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.GuardFields", fields, sizeof(sdrkit_lora_guard_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_lorawanwide_wide_fields_read(SDRKIT_HANDLE handle, sdrkit_lorawanwide_wide_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.WideFields", fields, sizeof(sdrkit_lorawanwide_wide_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_loraraw_raw_fields_read(SDRKIT_HANDLE handle, sdrkit_loraraw_raw_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.RawFields", fields, sizeof(sdrkit_loraraw_raw_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_aprs_fields_read(SDRKIT_HANDLE handle, sdrkit_aprs_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.AprsFields", fields, sizeof(sdrkit_aprs_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fanet_fields_read(SDRKIT_HANDLE handle, sdrkit_fanet_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.FanetFields", fields, sizeof(sdrkit_fanet_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_reticulum_rns_fields_read(SDRKIT_HANDLE handle, sdrkit_reticulum_rns_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.RnsFields", fields, sizeof(sdrkit_reticulum_rns_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_elrs_fields_read(SDRKIT_HANDLE handle, sdrkit_elrs_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ElrsFields", fields, sizeof(sdrkit_elrs_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "mesh" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_mesh_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "meshtastic" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_meshtastic_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "meshcore" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_meshcore_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "lorawan" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_lorawan_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "loraaprs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_loraaprs_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "meshcom" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_meshcom_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "loraraw" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_loraraw_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

// Fills `fields` with the result fields of the component "modulate" of the stack "aprs" in the snapshot of
// `handle`. A generic consumer reads them under the keys "modulate.<key>".
static inline int32_t sdrkit_aprs_modulate_fields_read(SDRKIT_HANDLE handle, sdrkit_lora_modulate_fields *fields) {
  return sdrkit_typed_fields(handle, "lora.ModulateFields@modulate", fields, sizeof(sdrkit_lora_modulate_fields));
}

#endif
