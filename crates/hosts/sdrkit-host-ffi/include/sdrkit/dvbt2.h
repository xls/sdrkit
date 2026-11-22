/*
 * sdrkit: the typed interface of the stack family "dvbt2", version 1.4. MIT licence.
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

#ifndef SDRKIT_DVBT2_H
#define SDRKIT_DVBT2_H

#include "../sdrkit.h"
#include "mpegts.h"
#include "ipcast.h"

// The version of the family that this header describes.
#define SDRKIT_DVBT2_VERSION_MAJOR 1
#define SDRKIT_DVBT2_VERSION_MINOR 4

// The stack "dvbt2": its id, and the keys of its event types.
#define SDRKIT_STACK_DVBT2 "dvbt2"

// The options of the setting "bandwidth" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_bandwidth {
  // 1.7 MHz
  SDRKIT_DVBT2_BANDWIDTH_1_7 = 0,
  // 5 MHz
  SDRKIT_DVBT2_BANDWIDTH_5_MHZ = 1,
  // 6 MHz
  SDRKIT_DVBT2_BANDWIDTH_6_MHZ = 2,
  // 7 MHz
  SDRKIT_DVBT2_BANDWIDTH_7_MHZ = 3,
  // 8 MHz
  SDRKIT_DVBT2_BANDWIDTH_8_MHZ = 4,
  // 10 MHz
  SDRKIT_DVBT2_BANDWIDTH_10_MHZ = 5,
} sdrkit_dvbt2_bandwidth;

// The options of the setting "gen_fft" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_fft {
  // 1K
  SDRKIT_DVBT2_GEN_FFT_1K = 0,
  // 2K
  SDRKIT_DVBT2_GEN_FFT_2K = 1,
  // 4K
  SDRKIT_DVBT2_GEN_FFT_4K = 2,
  // 8K
  SDRKIT_DVBT2_GEN_FFT_8K = 3,
  // 16K
  SDRKIT_DVBT2_GEN_FFT_16K = 4,
  // 32K
  SDRKIT_DVBT2_GEN_FFT_32K = 5,
} sdrkit_dvbt2_gen_fft;

// The options of the setting "gen_guard" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_guard {
  // 1/32
  SDRKIT_DVBT2_GEN_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBT2_GEN_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBT2_GEN_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBT2_GEN_GUARD_1_4 = 3,
  // 1/128
  SDRKIT_DVBT2_GEN_GUARD_1_128 = 4,
  // 19/128
  SDRKIT_DVBT2_GEN_GUARD_19_128 = 5,
  // 19/256
  SDRKIT_DVBT2_GEN_GUARD_19_256 = 6,
} sdrkit_dvbt2_gen_guard;

// The options of the setting "gen_pilots" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_pilots {
  // PP1
  SDRKIT_DVBT2_GEN_PILOTS_PP1 = 0,
  // PP2
  SDRKIT_DVBT2_GEN_PILOTS_PP2 = 1,
  // PP3
  SDRKIT_DVBT2_GEN_PILOTS_PP3 = 2,
  // PP4
  SDRKIT_DVBT2_GEN_PILOTS_PP4 = 3,
  // PP5
  SDRKIT_DVBT2_GEN_PILOTS_PP5 = 4,
  // PP6
  SDRKIT_DVBT2_GEN_PILOTS_PP6 = 5,
  // PP7
  SDRKIT_DVBT2_GEN_PILOTS_PP7 = 6,
  // PP8
  SDRKIT_DVBT2_GEN_PILOTS_PP8 = 7,
} sdrkit_dvbt2_gen_pilots;

// The options of the setting "gen_modulation" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_modulation {
  // QPSK
  SDRKIT_DVBT2_GEN_MODULATION_QPSK = 0,
  // 16-QAM
  SDRKIT_DVBT2_GEN_MODULATION_16QAM = 1,
  // 64-QAM
  SDRKIT_DVBT2_GEN_MODULATION_64QAM = 2,
  // 256-QAM
  SDRKIT_DVBT2_GEN_MODULATION_256QAM = 3,
} sdrkit_dvbt2_gen_modulation;

// The options of the setting "gen_rate" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_rate {
  // 1/2
  SDRKIT_DVBT2_GEN_RATE_1_2 = 0,
  // 3/5
  SDRKIT_DVBT2_GEN_RATE_3_5 = 1,
  // 2/3
  SDRKIT_DVBT2_GEN_RATE_2_3 = 2,
  // 3/4
  SDRKIT_DVBT2_GEN_RATE_3_4 = 3,
  // 4/5 (T2-base)
  SDRKIT_DVBT2_GEN_RATE_4_5 = 4,
  // 5/6 (T2-base)
  SDRKIT_DVBT2_GEN_RATE_5_6 = 5,
  // 1/3 (T2-Lite)
  SDRKIT_DVBT2_GEN_RATE_1_3 = 6,
  // 2/5 (T2-Lite)
  SDRKIT_DVBT2_GEN_RATE_2_5 = 7,
} sdrkit_dvbt2_gen_rate;

// The options of the setting "gen_fec_frame" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_fec_frame {
  // 16 200 bits
  SDRKIT_DVBT2_GEN_FEC_FRAME_16_200_BITS = 0,
  // 64 800 bits
  SDRKIT_DVBT2_GEN_FEC_FRAME_64_800_BITS = 1,
} sdrkit_dvbt2_gen_fec_frame;

// The options of the setting "gen_profile" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_profile {
  // T2-base
  SDRKIT_DVBT2_GEN_PROFILE_BASE = 0,
  // T2-Lite
  SDRKIT_DVBT2_GEN_PROFILE_LITE = 1,
} sdrkit_dvbt2_gen_profile;

// The options of the setting "gen_source" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_gen_source {
  // Test multiplex
  SDRKIT_DVBT2_GEN_SOURCE_MUX = 0,
  // Numbered packets
  SDRKIT_DVBT2_GEN_SOURCE_PATTERN = 1,
} sdrkit_dvbt2_gen_source;

// The options of the setting "tx_bandwidth" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_bandwidth {
  // 1.7 MHz
  SDRKIT_DVBT2_TX_BANDWIDTH_1_7 = 0,
  // 5 MHz
  SDRKIT_DVBT2_TX_BANDWIDTH_5_MHZ = 1,
  // 6 MHz
  SDRKIT_DVBT2_TX_BANDWIDTH_6_MHZ = 2,
  // 7 MHz
  SDRKIT_DVBT2_TX_BANDWIDTH_7_MHZ = 3,
  // 8 MHz
  SDRKIT_DVBT2_TX_BANDWIDTH_8_MHZ = 4,
  // 10 MHz
  SDRKIT_DVBT2_TX_BANDWIDTH_10_MHZ = 5,
} sdrkit_dvbt2_tx_bandwidth;

// The options of the setting "tx_fft" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_fft {
  // 1K
  SDRKIT_DVBT2_TX_FFT_1K = 0,
  // 2K
  SDRKIT_DVBT2_TX_FFT_2K = 1,
  // 4K
  SDRKIT_DVBT2_TX_FFT_4K = 2,
  // 8K
  SDRKIT_DVBT2_TX_FFT_8K = 3,
  // 16K
  SDRKIT_DVBT2_TX_FFT_16K = 4,
  // 32K
  SDRKIT_DVBT2_TX_FFT_32K = 5,
} sdrkit_dvbt2_tx_fft;

// The options of the setting "tx_guard" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_guard {
  // 1/32
  SDRKIT_DVBT2_TX_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBT2_TX_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBT2_TX_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBT2_TX_GUARD_1_4 = 3,
  // 1/128
  SDRKIT_DVBT2_TX_GUARD_1_128 = 4,
  // 19/128
  SDRKIT_DVBT2_TX_GUARD_19_128 = 5,
  // 19/256
  SDRKIT_DVBT2_TX_GUARD_19_256 = 6,
} sdrkit_dvbt2_tx_guard;

// The options of the setting "tx_pilots" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_pilots {
  // PP1
  SDRKIT_DVBT2_TX_PILOTS_PP1 = 0,
  // PP2
  SDRKIT_DVBT2_TX_PILOTS_PP2 = 1,
  // PP3
  SDRKIT_DVBT2_TX_PILOTS_PP3 = 2,
  // PP4
  SDRKIT_DVBT2_TX_PILOTS_PP4 = 3,
  // PP5
  SDRKIT_DVBT2_TX_PILOTS_PP5 = 4,
  // PP6
  SDRKIT_DVBT2_TX_PILOTS_PP6 = 5,
  // PP7
  SDRKIT_DVBT2_TX_PILOTS_PP7 = 6,
  // PP8
  SDRKIT_DVBT2_TX_PILOTS_PP8 = 7,
} sdrkit_dvbt2_tx_pilots;

// The options of the setting "tx_modulation" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_modulation {
  // QPSK
  SDRKIT_DVBT2_TX_MODULATION_QPSK = 0,
  // 16-QAM
  SDRKIT_DVBT2_TX_MODULATION_16QAM = 1,
  // 64-QAM
  SDRKIT_DVBT2_TX_MODULATION_64QAM = 2,
  // 256-QAM
  SDRKIT_DVBT2_TX_MODULATION_256QAM = 3,
} sdrkit_dvbt2_tx_modulation;

// The options of the setting "tx_rate" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_rate {
  // 1/2
  SDRKIT_DVBT2_TX_RATE_1_2 = 0,
  // 3/5
  SDRKIT_DVBT2_TX_RATE_3_5 = 1,
  // 2/3
  SDRKIT_DVBT2_TX_RATE_2_3 = 2,
  // 3/4
  SDRKIT_DVBT2_TX_RATE_3_4 = 3,
  // 4/5
  SDRKIT_DVBT2_TX_RATE_4_5 = 4,
  // 5/6
  SDRKIT_DVBT2_TX_RATE_5_6 = 5,
  // 1/3 (T2-Lite)
  SDRKIT_DVBT2_TX_RATE_1_3 = 6,
  // 2/5 (T2-Lite)
  SDRKIT_DVBT2_TX_RATE_2_5 = 7,
} sdrkit_dvbt2_tx_rate;

// The options of the setting "tx_fec_frame" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_fec_frame {
  // 16 200 bits
  SDRKIT_DVBT2_TX_FEC_FRAME_16_200_BITS = 0,
  // 64 800 bits
  SDRKIT_DVBT2_TX_FEC_FRAME_64_800_BITS = 1,
} sdrkit_dvbt2_tx_fec_frame;

// The options of the setting "tx_l1_mod" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_l1_mod {
  // BPSK
  SDRKIT_DVBT2_TX_L1_MOD_BPSK = 0,
  // QPSK
  SDRKIT_DVBT2_TX_L1_MOD_QPSK = 1,
  // 16-QAM
  SDRKIT_DVBT2_TX_L1_MOD_16QAM = 2,
  // 64-QAM
  SDRKIT_DVBT2_TX_L1_MOD_64QAM = 3,
} sdrkit_dvbt2_tx_l1_mod;

// The options of the setting "tx_profile" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_profile {
  // T2-base
  SDRKIT_DVBT2_TX_PROFILE_BASE = 0,
  // T2-Lite
  SDRKIT_DVBT2_TX_PROFILE_LITE = 1,
} sdrkit_dvbt2_tx_profile;

// The options of the setting "tx_miso" of the stack "dvbt2".
typedef enum sdrkit_dvbt2_tx_miso {
  // SISO
  SDRKIT_DVBT2_TX_MISO_SISO = 0,
  // MISO group 1
  SDRKIT_DVBT2_TX_MISO_GROUP1 = 1,
  // MISO group 2
  SDRKIT_DVBT2_TX_MISO_GROUP2 = 2,
} sdrkit_dvbt2_tx_miso;

// The options of `detection` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_detection {
  // Searching
  SDRKIT_DVBT2_FIELDS_DETECTION_SEARCHING = 0,
  // P1 found
  SDRKIT_DVBT2_FIELDS_DETECTION_P1 = 1,
  // L1 decoded
  SDRKIT_DVBT2_FIELDS_DETECTION_L1 = 2,
  // Flowing
  SDRKIT_DVBT2_FIELDS_DETECTION_FLOWING = 3,
} sdrkit_dvbt2_fields_detection;

// The options of `fft` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_fft {
  // 1K
  SDRKIT_DVBT2_FIELDS_FFT_1K = 0,
  // 2K
  SDRKIT_DVBT2_FIELDS_FFT_2K = 1,
  // 4K
  SDRKIT_DVBT2_FIELDS_FFT_4K = 2,
  // 8K
  SDRKIT_DVBT2_FIELDS_FFT_8K = 3,
  // 16K
  SDRKIT_DVBT2_FIELDS_FFT_16K = 4,
  // 32K
  SDRKIT_DVBT2_FIELDS_FFT_32K = 5,
} sdrkit_dvbt2_fields_fft;

// The options of `guard` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_guard {
  // 1/32
  SDRKIT_DVBT2_FIELDS_GUARD_1_32 = 0,
  // 1/16
  SDRKIT_DVBT2_FIELDS_GUARD_1_16 = 1,
  // 1/8
  SDRKIT_DVBT2_FIELDS_GUARD_1_8 = 2,
  // 1/4
  SDRKIT_DVBT2_FIELDS_GUARD_1_4 = 3,
  // 1/128
  SDRKIT_DVBT2_FIELDS_GUARD_1_128 = 4,
  // 19/128
  SDRKIT_DVBT2_FIELDS_GUARD_19_128 = 5,
  // 19/256
  SDRKIT_DVBT2_FIELDS_GUARD_19_256 = 6,
} sdrkit_dvbt2_fields_guard;

// The options of `pilot_pattern` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_pilot_pattern {
  // PP1
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP1 = 0,
  // PP2
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP2 = 1,
  // PP3
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP3 = 2,
  // PP4
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP4 = 3,
  // PP5
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP5 = 4,
  // PP6
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP6 = 5,
  // PP7
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP7 = 6,
  // PP8
  SDRKIT_DVBT2_FIELDS_PILOT_PATTERN_PP8 = 7,
} sdrkit_dvbt2_fields_pilot_pattern;

// The options of `profile` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_profile {
  // T2-base
  SDRKIT_DVBT2_FIELDS_PROFILE_BASE = 0,
  // T2-Lite
  SDRKIT_DVBT2_FIELDS_PROFILE_LITE = 1,
} sdrkit_dvbt2_fields_profile;

// The options of `l1_modulation` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_l1_modulation {
  // BPSK
  SDRKIT_DVBT2_FIELDS_L1_MODULATION_BPSK = 0,
  // QPSK
  SDRKIT_DVBT2_FIELDS_L1_MODULATION_QPSK = 1,
  // 16-QAM
  SDRKIT_DVBT2_FIELDS_L1_MODULATION_16QAM = 2,
  // 64-QAM
  SDRKIT_DVBT2_FIELDS_L1_MODULATION_64QAM = 3,
  // 256-QAM
  SDRKIT_DVBT2_FIELDS_L1_MODULATION_256QAM = 4,
} sdrkit_dvbt2_fields_l1_modulation;

// The options of `problem` of `sdrkit_dvbt2_fields`.
typedef enum sdrkit_dvbt2_fields_problem {
  // Time-frequency slicing is not supported
  SDRKIT_DVBT2_FIELDS_PROBLEM_TFS = 0,
  // Frame shape not supported
  SDRKIT_DVBT2_FIELDS_PROBLEM_FRAME_SHAPE = 1,
  // L1_POST_SIZE does not match the L1-post coding
  SDRKIT_DVBT2_FIELDS_PROBLEM_L1_POST_SIZE = 2,
  // The PLP is not in this multiplex
  SDRKIT_DVBT2_FIELDS_PROBLEM_NO_SUCH_PLP = 3,
  // No data PLP in this multiplex
  SDRKIT_DVBT2_FIELDS_PROBLEM_NO_DATA_PLP = 4,
  // A PLP uses an unknown coding
  SDRKIT_DVBT2_FIELDS_PROBLEM_UNKNOWN_CODING = 5,
} sdrkit_dvbt2_fields_problem;

// The result fields of `dvbt2`, which the receiver `rx` writes. A value of the frame shape has no value before the P2 symbols are found, and a value of the L1 signalling has no value before the L1-pre is decoded.
typedef struct sdrkit_dvbt2_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Detection: searching, P1 found, L1 decoded, or packets flowing. A value of `sdrkit_dvbt2_fields_detection`.
  int32_t detection;
  // True if `fft` holds a value.
  bool has_fft;
  // The FFT size. A value of `sdrkit_dvbt2_fields_fft`.
  int32_t fft;
  // True if `guard` holds a value.
  bool has_guard;
  // The guard interval. A value of `sdrkit_dvbt2_fields_guard`.
  int32_t guard;
  // True if `pilot_pattern` holds a value.
  bool has_pilot_pattern;
  // The scattered pilot pattern. A value of `sdrkit_dvbt2_fields_pilot_pattern`.
  int32_t pilot_pattern;
  // True if `extended_carriers` holds a value.
  bool has_extended_carriers;
  // Extended carrier mode.
  bool extended_carriers;
  // True if `miso` holds a value.
  bool has_miso;
  // MISO.
  bool miso;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile. A value of `sdrkit_dvbt2_fields_profile`.
  int32_t profile;
  // True if `l1_modulation` holds a value.
  bool has_l1_modulation;
  // The L1-post constellation. A value of `sdrkit_dvbt2_fields_l1_modulation`.
  int32_t l1_modulation;
  // True if `cell_id` holds a value.
  bool has_cell_id;
  // CELL_ID.
  int64_t cell_id;
  // True if `network_id` holds a value.
  bool has_network_id;
  // NETWORK_ID.
  int64_t network_id;
  // True if `t2_system_id` holds a value.
  bool has_t2_system_id;
  // T2_SYSTEM_ID.
  int64_t t2_system_id;
  // True if `frames_per_superframe` holds a value.
  bool has_frames_per_superframe;
  // NUM_T2_FRAMES.
  int64_t frames_per_superframe;
  // True if `data_symbols` holds a value.
  bool has_data_symbols;
  // NUM_DATA_SYMBOLS.
  int64_t data_symbols;
  // True if `frame_length` holds a value.
  bool has_frame_length;
  // The T2-frame length. A length of time in seconds.
  double frame_length;
  // T2-frames decoded.
  int64_t t2_frames;
  // L1 signalling failures.
  int64_t l1_errors;
  // True if `plp` holds a value.
  bool has_plp;
  // The data PLP decoded.
  int64_t plp;
  // True if `common_plp` holds a value.
  bool has_common_plp;
  // Its common PLP.
  int64_t common_plp;
  // Mean LDPC iterations per FEC block.
  double ldpc_iterations;
  // True if `net_rate_bit_s` holds a value.
  bool has_net_rate_bit_s;
  // The net bit rate of the decoded data PLP. Unit: bit/s.
  double net_rate_bit_s;
  // True if `problem` holds a value.
  bool has_problem;
  // A configuration that the receiver cannot follow. A value of `sdrkit_dvbt2_fields_problem`.
  int32_t problem;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset from the centre. No value while the receiver searches. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR. Unit: dB.
  double snr_db;
  // FEC blocks good.
  int64_t frames_ok;
  // FEC blocks that failed.
  int64_t frames_bad;
  // Bits that the BCH decoder corrected.
  int64_t bits_corrected;
} sdrkit_dvbt2_fields;

// The result fields of the sender `transmit` of `dvbt2`.
typedef struct sdrkit_dvbt2_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether a transmission is under way.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the centre of the channel sent. No value while the stack does not transmit. A frequency in Hz.
  double tx_freq_hz;
  // The bit rate of the transport stream that the settings carry: the rate that the host feeds at most. 0 while the settings cannot be sent. Unit: bit/s.
  double tx_bitrate_bps;
  // The packets of the host sent in this transmission, or in the last one.
  int64_t tx_packets;
  // The null packets put in slots that found no packet of the host.
  int64_t tx_nulls;
  // The packets that wait to be sent.
  int64_t tx_queue;
  // The time that the waiting packets take at the useful bit rate. A length of time in seconds.
  double tx_queue_s;
  // The PCRs rewritten to the time of their slot.
  int64_t tx_pcr_restamped;
  // True if `tx_frame_s` holds a value.
  bool has_tx_frame_s;
  // The length of a T2-frame. No value while the settings cannot be sent. A length of time in seconds.
  double tx_frame_s;
  // True if `tx_fec_blocks` holds a value.
  bool has_tx_fec_blocks;
  // The FEC blocks of a T2-frame. No value while the settings cannot be sent.
  int64_t tx_fec_blocks;
  // True if `tx_problem` holds a value.
  bool has_tx_problem;
  // Why the settings cannot be sent. No value while they can.
  sdrkit_string tx_problem;
} sdrkit_dvbt2_tx_fields;

// The options of `plp_type` of `sdrkit_dvbt2_plp_row`.
typedef enum sdrkit_dvbt2_plp_row_plp_type {
  // Common
  SDRKIT_DVBT2_PLP_ROW_PLP_TYPE_COMMON = 0,
  // Data type 1
  SDRKIT_DVBT2_PLP_ROW_PLP_TYPE_DATA1 = 1,
  // Data type 2
  SDRKIT_DVBT2_PLP_ROW_PLP_TYPE_DATA2 = 2,
  // Reserved
  SDRKIT_DVBT2_PLP_ROW_PLP_TYPE_RESERVED = 3,
} sdrkit_dvbt2_plp_row_plp_type;

// The options of `payload` of `sdrkit_dvbt2_plp_row`.
typedef enum sdrkit_dvbt2_plp_row_payload {
  // GFPS
  SDRKIT_DVBT2_PLP_ROW_PAYLOAD_GFPS = 0,
  // GCS
  SDRKIT_DVBT2_PLP_ROW_PAYLOAD_GCS = 1,
  // GSE
  SDRKIT_DVBT2_PLP_ROW_PAYLOAD_GSE = 2,
  // TS
  SDRKIT_DVBT2_PLP_ROW_PAYLOAD_TS = 3,
  // Reserved
  SDRKIT_DVBT2_PLP_ROW_PAYLOAD_RESERVED = 4,
} sdrkit_dvbt2_plp_row_payload;

// The options of `modulation` of `sdrkit_dvbt2_plp_row`.
typedef enum sdrkit_dvbt2_plp_row_modulation {
  // BPSK
  SDRKIT_DVBT2_PLP_ROW_MODULATION_BPSK = 0,
  // QPSK
  SDRKIT_DVBT2_PLP_ROW_MODULATION_QPSK = 1,
  // 16-QAM
  SDRKIT_DVBT2_PLP_ROW_MODULATION_16QAM = 2,
  // 64-QAM
  SDRKIT_DVBT2_PLP_ROW_MODULATION_64QAM = 3,
  // 256-QAM
  SDRKIT_DVBT2_PLP_ROW_MODULATION_256QAM = 4,
} sdrkit_dvbt2_plp_row_modulation;

// The options of `code_rate` of `sdrkit_dvbt2_plp_row`.
typedef enum sdrkit_dvbt2_plp_row_code_rate {
  // 1/2
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_1_2 = 0,
  // 3/5
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_3_5 = 1,
  // 2/3
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_2_3 = 2,
  // 3/4
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_3_4 = 3,
  // 4/5
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_4_5 = 4,
  // 5/6
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_5_6 = 5,
  // 1/3
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_1_3 = 6,
  // 2/5
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_2_5 = 7,
  // 1/4
  SDRKIT_DVBT2_PLP_ROW_CODE_RATE_1_4 = 8,
} sdrkit_dvbt2_plp_row_code_rate;

// The options of `time_interleaving` of `sdrkit_dvbt2_plp_row`.
typedef enum sdrkit_dvbt2_plp_row_time_interleaving {
  // Off
  SDRKIT_DVBT2_PLP_ROW_TIME_INTERLEAVING_OFF = 0,
  // TI-blocks per frame
  SDRKIT_DVBT2_PLP_ROW_TIME_INTERLEAVING_BLOCKS = 1,
  // Over T2-frames
  SDRKIT_DVBT2_PLP_ROW_TIME_INTERLEAVING_FRAMES = 2,
} sdrkit_dvbt2_plp_row_time_interleaving;

// A row of the PLPs of the multiplex. The row key is PLP_ID in decimal.
typedef struct sdrkit_dvbt2_plp_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // PLP_ID.
  int64_t plp_id;
  // PLP_TYPE. A value of `sdrkit_dvbt2_plp_row_plp_type`.
  int32_t plp_type;
  // PLP_PAYLOAD_TYPE. A value of `sdrkit_dvbt2_plp_row_payload`.
  int32_t payload;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The constellation. No value for a reserved code. A value of `sdrkit_dvbt2_plp_row_modulation`.
  int32_t modulation;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The code rate. No value for a code that the profile does not use. A value of `sdrkit_dvbt2_plp_row_code_rate`.
  int32_t code_rate;
  // True if `fec_frame_bits` holds a value.
  bool has_fec_frame_bits;
  // The FEC frame length. No value for a reserved code. Unit: bits.
  int64_t fec_frame_bits;
  // Rotated constellation.
  bool rotated;
  // PLP_NUM_BLOCKS of this frame.
  int64_t fec_blocks;
  // PLP_GROUP_ID.
  int64_t group_id;
  // How the PLP is time interleaved. A value of `sdrkit_dvbt2_plp_row_time_interleaving`.
  int32_t time_interleaving;
  // TIME_IL_LENGTH: TI-blocks per frame, or T2-frames per TI-block.
  int64_t time_il_length;
} sdrkit_dvbt2_plp_row;

// The arguments of `service.select`.
typedef struct sdrkit_dvbt2_service_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the service in `services`. Empty: the first service that can play.
  sdrkit_string row;
} sdrkit_dvbt2_service_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT2_SERVICE_SELECT_ARGS_INIT {sizeof(sdrkit_dvbt2_service_select_args), {0}}

// The arguments of `teletext.select`.
typedef struct sdrkit_dvbt2_teletext_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the page in `teletext_pages`.
  sdrkit_string row;
} sdrkit_dvbt2_teletext_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT2_TELETEXT_SELECT_ARGS_INIT {sizeof(sdrkit_dvbt2_teletext_select_args), {0}}

// The arguments of `track.select`.
typedef struct sdrkit_dvbt2_track_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the track in `tracks`.
  sdrkit_string row;
} sdrkit_dvbt2_track_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT2_TRACK_SELECT_ARGS_INIT {sizeof(sdrkit_dvbt2_track_select_args), {0}}

// The arguments of the `append` command of a sender that takes transport packets.
typedef struct sdrkit_dvbt2_tx_append_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The packets.
  sdrkit_bytes packets;
} sdrkit_dvbt2_tx_append_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DVBT2_TX_APPEND_ARGS_INIT {sizeof(sdrkit_dvbt2_tx_append_args), {0}}

// Fills `rows` with the changed rows of the table "plps" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_plps_rows(SDRKIT_HANDLE handle,
                                            sdrkit_dvbt2_plp_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dvbt2.Dvbt2PlpRow", "plps", rows, sizeof(sdrkit_dvbt2_plp_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_services_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mpegts_ts_service_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_tracks_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mpegts_ts_track_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_guide_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_guide_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_guide_rating_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_pages" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_teletext_pages_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_teletext_page_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextPageRow", "teletext_pages", rows, sizeof(sdrkit_mpegts_teletext_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "teletext_subpages" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_teletext_subpages_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_mpegts_teletext_subpage_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TeletextSubpageRow", "teletext_subpages", rows, sizeof(sdrkit_mpegts_teletext_subpage_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_pid_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dvbt2" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dvbt2_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_section_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "dvbt2" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_dvbt2_service_select(SDRKIT_HANDLE handle, const sdrkit_dvbt2_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "dvbt2.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "teletext.select" (Select a teletext page) of the stack "dvbt2" on the session of `handle`. Shows a teletext page.
static inline int32_t sdrkit_dvbt2_teletext_select(SDRKIT_HANDLE handle, const sdrkit_dvbt2_teletext_select_args *args) {
  return sdrkit_typed_command(handle, "teletext.select", "dvbt2.TeletextSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "dvbt2" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_dvbt2_track_select(SDRKIT_HANDLE handle, const sdrkit_dvbt2_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "dvbt2.TrackSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "start" (Start sending) of the stack "dvbt2" on the session of `handle`. Sends a DVB-T2 signal with the tx_* settings from the start of a T2-frame, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_dvbt2_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "dvbt2" on the session of `handle`. Ends the transmission after the T2-frame on air with a fall of 5 ms. The packets that wait are dropped.
static inline int32_t sdrkit_dvbt2_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "append" (Add packets) of the stack "dvbt2" on the session of `handle`. Adds transport packets to the queue of the stream. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.
static inline int32_t sdrkit_dvbt2_append(SDRKIT_HANDLE handle, const sdrkit_dvbt2_tx_append_args *args) {
  return sdrkit_typed_command(handle, "append", "dvbt2.TxAppendArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dvbt2" are the sdrkit_dvbt2_gen_*() functions
// after the settings of its receiver.

// The setting "bandwidth" (Channel bandwidth) of the stack "dvbt2". A value of `sdrkit_dvbt2_bandwidth`.
static inline int32_t sdrkit_dvbt2_get_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt2_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt2_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "bandwidth", (int32_t)value);
}

// The setting "plp" (PLP) of the stack "dvbt2". The data PLP to decode. -1 decodes the first data PLP. Its common PLP is decoded with it.
static inline int32_t sdrkit_dvbt2_get_plp(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "plp", value);
}
static inline int32_t sdrkit_dvbt2_set_plp(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "plp", value);
}

// The setting "decode_video" (Decode video) of the stack "dvbt2". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dvbt2_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dvbt2_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "tx_bandwidth" (Channel width) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_bandwidth`. The width of the channel sent. It sets the elementary rate of the signal.
static inline int32_t sdrkit_dvbt2_get_tx_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_bandwidth(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_bandwidth", (int32_t)value);
}

// The setting "tx_fft" (FFT size) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_fft`.
static inline int32_t sdrkit_dvbt2_get_tx_fft(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_fft *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_fft", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_fft)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_fft(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_fft value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_fft", (int32_t)value);
}

// The setting "tx_guard" (Guard interval) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_guard`. A guard interval that the FFT size does not take is refused at start.
static inline int32_t sdrkit_dvbt2_get_tx_guard(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_guard)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_guard(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_guard", (int32_t)value);
}

// The setting "tx_pilots" (Pilot pattern) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_pilots`. A pattern that EN 302 755 tables 58 and 59 do not allow with the FFT size and the guard interval is refused at start.
static inline int32_t sdrkit_dvbt2_get_tx_pilots(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_pilots)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_pilots(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_pilots", (int32_t)value);
}

// The setting "tx_extended" (Extended carriers) of the stack "dvbt2". 8K, 16K and 32K only.
static inline int32_t sdrkit_dvbt2_get_tx_extended(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_extended", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_extended(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_extended", value);
}

// The setting "tx_modulation" (Constellation) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_modulation`.
static inline int32_t sdrkit_dvbt2_get_tx_modulation(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_modulation)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_modulation(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_modulation", (int32_t)value);
}

// The setting "tx_rate" (Code rate) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_rate`.
static inline int32_t sdrkit_dvbt2_get_tx_rate(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_rate)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_rate(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_rate", (int32_t)value);
}

// The setting "tx_rotation" (Rotated constellation) of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_get_tx_rotation(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_rotation", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_rotation(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_rotation", value);
}

// The setting "tx_fec_frame" (FEC frame) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_fec_frame`.
static inline int32_t sdrkit_dvbt2_get_tx_fec_frame(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_fec_frame *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_fec_frame", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_fec_frame)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_fec_frame(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_fec_frame value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_fec_frame", (int32_t)value);
}

// The setting "tx_ti_blocks" (Time interleaver) of the stack "dvbt2". Unit: TI-blocks. The TI-blocks of a T2-frame. 0: no time interleaving. Fewer TI-blocks interleave deeper and hold fewer FEC blocks a frame.
static inline int32_t sdrkit_dvbt2_get_tx_ti_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_ti_blocks", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_ti_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_ti_blocks", value);
}

// The setting "tx_frame_ms" (Longest frame) of the stack "dvbt2". Unit: ms. The T2-frame holds whole symbols and whole FEC blocks and lasts this long at most, and never less than the shortest frame of the FFT size.
static inline int32_t sdrkit_dvbt2_get_tx_frame_ms(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_frame_ms", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_frame_ms(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_frame_ms", value);
}

// The setting "tx_l1_mod" (L1-post constellation) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_l1_mod`.
static inline int32_t sdrkit_dvbt2_get_tx_l1_mod(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_l1_mod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_l1_mod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_l1_mod)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_l1_mod(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_l1_mod value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_l1_mod", (int32_t)value);
}

// The setting "tx_cell_id" (Cell id) of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_get_tx_cell_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_cell_id", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_cell_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_cell_id", value);
}

// The setting "tx_network_id" (Network id) of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_get_tx_network_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_network_id", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_network_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_network_id", value);
}

// The setting "tx_system_id" (T2 system id) of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_get_tx_system_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_system_id", value);
}
static inline int32_t sdrkit_dvbt2_set_tx_system_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_system_id", value);
}

// The setting "tx_profile" (Profile) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_profile`. T2-Lite (EN 302 755 annex I) takes 2K to 16K, the code rates 1/2 to 3/4, 1/3 and 2/5, always the 16 200-bit FEC frame, 256-QAM only with 1/3, 2/5, 1/2 and 3/5 and never rotated, and at most 4 Mbit/s. A pattern that tables I.5 and I.6 do not allow is refused at start.
static inline int32_t sdrkit_dvbt2_get_tx_profile(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_profile)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_profile(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_profile", (int32_t)value);
}

// The setting "tx_miso" (MISO) of the stack "dvbt2". A value of `sdrkit_dvbt2_tx_miso`. In MISO each transmitter of group 2 sends the modified Alamouti code of the cells that group 1 sends, and its own pilot signs (EN 302 755 clause 9.1). A receiver decodes the sum of both groups. A pattern that table 59 does not allow is refused at start.
static inline int32_t sdrkit_dvbt2_get_tx_miso(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_miso *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_miso", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_tx_miso)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_set_tx_miso(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_miso value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_miso", (int32_t)value);
}

// The setting "gen_fft" (FFT size) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_fft`.
static inline int32_t sdrkit_dvbt2_gen_get_fft(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_fft *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_fft", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_fft)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_fft(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_fft value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_fft", (int32_t)value);
}

// The setting "gen_guard" (Guard interval) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_guard`.
static inline int32_t sdrkit_dvbt2_gen_get_guard(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_guard)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_guard(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_guard", (int32_t)value);
}

// The setting "gen_pilots" (Pilot pattern) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_pilots`.
static inline int32_t sdrkit_dvbt2_gen_get_pilots(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_pilots *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_pilots", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_pilots)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_pilots(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_pilots value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_pilots", (int32_t)value);
}

// The setting "gen_extended" (Extended carriers) of the test signal generator of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_gen_get_extended(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_extended", value);
}
static inline int32_t sdrkit_dvbt2_gen_set_extended(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_extended", value);
}

// The setting "gen_modulation" (Constellation) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_modulation`.
static inline int32_t sdrkit_dvbt2_gen_get_modulation(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_modulation)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_modulation(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_modulation", (int32_t)value);
}

// The setting "gen_rate" (Code rate) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_rate`.
static inline int32_t sdrkit_dvbt2_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// The setting "gen_fec_frame" (FEC frame) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_fec_frame`.
static inline int32_t sdrkit_dvbt2_gen_get_fec_frame(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_fec_frame *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_fec_frame", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_fec_frame)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_fec_frame(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_fec_frame value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_fec_frame", (int32_t)value);
}

// The setting "gen_rotation" (Rotated constellation) of the test signal generator of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_gen_get_rotation(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_rotation", value);
}
static inline int32_t sdrkit_dvbt2_gen_set_rotation(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_rotation", value);
}

// The setting "gen_profile" (Profile) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_profile`.
static inline int32_t sdrkit_dvbt2_gen_get_profile(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_profile)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_profile(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_profile", (int32_t)value);
}

// The setting "gen_miso" (MISO) of the test signal generator of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_gen_get_miso(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_miso", value);
}
static inline int32_t sdrkit_dvbt2_gen_set_miso(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_miso", value);
}

// The setting "gen_blocks" (FEC blocks per frame) of the test signal generator of the stack "dvbt2".
static inline int32_t sdrkit_dvbt2_gen_get_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_blocks", value);
}
static inline int32_t sdrkit_dvbt2_gen_set_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_blocks", value);
}

// The setting "gen_source" (Content) of the test signal generator of the stack "dvbt2". A value of `sdrkit_dvbt2_gen_source`.
static inline int32_t sdrkit_dvbt2_gen_get_source(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_source *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_source", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dvbt2_gen_source)index;
  return code;
}
static inline int32_t sdrkit_dvbt2_gen_set_source(SDRKIT_HANDLE handle, sdrkit_dvbt2_gen_source value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_source", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbt2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt2.Dvbt2Fields", fields, sizeof(sdrkit_dvbt2_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbt2_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbt2_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbt2.Dvbt2TxFields", fields, sizeof(sdrkit_dvbt2_tx_fields));
}

// Fills `fields` with the result fields of the component "ts" of the stack "dvbt2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts.<key>".
static inline int32_t sdrkit_dvbt2_ts_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@ts", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dvbt2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dvbt2_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dvbt2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dvbt2_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "dvbt2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_dvbt2_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "dvbt2" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_dvbt2_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

#endif
