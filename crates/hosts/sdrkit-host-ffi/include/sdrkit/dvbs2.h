/*
 * sdrkit: the typed interface of the shared interface "dvbs2", whose items the stacks of several families show, version 1.0. MIT licence.
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

#ifndef SDRKIT_DVBS2_H
#define SDRKIT_DVBS2_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_DVBS2_VERSION_MAJOR 1
#define SDRKIT_DVBS2_VERSION_MINOR 0

// The keys of the event types.

// The options of `standard` of `sdrkit_dvbs2_fields`.
typedef enum sdrkit_dvbs2_fields_standard {
  // DVB-S
  SDRKIT_DVBS2_FIELDS_STANDARD_DVBS = 0,
  // DVB-S2
  SDRKIT_DVBS2_FIELDS_STANDARD_DVBS2 = 1,
} sdrkit_dvbs2_fields_standard;

// The options of `signal_note` of `sdrkit_dvbs2_fields`.
typedef enum sdrkit_dvbs2_fields_signal_note {
  // No carrier found. Most often the LNB is on the other polarisation (13 V vertical, 18 V horizontal) or the other band (22 kHz tone), or the dish is off target.
  SDRKIT_DVBS2_FIELDS_SIGNAL_NOTE_NO_CARRIER = 0,
  // Frames followed, not decoded (no code of this size)
  SDRKIT_DVBS2_FIELDS_SIGNAL_NOTE_NOT_DECODED = 1,
} sdrkit_dvbs2_fields_signal_note;

// The options of `roll_off_source` of `sdrkit_dvbs2_fields`.
typedef enum sdrkit_dvbs2_fields_roll_off_source {
  // Assumed
  SDRKIT_DVBS2_FIELDS_ROLL_OFF_SOURCE_ASSUMED = 0,
  // Measured
  SDRKIT_DVBS2_FIELDS_ROLL_OFF_SOURCE_MEASURED = 1,
  // Signalled
  SDRKIT_DVBS2_FIELDS_ROLL_OFF_SOURCE_SIGNALLED = 2,
} sdrkit_dvbs2_fields_roll_off_source;

// The options of `modulation` of `sdrkit_dvbs2_fields`.
typedef enum sdrkit_dvbs2_fields_modulation {
  // QPSK
  SDRKIT_DVBS2_FIELDS_MODULATION_Q_P_S_K = 0,
  // 8PSK
  SDRKIT_DVBS2_FIELDS_MODULATION_8_P_S_K = 1,
  // 16APSK
  SDRKIT_DVBS2_FIELDS_MODULATION_16_A_P_S_K = 2,
  // 32APSK
  SDRKIT_DVBS2_FIELDS_MODULATION_32_A_P_S_K = 3,
} sdrkit_dvbs2_fields_modulation;

// The options of `code_rate` of `sdrkit_dvbs2_fields`.
typedef enum sdrkit_dvbs2_fields_code_rate {
  // 1/4
  SDRKIT_DVBS2_FIELDS_CODE_RATE_1_4 = 0,
  // 1/3
  SDRKIT_DVBS2_FIELDS_CODE_RATE_1_3 = 1,
  // 2/5
  SDRKIT_DVBS2_FIELDS_CODE_RATE_2_5 = 2,
  // 1/2
  SDRKIT_DVBS2_FIELDS_CODE_RATE_1_2 = 3,
  // 3/5
  SDRKIT_DVBS2_FIELDS_CODE_RATE_3_5 = 4,
  // 2/3
  SDRKIT_DVBS2_FIELDS_CODE_RATE_2_3 = 5,
  // 3/4
  SDRKIT_DVBS2_FIELDS_CODE_RATE_3_4 = 6,
  // 4/5
  SDRKIT_DVBS2_FIELDS_CODE_RATE_4_5 = 7,
  // 5/6
  SDRKIT_DVBS2_FIELDS_CODE_RATE_5_6 = 8,
  // 7/8
  SDRKIT_DVBS2_FIELDS_CODE_RATE_7_8 = 9,
  // 8/9
  SDRKIT_DVBS2_FIELDS_CODE_RATE_8_9 = 10,
  // 9/10
  SDRKIT_DVBS2_FIELDS_CODE_RATE_9_10 = 11,
} sdrkit_dvbs2_fields_code_rate;

// The options of `frame` of `sdrkit_dvbs2_fields`.
typedef enum sdrkit_dvbs2_fields_frame {
  // Normal (64800)
  SDRKIT_DVBS2_FIELDS_FRAME_NORMAL = 0,
  // Short (16200)
  SDRKIT_DVBS2_FIELDS_FRAME_SHORT = 1,
} sdrkit_dvbs2_fields_frame;

// The result fields of the DVB-S/S2 receiver: the carrier search, the frames, the code and the locks. A measurement of the carrier search has no value without a carrier.
typedef struct sdrkit_dvbs2_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `standard` holds a value.
  bool has_standard;
  // The standard received. A value of `sdrkit_dvbs2_fields_standard`.
  int32_t standard;
  // True if `signal_note` holds a value.
  bool has_signal_note;
  // A note on the signal for the user. A value of `sdrkit_dvbs2_fields_signal_note`.
  int32_t signal_note;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate in use. No value before a rate is set or measured. A frequency in Hz.
  double symbol_rate_hz;
  // True if `spectrum_rate` holds a value.
  bool has_spectrum_rate;
  // The symbol rate from the spectrum (width or cyclic line). A frequency in Hz.
  double spectrum_rate;
  // The error of the symbol clock. Unit: ppm.
  double clock_ppm;
  // The symbol rate was set by hand.
  bool manual_rate;
  // True if `spectrum_snr_db` holds a value.
  bool has_spectrum_snr_db;
  // The plateau of the carrier over the noise floor. Unit: dB.
  double spectrum_snr_db;
  // True if `misfit` holds a value.
  bool has_misfit;
  // The misfit of the raised cosine fit.
  double misfit;
  // True if `edge_low` holds a value.
  bool has_edge_low;
  // The lower -3 dB edge of the carrier, from the centre. A frequency in Hz.
  double edge_low;
  // True if `edge_high` holds a value.
  bool has_edge_high;
  // The upper -3 dB edge of the carrier, from the centre. A frequency in Hz.
  double edge_high;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset from the centre. No value without a carrier. A frequency in Hz.
  double offset_hz;
  // The spectrum is inverted.
  bool inverted;
  // The roll-off.
  double roll_off;
  // Where the roll-off came from. A value of `sdrkit_dvbs2_fields_roll_off_source`.
  int32_t roll_off_source;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The modulation. A value of `sdrkit_dvbs2_fields_modulation`.
  int32_t modulation;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The code rate. A value of `sdrkit_dvbs2_fields_code_rate`.
  int32_t code_rate;
  // True if `modcod` holds a value.
  bool has_modcod;
  // The MODCOD (DVB-S2).
  int64_t modcod;
  // True if `frame` holds a value.
  bool has_frame;
  // The FEC frame size (DVB-S2). A value of `sdrkit_dvbs2_fields_frame`.
  int32_t frame;
  // Pilots are on.
  bool pilots;
  // The MODCOD changes between frames.
  bool vcm;
  // The carrier has one input stream.
  bool single_stream;
  // The PL scrambling code in use.
  int64_t pl_scrambling_code;
  // PLFRAMEs seen.
  int64_t plframes;
  // Dummy PLFRAMEs seen.
  int64_t dummy_frames;
  // Frames followed but not decoded.
  int64_t unsupported_frames;
  // Frames skipped by the MODCOD rule or the frame length rule.
  int64_t other_modcod_frames;
  // Frames whose pilots differ from the setting.
  int64_t pilot_mismatch;
  // BBHEADERs refused (CRC-8 or length).
  int64_t bbheader_bad;
  // The net bit rate of the data fields. Unit: bit/s.
  double net_bit_rate;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // Es/N0. Unit: dB.
  double esn0_db;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The modulation error ratio. Unit: dB.
  double mer_db;
  // True if `ebn0_db` holds a value.
  bool has_ebn0_db;
  // Eb/N0. Unit: dB.
  double ebn0_db;
  // True if `margin_to_qef_db` holds a value.
  bool has_margin_to_qef_db;
  // Es/N0 above the quasi-error-free level of EN 302 307-1 table 13. Unit: dB.
  double margin_to_qef_db;
  // LDPC iterations, recent mean.
  double ldpc_iterations;
  // BCH words good.
  int64_t bch_ok;
  // BCH words that failed.
  int64_t bch_bad;
  // Reed-Solomon words with no error (DVB-S).
  int64_t rs_clean;
  // Reed-Solomon words corrected (DVB-S).
  int64_t rs_corrected;
  // Reed-Solomon words that failed (DVB-S).
  int64_t rs_failed;
  // A carrier is found.
  bool carrier_lock;
  // The symbol timing runs.
  bool timing_lock;
  // The carrier phase and the framing hold.
  bool frame_lock;
  // The FEC decodes words.
  bool fec_lock;
  // The transport stream is valid.
  bool ts_lock;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // The LNB supply selects the other polarisation than the transponder's.
  bool lnb_mismatch;
} sdrkit_dvbs2_fields;

// A row of the carriers of the survey. The row key is the offset in kHz.
typedef struct sdrkit_dvbs2_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset of the carrier from the centre. A frequency in Hz.
  double offset;
  // Its symbol rate. A frequency in Hz.
  double symbol_rate;
  // Its roll-off.
  double roll_off;
  // Its plateau over the floor. Unit: dB.
  double snr_db;
  // Its level against the strongest. Unit: dB.
  double level_db;
  // It is the carrier decoded.
  bool decoded;
} sdrkit_dvbs2_carrier_row;

// The options of `carries` of `sdrkit_dvbs2_stream_row`.
typedef enum sdrkit_dvbs2_stream_row_carries {
  // Transport stream
  SDRKIT_DVBS2_STREAM_ROW_CARRIES_TS = 0,
  // GSE
  SDRKIT_DVBS2_STREAM_ROW_CARRIES_GSE = 1,
  // GSE-HEM
  SDRKIT_DVBS2_STREAM_ROW_CARRIES_GSE_HEM = 2,
  // Transport packets in a generic stream
  SDRKIT_DVBS2_STREAM_ROW_CARRIES_TS_GENERIC = 3,
  // Packets
  SDRKIT_DVBS2_STREAM_ROW_CARRIES_PACKETS = 4,
} sdrkit_dvbs2_stream_row_carries;

// A row of the input streams. The row key is the ISI.
typedef struct sdrkit_dvbs2_stream_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The input stream identifier.
  int64_t isi;
  // What the stream carries. A value of `sdrkit_dvbs2_stream_row_carries`.
  int32_t carries;
  // True if `packet_bytes` holds a value.
  bool has_packet_bytes;
  // The user packet length. No value for a stream without user packets. Unit: bytes.
  int64_t packet_bytes;
  // True if `sync_byte` holds a value.
  bool has_sync_byte;
  // The sync byte of the user packets.
  int64_t sync_byte;
  // Constant coding and modulation.
  bool ccm;
  // Frames of the stream.
  int64_t frames;
  // Data field bits of the stream.
  int64_t bits;
} sdrkit_dvbs2_stream_row;

// The options of `modulation` of `sdrkit_dvbs2_stream_modcod_row`.
typedef enum sdrkit_dvbs2_stream_modcod_row_modulation {
  // QPSK
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_MODULATION_Q_P_S_K = 0,
  // 8PSK
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_MODULATION_8_P_S_K = 1,
  // 16APSK
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_MODULATION_16_A_P_S_K = 2,
  // 32APSK
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_MODULATION_32_A_P_S_K = 3,
} sdrkit_dvbs2_stream_modcod_row_modulation;

// The options of `code_rate` of `sdrkit_dvbs2_stream_modcod_row`.
typedef enum sdrkit_dvbs2_stream_modcod_row_code_rate {
  // 1/4
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_1_4 = 0,
  // 1/3
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_1_3 = 1,
  // 2/5
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_2_5 = 2,
  // 1/2
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_1_2 = 3,
  // 3/5
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_3_5 = 4,
  // 2/3
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_2_3 = 5,
  // 3/4
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_3_4 = 6,
  // 4/5
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_4_5 = 7,
  // 5/6
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_5_6 = 8,
  // 7/8
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_7_8 = 9,
  // 8/9
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_8_9 = 10,
  // 9/10
  SDRKIT_DVBS2_STREAM_MODCOD_ROW_CODE_RATE_9_10 = 11,
} sdrkit_dvbs2_stream_modcod_row_code_rate;

// A row of the MODCODs of the input streams. The row key is `<isi>/<modcod>`.
typedef struct sdrkit_dvbs2_stream_modcod_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The input stream, a row key of the stream table.
  sdrkit_string stream;
  // The MODCOD.
  int64_t modcod;
  // The modulation of the MODCOD. A value of `sdrkit_dvbs2_stream_modcod_row_modulation`.
  int32_t modulation;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The code rate of the MODCOD. A value of `sdrkit_dvbs2_stream_modcod_row_code_rate`.
  int32_t code_rate;
  // The frames of the stream with this MODCOD.
  int64_t frames;
  // The share of the stream's frames with this MODCOD. Unit: %.
  double share_pct;
} sdrkit_dvbs2_stream_modcod_row;

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dvbs2_fields_read(SDRKIT_HANDLE handle, sdrkit_dvbs2_fields *fields) {
  return sdrkit_typed_fields(handle, "dvbs2.Dvbs2Fields", fields, sizeof(sdrkit_dvbs2_fields));
}

#endif
