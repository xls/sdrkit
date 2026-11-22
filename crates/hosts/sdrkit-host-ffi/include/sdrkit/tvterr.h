/*
 * sdrkit: the typed interface of the stack family "tvterr", version 1.7. MIT licence.
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

#ifndef SDRKIT_TVTERR_H
#define SDRKIT_TVTERR_H

#include "../sdrkit.h"
#include "mpegts.h"
#include "ipcast.h"
#include "graph.h"

// The version of the family that this header describes.
#define SDRKIT_TVTERR_VERSION_MAJOR 1
#define SDRKIT_TVTERR_VERSION_MINOR 7

// The stack "isdbt": its id, and the keys of its event types.
#define SDRKIT_STACK_ISDBT "isdbt"
#define SDRKIT_EVENT_ISDBT_TMCC "isdbt.tmcc"

// The stack "isdbtsb": its id, and the keys of its event types.
#define SDRKIT_STACK_ISDBTSB "isdbtsb"

// The stack "atsc": its id, and the keys of its event types.
#define SDRKIT_STACK_ATSC "atsc"

// The stack "atsc3": its id, and the keys of its event types.
#define SDRKIT_STACK_ATSC3 "atsc3"

// The stack "dtmb": its id, and the keys of its event types.
#define SDRKIT_STACK_DTMB "dtmb"

// The stack "tv-auto": its id, and the keys of its event types.
#define SDRKIT_STACK_TV_AUTO "tv-auto"
#define SDRKIT_EVENT_TV_AUTO_FOUND "tv-auto.found"

// The options of the setting "width" of the stack "isdbt".
typedef enum sdrkit_isdbt_width {
  // 6 MHz
  SDRKIT_ISDBT_WIDTH_6_MHZ = 0,
  // 7 MHz
  SDRKIT_ISDBT_WIDTH_7_MHZ = 1,
  // 8 MHz (Maldives)
  SDRKIT_ISDBT_WIDTH_8_MHZ = 2,
  // Automatic
  SDRKIT_ISDBT_WIDTH_AUTO = 3,
} sdrkit_isdbt_width;

// The options of the setting "width" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_width {
  // 6 MHz
  SDRKIT_ISDBTSB_WIDTH_6_MHZ = 0,
  // 7 MHz
  SDRKIT_ISDBTSB_WIDTH_7_MHZ = 1,
  // 8 MHz (Maldives)
  SDRKIT_ISDBTSB_WIDTH_8_MHZ = 2,
  // Automatic
  SDRKIT_ISDBTSB_WIDTH_AUTO = 3,
} sdrkit_isdbtsb_width;

// The options of the setting "segments" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_segments {
  // Automatic
  SDRKIT_ISDBTSB_SEGMENTS_AUTO = 0,
  // 1
  SDRKIT_ISDBTSB_SEGMENTS_1 = 1,
  // 3
  SDRKIT_ISDBTSB_SEGMENTS_3 = 2,
} sdrkit_isdbtsb_segments;

// The options of the setting "edge_pilot" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_edge_pilot {
  // Pilot sequence
  SDRKIT_ISDBTSB_EDGE_PILOT_SEQUENCE = 0,
  // +4/3
  SDRKIT_ISDBTSB_EDGE_PILOT_PLUS = 1,
  // -4/3
  SDRKIT_ISDBTSB_EDGE_PILOT_MINUS = 2,
} sdrkit_isdbtsb_edge_pilot;

// The options of the setting "gen_mode" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_gen_mode {
  // Mode 1
  SDRKIT_ISDBTSB_GEN_MODE_MODE_1 = 0,
  // Mode 2
  SDRKIT_ISDBTSB_GEN_MODE_MODE_2 = 1,
  // Mode 3
  SDRKIT_ISDBTSB_GEN_MODE_MODE_3 = 2,
} sdrkit_isdbtsb_gen_mode;

// The options of the setting "gen_guard" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_gen_guard {
  // 1/4
  SDRKIT_ISDBTSB_GEN_GUARD_1_4 = 0,
  // 1/8
  SDRKIT_ISDBTSB_GEN_GUARD_1_8 = 1,
  // 1/16
  SDRKIT_ISDBTSB_GEN_GUARD_1_16 = 2,
  // 1/32
  SDRKIT_ISDBTSB_GEN_GUARD_1_32 = 3,
} sdrkit_isdbtsb_gen_guard;

// The options of the setting "gen_modulation" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_gen_modulation {
  // QPSK
  SDRKIT_ISDBTSB_GEN_MODULATION_QPSK = 0,
  // 16QAM
  SDRKIT_ISDBTSB_GEN_MODULATION_16QAM = 1,
  // 64QAM
  SDRKIT_ISDBTSB_GEN_MODULATION_64QAM = 2,
} sdrkit_isdbtsb_gen_modulation;

// The options of the setting "gen_code_rate" of the stack "isdbtsb".
typedef enum sdrkit_isdbtsb_gen_code_rate {
  // 1/2
  SDRKIT_ISDBTSB_GEN_CODE_RATE_1_2 = 0,
  // 2/3
  SDRKIT_ISDBTSB_GEN_CODE_RATE_2_3 = 1,
  // 3/4
  SDRKIT_ISDBTSB_GEN_CODE_RATE_3_4 = 2,
  // 5/6
  SDRKIT_ISDBTSB_GEN_CODE_RATE_5_6 = 3,
  // 7/8
  SDRKIT_ISDBTSB_GEN_CODE_RATE_7_8 = 4,
} sdrkit_isdbtsb_gen_code_rate;

// The options of the setting "width" of the stack "dtmb".
typedef enum sdrkit_dtmb_width {
  // 8 MHz
  SDRKIT_DTMB_WIDTH_8_MHZ = 0,
  // 6 MHz (Cuba)
  SDRKIT_DTMB_WIDTH_6_MHZ = 1,
} sdrkit_dtmb_width;

// The options of `guard` of `sdrkit_isdbt_fields`.
typedef enum sdrkit_isdbt_fields_guard {
  // 1/4
  SDRKIT_ISDBT_FIELDS_GUARD_1_4 = 0,
  // 1/8
  SDRKIT_ISDBT_FIELDS_GUARD_1_8 = 1,
  // 1/16
  SDRKIT_ISDBT_FIELDS_GUARD_1_16 = 2,
  // 1/32
  SDRKIT_ISDBT_FIELDS_GUARD_1_32 = 3,
} sdrkit_isdbt_fields_guard;

// The options of `quality` of `sdrkit_isdbt_fields`.
typedef enum sdrkit_isdbt_fields_quality {
  // Excellent
  SDRKIT_ISDBT_FIELDS_QUALITY_EXCELLENT = 0,
  // Good
  SDRKIT_ISDBT_FIELDS_QUALITY_GOOD = 1,
  // Marginal
  SDRKIT_ISDBT_FIELDS_QUALITY_MARGINAL = 2,
  // Poor
  SDRKIT_ISDBT_FIELDS_QUALITY_POOR = 3,
  // No lock
  SDRKIT_ISDBT_FIELDS_QUALITY_NO_LOCK = 4,
} sdrkit_isdbt_fields_quality;

// The options of `step_mode` of `sdrkit_isdbt_fields`.
typedef enum sdrkit_isdbt_fields_step_mode {
  // Off
  SDRKIT_ISDBT_FIELDS_STEP_MODE_OFF = 0,
  // Amber
  SDRKIT_ISDBT_FIELDS_STEP_MODE_AMBER = 1,
  // Green
  SDRKIT_ISDBT_FIELDS_STEP_MODE_GREEN = 2,
  // Red
  SDRKIT_ISDBT_FIELDS_STEP_MODE_RED = 3,
} sdrkit_isdbt_fields_step_mode;

// The options of `step_tmcc` of `sdrkit_isdbt_fields`.
typedef enum sdrkit_isdbt_fields_step_tmcc {
  // Off
  SDRKIT_ISDBT_FIELDS_STEP_TMCC_OFF = 0,
  // Amber
  SDRKIT_ISDBT_FIELDS_STEP_TMCC_AMBER = 1,
  // Green
  SDRKIT_ISDBT_FIELDS_STEP_TMCC_GREEN = 2,
  // Red
  SDRKIT_ISDBT_FIELDS_STEP_TMCC_RED = 3,
} sdrkit_isdbt_fields_step_tmcc;

// The options of `step_layers` of `sdrkit_isdbt_fields`.
typedef enum sdrkit_isdbt_fields_step_layers {
  // Off
  SDRKIT_ISDBT_FIELDS_STEP_LAYERS_OFF = 0,
  // Amber
  SDRKIT_ISDBT_FIELDS_STEP_LAYERS_AMBER = 1,
  // Green
  SDRKIT_ISDBT_FIELDS_STEP_LAYERS_GREEN = 2,
  // Red
  SDRKIT_ISDBT_FIELDS_STEP_LAYERS_RED = 3,
} sdrkit_isdbt_fields_step_layers;

// The options of `step_ts` of `sdrkit_isdbt_fields`.
typedef enum sdrkit_isdbt_fields_step_ts {
  // Off
  SDRKIT_ISDBT_FIELDS_STEP_TS_OFF = 0,
  // Amber
  SDRKIT_ISDBT_FIELDS_STEP_TS_AMBER = 1,
  // Green
  SDRKIT_ISDBT_FIELDS_STEP_TS_GREEN = 2,
  // Red
  SDRKIT_ISDBT_FIELDS_STEP_TS_RED = 3,
} sdrkit_isdbt_fields_step_ts;

// The result fields of `isdbt` and `isdbtsb`: the detection, the transmission parameters, the signal and the TMCC. A parameter of the mode has no value before the mode is found. A value of the TMCC has no value while no TMCC is decoded.
typedef struct sdrkit_isdbt_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Detection level: 0 nothing, 1 a periodic cyclic prefix, 2 TMCC decoded, 3 transport stream flowing.
  int64_t detection;
  // The channel width in use. A frequency in Hz.
  double width;
  // The width setting is automatic.
  bool width_auto;
  // The IFFT sampling rate of the width. A frequency in Hz.
  double ifft_rate;
  // True if `mode` holds a value.
  bool has_mode;
  // Mode 1, 2 or 3.
  int64_t mode;
  // True if `guard` holds a value.
  bool has_guard;
  // The guard interval. A value of `sdrkit_isdbt_fields_guard`.
  int32_t guard;
  // True if `fft_size` holds a value.
  bool has_fft_size;
  // The FFT size.
  int64_t fft_size;
  // True if `carriers` holds a value.
  bool has_carriers;
  // The carriers in the band.
  int64_t carriers;
  // True if `carrier_spacing` holds a value.
  bool has_carrier_spacing;
  // The carrier spacing. A frequency in Hz.
  double carrier_spacing;
  // True if `frame_length` holds a value.
  bool has_frame_length;
  // The frame length (204 symbols). A length of time in seconds.
  double frame_length;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the sample clock. Unit: ppm.
  double clock_ppm;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the equalised cells. Unit: dB.
  double snr_db;
  // Reed-Solomon words clean or corrected.
  int64_t frames_ok;
  // Reed-Solomon words that failed.
  int64_t frames_bad;
  // True if `carrier_shift` holds a value.
  bool has_carrier_shift;
  // The integer carrier shift that the TMCC search found.
  int64_t carrier_shift;
  // True if `frame_symbol` holds a value.
  bool has_frame_symbol;
  // The index of the symbol in the frame. No value while no TMCC is decoded.
  int64_t frame_symbol;
  // True if `since_tmcc` holds a value.
  bool has_since_tmcc;
  // The time since the last good TMCC. A length of time in seconds.
  double since_tmcc;
  // I and Q are swapped.
  bool mirrored;
  // True if `system_id` holds a value.
  bool has_system_id;
  // The system identification of the TMCC (0 ISDB-T, 1 ISDB-TSB).
  int64_t system_id;
  // True if `switch_countdown` holds a value.
  bool has_switch_countdown;
  // The parameter switching countdown of the TMCC (15: no switch announced).
  int64_t switch_countdown;
  // True if `emergency_alarm` holds a value.
  bool has_emergency_alarm;
  // The emergency alarm start flag of the TMCC.
  bool emergency_alarm;
  // True if `partial_reception` holds a value.
  bool has_partial_reception;
  // The partial reception flag of the TMCC.
  bool partial_reception;
  // The share of the Reed-Solomon words that are clean or corrected. Unit: %.
  double packets_ok_pct;
  // The net bit rate of every layer together. Unit: bit/s.
  double net_rate_bit_s;
  // The signal quality score. Unit: %.
  double quality_pct;
  // The signal quality class. A value of `sdrkit_isdbt_fields_quality`.
  int32_t quality;
  // The mode step: the mode and the guard are found. A value of `sdrkit_isdbt_fields_step_mode`.
  int32_t step_mode;
  // The TMCC step. A value of `sdrkit_isdbt_fields_step_tmcc`.
  int32_t step_tmcc;
  // The layers step: a layer is synchronised. A value of `sdrkit_isdbt_fields_step_layers`.
  int32_t step_layers;
  // The transport stream step. A value of `sdrkit_isdbt_fields_step_ts`.
  int32_t step_ts;
} sdrkit_isdbt_fields;

// The options of `layer` of `sdrkit_isdbt_layer_row`.
typedef enum sdrkit_isdbt_layer_row_layer {
  // A
  SDRKIT_ISDBT_LAYER_ROW_LAYER_A = 0,
  // B
  SDRKIT_ISDBT_LAYER_ROW_LAYER_B = 1,
  // C
  SDRKIT_ISDBT_LAYER_ROW_LAYER_C = 2,
} sdrkit_isdbt_layer_row_layer;

// The options of `modulation` of `sdrkit_isdbt_layer_row`.
typedef enum sdrkit_isdbt_layer_row_modulation {
  // DQPSK
  SDRKIT_ISDBT_LAYER_ROW_MODULATION_DQPSK = 0,
  // QPSK
  SDRKIT_ISDBT_LAYER_ROW_MODULATION_QPSK = 1,
  // 16QAM
  SDRKIT_ISDBT_LAYER_ROW_MODULATION_16QAM = 2,
  // 64QAM
  SDRKIT_ISDBT_LAYER_ROW_MODULATION_64QAM = 3,
} sdrkit_isdbt_layer_row_modulation;

// The options of `code_rate` of `sdrkit_isdbt_layer_row`.
typedef enum sdrkit_isdbt_layer_row_code_rate {
  // 1/2
  SDRKIT_ISDBT_LAYER_ROW_CODE_RATE_1_2 = 0,
  // 2/3
  SDRKIT_ISDBT_LAYER_ROW_CODE_RATE_2_3 = 1,
  // 3/4
  SDRKIT_ISDBT_LAYER_ROW_CODE_RATE_3_4 = 2,
  // 5/6
  SDRKIT_ISDBT_LAYER_ROW_CODE_RATE_5_6 = 3,
  // 7/8
  SDRKIT_ISDBT_LAYER_ROW_CODE_RATE_7_8 = 4,
} sdrkit_isdbt_layer_row_code_rate;

// A row of the layer table of `isdbt` and `isdbtsb`, one for each layer in use. The row key is `A`, `B` or `C`.
typedef struct sdrkit_isdbt_layer_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The layer. A value of `sdrkit_isdbt_layer_row_layer`.
  int32_t layer;
  // The layer is the partial-reception segment (one-seg).
  bool partial;
  // The segments of the layer.
  int64_t segments;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The carrier modulation. No value for a code that the standard reserves. A value of `sdrkit_isdbt_layer_row_modulation`.
  int32_t modulation;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The inner code rate. No value for a code that the standard reserves. A value of `sdrkit_isdbt_layer_row_code_rate`.
  int32_t code_rate;
  // The time interleaving length I.
  int64_t interleave_length;
  // The net bit rate of the layer. Unit: bit/s.
  double rate_bit_s;
  // The packets delivered.
  int64_t packets;
  // Reed-Solomon words clean.
  int64_t rs_clean;
  // Reed-Solomon words corrected.
  int64_t rs_corrected;
  // Reed-Solomon words that failed.
  int64_t rs_failed;
  // Reed-Solomon has synchronised the layer.
  bool rs_locked;
  // The Viterbi margin (1: no noise).
  double viterbi_margin;
} sdrkit_isdbt_layer_row;

// The result fields that a narrow-band receiver (`isdbtsb`) adds.
typedef struct sdrkit_isdbt_narrow_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The segment count that the receiver uses (1 or 3).
  int64_t band_segments;
  // What the receiver decodes and which of its values no public document confirms.
  sdrkit_string note;
} sdrkit_isdbt_narrow_fields;

// The options of `quality` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_quality {
  // Excellent
  SDRKIT_ATSC_FIELDS_QUALITY_EXCELLENT = 0,
  // Good
  SDRKIT_ATSC_FIELDS_QUALITY_GOOD = 1,
  // Marginal
  SDRKIT_ATSC_FIELDS_QUALITY_MARGINAL = 2,
  // Poor
  SDRKIT_ATSC_FIELDS_QUALITY_POOR = 3,
  // No lock
  SDRKIT_ATSC_FIELDS_QUALITY_NO_LOCK = 4,
} sdrkit_atsc_fields_quality;

// The options of `step_pilot` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_pilot {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_PILOT_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_PILOT_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_PILOT_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_PILOT_RED = 3,
} sdrkit_atsc_fields_step_pilot;

// The options of `step_segment` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_segment {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_SEGMENT_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_SEGMENT_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_SEGMENT_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_SEGMENT_RED = 3,
} sdrkit_atsc_fields_step_segment;

// The options of `step_field` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_field {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_FIELD_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_FIELD_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_FIELD_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_FIELD_RED = 3,
} sdrkit_atsc_fields_step_field;

// The options of `step_equaliser` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_equaliser {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_EQUALISER_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_EQUALISER_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_EQUALISER_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_EQUALISER_RED = 3,
} sdrkit_atsc_fields_step_equaliser;

// The options of `step_trellis` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_trellis {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_TRELLIS_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_TRELLIS_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_TRELLIS_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_TRELLIS_RED = 3,
} sdrkit_atsc_fields_step_trellis;

// The options of `step_rs` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_rs {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_RS_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_RS_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_RS_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_RS_RED = 3,
} sdrkit_atsc_fields_step_rs;

// The options of `step_ts` of `sdrkit_atsc_fields`.
typedef enum sdrkit_atsc_fields_step_ts {
  // Off
  SDRKIT_ATSC_FIELDS_STEP_TS_OFF = 0,
  // Amber
  SDRKIT_ATSC_FIELDS_STEP_TS_AMBER = 1,
  // Green
  SDRKIT_ATSC_FIELDS_STEP_TS_GREEN = 2,
  // Red
  SDRKIT_ATSC_FIELDS_STEP_TS_RED = 3,
} sdrkit_atsc_fields_step_ts;

// The result fields of `atsc`: the detection, the locks, the signal and the counts of the decoder. A measurement of a stage has no value before the stage locks.
typedef struct sdrkit_atsc_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Detection level: 0 nothing, 1 pilot, 2 segment sync, 3 field sync, 4 transport stream flowing.
  int64_t detection;
  // The pilot loop is locked.
  bool pilot_locked;
  // The segment sync and the symbol clock are locked.
  bool segment_sync;
  // Fields are framed.
  bool field_sync;
  // The equaliser is trained (SNR on the known symbols at least 9 dB).
  bool equaliser_trained;
  // The transport stream flows.
  bool ts_flowing;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. No value before the pilot locks. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock. No value before the segment sync. Unit: ppm.
  double clock_ppm;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR on the known symbols. No value before the field sync. Unit: dB.
  double snr_db;
  // Reed-Solomon words good.
  int64_t frames_ok;
  // Reed-Solomon words that failed.
  int64_t frames_bad;
  // The quality of the segment sync correlation, 0 to 1.
  double segment_correlation;
  // True if `field_parity` holds a value.
  bool has_field_parity;
  // The field parity, 1 or 2. No value without the field sync.
  int64_t field_parity;
  // True if `data_snr_db` holds a value.
  bool has_data_snr_db;
  // The SNR of the data symbols against the decoded trellis path. No value before a field decodes. Unit: dB.
  double data_snr_db;
  // True if `pilot_db` holds a value.
  bool has_pilot_db;
  // The pilot level over the data level. No value before the pilot locks. Unit: dB.
  double pilot_db;
  // Fields decoded.
  int64_t fields;
  // Reed-Solomon words clean.
  int64_t rs_clean;
  // Reed-Solomon words corrected.
  int64_t rs_corrected;
  // Reed-Solomon words that failed.
  int64_t rs_failed;
  // Packets delivered.
  int64_t packets;
  // Field syncs expected but not found.
  int64_t field_sync_misses;
  // Times the field sync was lost.
  int64_t lock_losses;
  // The signal quality score. Unit: %.
  double quality_pct;
  // The signal quality class. A value of `sdrkit_atsc_fields_quality`.
  int32_t quality;
  // The pilot step. A value of `sdrkit_atsc_fields_step_pilot`.
  int32_t step_pilot;
  // The segment sync step. A value of `sdrkit_atsc_fields_step_segment`.
  int32_t step_segment;
  // The field sync step. A value of `sdrkit_atsc_fields_step_field`.
  int32_t step_field;
  // The equaliser step. A value of `sdrkit_atsc_fields_step_equaliser`.
  int32_t step_equaliser;
  // The trellis step. A value of `sdrkit_atsc_fields_step_trellis`.
  int32_t step_trellis;
  // The Reed-Solomon step. A value of `sdrkit_atsc_fields_step_rs`.
  int32_t step_rs;
  // The transport stream step. A value of `sdrkit_atsc_fields_step_ts`.
  int32_t step_ts;
} sdrkit_atsc_fields;

// The options of `system_bandwidth` of `sdrkit_atsc3_fields`.
typedef enum sdrkit_atsc3_fields_system_bandwidth {
  // 6 MHz
  SDRKIT_ATSC3_FIELDS_SYSTEM_BANDWIDTH_6_MHZ = 0,
  // 7 MHz
  SDRKIT_ATSC3_FIELDS_SYSTEM_BANDWIDTH_7_MHZ = 1,
  // 8 MHz
  SDRKIT_ATSC3_FIELDS_SYSTEM_BANDWIDTH_8_MHZ = 2,
  // more than 8 MHz
  SDRKIT_ATSC3_FIELDS_SYSTEM_BANDWIDTH_MORE = 3,
} sdrkit_atsc3_fields_system_bandwidth;

// The options of `step_bootstrap` of `sdrkit_atsc3_fields`.
typedef enum sdrkit_atsc3_fields_step_bootstrap {
  // Off
  SDRKIT_ATSC3_FIELDS_STEP_BOOTSTRAP_OFF = 0,
  // Amber
  SDRKIT_ATSC3_FIELDS_STEP_BOOTSTRAP_AMBER = 1,
  // Green
  SDRKIT_ATSC3_FIELDS_STEP_BOOTSTRAP_GREEN = 2,
  // Red
  SDRKIT_ATSC3_FIELDS_STEP_BOOTSTRAP_RED = 3,
} sdrkit_atsc3_fields_step_bootstrap;

// The options of `step_l1` of `sdrkit_atsc3_fields`.
typedef enum sdrkit_atsc3_fields_step_l1 {
  // Off
  SDRKIT_ATSC3_FIELDS_STEP_L1_OFF = 0,
  // Amber
  SDRKIT_ATSC3_FIELDS_STEP_L1_AMBER = 1,
  // Green
  SDRKIT_ATSC3_FIELDS_STEP_L1_GREEN = 2,
  // Red
  SDRKIT_ATSC3_FIELDS_STEP_L1_RED = 3,
} sdrkit_atsc3_fields_step_l1;

// The options of `step_service` of `sdrkit_atsc3_fields`.
typedef enum sdrkit_atsc3_fields_step_service {
  // Off
  SDRKIT_ATSC3_FIELDS_STEP_SERVICE_OFF = 0,
  // Amber
  SDRKIT_ATSC3_FIELDS_STEP_SERVICE_AMBER = 1,
  // Green
  SDRKIT_ATSC3_FIELDS_STEP_SERVICE_GREEN = 2,
  // Red
  SDRKIT_ATSC3_FIELDS_STEP_SERVICE_RED = 3,
} sdrkit_atsc3_fields_step_service;

// The result fields of `atsc3`: the bootstrap, the frames, the service list and the media. A value of the bootstrap has no value before a bootstrap is found.
typedef struct sdrkit_atsc3_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A bootstrap was seen recently.
  bool bootstrap_seen;
  // Bootstraps found.
  int64_t bootstraps;
  // Frames taken.
  int64_t frames_taken;
  // Frames decoded.
  int64_t frames_ok;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last bootstrap. A frequency in Hz.
  double offset_hz;
  // True if `minor_version` holds a value.
  bool has_minor_version;
  // The minor version of the bootstrap.
  int64_t minor_version;
  // True if `system_bandwidth` holds a value.
  bool has_system_bandwidth;
  // The system bandwidth that the bootstrap signals. A value of `sdrkit_atsc3_fields_system_bandwidth`.
  int32_t system_bandwidth;
  // True if `preamble_structure` holds a value.
  bool has_preamble_structure;
  // The preamble structure, 0 to 159.
  int64_t preamble_structure;
  // True if `min_time_next` holds a value.
  bool has_min_time_next;
  // The minimum time to the next frame. A length of time in seconds.
  double min_time_next;
  // True if `post_rate` holds a value.
  bool has_post_rate;
  // The sample rate after the bootstrap. A frequency in Hz.
  double post_rate;
  // True if `ea_wake_up` holds a value.
  bool has_ea_wake_up;
  // The emergency alert wake-up.
  bool ea_wake_up;
  // True if `frame_length` holds a value.
  bool has_frame_length;
  // The time between the last two bootstraps. A length of time in seconds.
  double frame_length;
  // Service list tables read.
  int64_t lls_tables;
  // Signed multi-tables or signed signalling packages arrived in the last 10 s.
  bool signed_signalling;
  // True if `leap_seconds` holds a value.
  bool has_leap_seconds;
  // TAI minus UTC from the system time table. Unit: s.
  int64_t leap_seconds;
  // True if `service` holds a value.
  bool has_service;
  // The service chosen with `service.select`. No value for the automatic choice.
  int64_t service;
  // True if `service_received` holds a value.
  bool has_service_received;
  // The service received: the chosen one, or the first visible linear TV service delivered by ROUTE.
  int64_t service_received;
  // ROUTE objects taken.
  int64_t route_objects;
  // Media samples given out.
  int64_t media_samples;
  // The bootstrap step. A value of `sdrkit_atsc3_fields_step_bootstrap`.
  int32_t step_bootstrap;
  // The L1 step. A value of `sdrkit_atsc3_fields_step_l1`.
  int32_t step_l1;
  // The service signalling step. A value of `sdrkit_atsc3_fields_step_service`.
  int32_t step_service;
} sdrkit_atsc3_fields;

// The options of `not_decoded` of `sdrkit_atsc3_frame_fields`.
typedef enum sdrkit_atsc3_frame_fields_not_decoded {
  // reserved preamble structure
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_STRUCTURE = 0,
  // sample rate other than 6.144 Msps
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_SAMPLE_RATE = 1,
  // L1-Basic not read
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_L1_BASIC = 2,
  // L1-Detail not read
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_L1_DETAIL = 3,
  // L1-Detail blocks with additional parity
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_ADDITIONAL_PARITY = 4,
  // reserved value in the L1 signalling
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_SIGNALLING = 5,
  // MIMO
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_MIMO = 6,
  // MISO
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_MISO = 7,
  // tone reservation
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_TONE_RESERVATION = 8,
  // no PLP that the stack decodes
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_PLP = 9,
  // samples missing
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_SAMPLES = 10,
  // layered division multiplexing
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_LDM = 11,
  // subframe longer than 1 s
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_LENGTH = 12,
  // FEC blocks not decoded
  SDRKIT_ATSC3_FRAME_FIELDS_NOT_DECODED_FEC = 13,
} sdrkit_atsc3_frame_fields_not_decoded;

// The options of `step_plp` of `sdrkit_atsc3_frame_fields`.
typedef enum sdrkit_atsc3_frame_fields_step_plp {
  // Off
  SDRKIT_ATSC3_FRAME_FIELDS_STEP_PLP_OFF = 0,
  // Amber
  SDRKIT_ATSC3_FRAME_FIELDS_STEP_PLP_AMBER = 1,
  // Green
  SDRKIT_ATSC3_FRAME_FIELDS_STEP_PLP_GREEN = 2,
  // Red
  SDRKIT_ATSC3_FRAME_FIELDS_STEP_PLP_RED = 3,
} sdrkit_atsc3_frame_fields_step_plp;

// The result fields of `atsc3` about the frames: what the L1 signalling says, the PLP that is decoded and its FEC blocks. A value of the signalling has no value before L1-Basic is read, a value of the PLP none before a FEC block is decoded.
typedef struct sdrkit_atsc3_frame_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames that were not decoded.
  int64_t frames_failed;
  // True if `not_decoded` holds a value.
  bool has_not_decoded;
  // Why the last frame was not decoded. No value after a decoded frame. A value of `sdrkit_atsc3_frame_fields_not_decoded`.
  int32_t not_decoded;
  // True if `fft_size` holds a value.
  bool has_fft_size;
  // The FFT size of the Preamble and the first subframe, points.
  int64_t fft_size;
  // True if `guard_samples` holds a value.
  bool has_guard_samples;
  // The guard interval of the Preamble and the first subframe. Unit: samples.
  int64_t guard_samples;
  // True if `l1_basic_mode` holds a value.
  bool has_l1_basic_mode;
  // The L1-Basic mode that the bootstrap signals, 1 to 5.
  int64_t l1_basic_mode;
  // True if `l1_detail_mode` holds a value.
  bool has_l1_detail_mode;
  // The L1-Detail mode, 1 to 7.
  int64_t l1_detail_mode;
  // True if `preamble_symbols` holds a value.
  bool has_preamble_symbols;
  // The Preamble symbols of the frame.
  int64_t preamble_symbols;
  // True if `subframes` holds a value.
  bool has_subframes;
  // The subframes of the frame. The first one is decoded.
  int64_t subframes;
  // True if `subframe_symbols` holds a value.
  bool has_subframe_symbols;
  // The data and subframe boundary symbols of the first subframe.
  int64_t subframe_symbols;
  // True if `pilot_dx` holds a value.
  bool has_pilot_dx;
  // The spacing of the carriers that bear scattered pilots in the first subframe.
  int64_t pilot_dx;
  // True if `pilot_dy` holds a value.
  bool has_pilot_dy;
  // The symbols of one scattered pilot sequence in the first subframe.
  int64_t pilot_dy;
  // True if `plps` holds a value.
  bool has_plps;
  // The PLPs of the frame: the different PLP ids in all its subframes.
  int64_t plps;
  // True if `plp_id` holds a value.
  bool has_plp_id;
  // The id of the PLP that is decoded.
  int64_t plp_id;
  // True if `plp_points` holds a value.
  bool has_plp_points;
  // The constellation points of the PLP that is decoded: 4, 16, 64 or 256.
  int64_t plp_points;
  // True if `plp_code_rate` holds a value.
  bool has_plp_code_rate;
  // The code rate of the PLP that is decoded, in fifteenths. Unit: /15.
  int64_t plp_code_rate;
  // True if `plp_ldpc_bits` holds a value.
  bool has_plp_ldpc_bits;
  // The LDPC code word of the PLP that is decoded: 16200 or 64800 bits. Unit: bit.
  int64_t plp_ldpc_bits;
  // FEC blocks of the PLP that ended in the last frame.
  int64_t fec_blocks;
  // Those of them that decoded.
  int64_t fec_blocks_good;
  // Baseband packets given out.
  int64_t baseband_packets;
  // FEC blocks whose baseband packet was not good.
  int64_t baseband_packets_bad;
  // Bits that the outer BCH code corrected after the LDPC decoder.
  int64_t outer_corrected;
  // True if `ldpc_iterations` holds a value.
  bool has_ldpc_iterations;
  // The mean LDPC iterations of the FEC blocks of the last frame.
  double ldpc_iterations;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The MER of the cells of the decoded PLP against the nearest constellation points. Unit: dB.
  double mer_db;
  // True if `pilot_snr_db` holds a value.
  bool has_pilot_snr_db;
  // The signal to noise ratio that the pilots of the first subframe give. Unit: dB.
  double pilot_snr_db;
  // The PLP step: green when more than 99 % of the baseband packets are good, amber above 50 %, red below. A value of `sdrkit_atsc3_frame_fields_step_plp`.
  int32_t step_plp;
} sdrkit_atsc3_frame_fields;

// The options of `category` of `sdrkit_atsc3_service_row`.
typedef enum sdrkit_atsc3_service_row_category {
  // TV
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_TV = 0,
  // Radio
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_RADIO = 1,
  // App
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_APP = 2,
  // Guide
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_GUIDE = 3,
  // Alerts
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_ALERTS = 4,
  // DRM
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_DRM = 5,
  // Other
  SDRKIT_ATSC3_SERVICE_ROW_CATEGORY_OTHER = 6,
} sdrkit_atsc3_service_row_category;

// The options of `delivery` of `sdrkit_atsc3_service_row`.
typedef enum sdrkit_atsc3_service_row_delivery {
  // ROUTE
  SDRKIT_ATSC3_SERVICE_ROW_DELIVERY_ROUTE = 0,
  // MMTP
  SDRKIT_ATSC3_SERVICE_ROW_DELIVERY_MMTP = 1,
  // Other
  SDRKIT_ATSC3_SERVICE_ROW_DELIVERY_OTHER = 2,
} sdrkit_atsc3_service_row_delivery;

// The options of `action` of `sdrkit_atsc3_service_row`.
typedef enum sdrkit_atsc3_service_row_action {
  // Receive
  SDRKIT_ATSC3_SERVICE_ROW_ACTION_RECEIVE = 0,
  // Waiting for its signalling
  SDRKIT_ATSC3_SERVICE_ROW_ACTION_WAITING = 1,
  // Receiving
  SDRKIT_ATSC3_SERVICE_ROW_ACTION_RECEIVING = 2,
  // Not supported
  SDRKIT_ATSC3_SERVICE_ROW_ACTION_UNSUPPORTED = 3,
} sdrkit_atsc3_service_row_action;

// A row of the services of the service list. The row key is the service id; `service.select` takes it.
typedef struct sdrkit_atsc3_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The major channel number.
  int64_t major;
  // The minor channel number.
  int64_t minor;
  // The short name.
  sdrkit_string name;
  // The service is hidden.
  bool hidden;
  // The service id.
  int64_t id;
  // The category of the service. A value of `sdrkit_atsc3_service_row_category`.
  int32_t category;
  // How the signalling is delivered. A value of `sdrkit_atsc3_service_row_delivery`.
  int32_t delivery;
  // What the receiver does with the service. A value of `sdrkit_atsc3_service_row_action`.
  int32_t action;
  // Service layer signalling objects received on TSI 0 of the ROUTE session of the service.
  int64_t sls_objects;
  // Signalling documents (USBD, S-TSID, MPD and others) read from those objects.
  int64_t sls_documents;
  // Those objects that were signed packages. The signature is not verified.
  int64_t sls_signed_packages;
} sdrkit_atsc3_service_row;

// A row of the media components seen, in the order they appeared. The row key is the ROUTE session.
typedef struct sdrkit_atsc3_component_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The ROUTE session of the component.
  int64_t tsi;
  // The four-character code of the codec of the component.
  sdrkit_string fourcc;
} sdrkit_atsc3_component_row;

// A row of the PLPs of the last frame: one row for each PLP of each subframe. The row key is the subframe number, a point and the PLP id.
typedef struct sdrkit_atsc3_plp_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PLP id.
  int64_t id;
  // The layer: 0 core, above 0 enhanced.
  int64_t layer;
  // True if `points` holds a value.
  bool has_points;
  // The constellation points, also of a coding that the stack does not decode. No value for a reserved code.
  int64_t points;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The code rate in fifteenths. Unit: /15.
  int64_t code_rate;
  // True if `ldpc_bits` holds a value.
  bool has_ldpc_bits;
  // The LDPC code word, bits. Unit: bit.
  int64_t ldpc_bits;
  // The data cells of the PLP in the subframe.
  int64_t cells;
  // The time interleaver: 0 none, 1 convolutional, 2 hybrid.
  int64_t time_interleaver;
  // The PLP is the one that the stack decodes.
  bool decoded;
  // The number of the subframe in the frame, from 0.
  int64_t subframe;
  // True if `bonding` holds a value.
  bool has_bonding;
  // The channel bonding format (table 9.25): 0 plain, 1 SNR averaging, 2 and 3 reserved. No value when the PLP is not bonded with another RF channel.
  int64_t bonding;
  // True if `bonded_bsid` holds a value.
  bool has_bonded_bsid;
  // The BSID of the RF channel that the PLP is bonded with, as L1-Detail lists it.
  int64_t bonded_bsid;
} sdrkit_atsc3_plp_row;

// The options of `header` of `sdrkit_dtmb_fields`.
typedef enum sdrkit_dtmb_fields_header {
  // PN420
  SDRKIT_DTMB_FIELDS_HEADER_PN420 = 0,
  // PN595
  SDRKIT_DTMB_FIELDS_HEADER_PN595 = 1,
  // PN945
  SDRKIT_DTMB_FIELDS_HEADER_PN945 = 2,
} sdrkit_dtmb_fields_header;

// The options of `step_pn_sync` of `sdrkit_dtmb_fields`.
typedef enum sdrkit_dtmb_fields_step_pn_sync {
  // Off
  SDRKIT_DTMB_FIELDS_STEP_PN_SYNC_OFF = 0,
  // Amber
  SDRKIT_DTMB_FIELDS_STEP_PN_SYNC_AMBER = 1,
  // Green
  SDRKIT_DTMB_FIELDS_STEP_PN_SYNC_GREEN = 2,
  // Red
  SDRKIT_DTMB_FIELDS_STEP_PN_SYNC_RED = 3,
} sdrkit_dtmb_fields_step_pn_sync;

// The options of `mapping` of `sdrkit_dtmb_fields`.
typedef enum sdrkit_dtmb_fields_mapping {
  // 4QAM-NR
  SDRKIT_DTMB_FIELDS_MAPPING_4QAM_NR = 0,
  // 4QAM
  SDRKIT_DTMB_FIELDS_MAPPING_4QAM = 1,
  // 16QAM
  SDRKIT_DTMB_FIELDS_MAPPING_16QAM = 2,
  // 32QAM
  SDRKIT_DTMB_FIELDS_MAPPING_32QAM = 3,
  // 64QAM
  SDRKIT_DTMB_FIELDS_MAPPING_64QAM = 4,
} sdrkit_dtmb_fields_mapping;

// The options of `code_rate` of `sdrkit_dtmb_fields`.
typedef enum sdrkit_dtmb_fields_code_rate {
  // 0.4
  SDRKIT_DTMB_FIELDS_CODE_RATE_0_4 = 0,
  // 0.6
  SDRKIT_DTMB_FIELDS_CODE_RATE_0_6 = 1,
  // 0.8
  SDRKIT_DTMB_FIELDS_CODE_RATE_0_8 = 2,
} sdrkit_dtmb_fields_code_rate;

// The options of `interleaver` of `sdrkit_dtmb_fields`.
typedef enum sdrkit_dtmb_fields_interleaver {
  // mode 1 (M = 240)
  SDRKIT_DTMB_FIELDS_INTERLEAVER_MODE_1 = 0,
  // mode 2 (M = 720)
  SDRKIT_DTMB_FIELDS_INTERLEAVER_MODE_2 = 1,
} sdrkit_dtmb_fields_interleaver;

// The options of `carriers` of `sdrkit_dtmb_fields`.
typedef enum sdrkit_dtmb_fields_carriers {
  // 3780 (multi-carrier)
  SDRKIT_DTMB_FIELDS_CARRIERS_3780 = 0,
  // 1 (single carrier)
  SDRKIT_DTMB_FIELDS_CARRIERS_1 = 1,
} sdrkit_dtmb_fields_carriers;

// The result fields of `dtmb`: the frame headers, the system information, the decoder and the signal. A value of the header has no value while no header is tracked, and a value of the profile none before the system information is decoded.
typedef struct sdrkit_dtmb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Detection level: 0 nothing, 1 frame headers tracked, 2 system information decoded, 3 transport stream locked.
  int64_t detection;
  // The header grid holds.
  bool tracking;
  // True if `header` holds a value.
  bool has_header;
  // The frame header. A value of `sdrkit_dtmb_fields_header`.
  int32_t header;
  // True if `pn_phase_rotates` holds a value.
  bool has_pn_phase_rotates;
  // The phase of the header changes from frame to frame.
  bool pn_phase_rotates;
  // The channel width. A frequency in Hz.
  double width;
  // True if `frame_length` holds a value.
  bool has_frame_length;
  // The length of a signal frame (header and body). A length of time in seconds.
  double frame_length;
  // True if `cn_db` holds a value.
  bool has_cn_db;
  // C/N from the fit of the header. Unit: dB.
  double cn_db;
  // True if `echo_span` holds a value.
  bool has_echo_span;
  // The time between the first and the last significant path. A length of time in seconds.
  double echo_span;
  // Frames with a usable header.
  int64_t frames;
  // Frames erased: their header window was exact silence (a zero-filled gap of the source).
  int64_t frames_erased;
  // The share of recent frames without a usable header. Unit: %.
  double no_header_pct;
  // The input rate is enough for the width.
  bool rate_ok;
  // The header sync step. A value of `sdrkit_dtmb_fields_step_pn_sync`.
  int32_t step_pn_sync;
  // True if `si_index` holds a value.
  bool has_si_index;
  // The system information index, 3 to 24.
  int64_t si_index;
  // True if `si_corr` holds a value.
  bool has_si_corr;
  // The normalised score of the leading system information word (1 is perfect). No value while no header is tracked.
  double si_corr;
  // True if `mapping` holds a value.
  bool has_mapping;
  // The mapping. A value of `sdrkit_dtmb_fields_mapping`.
  int32_t mapping;
  // True if `code_rate` holds a value.
  bool has_code_rate;
  // The LDPC rate. A value of `sdrkit_dtmb_fields_code_rate`.
  int32_t code_rate;
  // True if `interleaver` holds a value.
  bool has_interleaver;
  // The time interleaver. A value of `sdrkit_dtmb_fields_interleaver`.
  int32_t interleaver;
  // True if `carriers` holds a value.
  bool has_carriers;
  // The carrier form. A value of `sdrkit_dtmb_fields_carriers`.
  int32_t carriers;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The MER of the equalised data symbols. Unit: dB.
  double mer_db;
  // LDPC code words that decoded.
  int64_t ldpc_ok;
  // LDPC code words that did not.
  int64_t ldpc_bad;
  // The mean LDPC iterations of recent code words.
  double ldpc_iterations;
  // BCH blocks with one bit corrected.
  int64_t bch_corrected;
  // Code words dropped for lack of time.
  int64_t codewords_dropped;
  // Code words not tried during a long run of failures.
  int64_t codewords_skipped;
  // Transport packets delivered.
  int64_t packets;
  // A good packet came out within the last 0.7 s.
  bool ts_lock;
  // True if `payload_bps` holds a value.
  bool has_payload_bps;
  // The net bit rate of the signalled profile. No value before the profile and the header are known. Unit: bit/s.
  double payload_bps;
  // What the stack decodes and which of its tables no public document confirms.
  sdrkit_string note;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. No value while no header is tracked. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock. No value while no header is tracked. Unit: ppm.
  double clock_ppm;
  // The level of the input. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR: the MER once the profile is known and it lies between 8 dB and the C/N, else the C/N. Unit: dB.
  double snr_db;
} sdrkit_dtmb_fields;

// The options of `standard` of `sdrkit_tv_auto_fields`.
typedef enum sdrkit_tv_auto_fields_standard {
  // No TV signal
  SDRKIT_TV_AUTO_FIELDS_STANDARD_NONE = 0,
  // Unknown
  SDRKIT_TV_AUTO_FIELDS_STANDARD_UNKNOWN = 1,
  // DVB-T
  SDRKIT_TV_AUTO_FIELDS_STANDARD_DVBT = 2,
  // DVB-T2
  SDRKIT_TV_AUTO_FIELDS_STANDARD_DVBT2 = 3,
  // ISDB-T
  SDRKIT_TV_AUTO_FIELDS_STANDARD_ISDBT = 4,
  // ATSC
  SDRKIT_TV_AUTO_FIELDS_STANDARD_ATSC = 5,
  // ATSC 3.0
  SDRKIT_TV_AUTO_FIELDS_STANDARD_ATSC3 = 6,
  // DTMB
  SDRKIT_TV_AUTO_FIELDS_STANDARD_DTMB = 7,
  // DVB-C
  SDRKIT_TV_AUTO_FIELDS_STANDARD_DVBC = 8,
  // DVB-S/S2
  SDRKIT_TV_AUTO_FIELDS_STANDARD_DVBS = 9,
  // Analog
  SDRKIT_TV_AUTO_FIELDS_STANDARD_ANALOG = 10,
} sdrkit_tv_auto_fields_standard;

// The options of `verdict` of `sdrkit_tv_auto_fields`.
typedef enum sdrkit_tv_auto_fields_verdict {
  // No signal
  SDRKIT_TV_AUTO_FIELDS_VERDICT_NO_SIGNAL = 0,
  // Unsure
  SDRKIT_TV_AUTO_FIELDS_VERDICT_UNSURE = 1,
  // Likely
  SDRKIT_TV_AUTO_FIELDS_VERDICT_LIKELY = 2,
  // Sure
  SDRKIT_TV_AUTO_FIELDS_VERDICT_SURE = 3,
} sdrkit_tv_auto_fields_verdict;

// The result fields of `tv-auto`: the line shown, the capture and the last detection. A value of the detection has no value before the first detection.
typedef struct sdrkit_tv_auto_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The line shown ("Listening to the channel...").
  sdrkit_string line;
  // The progress of the capture. Unit: %.
  double progress_pct;
  // Captures looked at.
  int64_t tries;
  // True if `switch_to` holds a value.
  bool has_switch_to;
  // The id of the stack requested, when one is found.
  sdrkit_string switch_to;
  // True if `standard` holds a value.
  bool has_standard;
  // The standard of the last detection. A value of `sdrkit_tv_auto_fields_standard`.
  int32_t standard;
  // True if `headline` holds a value.
  bool has_headline;
  // The headline of the last detection.
  sdrkit_string headline;
  // True if `verdict` holds a value.
  bool has_verdict;
  // The verdict of the last detection. A value of `sdrkit_tv_auto_fields_verdict`.
  int32_t verdict;
  // True if `confidence_pct` holds a value.
  bool has_confidence_pct;
  // The confidence of the last detection. Unit: %.
  double confidence_pct;
  // True if `width` holds a value.
  bool has_width;
  // The channel width found. A frequency in Hz.
  double width;
  // True if `centre_offset` holds a value.
  bool has_centre_offset;
  // The centre of the channel from the centre of the capture. A frequency in Hz.
  double centre_offset;
  // The detector stopped: the command `try_again` starts it again.
  bool try_again_offered;
} sdrkit_tv_auto_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_tv_auto_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_tv_auto_decoder_fields;

// The options of `urgency` of `sdrkit_isdbt_tmcc_event`.
typedef enum sdrkit_isdbt_tmcc_event_urgency {
  // None
  SDRKIT_ISDBT_TMCC_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_ISDBT_TMCC_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_ISDBT_TMCC_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_ISDBT_TMCC_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_isdbt_tmcc_event_urgency;

// The options of `family` of `sdrkit_isdbt_tmcc_event`.
typedef enum sdrkit_isdbt_tmcc_event_family {
  // Aircraft
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ISDBT_TMCC_EVENT_FAMILY_OTHER = 12,
} sdrkit_isdbt_tmcc_event_family;

// The fields of the event `isdbt.tmcc`.
typedef struct sdrkit_isdbt_tmcc_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The system identification (0 ISDB-T, 1 ISDB-TSB).
  int64_t system_id;
  // The parameter switching countdown (15: no switch announced).
  int64_t switch_countdown;
  // The emergency alarm start flag.
  bool emergency_alarm;
  // The partial reception flag.
  bool partial_reception;
  // The layers in use ("A 1 QPSK 2/3, B 12 64QAM 3/4").
  sdrkit_string text;
  // An emergency when the emergency alarm start flag is set. A value of `sdrkit_isdbt_tmcc_event_urgency`.
  int32_t urgency;
  // The family of the source: a broadcast. A value of `sdrkit_isdbt_tmcc_event_family`.
  int32_t family;
} sdrkit_isdbt_tmcc_event;

// The options of `standard` of `sdrkit_tv_auto_found_event`.
typedef enum sdrkit_tv_auto_found_event_standard {
  // No TV signal
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_NONE = 0,
  // Unknown
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_UNKNOWN = 1,
  // DVB-T
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_DVBT = 2,
  // DVB-T2
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_DVBT2 = 3,
  // ISDB-T
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_ISDBT = 4,
  // ATSC
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_ATSC = 5,
  // ATSC 3.0
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_ATSC3 = 6,
  // DTMB
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_DTMB = 7,
  // DVB-C
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_DVBC = 8,
  // DVB-S/S2
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_DVBS = 9,
  // Analog
  SDRKIT_TV_AUTO_FOUND_EVENT_STANDARD_ANALOG = 10,
} sdrkit_tv_auto_found_event_standard;

// The options of `family` of `sdrkit_tv_auto_found_event`.
typedef enum sdrkit_tv_auto_found_event_family {
  // Aircraft
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TV_AUTO_FOUND_EVENT_FAMILY_OTHER = 12,
} sdrkit_tv_auto_found_event_family;

// The fields of a standard found. The text is the headline.
typedef struct sdrkit_tv_auto_found_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The id of the stack that receives the standard found.
  sdrkit_string stack;
  // The standard found. A value of `sdrkit_tv_auto_found_event_standard`.
  int32_t standard;
  // The headline.
  sdrkit_string text;
  // The channel width found. A frequency in Hz.
  double width;
  // The detector is sure.
  bool sure;
  // The family of the source: a receiver. A value of `sdrkit_tv_auto_found_event_family`.
  int32_t family;
} sdrkit_tv_auto_found_event;

// The arguments of `service.select`.
typedef struct sdrkit_tvterr_service_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the service in `services`. Empty: the first service that can play.
  sdrkit_string row;
} sdrkit_tvterr_service_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_TVTERR_SERVICE_SELECT_ARGS_INIT {sizeof(sdrkit_tvterr_service_select_args), {0}}

// The arguments of `track.select`.
typedef struct sdrkit_tvterr_track_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the track in `tracks`.
  sdrkit_string row;
} sdrkit_tvterr_track_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_TVTERR_TRACK_SELECT_ARGS_INIT {sizeof(sdrkit_tvterr_track_select_args), {0}}

// Fills `rows` with the changed rows of the table "layers" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_layers_rows(SDRKIT_HANDLE handle,
                                              sdrkit_isdbt_layer_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tvterr.IsdbtLayerRow", "layers", rows, sizeof(sdrkit_isdbt_layer_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_services_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mpegts_ts_service_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_tracks_rows(SDRKIT_HANDLE handle,
                                              sdrkit_mpegts_ts_track_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_guide_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_guide_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_guide_rating_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_mpegts_ts_pid_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "isdbt" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbt_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_section_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "isdbt.tmcc"
// (TMCC). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_isdbt_tmcc_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_isdbt_tmcc_event *event) {
  return sdrkit_typed_event(handle, n, "isdbt.tmcc", "tvterr.IsdbtTmccEvent", event, sizeof(sdrkit_isdbt_tmcc_event));
}

// Runs the command "service.select" (Select a service) of the stack "isdbt" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_isdbt_service_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "tvterr.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "isdbt" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_isdbt_track_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "tvterr.TrackSelectArgs", args, NULL, NULL, 0);
}

// The setting "width" (Channel width) of the stack "isdbt". A value of `sdrkit_isdbt_width`.
static inline int32_t sdrkit_isdbt_get_width(SDRKIT_HANDLE handle, sdrkit_isdbt_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbt_width)index;
  return code;
}
static inline int32_t sdrkit_isdbt_set_width(SDRKIT_HANDLE handle, sdrkit_isdbt_width value) {
  return sdrkit_typed_setting_set_enum(handle, "width", (int32_t)value);
}

// The setting "decode_video" (Decode video) of the stack "isdbt". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_isdbt_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_isdbt_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// Fills `rows` with the changed rows of the table "layers" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_layers_rows(SDRKIT_HANDLE handle,
                                                sdrkit_isdbt_layer_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tvterr.IsdbtLayerRow", "layers", rows, sizeof(sdrkit_isdbt_layer_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "services" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_services_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mpegts_ts_service_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_tracks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mpegts_ts_track_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_guide_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mpegts_ts_guide_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_guide_rating_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_mpegts_ts_pid_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "isdbtsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_isdbtsb_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_mpegts_ts_section_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "isdbtsb" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_isdbtsb_service_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "tvterr.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "isdbtsb" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_isdbtsb_track_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "tvterr.TrackSelectArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "isdbtsb" are the sdrkit_isdbtsb_gen_*() functions
// after the settings of its receiver.

// The setting "width" (Channel width) of the stack "isdbtsb". A value of `sdrkit_isdbtsb_width`.
static inline int32_t sdrkit_isdbtsb_get_width(SDRKIT_HANDLE handle, sdrkit_isdbtsb_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_width)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_set_width(SDRKIT_HANDLE handle, sdrkit_isdbtsb_width value) {
  return sdrkit_typed_setting_set_enum(handle, "width", (int32_t)value);
}

// The setting "segments" (Segments) of the stack "isdbtsb". A value of `sdrkit_isdbtsb_segments`.
static inline int32_t sdrkit_isdbtsb_get_segments(SDRKIT_HANDLE handle, sdrkit_isdbtsb_segments *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "segments", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_segments)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_set_segments(SDRKIT_HANDLE handle, sdrkit_isdbtsb_segments value) {
  return sdrkit_typed_setting_set_enum(handle, "segments", (int32_t)value);
}

// The setting "edge_pilot" (Upper-edge pilot) of the stack "isdbtsb". A value of `sdrkit_isdbtsb_edge_pilot`.
static inline int32_t sdrkit_isdbtsb_get_edge_pilot(SDRKIT_HANDLE handle, sdrkit_isdbtsb_edge_pilot *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "edge_pilot", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_edge_pilot)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_set_edge_pilot(SDRKIT_HANDLE handle, sdrkit_isdbtsb_edge_pilot value) {
  return sdrkit_typed_setting_set_enum(handle, "edge_pilot", (int32_t)value);
}

// The setting "decode_video" (Decode video) of the stack "isdbtsb". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_isdbtsb_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_isdbtsb_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// The setting "gen_mode" (Test signal mode) of the test signal generator of the stack "isdbtsb". A value of `sdrkit_isdbtsb_gen_mode`.
static inline int32_t sdrkit_isdbtsb_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_guard" (Test signal guard) of the test signal generator of the stack "isdbtsb". A value of `sdrkit_isdbtsb_gen_guard`.
static inline int32_t sdrkit_isdbtsb_gen_get_guard(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_guard *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_gen_guard)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_gen_set_guard(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_guard value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_guard", (int32_t)value);
}

// The setting "gen_modulation" (Test signal layer B modulation) of the test signal generator of the stack "isdbtsb". A value of `sdrkit_isdbtsb_gen_modulation`.
static inline int32_t sdrkit_isdbtsb_gen_get_modulation(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_modulation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_gen_modulation)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_gen_set_modulation(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_modulation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_modulation", (int32_t)value);
}

// The setting "gen_code_rate" (Test signal layer B code rate) of the test signal generator of the stack "isdbtsb". A value of `sdrkit_isdbtsb_gen_code_rate`.
static inline int32_t sdrkit_isdbtsb_gen_get_code_rate(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_code_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_code_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_isdbtsb_gen_code_rate)index;
  return code;
}
static inline int32_t sdrkit_isdbtsb_gen_set_code_rate(SDRKIT_HANDLE handle, sdrkit_isdbtsb_gen_code_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_code_rate", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "atsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc_services_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mpegts_ts_service_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "atsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc_tracks_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_track_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "atsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc_guide_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mpegts_ts_guide_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "atsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_guide_rating_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "atsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_pid_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "atsc" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_ts_section_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "atsc" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_atsc_service_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "tvterr.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "atsc" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_atsc_track_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "tvterr.TrackSelectArgs", args, NULL, NULL, 0);
}

// The setting "decode_video" (Decode video) of the stack "atsc". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_atsc_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_atsc_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "atsc3" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_atsc3_services_rows(SDRKIT_HANDLE handle,
                                                sdrkit_atsc3_service_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tvterr.Atsc3ServiceRow", "services", rows, sizeof(sdrkit_atsc3_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "components" of the stack "atsc3" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc3_components_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_atsc3_component_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tvterr.Atsc3ComponentRow", "components", rows, sizeof(sdrkit_atsc3_component_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "plps" of the stack "atsc3" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_atsc3_plps_rows(SDRKIT_HANDLE handle,
                                            sdrkit_atsc3_plp_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "tvterr.Atsc3PlpRow", "plps", rows, sizeof(sdrkit_atsc3_plp_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "atsc3" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_atsc3_service_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "tvterr.ServiceSelectArgs", args, NULL, NULL, 0);
}

// The setting "plp" (PLP) of the stack "atsc3". The id of the PLP to decode. -1 takes the first core PLP with low level signalling that the stack decodes, or without one the first core PLP that it decodes.
static inline int32_t sdrkit_atsc3_get_plp(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "plp", value);
}
static inline int32_t sdrkit_atsc3_set_plp(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "plp", value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "dtmb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dtmb_services_rows(SDRKIT_HANDLE handle,
                                               sdrkit_mpegts_ts_service_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsServiceRow", "services", rows, sizeof(sdrkit_mpegts_ts_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tracks" of the stack "dtmb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dtmb_tracks_rows(SDRKIT_HANDLE handle,
                                             sdrkit_mpegts_ts_track_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsTrackRow", "tracks", rows, sizeof(sdrkit_mpegts_ts_track_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide" of the stack "dtmb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dtmb_guide_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mpegts_ts_guide_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRow", "guide", rows, sizeof(sdrkit_mpegts_ts_guide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_ratings" of the stack "dtmb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dtmb_guide_ratings_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_guide_rating_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsGuideRatingRow", "guide_ratings", rows, sizeof(sdrkit_mpegts_ts_guide_rating_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ts_demux.pids" of the stack "dtmb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dtmb_ts_demux_pids_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_mpegts_ts_pid_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsPidRow", "ts_demux.pids", rows, sizeof(sdrkit_mpegts_ts_pid_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "psi_si.sections" of the stack "dtmb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dtmb_psi_si_sections_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_mpegts_ts_section_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "mpegts.TsSectionRow", "psi_si.sections", rows, sizeof(sdrkit_mpegts_ts_section_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "dtmb" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_dtmb_service_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "tvterr.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "track.select" (Select a track) of the stack "dtmb" on the session of `handle`. Plays an audio track or shows a subtitle track.
static inline int32_t sdrkit_dtmb_track_select(SDRKIT_HANDLE handle, const sdrkit_tvterr_track_select_args *args) {
  return sdrkit_typed_command(handle, "track.select", "tvterr.TrackSelectArgs", args, NULL, NULL, 0);
}

// The setting "width" (Channel) of the stack "dtmb". A value of `sdrkit_dtmb_width`. A stack set to the wrong width finds nothing.
static inline int32_t sdrkit_dtmb_get_width(SDRKIT_HANDLE handle, sdrkit_dtmb_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dtmb_width)index;
  return code;
}
static inline int32_t sdrkit_dtmb_set_width(SDRKIT_HANDLE handle, sdrkit_dtmb_width value) {
  return sdrkit_typed_setting_set_enum(handle, "width", (int32_t)value);
}

// The setting "decode_video" (Decode video) of the stack "dtmb". MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_dtmb_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_dtmb_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tv-auto.found"
// (TV standard found). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tv_auto_found_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tv_auto_found_event *event) {
  return sdrkit_typed_event(handle, n, "tv-auto.found", "tvterr.AutoFoundEvent", event, sizeof(sdrkit_tv_auto_found_event));
}

// Runs the command "try_again" (Clear) of the stack "tv-auto" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_tv_auto_try_again(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "try_again", NULL, NULL, NULL, NULL, 0);
}

// The setting "satellite" (Satellite input) of the stack "tv-auto". The input comes from a dish (an LNB).
static inline int32_t sdrkit_tv_auto_get_satellite(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "satellite", value);
}
static inline int32_t sdrkit_tv_auto_set_satellite(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "satellite", value);
}

// The setting "decode_video" (Decode video) of the stack "tv-auto". The stack that TV (auto) switches on sends MPEG-2 video as frames of raw pixels (I420) in display order. H.264 and HEVC always leave as coded pictures.
static inline int32_t sdrkit_tv_auto_get_decode_video(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_video", value);
}
static inline int32_t sdrkit_tv_auto_set_decode_video(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_video", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_isdbt_fields_read(SDRKIT_HANDLE handle, sdrkit_isdbt_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.IsdbtFields", fields, sizeof(sdrkit_isdbt_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_isdbt_narrow_fields_read(SDRKIT_HANDLE handle, sdrkit_isdbt_narrow_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.IsdbtNarrowFields", fields, sizeof(sdrkit_isdbt_narrow_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_atsc_fields_read(SDRKIT_HANDLE handle, sdrkit_atsc_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.AtscFields", fields, sizeof(sdrkit_atsc_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_atsc3_fields_read(SDRKIT_HANDLE handle, sdrkit_atsc3_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.Atsc3Fields", fields, sizeof(sdrkit_atsc3_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_atsc3_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_atsc3_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.Atsc3FrameFields", fields, sizeof(sdrkit_atsc3_frame_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dtmb_fields_read(SDRKIT_HANDLE handle, sdrkit_dtmb_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.DtmbFields", fields, sizeof(sdrkit_dtmb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tv_auto_fields_read(SDRKIT_HANDLE handle, sdrkit_tv_auto_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.AutoFields", fields, sizeof(sdrkit_tv_auto_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tv_auto_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_tv_auto_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "tvterr.DecoderFields", fields, sizeof(sdrkit_tv_auto_decoder_fields));
}

// Fills `fields` with the result fields of the component "dc" of the stack "isdbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dc.<key>".
static inline int32_t sdrkit_isdbt_dc_removal_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_dc_removal_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.DcRemovalFields@dc", fields, sizeof(sdrkit_graph_dc_removal_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "isdbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_isdbt_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "isdbt" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_isdbt_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "dc" of the stack "isdbtsb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dc.<key>".
static inline int32_t sdrkit_isdbtsb_dc_removal_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_dc_removal_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.DcRemovalFields@dc", fields, sizeof(sdrkit_graph_dc_removal_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "isdbtsb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_isdbtsb_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "isdbtsb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_isdbtsb_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "atsc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_atsc_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "atsc" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_atsc_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "alp" of the stack "atsc3" in the snapshot of
// `handle`. A generic consumer reads them under the keys "alp.<key>".
static inline int32_t sdrkit_atsc3_alp_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_alp_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.AlpFields@alp", fields, sizeof(sdrkit_ipcast_alp_fields));
}

// Fills `fields` with the result fields of the component "ip" of the stack "atsc3" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ip.<key>".
static inline int32_t sdrkit_atsc3_ip_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_ip_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.IpFields@ip", fields, sizeof(sdrkit_ipcast_ip_fields));
}

// Fills `fields` with the result fields of the component "route" of the stack "atsc3" in the snapshot of
// `handle`. A generic consumer reads them under the keys "route.<key>".
static inline int32_t sdrkit_atsc3_route_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_route_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.RouteFields@route", fields, sizeof(sdrkit_ipcast_route_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "dtmb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_dtmb_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "dtmb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_dtmb_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

#endif
