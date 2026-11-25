/*
 * sdrkit: the typed interface of the stack family "general", version 2.0. MIT licence.
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

#ifndef SDRKIT_GENERAL_H
#define SDRKIT_GENERAL_H

#include "../sdrkit.h"
#include "tones.h"

// The version of the family that this header describes.
#define SDRKIT_GENERAL_VERSION_MAJOR 2
#define SDRKIT_GENERAL_VERSION_MINOR 0

// The stack "general": its id, and the keys of its event types.
#define SDRKIT_STACK_GENERAL "general"

// The options of the setting "demod" of the stack "general".
typedef enum sdrkit_general_demod {
  // NFM
  SDRKIT_GENERAL_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_DEMOD_CW = 5,
} sdrkit_general_demod;

// The options of the setting "squelch" of the stack "general".
typedef enum sdrkit_general_squelch {
  // Off
  SDRKIT_GENERAL_SQUELCH_OFF = 0,
  // Auto
  SDRKIT_GENERAL_SQUELCH_AUTO = 1,
  // Level
  SDRKIT_GENERAL_SQUELCH_LEVEL = 2,
} sdrkit_general_squelch;

// The options of the setting "deemph_nfm" of the stack "general".
typedef enum sdrkit_general_deemph_nfm {
  // Off
  SDRKIT_GENERAL_DEEMPH_NFM_OFF = 0,
  // 50 µs
  SDRKIT_GENERAL_DEEMPH_NFM_50_US = 1,
  // 75 µs
  SDRKIT_GENERAL_DEEMPH_NFM_75_US = 2,
  // 750 µs
  SDRKIT_GENERAL_DEEMPH_NFM_750_US = 3,
} sdrkit_general_deemph_nfm;

// The options of the setting "deemph_wfm" of the stack "general".
typedef enum sdrkit_general_deemph_wfm {
  // Off
  SDRKIT_GENERAL_DEEMPH_WFM_OFF = 0,
  // 50 µs
  SDRKIT_GENERAL_DEEMPH_WFM_50_US = 1,
  // 75 µs
  SDRKIT_GENERAL_DEEMPH_WFM_75_US = 2,
  // 750 µs
  SDRKIT_GENERAL_DEEMPH_WFM_750_US = 3,
} sdrkit_general_deemph_wfm;

// The options of the setting "nfm_lowpass" of the stack "general".
typedef enum sdrkit_general_nfm_lowpass {
  // 3 kHz
  SDRKIT_GENERAL_NFM_LOWPASS_3KHZ = 0,
  // 4 kHz
  SDRKIT_GENERAL_NFM_LOWPASS_4KHZ = 1,
} sdrkit_general_nfm_lowpass;

// The options of the setting "am_detector" of the stack "general".
typedef enum sdrkit_general_am_detector {
  // Auto
  SDRKIT_GENERAL_AM_DETECTOR_AUTO = 0,
  // Envelope
  SDRKIT_GENERAL_AM_DETECTOR_ENVELOPE = 1,
  // Synchronous
  SDRKIT_GENERAL_AM_DETECTOR_SYNCHRONOUS = 2,
  // Upper only
  SDRKIT_GENERAL_AM_DETECTOR_UPPER = 3,
  // Lower only
  SDRKIT_GENERAL_AM_DETECTOR_LOWER = 4,
} sdrkit_general_am_detector;

// The options of the setting "am_nr" of the stack "general".
typedef enum sdrkit_general_am_nr {
  // Auto
  SDRKIT_GENERAL_AM_NR_AUTO = 0,
  // Off
  SDRKIT_GENERAL_AM_NR_OFF = 1,
  // Gentle
  SDRKIT_GENERAL_AM_NR_GENTLE = 2,
  // Strong
  SDRKIT_GENERAL_AM_NR_STRONG = 3,
} sdrkit_general_am_nr;

// The options of the setting "extra1_demod" of the stack "general".
typedef enum sdrkit_general_extra1_demod {
  // NFM
  SDRKIT_GENERAL_EXTRA1_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_EXTRA1_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_EXTRA1_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_EXTRA1_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_EXTRA1_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_EXTRA1_DEMOD_CW = 5,
} sdrkit_general_extra1_demod;

// The options of the setting "extra2_demod" of the stack "general".
typedef enum sdrkit_general_extra2_demod {
  // NFM
  SDRKIT_GENERAL_EXTRA2_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_EXTRA2_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_EXTRA2_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_EXTRA2_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_EXTRA2_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_EXTRA2_DEMOD_CW = 5,
} sdrkit_general_extra2_demod;

// The options of the setting "extra3_demod" of the stack "general".
typedef enum sdrkit_general_extra3_demod {
  // NFM
  SDRKIT_GENERAL_EXTRA3_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_EXTRA3_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_EXTRA3_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_EXTRA3_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_EXTRA3_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_EXTRA3_DEMOD_CW = 5,
} sdrkit_general_extra3_demod;

// The options of the setting "tx_preemphasis" of the stack "general".
typedef enum sdrkit_general_tx_preemphasis {
  // Off
  SDRKIT_GENERAL_TX_PREEMPHASIS_OFF = 0,
  // 50 µs
  SDRKIT_GENERAL_TX_PREEMPHASIS_50_US = 1,
  // 75 µs
  SDRKIT_GENERAL_TX_PREEMPHASIS_75_US = 2,
  // 750 µs
  SDRKIT_GENERAL_TX_PREEMPHASIS_750_US = 3,
} sdrkit_general_tx_preemphasis;

// The options of the setting "tx_ssb_method" of the stack "general".
typedef enum sdrkit_general_tx_ssb_method {
  // Filter
  SDRKIT_GENERAL_TX_SSB_METHOD_FILTER = 0,
  // Phasing
  SDRKIT_GENERAL_TX_SSB_METHOD_PHASING = 1,
} sdrkit_general_tx_ssb_method;

// The options of the setting "tx_ssb_band" of the stack "general".
typedef enum sdrkit_general_tx_ssb_band {
  // 300 to 2700 Hz
  SDRKIT_GENERAL_TX_SSB_BAND_FIXED = 0,
  // As the filter
  SDRKIT_GENERAL_TX_SSB_BAND_FILTER = 1,
} sdrkit_general_tx_ssb_band;

// The options of the setting "gen_audio" of the stack "general".
typedef enum sdrkit_general_gen_audio {
  // Tone
  SDRKIT_GENERAL_GEN_AUDIO_TONE = 0,
  // Voice
  SDRKIT_GENERAL_GEN_AUDIO_VOICE = 1,
} sdrkit_general_gen_audio;

// The options of `am_detector` of `sdrkit_general_am_fields`.
typedef enum sdrkit_general_am_fields_am_detector {
  // Envelope
  SDRKIT_GENERAL_AM_FIELDS_AM_DETECTOR_ENVELOPE = 0,
  // Synchronous
  SDRKIT_GENERAL_AM_FIELDS_AM_DETECTOR_SYNCHRONOUS = 1,
  // Upper only
  SDRKIT_GENERAL_AM_FIELDS_AM_DETECTOR_UPPER = 2,
  // Lower only
  SDRKIT_GENERAL_AM_FIELDS_AM_DETECTOR_LOWER = 3,
} sdrkit_general_am_fields_am_detector;

// The result fields of the AM processor. The measurements have no value while the processor does not run.
typedef struct sdrkit_general_am_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the AM processor runs.
  bool am_active;
  // Whether it found a carrier.
  bool am_carrier_found;
  // Whether its loop is locked.
  bool am_locked;
  // True if `am_carrier_offset_hz` holds a value.
  bool has_am_carrier_offset_hz;
  // The carrier relative to the channel. No value without a carrier. A frequency in Hz.
  double am_carrier_offset_hz;
  // True if `am_carrier_dbfs` holds a value.
  bool has_am_carrier_dbfs;
  // The carrier level. Unit: dBFS.
  double am_carrier_dbfs;
  // True if `am_programme_snr_db` holds a value.
  bool has_am_programme_snr_db;
  // The programme SNR. Unit: dB.
  double am_programme_snr_db;
  // True if `am_detector` holds a value.
  bool has_am_detector;
  // The detector in use. A value of `sdrkit_general_am_fields_am_detector`.
  int32_t am_detector;
  // Whether one sideband was chosen because of interference.
  bool am_one_side;
  // True if `am_interference_upper` holds a value.
  bool has_am_interference_upper;
  // The interference measure above the carrier.
  double am_interference_upper;
  // True if `am_interference_lower` holds a value.
  bool has_am_interference_lower;
  // The interference measure below the carrier.
  double am_interference_lower;
  // True if `am_bandwidth_hz` holds a value.
  bool has_am_bandwidth_hz;
  // The audio bandwidth in use. A frequency in Hz.
  double am_bandwidth_hz;
  // True if `am_nr_floor_db` holds a value.
  bool has_am_nr_floor_db;
  // The noise reduction floor. No value with the noise reduction off. Unit: dB.
  double am_nr_floor_db;
  // True if `am_blanked_pct` holds a value.
  bool has_am_blanked_pct;
  // The share of samples the AM blanker removed. Unit: %.
  double am_blanked_pct;
  // True if `am_blanker_db` holds a value.
  bool has_am_blanker_db;
  // The threshold of the AM blanker. Unit: dB.
  double am_blanker_db;
  // True if `am_hum_hz` holds a value.
  bool has_am_hum_hz;
  // The hum notched (50 or 60 Hz). No value for none. A frequency in Hz.
  double am_hum_hz;
  // Whether the loop holds through a fade.
  bool am_fading;
  // Whether the synthetic treble is on.
  bool am_enhanced;
  // True if `am_leveller_db` holds a value.
  bool has_am_leveller_db;
  // The gain of the leveller. Unit: dB.
  double am_leveller_db;
  // True if `am_status` holds a value.
  bool has_am_status;
  // The state of the AM processor in words.
  sdrkit_string am_status;
} sdrkit_general_am_fields;

// The options of `demod` of `sdrkit_general_fields`.
typedef enum sdrkit_general_fields_demod {
  // NFM
  SDRKIT_GENERAL_FIELDS_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_FIELDS_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_FIELDS_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_FIELDS_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_FIELDS_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_FIELDS_DEMOD_CW = 5,
} sdrkit_general_fields_demod;

// The result fields of the report.
typedef struct sdrkit_general_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The demodulator. A value of `sdrkit_general_fields_demod`.
  int32_t demod;
  // The filter width. A frequency in Hz.
  double width_hz;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // The demodulation rate (48 kHz, WFM 240 kHz). A frequency in Hz.
  double demod_rate_hz;
  // The channel relative to the dial. A frequency in Hz.
  double channel_offset_hz;
  // The dial frequency. A frequency in Hz.
  double dial_hz;
  // The listening frequency: the dial plus the channel offset. A frequency in Hz.
  double listening_hz;
  // The low edge of the passband relative to the channel. A frequency in Hz.
  double filter_lo_hz;
  // The high edge of the passband relative to the channel. A frequency in Hz.
  double filter_hi_hz;
  // The low edge of the passband. A frequency in Hz.
  double passband_lo_hz;
  // The high edge of the passband. A frequency in Hz.
  double passband_hi_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level in the channel. No value before the first squelch block. Unit: dBFS.
  double level_dbfs;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // The noise in the channel. No value while the noise is not measured. Unit: dBFS.
  double noise_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR. No value while the noise is not measured. Unit: dB.
  double snr_db;
  // True if `squelch_threshold_dbfs` holds a value.
  bool has_squelch_threshold_dbfs;
  // The squelch threshold. No value with the squelch off or the noise not measured. Unit: dBFS.
  double squelch_threshold_dbfs;
  // Whether the squelch is open.
  bool squelch_open;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The frequency offset of an FM signal. No value with the squelch closed or another demodulator. A frequency in Hz.
  double offset_hz;
  // True if `audio_dbfs` holds a value.
  bool has_audio_dbfs;
  // The power of the audio played over the last report. No value without audio. Unit: dBFS.
  double audio_dbfs;
  // True if `blocks_open` holds a value.
  bool has_blocks_open;
  // 10 ms blocks with the squelch open. No value before the first squelch block.
  int64_t blocks_open;
  // True if `blocks_closed` holds a value.
  bool has_blocks_closed;
  // 10 ms blocks with the squelch closed. No value before the first squelch block.
  int64_t blocks_closed;
  // The share of samples the blanker removed. Unit: %.
  double blanked_pct;
  // The speech probability of the voice noise remover for the last 10 ms (0 to 1).
  double speech_probability;
  // True if `needle` holds a value.
  bool has_needle;
  // The position of the needle of the S-meter (0 to 1). No value without a level.
  double needle;
  // True if `ctcss_hz` holds a value.
  bool has_ctcss_hz;
  // The CTCSS tone found. A frequency in Hz.
  double ctcss_hz;
  // True if `ctcss_measured_hz` holds a value.
  bool has_ctcss_measured_hz;
  // The measured frequency of the CTCSS tone. A frequency in Hz.
  double ctcss_measured_hz;
  // True if `ctcss_db` holds a value.
  bool has_ctcss_db;
  // The level of the CTCSS tone against 5 kHz deviation. Unit: dB.
  double ctcss_db;
  // True if `dcs` holds a value.
  bool has_dcs;
  // The DCS code found ("D023N").
  sdrkit_string dcs;
  // True if `dcs_alias` holds a value.
  bool has_dcs_alias;
  // Another standard code that the same waveform reads as.
  sdrkit_string dcs_alias;
  // True if `tone` holds a value.
  bool has_tone;
  // The CTCSS tone or DCS code heard, as a label for lists of things heard ("CTCSS 88.5 Hz", "DCS D023N").
  sdrkit_string tone;
  // Whether the tone gate lets the audio through.
  bool tone_gate_open;
} sdrkit_general_fields;

// The options of `tx_kind` of `sdrkit_general_transmit_fields`.
typedef enum sdrkit_general_transmit_fields_tx_kind {
  // Voice
  SDRKIT_GENERAL_TRANSMIT_FIELDS_TX_KIND_VOICE = 0,
  // Text
  SDRKIT_GENERAL_TRANSMIT_FIELDS_TX_KIND_TEXT = 1,
  // Carrier
  SDRKIT_GENERAL_TRANSMIT_FIELDS_TX_KIND_CARRIER = 2,
} sdrkit_general_transmit_fields_tx_kind;

// The result fields of the transmitter. The measurements have no value while the stack does not transmit.
typedef struct sdrkit_general_transmit_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the stack transmits now. The receive audio is muted meanwhile.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the listening frequency plus the transmit offset. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_kind` holds a value.
  bool has_tx_kind;
  // What the transmission sends. A value of `sdrkit_general_transmit_fields_tx_kind`.
  int32_t tx_kind;
  // True if `tx_time_s` holds a value.
  bool has_tx_time_s;
  // The time on air of the transmission. A length of time in seconds.
  double tx_time_s;
  // True if `tx_mic_dbfs` holds a value.
  bool has_tx_mic_dbfs;
  // The peak of the microphone audio over the last report. No value without voice. Unit: dBFS.
  double tx_mic_dbfs;
  // True if `tx_peak_db` holds a value.
  bool has_tx_peak_db;
  // The peak of the modulation over the last report, against full modulation (deviation, depth or full scale). Unit: dB.
  double tx_peak_db;
  // True if `tx_level_db` holds a value.
  bool has_tx_level_db;
  // The gain of the level control of the speech processor. Unit: dB.
  double tx_level_db;
  // True if `tx_limit_db` holds a value.
  bool has_tx_limit_db;
  // The most gain reduction of the limiter and the compressor over the last report: 0 when they did not act. Unit: dB.
  double tx_limit_db;
} sdrkit_general_transmit_fields;

// A row of the width presets. The row key is the place of the preset ("00", "01" ...).
typedef struct sdrkit_general_width_preset_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The width. A frequency in Hz.
  double width_hz;
} sdrkit_general_width_preset_row;

// The options of `demod` of `sdrkit_general_preset_row`.
typedef enum sdrkit_general_preset_row_demod {
  // NFM
  SDRKIT_GENERAL_PRESET_ROW_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_PRESET_ROW_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_PRESET_ROW_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_PRESET_ROW_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_PRESET_ROW_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_PRESET_ROW_DEMOD_CW = 5,
} sdrkit_general_preset_row_demod;

// The options of `squelch` of `sdrkit_general_preset_row`.
typedef enum sdrkit_general_preset_row_squelch {
  // Off
  SDRKIT_GENERAL_PRESET_ROW_SQUELCH_OFF = 0,
  // Auto
  SDRKIT_GENERAL_PRESET_ROW_SQUELCH_AUTO = 1,
  // Level
  SDRKIT_GENERAL_PRESET_ROW_SQUELCH_LEVEL = 2,
} sdrkit_general_preset_row_squelch;

// A row of the built-in presets. The row key is the key of the preset.
typedef struct sdrkit_general_preset_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the preset.
  sdrkit_string name;
  // The frequency of the preset. A frequency in Hz.
  double freq_hz;
  // The demodulator of the preset. A value of `sdrkit_general_preset_row_demod`.
  int32_t demod;
  // The filter width of the preset. A frequency in Hz.
  double width;
  // The squelch of the preset. A value of `sdrkit_general_preset_row_squelch`.
  int32_t squelch;
} sdrkit_general_preset_row;

// The options of `demod` of `sdrkit_general_channel_row`.
typedef enum sdrkit_general_channel_row_demod {
  // NFM
  SDRKIT_GENERAL_CHANNEL_ROW_DEMOD_NFM = 0,
  // WFM
  SDRKIT_GENERAL_CHANNEL_ROW_DEMOD_WFM = 1,
  // AM
  SDRKIT_GENERAL_CHANNEL_ROW_DEMOD_AM = 2,
  // USB
  SDRKIT_GENERAL_CHANNEL_ROW_DEMOD_USB = 3,
  // LSB
  SDRKIT_GENERAL_CHANNEL_ROW_DEMOD_LSB = 4,
  // CW
  SDRKIT_GENERAL_CHANNEL_ROW_DEMOD_CW = 5,
} sdrkit_general_channel_row_demod;

// A row of the extra channels. The row key is the number of the channel ("1" to "3").
typedef struct sdrkit_general_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the channel.
  sdrkit_string channel;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The listening frequency of the channel. A frequency in Hz.
  double freq_hz;
  // True if `demod` holds a value.
  bool has_demod;
  // The demodulator of the channel. A value of `sdrkit_general_channel_row_demod`.
  int32_t demod;
  // True if `squelch_open` holds a value.
  bool has_squelch_open;
  // Whether the squelch of the channel is open.
  bool squelch_open;
  // The summary line of the channel.
  sdrkit_string summary;
} sdrkit_general_channel_row;

// The arguments of `preset.apply`.
typedef struct sdrkit_general_preset_apply_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of a preset.
  sdrkit_string row;
} sdrkit_general_preset_apply_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_GENERAL_PRESET_APPLY_ARGS_INIT {sizeof(sdrkit_general_preset_apply_args), {0}}

// The arguments of `ptt`.
typedef struct sdrkit_general_ptt_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Push-to-talk pressed (on) or released (off).
  bool on;
} sdrkit_general_ptt_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_GENERAL_PTT_ARGS_INIT {sizeof(sdrkit_general_ptt_args), 0}

// The arguments of `send`.
typedef struct sdrkit_general_send_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The text to key.
  sdrkit_string text;
} sdrkit_general_send_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_GENERAL_SEND_ARGS_INIT {sizeof(sdrkit_general_send_args), {0}}

// Fills `rows` with the changed rows of the table "width_presets" of the stack "general" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_general_width_presets_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_general_width_preset_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "general.WidthPresetRow", "width_presets", rows, sizeof(sdrkit_general_width_preset_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "presets" of the stack "general" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_general_presets_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_general_preset_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "general.PresetRow", "presets", rows, sizeof(sdrkit_general_preset_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "general" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_general_channels_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_general_channel_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "general.ChannelRow", "channels", rows, sizeof(sdrkit_general_channel_row), capacity, info);
}

// Runs the command "preset.apply" (Apply preset) of the stack "general" on the session of `handle`. Sets the demodulator, the filter width and the squelch of a preset, and tunes to its frequency.
static inline int32_t sdrkit_general_preset_apply(SDRKIT_HANDLE handle, const sdrkit_general_preset_apply_args *args) {
  return sdrkit_typed_command(handle, "preset.apply", "general.PresetApplyArgs", args, NULL, NULL, 0);
}

// Runs the command "ptt" (Push to talk) of the stack "general" on the session of `handle`. With on true, sends the speech of the microphone input in the mode of the demodulator (NFM, AM, USB or LSB) until ptt with on false, halt or the time-out. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
static inline int32_t sdrkit_general_ptt(SDRKIT_HANDLE handle, const sdrkit_general_ptt_args *args) {
  return sdrkit_typed_command(handle, "ptt", "general.PttArgs", args, NULL, NULL, 0);
}

// Runs the command "send" (Send Morse) of the stack "general" on the session of `handle`. Keys the text in Morse code at the CW speed, with the demodulator on CW, the carrier at the listening frequency plus the transmit offset. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_general_send(SDRKIT_HANDLE handle, const sdrkit_general_send_args *args) {
  return sdrkit_typed_command(handle, "send", "general.SendArgs", args, NULL, NULL, 0);
}

// Runs the command "tune" (Tune) of the stack "general" on the session of `handle`. Sends an unmodulated carrier at the listening frequency plus the transmit offset for the tune time. It transmits (carrier). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
static inline int32_t sdrkit_general_tune(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "tune", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "halt" (Halt) of the stack "general" on the session of `handle`. Ends speech, Morse code or the tune carrier at once.
static inline int32_t sdrkit_general_halt(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "halt", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "general" are the sdrkit_general_gen_*() functions
// after the settings of its receiver.

// The setting "demod" (Demodulator) of the stack "general". A value of `sdrkit_general_demod`. NFM, WFM (mono), AM, USB, LSB or CW.
static inline int32_t sdrkit_general_get_demod(SDRKIT_HANDLE handle, sdrkit_general_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_demod)index;
  return code;
}
static inline int32_t sdrkit_general_set_demod(SDRKIT_HANDLE handle, sdrkit_general_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "width" (Filter) of the stack "general". A frequency in Hz. The width of the channel filter. Each demodulator keeps its own. A width outside the range of the demodulator is brought into that range.
static inline int32_t sdrkit_general_get_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "width", value);
}
static inline int32_t sdrkit_general_set_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "width", value);
}

// The setting "channel_offset" (Channel offset) of the stack "general". A frequency in Hz. Where the listening channel sits relative to the dial, inside the capture.
static inline int32_t sdrkit_general_get_channel_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_offset", value);
}
static inline int32_t sdrkit_general_set_channel_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_offset", value);
}

// The setting "step" (Step) of the stack "general". A frequency in Hz. The tuning step. Each demodulator keeps its own.
static inline int32_t sdrkit_general_get_step(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "step", value);
}
static inline int32_t sdrkit_general_set_step(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "step", value);
}

// The setting "custom_filter" (Custom baseband filter) of the stack "general". Low-pass prototype taps, separated by commas, in place of the built-in channel filter. Empty for the built-in one.
static inline int32_t sdrkit_general_get_custom_filter(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "custom_filter", value);
}
static inline int32_t sdrkit_general_set_custom_filter(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "custom_filter", value);
}

// The setting "squelch" (Squelch) of the stack "general". A value of `sdrkit_general_squelch`. Auto opens 6 dB above the measured noise. Level opens at a level in dBFS.
static inline int32_t sdrkit_general_get_squelch(SDRKIT_HANDLE handle, sdrkit_general_squelch *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "squelch", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_squelch)index;
  return code;
}
static inline int32_t sdrkit_general_set_squelch(SDRKIT_HANDLE handle, sdrkit_general_squelch value) {
  return sdrkit_typed_setting_set_enum(handle, "squelch", (int32_t)value);
}

// The setting "squelch_level" (Squelch level) of the stack "general". Unit: dBFS.
static inline int32_t sdrkit_general_get_squelch_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch_level", value);
}
static inline int32_t sdrkit_general_set_squelch_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch_level", value);
}

// The setting "agc" (AGC) of the stack "general". AM follows the carrier. SSB and CW follow the envelope.
static inline int32_t sdrkit_general_get_agc(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "agc", value);
}
static inline int32_t sdrkit_general_set_agc(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "agc", value);
}

// The setting "gain" (Gain) of the stack "general". Unit: dB. Used when the AGC is off.
static inline int32_t sdrkit_general_get_gain(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gain", value);
}
static inline int32_t sdrkit_general_set_gain(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gain", value);
}

// The setting "deemph_nfm" (De-emphasis (NFM)) of the stack "general". A value of `sdrkit_general_deemph_nfm`.
static inline int32_t sdrkit_general_get_deemph_nfm(SDRKIT_HANDLE handle, sdrkit_general_deemph_nfm *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deemph_nfm", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_deemph_nfm)index;
  return code;
}
static inline int32_t sdrkit_general_set_deemph_nfm(SDRKIT_HANDLE handle, sdrkit_general_deemph_nfm value) {
  return sdrkit_typed_setting_set_enum(handle, "deemph_nfm", (int32_t)value);
}

// The setting "deemph_wfm" (De-emphasis (WFM)) of the stack "general". A value of `sdrkit_general_deemph_wfm`.
static inline int32_t sdrkit_general_get_deemph_wfm(SDRKIT_HANDLE handle, sdrkit_general_deemph_wfm *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deemph_wfm", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_deemph_wfm)index;
  return code;
}
static inline int32_t sdrkit_general_set_deemph_wfm(SDRKIT_HANDLE handle, sdrkit_general_deemph_wfm value) {
  return sdrkit_typed_setting_set_enum(handle, "deemph_wfm", (int32_t)value);
}

// The setting "nfm_lowpass" (Audio low-pass (NFM)) of the stack "general". A value of `sdrkit_general_nfm_lowpass`. 3 kHz has about 3.7 dB less noise and keeps speech. 4 kHz passes more treble and more noise.
static inline int32_t sdrkit_general_get_nfm_lowpass(SDRKIT_HANDLE handle, sdrkit_general_nfm_lowpass *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "nfm_lowpass", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_nfm_lowpass)index;
  return code;
}
static inline int32_t sdrkit_general_set_nfm_lowpass(SDRKIT_HANDLE handle, sdrkit_general_nfm_lowpass value) {
  return sdrkit_typed_setting_set_enum(handle, "nfm_lowpass", (int32_t)value);
}

// The setting "cw_pitch" (Pitch) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_cw_pitch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "cw_pitch", value);
}
static inline int32_t sdrkit_general_set_cw_pitch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "cw_pitch", value);
}

// The setting "am_auto" (Auto quality) of the stack "general". Locks to the carrier, picks the cleaner sideband, removes whistles, hum and clicks, and levels the sound.
static inline int32_t sdrkit_general_get_am_auto(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "am_auto", value);
}
static inline int32_t sdrkit_general_set_am_auto(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "am_auto", value);
}

// The setting "am_detector" (Detector) of the stack "general". A value of `sdrkit_general_am_detector`.
static inline int32_t sdrkit_general_get_am_detector(SDRKIT_HANDLE handle, sdrkit_general_am_detector *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "am_detector", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_am_detector)index;
  return code;
}
static inline int32_t sdrkit_general_set_am_detector(SDRKIT_HANDLE handle, sdrkit_general_am_detector value) {
  return sdrkit_typed_setting_set_enum(handle, "am_detector", (int32_t)value);
}

// The setting "am_nr" (Noise) of the stack "general". A value of `sdrkit_general_am_nr`.
static inline int32_t sdrkit_general_get_am_nr(SDRKIT_HANDLE handle, sdrkit_general_am_nr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "am_nr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_am_nr)index;
  return code;
}
static inline int32_t sdrkit_general_set_am_nr(SDRKIT_HANDLE handle, sdrkit_general_am_nr value) {
  return sdrkit_typed_setting_set_enum(handle, "am_nr", (int32_t)value);
}

// The setting "am_bandwidth" (Audio bandwidth) of the stack "general". A frequency in Hz. A width is 2500 to 10 000 Hz.
static inline int32_t sdrkit_general_get_am_bandwidth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "am_bandwidth", value);
}
static inline int32_t sdrkit_general_set_am_bandwidth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "am_bandwidth", value);
}

// The setting "am_enhance" (Enhance (synthetic treble)) of the stack "general".
static inline int32_t sdrkit_general_get_am_enhance(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "am_enhance", value);
}
static inline int32_t sdrkit_general_set_am_enhance(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "am_enhance", value);
}

// The setting "nb" (Blanker) of the stack "general". Removes clicks and ignition noise. Not offered for WFM.
static inline int32_t sdrkit_general_get_nb(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "nb", value);
}
static inline int32_t sdrkit_general_set_nb(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "nb", value);
}

// The setting "nb_threshold" (Blanker threshold) of the stack "general". Unit: dB.
static inline int32_t sdrkit_general_get_nb_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "nb_threshold", value);
}
static inline int32_t sdrkit_general_set_nb_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "nb_threshold", value);
}

// The setting "af_highpass" (High-pass) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_af_highpass(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "af_highpass", value);
}
static inline int32_t sdrkit_general_set_af_highpass(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "af_highpass", value);
}

// The setting "af_lowpass" (Low-pass) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_af_lowpass(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "af_lowpass", value);
}
static inline int32_t sdrkit_general_set_af_lowpass(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "af_lowpass", value);
}

// The setting "af_notch" (Notch) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_af_notch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "af_notch", value);
}
static inline int32_t sdrkit_general_set_af_notch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "af_notch", value);
}

// The setting "voice_nr" (Noise remover (neural)) of the stack "general". For NFM, WFM and AM speech. Each demodulator keeps its own.
static inline int32_t sdrkit_general_get_voice_nr(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "voice_nr", value);
}
static inline int32_t sdrkit_general_set_voice_nr(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "voice_nr", value);
}

// The setting "voice_nr_strength" (Strength) of the stack "general". Unit: %.
static inline int32_t sdrkit_general_get_voice_nr_strength(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "voice_nr_strength", value);
}
static inline int32_t sdrkit_general_set_voice_nr_strength(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "voice_nr_strength", value);
}

// The setting "presets" (Presets) of the stack "general". Entries separated by ";", each a key, "=", then the frequency in Hz, the demodulator, the filter width in Hz and the name separated by ",", for example "cw=7030000,cw,500,CW". At most 64 entries. The host fills it from a preset file. Empty: no presets.
static inline int32_t sdrkit_general_get_presets(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "presets", value);
}
static inline int32_t sdrkit_general_set_presets(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "presets", value);
}

// The setting "tone_gate" (Tone gate) of the stack "general". open, any, tone:<Hz> or code:<023N>.
static inline int32_t sdrkit_general_get_tone_gate(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "tone_gate", value);
}
static inline int32_t sdrkit_general_set_tone_gate(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "tone_gate", value);
}

// The setting "listen_channel" (Heard channel) of the stack "general". 0 for the main channel, 1 to 3 for an extra channel.
static inline int32_t sdrkit_general_get_listen_channel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "listen_channel", value);
}
static inline int32_t sdrkit_general_set_listen_channel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "listen_channel", value);
}

// The setting "extra1" (Extra channel 1) of the stack "general".
static inline int32_t sdrkit_general_get_extra1(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "extra1", value);
}
static inline int32_t sdrkit_general_set_extra1(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "extra1", value);
}

// The setting "extra1_offset" (Extra 1 offset) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_extra1_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "extra1_offset", value);
}
static inline int32_t sdrkit_general_set_extra1_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "extra1_offset", value);
}

// The setting "extra1_demod" (Extra 1 demodulator) of the stack "general". A value of `sdrkit_general_extra1_demod`.
static inline int32_t sdrkit_general_get_extra1_demod(SDRKIT_HANDLE handle, sdrkit_general_extra1_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "extra1_demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_extra1_demod)index;
  return code;
}
static inline int32_t sdrkit_general_set_extra1_demod(SDRKIT_HANDLE handle, sdrkit_general_extra1_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "extra1_demod", (int32_t)value);
}

// The setting "extra1_width" (Extra 1 filter) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_extra1_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "extra1_width", value);
}
static inline int32_t sdrkit_general_set_extra1_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "extra1_width", value);
}

// The setting "extra2" (Extra channel 2) of the stack "general".
static inline int32_t sdrkit_general_get_extra2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "extra2", value);
}
static inline int32_t sdrkit_general_set_extra2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "extra2", value);
}

// The setting "extra2_offset" (Extra 2 offset) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_extra2_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "extra2_offset", value);
}
static inline int32_t sdrkit_general_set_extra2_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "extra2_offset", value);
}

// The setting "extra2_demod" (Extra 2 demodulator) of the stack "general". A value of `sdrkit_general_extra2_demod`.
static inline int32_t sdrkit_general_get_extra2_demod(SDRKIT_HANDLE handle, sdrkit_general_extra2_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "extra2_demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_extra2_demod)index;
  return code;
}
static inline int32_t sdrkit_general_set_extra2_demod(SDRKIT_HANDLE handle, sdrkit_general_extra2_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "extra2_demod", (int32_t)value);
}

// The setting "extra2_width" (Extra 2 filter) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_extra2_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "extra2_width", value);
}
static inline int32_t sdrkit_general_set_extra2_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "extra2_width", value);
}

// The setting "extra3" (Extra channel 3) of the stack "general".
static inline int32_t sdrkit_general_get_extra3(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "extra3", value);
}
static inline int32_t sdrkit_general_set_extra3(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "extra3", value);
}

// The setting "extra3_offset" (Extra 3 offset) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_extra3_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "extra3_offset", value);
}
static inline int32_t sdrkit_general_set_extra3_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "extra3_offset", value);
}

// The setting "extra3_demod" (Extra 3 demodulator) of the stack "general". A value of `sdrkit_general_extra3_demod`.
static inline int32_t sdrkit_general_get_extra3_demod(SDRKIT_HANDLE handle, sdrkit_general_extra3_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "extra3_demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_extra3_demod)index;
  return code;
}
static inline int32_t sdrkit_general_set_extra3_demod(SDRKIT_HANDLE handle, sdrkit_general_extra3_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "extra3_demod", (int32_t)value);
}

// The setting "extra3_width" (Extra 3 filter) of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_get_extra3_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "extra3_width", value);
}
static inline int32_t sdrkit_general_set_extra3_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "extra3_width", value);
}

// The setting "tx_split" (Transmit offset) of the stack "general". A frequency in Hz. The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.
static inline int32_t sdrkit_general_get_tx_split(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_split", value);
}
static inline int32_t sdrkit_general_set_tx_split(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_split", value);
}

// The setting "tx_deviation" (Deviation (NFM)) of the stack "general". A frequency in Hz. The peak deviation of NFM speech with its tone: 5000 Hz in 25 kHz channels, 2500 Hz in 12.5 kHz channels.
static inline int32_t sdrkit_general_get_tx_deviation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_deviation", value);
}
static inline int32_t sdrkit_general_set_tx_deviation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_deviation", value);
}

// The setting "tx_preemphasis" (Pre-emphasis (NFM)) of the stack "general". A value of `sdrkit_general_tx_preemphasis`. Lifts the treble of NFM speech. A receiver with the same de-emphasis hears it flat.
static inline int32_t sdrkit_general_get_tx_preemphasis(SDRKIT_HANDLE handle, sdrkit_general_tx_preemphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_preemphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_tx_preemphasis)index;
  return code;
}
static inline int32_t sdrkit_general_set_tx_preemphasis(SDRKIT_HANDLE handle, sdrkit_general_tx_preemphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_preemphasis", (int32_t)value);
}

// The setting "tx_ctcss" (CTCSS tone) of the stack "general". A frequency in Hz. A sub-audio tone sent under NFM speech at 12 % of the deviation.
static inline int32_t sdrkit_general_get_tx_ctcss(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_ctcss", value);
}
static inline int32_t sdrkit_general_set_tx_ctcss(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_ctcss", value);
}

// The setting "tx_am_depth" (Modulation depth (AM)) of the stack "general". Unit: %. The largest modulation depth of AM. The carrier and both sidebands stay within full scale.
static inline int32_t sdrkit_general_get_tx_am_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_am_depth", value);
}
static inline int32_t sdrkit_general_set_tx_am_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_am_depth", value);
}

// The setting "tx_ssb_method" (Sideband method (SSB)) of the stack "general". A value of `sdrkit_general_tx_ssb_method`. Filter: a band-pass filter passes one sideband. Phasing: a Hilbert transformer cancels the other one.
static inline int32_t sdrkit_general_get_tx_ssb_method(SDRKIT_HANDLE handle, sdrkit_general_tx_ssb_method *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_ssb_method", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_tx_ssb_method)index;
  return code;
}
static inline int32_t sdrkit_general_set_tx_ssb_method(SDRKIT_HANDLE handle, sdrkit_general_tx_ssb_method value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_ssb_method", (int32_t)value);
}

// The setting "tx_ssb_band" (Audio band (SSB)) of the stack "general". A value of `sdrkit_general_tx_ssb_band`. 300 to 2700 Hz, or from 300 Hz over the width of the receive filter.
static inline int32_t sdrkit_general_get_tx_ssb_band(SDRKIT_HANDLE handle, sdrkit_general_tx_ssb_band *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_ssb_band", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_tx_ssb_band)index;
  return code;
}
static inline int32_t sdrkit_general_set_tx_ssb_band(SDRKIT_HANDLE handle, sdrkit_general_tx_ssb_band value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_ssb_band", (int32_t)value);
}

// The setting "tx_compression" (Compression (SSB)) of the stack "general". Unit: dB. Raises the average level of SSB speech by up to this much. The peaks stay at full scale.
static inline int32_t sdrkit_general_get_tx_compression(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_compression", value);
}
static inline int32_t sdrkit_general_set_tx_compression(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_compression", value);
}

// The setting "tx_timeout" (Time-out) of the stack "general". A length of time in seconds. Push-to-talk ends after this time on air.
static inline int32_t sdrkit_general_get_tx_timeout(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_timeout", value);
}
static inline int32_t sdrkit_general_set_tx_timeout(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_timeout", value);
}

// The setting "tx_cw_wpm" (Speed (CW)) of the stack "general". Unit: wpm. The speed of the characters that send keys.
static inline int32_t sdrkit_general_get_tx_cw_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_cw_wpm", value);
}
static inline int32_t sdrkit_general_set_tx_cw_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_cw_wpm", value);
}

// The setting "tx_cw_farnsworth" (Overall speed (CW)) of the stack "general". Unit: wpm. Farnsworth spacing: the characters at the speed, the gaps stretched to this lower overall speed.
static inline int32_t sdrkit_general_get_tx_cw_farnsworth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_cw_farnsworth", value);
}
static inline int32_t sdrkit_general_set_tx_cw_farnsworth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_cw_farnsworth", value);
}

// The setting "tx_tune_limit" (Tune time) of the stack "general". A length of time in seconds. The tune carrier ends after this time.
static inline int32_t sdrkit_general_get_tx_tune_limit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_tune_limit", value);
}
static inline int32_t sdrkit_general_set_tx_tune_limit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_tune_limit", value);
}

// The setting "gen_noise_only" (Noise only) of the test signal generator of the stack "general". No station: only the noise of the test signal settings.
static inline int32_t sdrkit_general_gen_get_noise_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_noise_only", value);
}
static inline int32_t sdrkit_general_gen_set_noise_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_noise_only", value);
}

// The setting "gen_tune_offset" (Tune offset) of the test signal generator of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_gen_get_tune_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tune_offset", value);
}
static inline int32_t sdrkit_general_gen_set_tune_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tune_offset", value);
}

// The setting "gen_tone" (Tone) of the test signal generator of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_gen_get_tone(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone", value);
}
static inline int32_t sdrkit_general_gen_set_tone(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone", value);
}

// The setting "gen_mod" (Modulation) of the test signal generator of the stack "general".
static inline int32_t sdrkit_general_gen_get_mod(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_mod", value);
}
static inline int32_t sdrkit_general_gen_set_mod(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_mod", value);
}

// The setting "gen_ctcss" (CTCSS tone) of the test signal generator of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_gen_get_ctcss(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ctcss", value);
}
static inline int32_t sdrkit_general_gen_set_ctcss(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ctcss", value);
}

// The setting "gen_ctcss_dev" (CTCSS deviation) of the test signal generator of the stack "general". A frequency in Hz.
static inline int32_t sdrkit_general_gen_get_ctcss_dev(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ctcss_dev", value);
}
static inline int32_t sdrkit_general_gen_set_ctcss_dev(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ctcss_dev", value);
}

// The setting "gen_dcs" (DCS code) of the test signal generator of the stack "general". Three octal digits, with N or I for the polarity (023N). Empty for none.
static inline int32_t sdrkit_general_gen_get_dcs(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_dcs", value);
}
static inline int32_t sdrkit_general_gen_set_dcs(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_dcs", value);
}

// The setting "gen_audio" (Sound) of the test signal generator of the stack "general". A value of `sdrkit_general_gen_audio`. Tone: the steady tone of the setting Tone. Voice: noise of the voice band in syllables of 4 Hz with pauses as long, at the modulation of the setting Modulation, which fills the channel as speech does. Voice applies to FM and AM; SSB and CW keep the tone.
static inline int32_t sdrkit_general_gen_get_audio(SDRKIT_HANDLE handle, sdrkit_general_gen_audio *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_audio", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_general_gen_audio)index;
  return code;
}
static inline int32_t sdrkit_general_gen_set_audio(SDRKIT_HANDLE handle, sdrkit_general_gen_audio value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_audio", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_general_am_fields_read(SDRKIT_HANDLE handle, sdrkit_general_am_fields *fields) {
  return sdrkit_typed_fields(handle, "general.AmFields", fields, sizeof(sdrkit_general_am_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_general_fields_read(SDRKIT_HANDLE handle, sdrkit_general_fields *fields) {
  return sdrkit_typed_fields(handle, "general.GeneralFields", fields, sizeof(sdrkit_general_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_general_transmit_fields_read(SDRKIT_HANDLE handle, sdrkit_general_transmit_fields *fields) {
  return sdrkit_typed_fields(handle, "general.TransmitFields", fields, sizeof(sdrkit_general_transmit_fields));
}

#endif
