/*
 * sdrkit: the typed interface of the settings that every engine has, version 1.9. MIT licence.
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

#ifndef SDRKIT_ENGINE_H
#define SDRKIT_ENGINE_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_ENGINE_VERSION_MAJOR 1
#define SDRKIT_ENGINE_VERSION_MINOR 9

// The options of the setting "backend_fft" of the stack "engine".
typedef enum sdrkit_engine_backend_fft {
  // Default (accelerate)
  SDRKIT_ENGINE_BACKEND_FFT_AUTO = 0,
  // accelerate
  SDRKIT_ENGINE_BACKEND_FFT_ACCELERATE = 1,
  // rustfft
  SDRKIT_ENGINE_BACKEND_FFT_RUSTFFT = 2,
} sdrkit_engine_backend_fft;

// The options of the setting "backend_fir" of the stack "engine".
typedef enum sdrkit_engine_backend_fir {
  // Default (neon)
  SDRKIT_ENGINE_BACKEND_FIR_AUTO = 0,
  // neon
  SDRKIT_ENGINE_BACKEND_FIR_NEON = 1,
  // plain
  SDRKIT_ENGINE_BACKEND_FIR_PLAIN = 2,
} sdrkit_engine_backend_fir;

// The options of the setting "backend_resample" of the stack "engine".
typedef enum sdrkit_engine_backend_resample {
  // Default (neon)
  SDRKIT_ENGINE_BACKEND_RESAMPLE_AUTO = 0,
  // neon
  SDRKIT_ENGINE_BACKEND_RESAMPLE_NEON = 1,
  // plain
  SDRKIT_ENGINE_BACKEND_RESAMPLE_PLAIN = 2,
} sdrkit_engine_backend_resample;

// The options of the setting "backend_viterbi" of the stack "engine".
typedef enum sdrkit_engine_backend_viterbi {
  // Default (neon)
  SDRKIT_ENGINE_BACKEND_VITERBI_AUTO = 0,
  // neon
  SDRKIT_ENGINE_BACKEND_VITERBI_NEON = 1,
  // plain
  SDRKIT_ENGINE_BACKEND_VITERBI_PLAIN = 2,
} sdrkit_engine_backend_viterbi;

// The options of the setting "backend_rs" of the stack "engine".
typedef enum sdrkit_engine_backend_rs {
  // Default (neon)
  SDRKIT_ENGINE_BACKEND_RS_AUTO = 0,
  // neon
  SDRKIT_ENGINE_BACKEND_RS_NEON = 1,
  // plain
  SDRKIT_ENGINE_BACKEND_RS_PLAIN = 2,
} sdrkit_engine_backend_rs;

// The options of the setting "backend_ldpc" of the stack "engine".
typedef enum sdrkit_engine_backend_ldpc {
  // Default (neon)
  SDRKIT_ENGINE_BACKEND_LDPC_AUTO = 0,
  // neon
  SDRKIT_ENGINE_BACKEND_LDPC_NEON = 1,
  // plain
  SDRKIT_ENGINE_BACKEND_LDPC_PLAIN = 2,
} sdrkit_engine_backend_ldpc;

// The calibration of the device and the cores of the engine.
typedef struct sdrkit_engine_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `core_units` holds a value.
  bool has_core_units;
  // The calibration units per second that one fast core of this device runs, with the kernels of this build. Absent until the calibration has finished.
  double core_units;
  // True if `core_units_slow` holds a value.
  bool has_core_units_slow;
  // The same of one slow core. Equal to the fast core where the platform has one class of cores.
  double core_units_slow;
  // True if `cores_fast` holds a value.
  bool has_cores_fast;
  // The fast cores of this device.
  int64_t cores_fast;
  // True if `cores_slow` holds a value.
  bool has_cores_slow;
  // The slow cores of this device. 0 where the platform has one class of cores.
  int64_t cores_slow;
  // The cores that the sessions may use: the setting `cores_max`, or the fast cores when it is 0.
  int64_t cores_usable;
} sdrkit_engine_fields;

// The options of `auto_gain` of `sdrkit_engine_stack_row`.
typedef enum sdrkit_engine_stack_row_auto_gain {
  // Radio AGC
  SDRKIT_ENGINE_STACK_ROW_AUTO_GAIN_RADIO = 0,
  // High fixed gain
  SDRKIT_ENGINE_STACK_ROW_AUTO_GAIN_HIGH = 1,
  // Found once and held
  SDRKIT_ENGINE_STACK_ROW_AUTO_GAIN_HOLD = 2,
} sdrkit_engine_stack_row_auto_gain;

// The options of `kind` of `sdrkit_engine_stack_row`.
typedef enum sdrkit_engine_stack_row_kind {
  // Broadcast radio
  SDRKIT_ENGINE_STACK_ROW_KIND_BROADCAST = 0,
  // Television and video
  SDRKIT_ENGINE_STACK_ROW_KIND_TELEVISION = 1,
  // Voice calls
  SDRKIT_ENGINE_STACK_ROW_KIND_VOICE = 2,
  // Keyboard text
  SDRKIT_ENGINE_STACK_ROW_KIND_TEXT_CHAT = 3,
  // Weak-signal messages
  SDRKIT_ENGINE_STACK_ROW_KIND_WEAK_SIGNAL = 4,
  // Mesh messaging
  SDRKIT_ENGINE_STACK_ROW_KIND_MESH = 5,
  // Position tracking
  SDRKIT_ENGINE_STACK_ROW_KIND_TRACKING = 6,
  // Datalink messages
  SDRKIT_ENGINE_STACK_ROW_KIND_DATALINK = 7,
  // Pictures and fax
  SDRKIT_ENGINE_STACK_ROW_KIND_PICTURES = 8,
  // Satellite imagery
  SDRKIT_ENGINE_STACK_ROW_KIND_SATELLITE_IMAGERY = 9,
  // Measurement and analysis
  SDRKIT_ENGINE_STACK_ROW_KIND_MEASUREMENT = 10,
  // Science and propagation
  SDRKIT_ENGINE_STACK_ROW_KIND_SCIENCE = 11,
  // Navigation aids
  SDRKIT_ENGINE_STACK_ROW_KIND_NAVIGATION = 12,
  // Networks and cells
  SDRKIT_ENGINE_STACK_ROW_KIND_NETWORKS = 13,
  // Paging
  SDRKIT_ENGINE_STACK_ROW_KIND_PAGING = 14,
  // Sensors and devices
  SDRKIT_ENGINE_STACK_ROW_KIND_DEVICES = 15,
  // Utility decoders
  SDRKIT_ENGINE_STACK_ROW_KIND_UTILITY = 16,
  // Satellite links
  SDRKIT_ENGINE_STACK_ROW_KIND_SATELLITE_LINKS = 17,
  // General receiver
  SDRKIT_ENGINE_STACK_ROW_KIND_GENERAL = 18,
} sdrkit_engine_stack_row_kind;

// The options of `audio` of `sdrkit_engine_stack_row`.
typedef enum sdrkit_engine_stack_row_audio {
  // Programme
  SDRKIT_ENGINE_STACK_ROW_AUDIO_PROGRAMME = 0,
  // Voice
  SDRKIT_ENGINE_STACK_ROW_AUDIO_VOICE = 1,
  // Monitor
  SDRKIT_ENGINE_STACK_ROW_AUDIO_MONITOR = 2,
} sdrkit_engine_stack_row_audio;

// The options of `tuning_kind` of `sdrkit_engine_stack_row`.
typedef enum sdrkit_engine_stack_row_tuning_kind {
  // Anywhere
  SDRKIT_ENGINE_STACK_ROW_TUNING_KIND_FREE = 0,
  // Listed frequencies
  SDRKIT_ENGINE_STACK_ROW_TUNING_KIND_CHANNELS = 1,
  // One frequency
  SDRKIT_ENGINE_STACK_ROW_TUNING_KIND_FIXED = 2,
} sdrkit_engine_stack_row_tuning_kind;

// The options of `listen_demod` of `sdrkit_engine_stack_row`.
typedef enum sdrkit_engine_stack_row_listen_demod {
  // AM
  SDRKIT_ENGINE_STACK_ROW_LISTEN_DEMOD_AM = 0,
  // NFM
  SDRKIT_ENGINE_STACK_ROW_LISTEN_DEMOD_NFM = 1,
  // WFM
  SDRKIT_ENGINE_STACK_ROW_LISTEN_DEMOD_WFM = 2,
  // USB
  SDRKIT_ENGINE_STACK_ROW_LISTEN_DEMOD_USB = 3,
  // LSB
  SDRKIT_ENGINE_STACK_ROW_LISTEN_DEMOD_LSB = 4,
  // CW
  SDRKIT_ENGINE_STACK_ROW_LISTEN_DEMOD_CW = 5,
} sdrkit_engine_stack_row_listen_demod;

// A row of the table "stacks" (Stacks).
typedef struct sdrkit_engine_stack_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Name.
  sdrkit_string name;
  // Group.
  sdrkit_string group;
  // About.
  sdrkit_string blurb;
  // Lowest frequency. A frequency in Hz.
  double min_hz;
  // Highest frequency. A frequency in Hz.
  double max_hz;
  // Frequency. A frequency in Hz.
  double default_hz;
  // Sample rate. A frequency in Hz.
  double rate_hz;
  // Lowest sample rate. A frequency in Hz.
  double min_rate_hz;
  // Bandwidth. A frequency in Hz.
  double bandwidth_hz;
  // Automatic gain. A value of `sdrkit_engine_stack_row_auto_gain`.
  int32_t auto_gain;
  // Test signal.
  bool generator;
  // Settle time. A length of time in seconds.
  double lock_settle_s;
  // Lock timeout. A length of time in seconds.
  double lock_timeout_s;
  // Signal SNR. Unit: dB.
  double lock_snr_db;
  // Name time. A length of time in seconds.
  double lock_collect_s;
  // Transmit.
  bool transmit;
  // The channels of host audio that the graph of the stack takes for a transmission: 0 for none, 1 for mono, 2 for stereo.
  int64_t host_audio_channels;
  // True if `kind` holds a value.
  bool has_kind;
  // The kind of content of the stack. A front end chooses its view of the stack from it. No value for a stack without an interface description. A value of `sdrkit_engine_stack_row_kind`.
  int32_t kind;
  // True if `audio` holds a value.
  bool has_audio;
  // The kind of sound that the stack gives. No value for a stack without sound. A value of `sdrkit_engine_stack_row_audio`.
  int32_t audio;
  // The stack gives still pictures.
  bool pictures;
  // The stack gives moving pictures.
  bool video;
  // The stack gives running text for people: decoded characters or whole messages.
  bool text;
  // True if `tuning_kind` holds a value.
  bool has_tuning_kind;
  // Where the stack listens: anywhere in its range, only on the frequencies that the data lists for it, or on the one frequency of its system. A host keeps the present frequency on a change to a stack that listens anywhere and asks before it moves to another. No value for a stack that does not declare it. A value of `sdrkit_engine_stack_row_tuning_kind`.
  int32_t tuning_kind;
  // True if `listen_demod` holds a value.
  bool has_listen_demod;
  // The demodulator of the general stack that hears the signal of the stack by ear. No value for a signal that no plain demodulation suits. A value of `sdrkit_engine_stack_row_listen_demod`.
  int32_t listen_demod;
  // True if `listen_width_hz` holds a value.
  bool has_listen_width_hz;
  // The usual filter width of that demodulator. No value without a demodulator. A frequency in Hz.
  double listen_width_hz;
  // True if `listen_pitch_hz` holds a value.
  bool has_listen_pitch_hz;
  // The pitch of the beat tone of CW. No value for another demodulator. A frequency in Hz.
  double listen_pitch_hz;
} sdrkit_engine_stack_row;

// The options of `listen_demod` of `sdrkit_engine_submode_row`.
typedef enum sdrkit_engine_submode_row_listen_demod {
  // AM
  SDRKIT_ENGINE_SUBMODE_ROW_LISTEN_DEMOD_AM = 0,
  // NFM
  SDRKIT_ENGINE_SUBMODE_ROW_LISTEN_DEMOD_NFM = 1,
  // WFM
  SDRKIT_ENGINE_SUBMODE_ROW_LISTEN_DEMOD_WFM = 2,
  // USB
  SDRKIT_ENGINE_SUBMODE_ROW_LISTEN_DEMOD_USB = 3,
  // LSB
  SDRKIT_ENGINE_SUBMODE_ROW_LISTEN_DEMOD_LSB = 4,
  // CW
  SDRKIT_ENGINE_SUBMODE_ROW_LISTEN_DEMOD_CW = 5,
} sdrkit_engine_submode_row_listen_demod;

// A row of the table "submodes" (Variants).
typedef struct sdrkit_engine_submode_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Stack.
  sdrkit_string stack;
  // Name.
  sdrkit_string name;
  // Frequency. A frequency in Hz.
  double default_hz;
  // True if `listen_demod` holds a value.
  bool has_listen_demod;
  // Heard as. A value of `sdrkit_engine_submode_row_listen_demod`.
  int32_t listen_demod;
  // True if `listen_width_hz` holds a value.
  bool has_listen_width_hz;
  // Heard with width. A frequency in Hz.
  double listen_width_hz;
  // True if `listen_pitch_hz` holds a value.
  bool has_listen_pitch_hz;
  // Heard at pitch. A frequency in Hz.
  double listen_pitch_hz;
} sdrkit_engine_submode_row;

// The options of `signal` of `sdrkit_engine_stack_signal_row`.
typedef enum sdrkit_engine_stack_signal_row_signal {
  // Carrier
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_CARRIER = 0,
  // AM voice
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_AM_VOICE = 1,
  // AM broadcast
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_AM_BROADCAST = 2,
  // Narrow FM voice
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NARROW_FM_VOICE = 3,
  // Keyed voice
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_KEYED_VOICE = 4,
  // Wide FM
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WIDE_FM = 5,
  // FM broadcast
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FM_BROADCAST = 6,
  // SSB voice (upper)
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SSB_VOICE_UPPER = 7,
  // SSB voice (lower)
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SSB_VOICE_LOWER = 8,
  // CW (Morse)
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_CW_MORSE = 9,
  // Narrow digital
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NARROW_DIGITAL = 10,
  // Digital
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DIGITAL = 11,
  // Wide digital
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WIDE_DIGITAL = 12,
  // ADS-B / Mode S
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ADS_B_MODE_S = 13,
  // Mode A/C replies
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MODE_A_C_REPLIES = 14,
  // Mode S interrogations
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MODE_S_INTERROGATIONS = 15,
  // UAT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_UAT = 16,
  // DVB-T
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DVB_T = 17,
  // DVB-T2
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DVB_T2 = 18,
  // DAB / DAB+
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DAB = 19,
  // ISDB-T
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ISDB_T = 20,
  // ISDB-TSB
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ISDB_TSB = 21,
  // DTMB
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DTMB = 22,
  // ATSC 1.0
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ATSC_1_0 = 23,
  // DMR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DMR = 24,
  // P25 Phase 1
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_P25_PHASE_1 = 25,
  // P25 Phase 2
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_P25_PHASE_2 = 26,
  // NXDN96
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NXDN96 = 27,
  // NXDN48
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NXDN48 = 28,
  // dPMR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DPMR = 29,
  // Yaesu System Fusion
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_YAESU_SYSTEM_FUSION = 30,
  // M17
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_M17 = 31,
  // D-STAR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_D_STAR = 32,
  // TETRA
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_TETRA = 33,
  // POCSAG
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_POCSAG = 34,
  // AIS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_AIS = 35,
  // LoRa
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LORA = 36,
  // Packet (1200 bit/s AFSK)
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_PACKET_AFSK_1200 = 37,
  // RTTY
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_RTTY = 38,
  // SITOR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SITOR = 39,
  // PSK31
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_PSK31 = 40,
  // MFSK
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MFSK = 41,
  // DominoEX
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DOMINOEX = 42,
  // THOR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_THOR = 43,
  // Olivia
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_OLIVIA = 44,
  // Contestia
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_CONTESTIA = 45,
  // MT63
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MT63 = 46,
  // FSQ
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FSQ = 47,
  // IFKP
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_IFKP = 48,
  // Hellschreiber
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HELLSCHREIBER = 49,
  // SSTV
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SSTV = 50,
  // ARDOP
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ARDOP = 51,
  // ROS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ROS = 52,
  // FT8
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FT8 = 53,
  // FT4
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FT4 = 54,
  // FT2
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FT2 = 55,
  // WSPR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WSPR = 56,
  // JT65
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_JT65 = 57,
  // JT9
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_JT9 = 58,
  // JT4
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_JT4 = 59,
  // Q65
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_Q65 = 60,
  // FST4
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FST4 = 61,
  // FST4W
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FST4W = 62,
  // MSK144
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MSK144 = 63,
  // JS8
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_JS8 = 64,
  // DJI DroneID
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DJI_DRONEID = 65,
  // FLARM
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FLARM = 66,
  // ATSC 3.0
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ATSC_3_0 = 67,
  // Analog TV
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ANALOG_TV = 68,
  // MAC
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MAC = 69,
  // FM video
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FM_VIDEO = 70,
  // ACARS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ACARS = 71,
  // SELCAL
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SELCAL = 72,
  // HFDL
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HFDL = 73,
  // VDL Mode 2
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_VDL_MODE_2 = 74,
  // VDL Mode 4
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_VDL_MODE_4 = 75,
  // GNSS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_GNSS = 76,
  // SBAS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SBAS = 77,
  // LBJ
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LBJ = 78,
  // Balise
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_BALISE = 79,
  // GSM
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_GSM = 80,
  // NB-IoT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NB_IOT = 81,
  // LTE
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LTE = 82,
  // 5G NR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NR = 83,
  // DECT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DECT = 84,
  // ANT / ANT+
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ANT = 85,
  // IEEE 802.15.4
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_IEEE_802_15_4 = 86,
  // NFC
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NFC = 87,
  // nRF24
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NRF24 = 88,
  // Sigfox
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SIGFOX = 89,
  // Wireless M-Bus
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WIRELESS_M_BUS = 90,
  // Wi-SUN
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WI_SUN = 91,
  // Z-Wave
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_Z_WAVE = 92,
  // DVB-S
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DVB_S = 93,
  // DVB-S2
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DVB_S2 = 94,
  // DVB-C
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DVB_C = 95,
  // DVB-C2
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DVB_C2 = 96,
  // ISDB-S
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ISDB_S = 97,
  // TETRAPOL
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_TETRAPOL = 98,
  // FreeDV
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_FREEDV = 99,
  // MPT 1327
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MPT1327 = 100,
  // LTR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LTR = 101,
  // EDACS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_EDACS = 102,
  // Multitone paging
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_MULTITONE_PAGING = 103,
  // Wi-Fi
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WIFI = 104,
  // Wi-Fi HaLow
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WIFI_HALOW = 105,
  // Bluetooth LE
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_BLUETOOTH_LE = 106,
  // NDB
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_NDB = 107,
  // VOR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_VOR = 108,
  // ILS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ILS = 109,
  // DME
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DME = 110,
  // Loran-C
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LORAN_C = 111,
  // DGPS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DGPS = 112,
  // LRIT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LRIT = 113,
  // HRIT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HRIT = 114,
  // HRPT / AHRPT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HRPT = 115,
  // LRPT
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_LRPT = 116,
  // S-VISSR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_S_VISSR = 117,
  // DSC
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DSC = 118,
  // Weather fax
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_WEATHER_FAX = 119,
  // VDES
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_VDES = 120,
  // ATIS
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ATIS = 121,
  // DRM
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DRM = 122,
  // HamDRM
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HAMDRM = 123,
  // HD Radio
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HD_RADIO = 124,
  // CDR
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_CDR = 125,
  // PACTOR-I
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_PACTOR_1 = 126,
  // PACTOR-II
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_PACTOR_2 = 127,
  // PACTOR-III
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_PACTOR_3 = 128,
  // PACTOR-IV
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_PACTOR_4 = 129,
  // HF ocean radar
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_HF_OCEAN_RADAR = 130,
  // ALE (2G)
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ALE_2G = 131,
  // Radiosonde
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_RADIOSONDE = 132,
  // SAME
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_SAME = 133,
  // Time signal
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_TIME_SIGNAL = 134,
  // Data collection platform
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_DCP = 135,
  // Argos
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ARGOS = 136,
  // COSPAS-SARSAT 406 MHz
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_COSPAS_SARSAT_406 = 137,
  // Inmarsat-C
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_INMARSAT_C = 138,
  // Inmarsat Aero
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_INMARSAT_AERO = 139,
  // Orbcomm
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_ORBCOMM = 140,
  // Iridium
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_IRIDIUM = 141,
  // Globalstar
  SDRKIT_ENGINE_STACK_SIGNAL_ROW_SIGNAL_GLOBALSTAR = 142,
} sdrkit_engine_stack_signal_row_signal;

// A row of the table "stack_signals" (Signal types).
typedef struct sdrkit_engine_stack_signal_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Stack.
  sdrkit_string stack;
  // Signal type. A value of `sdrkit_engine_stack_signal_row_signal`.
  int32_t signal;
} sdrkit_engine_stack_signal_row;

// The options of `unavailable_why` of `sdrkit_engine_driver_row`.
typedef enum sdrkit_engine_driver_row_unavailable_why {
  // No USB access on this platform
  SDRKIT_ENGINE_DRIVER_ROW_UNAVAILABLE_WHY_NO_USB_HERE = 0,
  // No permission to open the device
  SDRKIT_ENGINE_DRIVER_ROW_UNAVAILABLE_WHY_NO_PERMISSION = 1,
  // A driver of the system holds the device
  SDRKIT_ENGINE_DRIVER_ROW_UNAVAILABLE_WHY_KERNEL_DRIVER = 2,
  // The device needs the WinUSB driver
  SDRKIT_ENGINE_DRIVER_ROW_UNAVAILABLE_WHY_NEEDS_WINUSB = 3,
  // The device is not attached
  SDRKIT_ENGINE_DRIVER_ROW_UNAVAILABLE_WHY_NOT_FOUND = 4,
} sdrkit_engine_driver_row_unavailable_why;

// A row of the table "drivers" (Drivers).
typedef struct sdrkit_engine_driver_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Name.
  sdrkit_string name;
  // Address.
  sdrkit_string address;
  // True if `min_rate_hz` holds a value.
  bool has_min_rate_hz;
  // Lowest sample rate. A frequency in Hz.
  double min_rate_hz;
  // True if `max_rate_hz` holds a value.
  bool has_max_rate_hz;
  // Highest sample rate. A frequency in Hz.
  double max_rate_hz;
  // Transmit.
  bool transmit;
  // True if `tx_max_rate_hz` holds a value.
  bool has_tx_max_rate_hz;
  // Highest transmit rate. A frequency in Hz.
  double tx_max_rate_hz;
  // False if no radio of the kind can be opened on this system, such as a USB radio on a platform without USB access.
  bool available;
  // True if `unavailable_why` holds a value.
  bool has_unavailable_why;
  // Why no radio of the kind can be opened. No value for a kind that is available. A value of `sdrkit_engine_driver_row_unavailable_why`.
  int32_t unavailable_why;
} sdrkit_engine_driver_row;

// A row of the table "component_kinds" (Component kinds).
typedef struct sdrkit_engine_component_kind_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Inputs.
  int64_t inputs;
  // Outputs.
  int64_t outputs;
  // Settings.
  int64_t settings;
} sdrkit_engine_component_kind_row;

// A row of the table "services" (Services).
typedef struct sdrkit_engine_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Name.
  sdrkit_string name;
} sdrkit_engine_service_row;

// A row of the table "radios" (Radios).
typedef struct sdrkit_engine_radio_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Driver.
  sdrkit_string driver;
  // Address.
  sdrkit_string address;
  // Transmit.
  bool transmit;
  // True if `tx_min_hz` holds a value.
  bool has_tx_min_hz;
  // Lowest transmit frequency. A frequency in Hz.
  double tx_min_hz;
  // True if `tx_max_hz` holds a value.
  bool has_tx_max_hz;
  // Highest transmit frequency. A frequency in Hz.
  double tx_max_hz;
  // True if `tx_max_rate_hz` holds a value.
  bool has_tx_max_rate_hz;
  // Highest transmit rate. A frequency in Hz.
  double tx_max_rate_hz;
} sdrkit_engine_radio_row;

// The options of `state` of `sdrkit_engine_session_row`.
typedef enum sdrkit_engine_session_row_state {
  // Waiting
  SDRKIT_ENGINE_SESSION_ROW_STATE_WAITING = 0,
  // Running
  SDRKIT_ENGINE_SESSION_ROW_STATE_RUNNING = 1,
  // Failed
  SDRKIT_ENGINE_SESSION_ROW_STATE_FAILED = 2,
  // Stopped
  SDRKIT_ENGINE_SESSION_ROW_STATE_STOPPED = 3,
} sdrkit_engine_session_row_state;

// A row of the table "sessions" (Sessions).
typedef struct sdrkit_engine_session_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `stack` holds a value.
  bool has_stack;
  // Stack.
  sdrkit_string stack;
  // Radio.
  sdrkit_string radio;
  // State. A value of `sdrkit_engine_session_row_state`.
  int32_t state;
} sdrkit_engine_session_row;

// The options of `kind` of `sdrkit_engine_request_row`.
typedef enum sdrkit_engine_request_row_kind {
  // Retune
  SDRKIT_ENGINE_REQUEST_ROW_KIND_RETUNE = 0,
  // Hop
  SDRKIT_ENGINE_REQUEST_ROW_KIND_HOP = 1,
  // Follow
  SDRKIT_ENGINE_REQUEST_ROW_KIND_FOLLOW = 2,
  // Release
  SDRKIT_ENGINE_REQUEST_ROW_KIND_RELEASE = 3,
  // Tune
  SDRKIT_ENGINE_REQUEST_ROW_KIND_TUNE = 4,
  // Store memory
  SDRKIT_ENGINE_REQUEST_ROW_KIND_STORE_MEMORY = 5,
  // Memory hit
  SDRKIT_ENGINE_REQUEST_ROW_KIND_MEMORY_HIT = 6,
  // Record
  SDRKIT_ENGINE_REQUEST_ROW_KIND_RECORD = 7,
  // Save rewind
  SDRKIT_ENGINE_REQUEST_ROW_KIND_REWIND = 8,
  // Run script
  SDRKIT_ENGINE_REQUEST_ROW_KIND_SCRIPT = 9,
  // Send to output
  SDRKIT_ENGINE_REQUEST_ROW_KIND_OUTPUT = 10,
} sdrkit_engine_request_row_kind;

// A row of the table "requests" (Requests).
typedef struct sdrkit_engine_request_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // From.
  sdrkit_string from;
  // Request. A value of `sdrkit_engine_request_row_kind`.
  int32_t kind;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // Frequency. A frequency in Hz.
  double freq_hz;
  // Done.
  bool done;
  // True if `refused` holds a value.
  bool has_refused;
  // Refused because.
  sdrkit_string refused;
  // True if `session` holds a value.
  bool has_session;
  // Session.
  sdrkit_string session;
} sdrkit_engine_request_row;

// A row of the table "benchmarks" (Benchmarks).
typedef struct sdrkit_engine_benchmark_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Stack.
  sdrkit_string stack;
  // Items per second. Unit: 1/s.
  double items_per_s;
  // Processor share.
  double cpu_share;
  // Threads.
  int64_t threads;
  // Fullest thread.
  double thread_cores;
  // Status.
  sdrkit_string status;
} sdrkit_engine_benchmark_row;

// A row of the table "benchmark.components" (Benchmark components).
typedef struct sdrkit_engine_benchmark_component_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Benchmark.
  sdrkit_string benchmark;
  // Component.
  sdrkit_string component;
  // Section.
  sdrkit_string section;
  // Stage.
  sdrkit_string stage;
  // Thread.
  int64_t thread;
  // Cost.
  double cost;
  // Calls.
  int64_t calls;
  // Items in.
  int64_t items_in;
  // Items out.
  int64_t items_out;
  // Mean call. Unit: us.
  double mean_us;
  // Worst call. Unit: us.
  double worst_us;
  // Units.
  double units;
} sdrkit_engine_benchmark_component_row;

// A row of the table "benchmark.stages" (Benchmark stages).
typedef struct sdrkit_engine_benchmark_stage_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Benchmark.
  sdrkit_string benchmark;
  // Stage.
  sdrkit_string stage;
  // Thread.
  int64_t thread;
  // Cost.
  double cost;
  // Units.
  double units;
  // True if `declared_units` holds a value.
  bool has_declared_units;
  // Declared units.
  double declared_units;
} sdrkit_engine_benchmark_stage_row;

// A row of the table "benchmark.connections" (Benchmark connections).
typedef struct sdrkit_engine_benchmark_connection_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Benchmark.
  sdrkit_string benchmark;
  // From.
  sdrkit_string from;
  // Output.
  sdrkit_string output;
  // To.
  sdrkit_string to;
  // Input.
  sdrkit_string input;
  // Format.
  sdrkit_string format;
  // Stage.
  sdrkit_string stage;
  // Items. Unit: 1/s.
  double items_per_s;
  // Bytes. Unit: B/s.
  double bytes_per_s;
  // Held mean. Unit: B.
  double held_mean_bytes;
  // Held most. Unit: B.
  int64_t held_max_bytes;
  // True if `capacity_bytes` holds a value.
  bool has_capacity_bytes;
  // Capacity. Unit: B.
  int64_t capacity_bytes;
  // True if `wait_mean_us` holds a value.
  bool has_wait_mean_us;
  // Wait. Unit: us.
  double wait_mean_us;
  // True if `wait_worst_us` holds a value.
  bool has_wait_worst_us;
  // Wait worst. Unit: us.
  double wait_worst_us;
  // Found full.
  int64_t full;
  // Found empty.
  int64_t empty;
  // Stage boundary.
  bool boundary;
} sdrkit_engine_benchmark_connection_row;

// The options of `content` of `sdrkit_engine_stack_transmit_row`.
typedef enum sdrkit_engine_stack_transmit_row_content {
  // Text
  SDRKIT_ENGINE_STACK_TRANSMIT_ROW_CONTENT_TEXT = 0,
  // Voice
  SDRKIT_ENGINE_STACK_TRANSMIT_ROW_CONTENT_VOICE = 1,
  // Beacon
  SDRKIT_ENGINE_STACK_TRANSMIT_ROW_CONTENT_BEACON = 2,
  // File
  SDRKIT_ENGINE_STACK_TRANSMIT_ROW_CONTENT_FILE = 3,
  // Frame
  SDRKIT_ENGINE_STACK_TRANSMIT_ROW_CONTENT_FRAME = 4,
  // Carrier
  SDRKIT_ENGINE_STACK_TRANSMIT_ROW_CONTENT_CARRIER = 5,
} sdrkit_engine_stack_transmit_row_content;

// A row of the table "stack_transmits" (Transmit commands).
typedef struct sdrkit_engine_stack_transmit_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Stack.
  sdrkit_string stack;
  // Command.
  sdrkit_string command;
  // Content. A value of `sdrkit_engine_stack_transmit_row_content`.
  int32_t content;
  // True if `stopped_by` holds a value.
  bool has_stopped_by;
  // Stopped by.
  sdrkit_string stopped_by;
} sdrkit_engine_stack_transmit_row;

// The options of `state` of `sdrkit_engine_device_row`.
typedef enum sdrkit_engine_device_row_state {
  // Free
  SDRKIT_ENGINE_DEVICE_ROW_STATE_FREE = 0,
  // Open
  SDRKIT_ENGINE_DEVICE_ROW_STATE_OPEN = 1,
  // No permission
  SDRKIT_ENGINE_DEVICE_ROW_STATE_NO_PERMISSION = 2,
  // Needs a driver
  SDRKIT_ENGINE_DEVICE_ROW_STATE_NEEDS_DRIVER = 3,
  // Busy
  SDRKIT_ENGINE_DEVICE_ROW_STATE_BUSY = 4,
} sdrkit_engine_device_row_state;

// A row of the table `devices`. The row key is the bus position, `<bus>-<port path>`, which stays while the device stays attached.
typedef struct sdrkit_engine_device_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The address that opens the device, `usb:<family>:<selector>`. It finds the same device after a re-plug.
  sdrkit_string address;
  // The driver kind that takes the device.
  sdrkit_string driver;
  // The manufacturer and product strings of the device, or the name of its family.
  sdrkit_string name;
  // True if `serial` holds a value.
  bool has_serial;
  // The serial number string.
  sdrkit_string serial;
  // The bus position, `<bus>-<port path>`.
  sdrkit_string path;
  // What can be done with the device. A value of `sdrkit_engine_device_row_state`.
  int32_t state;
  // True if `radio` holds a value.
  bool has_radio;
  // The radio that the device is open as.
  sdrkit_string radio;
} sdrkit_engine_device_row;

// A row of the engine table `shares`. The row key is the key of the share.
typedef struct sdrkit_engine_share_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The radio that the share serves.
  sdrkit_string radio;
  // The session of the share on the radio.
  sdrkit_string session;
} sdrkit_engine_share_row;

// The reply of `state.save`.
typedef struct sdrkit_engine_state_save_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // State.
  sdrkit_bytes state;
} sdrkit_engine_state_save_reply;

// The reply of the engine command `share.open`.
typedef struct sdrkit_engine_share_open_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The key of the share (`share.h1`).
  sdrkit_string share;
} sdrkit_engine_share_open_reply;

// The options of `kind` of `sdrkit_engine_request_event`.
typedef enum sdrkit_engine_request_event_kind {
  // Retune
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_RETUNE = 0,
  // Hop
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_HOP = 1,
  // Follow
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_FOLLOW = 2,
  // Release
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_RELEASE = 3,
  // Tune
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_TUNE = 4,
  // Store memory
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_STORE_MEMORY = 5,
  // Memory hit
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_MEMORY_HIT = 6,
  // Record
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_RECORD = 7,
  // Save rewind
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_REWIND = 8,
  // Run script
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_SCRIPT = 9,
  // Send to output
  SDRKIT_ENGINE_REQUEST_EVENT_KIND_OUTPUT = 10,
} sdrkit_engine_request_event_kind;

// The options of `family` of `sdrkit_engine_request_event`.
typedef enum sdrkit_engine_request_event_family {
  // Aircraft
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ENGINE_REQUEST_EVENT_FAMILY_OTHER = 12,
} sdrkit_engine_request_event_family;

// Request.
typedef struct sdrkit_engine_request_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // From.
  sdrkit_string from;
  // Request. A value of `sdrkit_engine_request_event_kind`.
  int32_t kind;
  // Done.
  bool done;
  // True if `refused` holds a value.
  bool has_refused;
  // Refused because.
  sdrkit_string refused;
  // True if `session` holds a value.
  bool has_session;
  // Session.
  sdrkit_string session;
  // Family. A value of `sdrkit_engine_request_event_family`.
  int32_t family;
} sdrkit_engine_request_event;

// The options of `state` of `sdrkit_engine_device_arrived_event`.
typedef enum sdrkit_engine_device_arrived_event_state {
  // Free
  SDRKIT_ENGINE_DEVICE_ARRIVED_EVENT_STATE_FREE = 0,
  // Open
  SDRKIT_ENGINE_DEVICE_ARRIVED_EVENT_STATE_OPEN = 1,
  // No permission
  SDRKIT_ENGINE_DEVICE_ARRIVED_EVENT_STATE_NO_PERMISSION = 2,
  // Needs a driver
  SDRKIT_ENGINE_DEVICE_ARRIVED_EVENT_STATE_NEEDS_DRIVER = 3,
  // Busy
  SDRKIT_ENGINE_DEVICE_ARRIVED_EVENT_STATE_BUSY = 4,
} sdrkit_engine_device_arrived_event_state;

// The fields of the event `device.arrived`.
typedef struct sdrkit_engine_device_arrived_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address that opens the device.
  sdrkit_string address;
  // The driver kind that takes the device.
  sdrkit_string driver;
  // The name of the device.
  sdrkit_string name;
  // The bus position: the row key of the device in the table `devices`.
  sdrkit_string path;
  // What can be done with the device. A value of `sdrkit_engine_device_arrived_event_state`.
  int32_t state;
} sdrkit_engine_device_arrived_event;

// The fields of the event `device.left`.
typedef struct sdrkit_engine_device_left_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The address that opened the device.
  sdrkit_string address;
  // The driver kind that took the device.
  sdrkit_string driver;
  // The name of the device.
  sdrkit_string name;
  // The bus position: the row key that the device had in the table `devices`.
  sdrkit_string path;
} sdrkit_engine_device_left_event;

// The arguments of `state.restore`.
typedef struct sdrkit_engine_state_restore_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The bytes that `state.save` returned.
  sdrkit_bytes state;
} sdrkit_engine_state_restore_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ENGINE_STATE_RESTORE_ARGS_INIT {sizeof(sdrkit_engine_state_restore_args), {0}}

// The arguments of `benchmark.run`.
typedef struct sdrkit_engine_benchmark_run_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // True if `seconds` holds a value.
  bool has_seconds;
  // Seconds. Unit: s.
  double seconds;
  // True if `only` holds a value.
  bool has_only;
  // Runs only the benchmarks whose name or stack id contains this text. Empty runs all.
  sdrkit_string only;
} sdrkit_engine_benchmark_run_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ENGINE_BENCHMARK_RUN_ARGS_INIT {sizeof(sdrkit_engine_benchmark_run_args), 0, 0, 0, {0}}

// The arguments of the engine command `share.open`.
typedef struct sdrkit_engine_share_open_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The radio.
  sdrkit_string radio;
} sdrkit_engine_share_open_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ENGINE_SHARE_OPEN_ARGS_INIT {sizeof(sdrkit_engine_share_open_args), {0}}

// The arguments of the engine command `share.close`.
typedef struct sdrkit_engine_share_close_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The share.
  sdrkit_string share;
} sdrkit_engine_share_close_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ENGINE_SHARE_CLOSE_ARGS_INIT {sizeof(sdrkit_engine_share_close_args), {0}}

// Fills `rows` with the changed rows of the table "stacks" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_engine_stacks_rows(SDRKIT_HANDLE handle,
                                               sdrkit_engine_stack_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.StackRow", "stacks", rows, sizeof(sdrkit_engine_stack_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "submodes" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_submodes_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_engine_submode_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.SubmodeRow", "submodes", rows, sizeof(sdrkit_engine_submode_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stack_signals" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_stack_signals_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_engine_stack_signal_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.StackSignalRow", "stack_signals", rows, sizeof(sdrkit_engine_stack_signal_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "drivers" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_drivers_rows(SDRKIT_HANDLE handle,
                                                sdrkit_engine_driver_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.DriverRow", "drivers", rows, sizeof(sdrkit_engine_driver_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "component_kinds" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_component_kinds_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_engine_component_kind_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.ComponentKindRow", "component_kinds", rows, sizeof(sdrkit_engine_component_kind_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_services_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_engine_service_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.ServiceRow", "services", rows, sizeof(sdrkit_engine_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "radios" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_radios_rows(SDRKIT_HANDLE handle,
                                               sdrkit_engine_radio_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.RadioRow", "radios", rows, sizeof(sdrkit_engine_radio_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sessions" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_sessions_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_engine_session_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.SessionRow", "sessions", rows, sizeof(sdrkit_engine_session_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "requests" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_requests_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_engine_request_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.RequestRow", "requests", rows, sizeof(sdrkit_engine_request_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "benchmarks" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_benchmarks_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_engine_benchmark_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.BenchmarkRow", "benchmarks", rows, sizeof(sdrkit_engine_benchmark_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "benchmark.components" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_benchmark_components_rows(SDRKIT_HANDLE handle,
                                                             sdrkit_engine_benchmark_component_row *rows,
                                                             uint32_t capacity,
                                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.BenchmarkComponentRow", "benchmark.components", rows, sizeof(sdrkit_engine_benchmark_component_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "benchmark.stages" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_benchmark_stages_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_engine_benchmark_stage_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.BenchmarkStageRow", "benchmark.stages", rows, sizeof(sdrkit_engine_benchmark_stage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "benchmark.connections" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_benchmark_connections_rows(SDRKIT_HANDLE handle,
                                                              sdrkit_engine_benchmark_connection_row *rows,
                                                              uint32_t capacity,
                                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.BenchmarkConnectionRow", "benchmark.connections", rows, sizeof(sdrkit_engine_benchmark_connection_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stack_transmits" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_stack_transmits_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_engine_stack_transmit_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.StackTransmitRow", "stack_transmits", rows, sizeof(sdrkit_engine_stack_transmit_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "devices" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_engine_devices_rows(SDRKIT_HANDLE handle,
                                                sdrkit_engine_device_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.DeviceRow", "devices", rows, sizeof(sdrkit_engine_device_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "shares" of the stack "engine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_engine_shares_rows(SDRKIT_HANDLE handle,
                                               sdrkit_engine_share_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "engine.ShareRow", "shares", rows, sizeof(sdrkit_engine_share_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "engine.request"
// (Request). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_engine_request_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_engine_request_event *event) {
  return sdrkit_typed_event(handle, n, "engine.request", "engine.RequestEvent", event, sizeof(sdrkit_engine_request_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "device.arrived"
// (USB device arrived). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_engine_device_arrived_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_engine_device_arrived_event *event) {
  return sdrkit_typed_event(handle, n, "device.arrived", "engine.DeviceArrivedEvent", event, sizeof(sdrkit_engine_device_arrived_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "device.left"
// (USB device left). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_engine_device_left_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_engine_device_left_event *event) {
  return sdrkit_typed_event(handle, n, "device.left", "engine.DeviceLeftEvent", event, sizeof(sdrkit_engine_device_left_event));
}

// Runs the command "state.save" (Save state) of the stack "engine" on the session of `handle`. Returns the saved state of every radio, session and service that has one.
static inline int32_t sdrkit_engine_state_save(SDRKIT_HANDLE handle, sdrkit_engine_state_save_reply *reply) {
  return sdrkit_typed_command(handle, "state.save", NULL, NULL, "engine.StateSaveReply", reply, sizeof(sdrkit_engine_state_save_reply));
}

// Runs the command "state.restore" (Restore state) of the stack "engine" on the session of `handle`. Restores the saved state of each owner named in the bytes of `state.save`. Every owner must be there.
static inline int32_t sdrkit_engine_state_restore(SDRKIT_HANDLE handle, const sdrkit_engine_state_restore_args *args) {
  return sdrkit_typed_command(handle, "state.restore", "engine.StateRestoreArgs", args, NULL, NULL, 0);
}

// Runs the command "benchmark.run" (Run benchmarks) of the stack "engine" on the session of `handle`. Runs the benchmarks of every stack, or those that `only` selects, on a thread of its own, each for about `seconds`. The table `benchmarks` shows the results.
static inline int32_t sdrkit_engine_benchmark_run(SDRKIT_HANDLE handle, const sdrkit_engine_benchmark_run_args *args) {
  return sdrkit_typed_command(handle, "benchmark.run", "engine.BenchmarkRunArgs", args, NULL, NULL, 0);
}

// Runs the command "share.open" (Share a radio) of the stack "engine" on the session of `handle`. Opens the share of a radio: an owner `share.h<n>` that serves the samples of the radio to network clients. A radio has one share. The share serves nobody until its command `listen` starts a listener.
static inline int32_t sdrkit_engine_share_open(SDRKIT_HANDLE handle, const sdrkit_engine_share_open_args *args, sdrkit_engine_share_open_reply *reply) {
  return sdrkit_typed_command(handle, "share.open", "engine.ShareOpenArgs", args, "engine.ShareOpenReply", reply, sizeof(sdrkit_engine_share_open_reply));
}

// Runs the command "share.close" (Stop sharing) of the stack "engine" on the session of `handle`. Closes a share: its listeners stop and its clients are disconnected.
static inline int32_t sdrkit_engine_share_close(SDRKIT_HANDLE handle, const sdrkit_engine_share_close_args *args) {
  return sdrkit_typed_command(handle, "share.close", "engine.ShareCloseArgs", args, NULL, NULL, 0);
}

// The setting "callsign" (Callsign) of every engine.
static inline int32_t sdrkit_engine_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_engine_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "locator" (Locator) of every engine.
static inline int32_t sdrkit_engine_get_locator(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "locator", value);
}
static inline int32_t sdrkit_engine_set_locator(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "locator", value);
}

// The setting "position" (Position) of every engine.
static inline int32_t sdrkit_engine_get_position(SDRKIT_HANDLE handle, sdrkit_position *value) {
  return sdrkit_typed_setting_get_position(handle, "position", value);
}
static inline int32_t sdrkit_engine_set_position(SDRKIT_HANDLE handle, sdrkit_position value) {
  return sdrkit_typed_setting_set_position(handle, "position", value);
}

// The setting "height_m" (Height) of every engine. Unit: m.
static inline int32_t sdrkit_engine_get_height_m(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "height_m", value);
}
static inline int32_t sdrkit_engine_set_height_m(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "height_m", value);
}

// The setting "time_zone" (Time zone) of every engine. Unit: min.
static inline int32_t sdrkit_engine_get_time_zone(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "time_zone", value);
}
static inline int32_t sdrkit_engine_set_time_zone(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "time_zone", value);
}

// The setting "requests_allowed" (Requests) of every engine. If off, the engine refuses every request of a receiver or a service.
static inline int32_t sdrkit_engine_get_requests_allowed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "requests_allowed", value);
}
static inline int32_t sdrkit_engine_set_requests_allowed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "requests_allowed", value);
}

// The setting "transmit_allowed" (Transmit) of every engine. If on, a stack that transmits sends through the radio of its session, if that radio can transmit. If off, nothing is sent and each own message fails. No stack and no radio has a switch of its own.
static inline int32_t sdrkit_engine_get_transmit_allowed(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "transmit_allowed", value);
}
static inline int32_t sdrkit_engine_set_transmit_allowed(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "transmit_allowed", value);
}

// The setting "backend_fft" (FFT backend) of every engine. A value of `sdrkit_engine_backend_fft`. The implementation that new objects of the family use. Objects that run keep theirs.
static inline int32_t sdrkit_engine_get_backend_fft(SDRKIT_HANDLE handle, sdrkit_engine_backend_fft *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "backend_fft", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_engine_backend_fft)index;
  return code;
}
static inline int32_t sdrkit_engine_set_backend_fft(SDRKIT_HANDLE handle, sdrkit_engine_backend_fft value) {
  return sdrkit_typed_setting_set_enum(handle, "backend_fft", (int32_t)value);
}

// The setting "backend_fir" (FIR backend) of every engine. A value of `sdrkit_engine_backend_fir`. The implementation that new objects of the family use. Objects that run keep theirs.
static inline int32_t sdrkit_engine_get_backend_fir(SDRKIT_HANDLE handle, sdrkit_engine_backend_fir *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "backend_fir", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_engine_backend_fir)index;
  return code;
}
static inline int32_t sdrkit_engine_set_backend_fir(SDRKIT_HANDLE handle, sdrkit_engine_backend_fir value) {
  return sdrkit_typed_setting_set_enum(handle, "backend_fir", (int32_t)value);
}

// The setting "backend_resample" (Resampler backend) of every engine. A value of `sdrkit_engine_backend_resample`. The implementation that new objects of the family use. Objects that run keep theirs.
static inline int32_t sdrkit_engine_get_backend_resample(SDRKIT_HANDLE handle, sdrkit_engine_backend_resample *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "backend_resample", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_engine_backend_resample)index;
  return code;
}
static inline int32_t sdrkit_engine_set_backend_resample(SDRKIT_HANDLE handle, sdrkit_engine_backend_resample value) {
  return sdrkit_typed_setting_set_enum(handle, "backend_resample", (int32_t)value);
}

// The setting "backend_viterbi" (Viterbi backend) of every engine. A value of `sdrkit_engine_backend_viterbi`. The implementation that new objects of the family use. Objects that run keep theirs.
static inline int32_t sdrkit_engine_get_backend_viterbi(SDRKIT_HANDLE handle, sdrkit_engine_backend_viterbi *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "backend_viterbi", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_engine_backend_viterbi)index;
  return code;
}
static inline int32_t sdrkit_engine_set_backend_viterbi(SDRKIT_HANDLE handle, sdrkit_engine_backend_viterbi value) {
  return sdrkit_typed_setting_set_enum(handle, "backend_viterbi", (int32_t)value);
}

// The setting "backend_rs" (Reed-Solomon backend) of every engine. A value of `sdrkit_engine_backend_rs`. The implementation that new objects of the family use. Objects that run keep theirs.
static inline int32_t sdrkit_engine_get_backend_rs(SDRKIT_HANDLE handle, sdrkit_engine_backend_rs *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "backend_rs", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_engine_backend_rs)index;
  return code;
}
static inline int32_t sdrkit_engine_set_backend_rs(SDRKIT_HANDLE handle, sdrkit_engine_backend_rs value) {
  return sdrkit_typed_setting_set_enum(handle, "backend_rs", (int32_t)value);
}

// The setting "backend_ldpc" (LDPC backend) of every engine. A value of `sdrkit_engine_backend_ldpc`. The implementation that new objects of the family use. Objects that run keep theirs.
static inline int32_t sdrkit_engine_get_backend_ldpc(SDRKIT_HANDLE handle, sdrkit_engine_backend_ldpc *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "backend_ldpc", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_engine_backend_ldpc)index;
  return code;
}
static inline int32_t sdrkit_engine_set_backend_ldpc(SDRKIT_HANDLE handle, sdrkit_engine_backend_ldpc value) {
  return sdrkit_typed_setting_set_enum(handle, "backend_ldpc", (int32_t)value);
}

// The setting "cores_max" (Cores) of every engine. The most processor cores that the sessions use. A session whose stack needs more than is free still starts, and its results say so.
static inline int32_t sdrkit_engine_get_cores_max(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "cores_max", value);
}
static inline int32_t sdrkit_engine_set_cores_max(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "cores_max", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_engine_fields_read(SDRKIT_HANDLE handle, sdrkit_engine_fields *fields) {
  return sdrkit_typed_fields(handle, "engine.EngineFields", fields, sizeof(sdrkit_engine_fields));
}

#endif
