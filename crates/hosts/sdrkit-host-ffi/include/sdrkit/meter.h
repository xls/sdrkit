/*
 * sdrkit: the typed interface of the stack family "meter", version 1.1. MIT licence.
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

#ifndef SDRKIT_METER_H
#define SDRKIT_METER_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_METER_VERSION_MAJOR 1
#define SDRKIT_METER_VERSION_MINOR 1

// The stack "fm_monitor": its id, and the keys of its event types.
#define SDRKIT_STACK_FM_MONITOR "fm_monitor"
#define SDRKIT_EVENT_FM_MONITOR_PILOT "fm_monitor.pilot"
#define SDRKIT_EVENT_FM_MONITOR_RDS "fm_monitor.rds"
#define SDRKIT_EVENT_FM_MONITOR_SCA "fm_monitor.sca"
#define SDRKIT_EVENT_FM_MONITOR_OVER_DEVIATION "fm_monitor.over_deviation"

// The stack "am_monitor": its id, and the keys of its event types.
#define SDRKIT_STACK_AM_MONITOR "am_monitor"
#define SDRKIT_EVENT_AM_MONITOR_CARRIER "am_monitor.carrier"
#define SDRKIT_EVENT_AM_MONITOR_OVER_MODULATION "am_monitor.over_modulation"

// The stack "occupancy": its id, and the keys of its event types.
#define SDRKIT_STACK_OCCUPANCY "occupancy"
#define SDRKIT_EVENT_OCCUPANCY_ACTIVITY "occupancy.activity"

// The stack "noise_floor": its id, and the keys of its event types.
#define SDRKIT_STACK_NOISE_FLOOR "noise_floor"
#define SDRKIT_EVENT_NOISE_FLOOR_EVENT "noise_floor.event"

// The stack "burst_finder": its id, and the keys of its event types.
#define SDRKIT_STACK_BURST_FINDER "burst_finder"
#define SDRKIT_EVENT_BURST_FOUND "burst.found"
#define SDRKIT_EVENT_BURST_REPEATING "burst.repeating"

// The stack "hopper": its id, and the keys of its event types.
#define SDRKIT_STACK_HOPPER "hopper"
#define SDRKIT_EVENT_HOPPER_CHANGED "hopper.changed"

// The stack "birdie": its id, and the keys of its event types.
#define SDRKIT_STACK_BIRDIE "birdie"
#define SDRKIT_EVENT_BIRDIE_EVENT "birdie.event"

// The stack "obw": its id, and the keys of its event types.
#define SDRKIT_STACK_OBW "obw"
#define SDRKIT_EVENT_OBW_READING "obw.reading"

// The stack "ptt_log": its id, and the keys of its event types.
#define SDRKIT_STACK_PTT_LOG "ptt_log"
#define SDRKIT_EVENT_PTT_TRANSMISSION "ptt.transmission"

// The stack "noise_locator": its id, and the keys of its event types.
#define SDRKIT_STACK_NOISE_LOCATOR "noise_locator"
#define SDRKIT_EVENT_NOISE_LOCATOR_PATTERN "noise_locator.pattern"

// The stack "freq_accuracy": its id, and the keys of its event types.
#define SDRKIT_STACK_FREQ_ACCURACY "freq_accuracy"
#define SDRKIT_EVENT_FREQ_ACCURACY_EVENT "freq_accuracy.event"

// The stack "sinad": its id, and the keys of its event types.
#define SDRKIT_STACK_SINAD "sinad"
#define SDRKIT_EVENT_SINAD_CROSSING "sinad.crossing"

// The stack "nfm_meter": its id, and the keys of its event types.
#define SDRKIT_STACK_NFM_METER "nfm_meter"

// The stack "tx_stability": its id, and the keys of its event types.
#define SDRKIT_STACK_TX_STABILITY "tx_stability"
#define SDRKIT_EVENT_TX_STABILITY_TRANSMISSION "tx_stability.transmission"

// The stack "ssb_test": its id, and the keys of its event types.
#define SDRKIT_STACK_SSB_TEST "ssb_test"

// The stack "afsk_tuner": its id, and the keys of its event types.
#define SDRKIT_STACK_AFSK_TUNER "afsk_tuner"
#define SDRKIT_EVENT_AFSK_TUNER_BURST "afsk_tuner.burst"

// The stack "phase_noise": its id, and the keys of its event types.
#define SDRKIT_STACK_PHASE_NOISE "phase_noise"

// The stack "cw_keying": its id, and the keys of its event types.
#define SDRKIT_STACK_CW_KEYING "cw_keying"

// The stack "fsk_analyser": its id, and the keys of its event types.
#define SDRKIT_STACK_FSK_ANALYSER "fsk_analyser"

// The stack "mer": its id, and the keys of its event types.
#define SDRKIT_STACK_MER "mer"

// The stack "papr": its id, and the keys of its event types.
#define SDRKIT_STACK_PAPR "papr"

// The stack "input_health": its id, and the keys of its event types.
#define SDRKIT_STACK_INPUT_HEALTH "input_health"

// The stack "noise_figure": its id, and the keys of its event types.
#define SDRKIT_STACK_NOISE_FIGURE "noise_figure"

// The stack "kurtosis": its id, and the keys of its event types.
#define SDRKIT_STACK_KURTOSIS "kurtosis"

// The stack "glitch": its id, and the keys of its event types.
#define SDRKIT_STACK_GLITCH "glitch"
#define SDRKIT_EVENT_GLITCH_EVENT "glitch.event"

// The stack "intermod": its id, and the keys of its event types.
#define SDRKIT_STACK_INTERMOD "intermod"
#define SDRKIT_EVENT_INTERMOD_PRODUCT "intermod.product"

// The stack "spur": its id, and the keys of its event types.
#define SDRKIT_STACK_SPUR "spur"

// The stack "fading": its id, and the keys of its event types.
#define SDRKIT_STACK_FADING "fading"

// The stack "beacon_monitor": its id, and the keys of its event types.
#define SDRKIT_STACK_BEACON_MONITOR "beacon_monitor"
#define SDRKIT_EVENT_BEACON_MONITOR_CHANGE "beacon_monitor.change"

// The options of the setting "deviation_limit" of the stack "fm_monitor".
typedef enum sdrkit_fm_monitor_deviation_limit {
  // 75 kHz
  SDRKIT_FM_MONITOR_DEVIATION_LIMIT_75_KHZ = 0,
  // 50 kHz
  SDRKIT_FM_MONITOR_DEVIATION_LIMIT_50_KHZ = 1,
} sdrkit_fm_monitor_deviation_limit;

// The options of the setting "gen_sideband" of the stack "ssb_test".
typedef enum sdrkit_ssb_test_gen_sideband {
  // Upper
  SDRKIT_SSB_TEST_GEN_SIDEBAND_UPPER = 0,
  // Lower
  SDRKIT_SSB_TEST_GEN_SIDEBAND_LOWER = 1,
} sdrkit_ssb_test_gen_sideband;

// The options of the setting "gen_kind" of the stack "papr".
typedef enum sdrkit_papr_gen_kind {
  // Noise
  SDRKIT_PAPR_GEN_KIND_NOISE = 0,
  // FM carrier
  SDRKIT_PAPR_GEN_KIND_FM = 1,
  // QPSK
  SDRKIT_PAPR_GEN_KIND_QPSK = 2,
  // Clipped noise
  SDRKIT_PAPR_GEN_KIND_CLIPPED_NOISE = 3,
  // Noise with impulses
  SDRKIT_PAPR_GEN_KIND_NOISE_WITH_IMPULSES = 4,
} sdrkit_papr_gen_kind;

// The options of `pilot` of `sdrkit_fm_monitor_fields`.
typedef enum sdrkit_fm_monitor_fields_pilot {
  // Absent
  SDRKIT_FM_MONITOR_FIELDS_PILOT_ABSENT = 0,
  // In the usual range
  SDRKIT_FM_MONITOR_FIELDS_PILOT_IN_RANGE = 1,
  // Outside the usual range
  SDRKIT_FM_MONITOR_FIELDS_PILOT_OUTSIDE = 2,
} sdrkit_fm_monitor_fields_pilot;

// The result fields of `fm_monitor`, measured once a second from the FM-demodulated multiplex (MPX).
typedef struct sdrkit_fm_monitor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The headline: stereo or mono, the peak, the pilot and RDS levels.
  sdrkit_string text;
  // True if `peak_deviation_khz` holds a value.
  bool has_peak_deviation_khz;
  // The peak deviation of the last second: the largest third-largest excursion of its 5 ms blocks. Unit: kHz.
  double peak_deviation_khz;
  // True if `rms_deviation_khz` holds a value.
  bool has_rms_deviation_khz;
  // The RMS deviation. Unit: kHz.
  double rms_deviation_khz;
  // True if `mpx_power_dbr` holds a value.
  bool has_mpx_power_dbr;
  // The MPX power of the last second (0 dBr is a sine of 19 kHz deviation). Unit: dBr.
  double mpx_power_dbr;
  // True if `mpx_power_60s_dbr` holds a value.
  bool has_mpx_power_60s_dbr;
  // The MPX power over the last 60 s in steps of 1 s, as ITU-R BS.412 integrates it; no value before 60 s. Unit: dBr.
  double mpx_power_60s_dbr;
  // True if `carrier_offset_khz` holds a value.
  bool has_carrier_offset_khz;
  // The carrier offset. Unit: kHz.
  double carrier_offset_khz;
  // True if `audio_khz` holds a value.
  bool has_audio_khz;
  // The audio (L+R), peak equivalent. Unit: kHz.
  double audio_khz;
  // True if `pilot_khz` holds a value.
  bool has_pilot_khz;
  // The 19 kHz pilot, peak equivalent. Unit: kHz.
  double pilot_khz;
  // True if `pilot_pct` holds a value.
  bool has_pilot_pct;
  // The pilot as a share of the deviation limit. Unit: %.
  double pilot_pct;
  // The pilot against its usual range. A value of `sdrkit_fm_monitor_fields_pilot`.
  int32_t pilot;
  // True if `stereo_band_khz` holds a value.
  bool has_stereo_band_khz;
  // The stereo (L-R) band, peak equivalent. Unit: kHz.
  double stereo_band_khz;
  // Stereo: a pilot and an L-R band over 0.5 kHz.
  bool stereo;
  // True if `rds_khz` holds a value.
  bool has_rds_khz;
  // The RDS injection, peak equivalent. Unit: kHz.
  double rds_khz;
  // RDS is present.
  bool rds;
  // True if `sca67_db` holds a value.
  bool has_sca67_db;
  // The 67 kHz subcarrier over the floor; no value when absent. Unit: dB.
  double sca67_db;
  // True if `sca92_db` holds a value.
  bool has_sca92_db;
  // The 92 kHz subcarrier over the floor; no value when absent or below a working rate of 250 ksps. Unit: dB.
  double sca92_db;
  // The seconds whose peak was over the threshold.
  int64_t over_deviation_s;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The input level. Unit: dB.
  double level_db;
  // What the monitor measures.
  sdrkit_string note;
} sdrkit_fm_monitor_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_meter_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_meter_decoder_fields;

// A row of the log of `fm_monitor`. The row key is the sequence number of the row.
typedef struct sdrkit_fm_monitor_log_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row. Unit: s.
  double signal_time_s;
  // The kind: `Pilot`, `RDS`, `SCA` or `Over-deviation`.
  sdrkit_string kind;
  // What changed, or the peak of an over-deviation.
  sdrkit_string identity;
  // The text of the row.
  sdrkit_string text;
} sdrkit_fm_monitor_log_row;

// The result fields of `am_monitor`, measured once a second. The measurements have no value while the carrier test fails.
typedef struct sdrkit_am_monitor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The headline: the offset, the modulation, the peaks and the audio bandwidth, or "no carrier".
  sdrkit_string text;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset: a tuning or station error. Unit: Hz.
  double carrier_offset_hz;
  // True if `carrier_db` holds a value.
  bool has_carrier_db;
  // The carrier level, relative to full scale. Unit: dB.
  double carrier_db;
  // True if `depth_pct` holds a value.
  bool has_depth_pct;
  // The modulation depth: the depth of a sine of the same RMS. Unit: %.
  double depth_pct;
  // True if `peak_pos_pct` holds a value.
  bool has_peak_pos_pct;
  // The positive peak against the mean carrier. Unit: %.
  double peak_pos_pct;
  // True if `peak_neg_pct` holds a value.
  bool has_peak_neg_pct;
  // The negative peak against the mean carrier (at most 100 %). Unit: %.
  double peak_neg_pct;
  // True if `asymmetry_pct` holds a value.
  bool has_asymmetry_pct;
  // The asymmetry: (positive - negative) / (positive + negative). Unit: %.
  double asymmetry_pct;
  // True if `hum_db` holds a value.
  bool has_hum_db;
  // The mains hum below the carrier; no value when no hum line is seen. Unit: dB.
  double hum_db;
  // True if `hum_hz` holds a value.
  bool has_hum_hz;
  // The frequency of the mains hum line. Unit: Hz.
  double hum_hz;
  // True if `bandwidth_khz` holds a value.
  bool has_bandwidth_khz;
  // The audio bandwidth: 99 % of the audio power lies below it. Unit: kHz.
  double bandwidth_khz;
  // True if `over_mod_pct` holds a value.
  bool has_over_mod_pct;
  // The share of the last second at zero carrier. Unit: %.
  double over_mod_pct;
  // The seconds that were over-modulated.
  int64_t over_mod_s;
  // The carrier is present (the carrier test and the level rule).
  bool carrier;
  // What the monitor measures.
  sdrkit_string note;
} sdrkit_am_monitor_fields;

// A row of the log of `am_monitor`. The row key is the sequence number of the row.
typedef struct sdrkit_am_monitor_log_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row. Unit: s.
  double signal_time_s;
  // The kind: `Carrier` or `Over-modulation`.
  sdrkit_string kind;
  // What changed: `present`, `lost`, `returned`, or the share of the second at zero carrier.
  sdrkit_string identity;
  // The text of the row.
  sdrkit_string text;
} sdrkit_am_monitor_log_row;

// The options of `state` of `sdrkit_meter_watch_fields`.
typedef enum sdrkit_meter_watch_fields_state {
  // Nothing
  SDRKIT_METER_WATCH_FIELDS_STATE_NOTHING = 0,
  // Energy
  SDRKIT_METER_WATCH_FIELDS_STATE_ENERGY = 1,
  // Measuring
  SDRKIT_METER_WATCH_FIELDS_STATE_MEASURING = 2,
} sdrkit_meter_watch_fields_state;

// The status of a spectrum watcher: the headline, the state, the counts of readings and the input level.
typedef struct sdrkit_meter_watch_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The headline.
  sdrkit_string text;
  // The state. A value of `sdrkit_meter_watch_fields_state`.
  int32_t state;
  // The good readings: what the stack counts as a measurement.
  int64_t good;
  // The bad readings: what the stack counts as a fault of the watched band.
  int64_t bad;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level of the last report, relative to full scale. Unit: dBFS.
  double level_dbfs;
  // What the stack measures.
  sdrkit_string note;
} sdrkit_meter_watch_fields;

// The result fields of `occupancy`.
typedef struct sdrkit_occupancy_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The channels of the grid.
  int64_t channels;
  // The width of a channel: whole bins. Unit: kHz.
  double channel_khz;
  // The span that the channels cover. Unit: MHz.
  double span_mhz;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The noise floor of a channel, relative, per bin; no value before the first decision. Unit: dB.
  double floor_db;
  // The channels busy now.
  int64_t busy_now;
  // True if `band_occupancy_pct` holds a value.
  bool has_band_occupancy_pct;
  // The busy share of the channel-time since the end of the warm-up; no value before it. Unit: %.
  double band_occupancy_pct;
  // The time measured after the warm-up. A length of time in seconds.
  double elapsed_s;
} sdrkit_occupancy_fields;

// A row of the list of busy channels: a channel that has been busy, the busiest first.
typedef struct sdrkit_occupancy_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The centre of the channel (an offset from the centre when the tuned frequency is not known). A frequency in Hz.
  double frequency;
  // The busy time over the time measured. Unit: %.
  double duty_pct;
  // The transmissions that ended.
  int64_t transmissions;
  // The longest transmission. A length of time in seconds.
  double longest_s;
  // The channel is busy now.
  bool busy;
} sdrkit_occupancy_channel_row;

// A row of the activity log: one transmission of at least the shortest listed length.
typedef struct sdrkit_occupancy_activity_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The end of the transmission, signal time. A length of time in seconds.
  double time_s;
  // The centre of the channel. A frequency in Hz.
  double frequency;
  // The length of the transmission. A length of time in seconds.
  double length_s;
  // The highest channel power over the floor. Unit: dB.
  double peak_over_floor_db;
  // The highest channel power, relative, per bin. Unit: dB.
  double level_db;
} sdrkit_occupancy_activity_row;

// The result fields of `noise_floor`.
typedef struct sdrkit_noise_floor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The floor of the band: the upper median of the sub-bands now, dB re full scale per bin. Unit: dB.
  double floor_db;
  // True if `floor_change_db` holds a value.
  bool has_floor_change_db;
  // The floor now against the oldest kept value (the baseline window ago, or the start). Unit: dB.
  double floor_change_db;
  // The sub-bands raised now.
  int64_t raised;
  // The seconds measured. A length of time in seconds.
  double elapsed_s;
} sdrkit_noise_floor_fields;

// A row of the list of sub-bands.
typedef struct sdrkit_noise_floor_sub_band_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The centre of the sub-band. A frequency in Hz.
  double frequency;
  // True if `now_db` holds a value.
  bool has_now_db;
  // The floor of the last second, dB re full scale per bin. Unit: dB.
  double now_db;
  // True if `baseline_db` holds a value.
  bool has_baseline_db;
  // The baseline: the 20th percentile of the baseline window. Unit: dB.
  double baseline_db;
  // True if `p10_db` holds a value.
  bool has_p10_db;
  // The 10th percentile of the kept history. Unit: dB.
  double p10_db;
  // True if `p50_db` holds a value.
  bool has_p50_db;
  // The median of the kept history. Unit: dB.
  double p50_db;
  // True if `p90_db` holds a value.
  bool has_p90_db;
  // The 90th percentile of the kept history. Unit: dB.
  double p90_db;
  // The minutes of history kept. Unit: min.
  double history_min;
  // The sub-band is raised.
  bool raised;
} sdrkit_noise_floor_sub_band_row;

// The options of `kind` of `sdrkit_noise_floor_event_row`.
typedef enum sdrkit_noise_floor_event_row_kind {
  // Noise rise
  SDRKIT_NOISE_FLOOR_EVENT_ROW_KIND_RISE = 0,
  // Noise back
  SDRKIT_NOISE_FLOOR_EVENT_ROW_KIND_BACK = 1,
  // Whole band
  SDRKIT_NOISE_FLOOR_EVENT_ROW_KIND_WHOLE_BAND = 2,
} sdrkit_noise_floor_event_row_kind;

// A row of the noise floor log.
typedef struct sdrkit_noise_floor_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_noise_floor_event_row_kind`.
  int32_t kind;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The centre of the sub-band; no value for the whole band. A frequency in Hz.
  double frequency;
  // True if `sub_band` holds a value.
  bool has_sub_band;
  // The sub-band, from 1; no value for the whole band.
  int64_t sub_band;
  // The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band. Unit: dB.
  double rise_db;
  // True if `baseline_db` holds a value.
  bool has_baseline_db;
  // The baseline of the sub-band. Unit: dB.
  double baseline_db;
  // True if `duration_s` holds a value.
  bool has_duration_s;
  // How long the rise lasted (rows of the fall back). A length of time in seconds.
  double duration_s;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The floor of the sub-band, or of the band, that second. Unit: dB.
  double level_db;
} sdrkit_noise_floor_event_row;

// The result fields of `burst_finder`.
typedef struct sdrkit_burst_finder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The bursts found.
  int64_t bursts;
  // The time listened. A length of time in seconds.
  double elapsed_s;
  // Bursts per second over the time listened. Unit: 1/s.
  double bursts_per_s;
  // The sources found repeating now.
  int64_t repeating;
  // The width of a bin: the frequency resolution. Unit: kHz.
  double bin_khz;
  // The frame step: the time resolution. Unit: us.
  double frame_us;
} sdrkit_burst_finder_fields;

// The options of `class_` of `sdrkit_burst_finder_burst_row`.
typedef enum sdrkit_burst_finder_burst_row_class_ {
  // Narrow
  SDRKIT_BURST_FINDER_BURST_ROW_CLASS__NARROW = 0,
  // Wide
  SDRKIT_BURST_FINDER_BURST_ROW_CLASS__WIDE = 1,
} sdrkit_burst_finder_burst_row_class_;

// A row of the burst log.
typedef struct sdrkit_burst_finder_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The start of the burst, signal time. A length of time in seconds.
  double time_s;
  // The median centre of the burst. A frequency in Hz.
  double frequency;
  // The length of the burst. Unit: ms.
  double length_ms;
  // The bandwidth class. A value of `sdrkit_burst_finder_burst_row_class_`.
  int32_t class_;
  // The median width, at least one bin. Unit: kHz.
  double width_khz;
  // The mean peak over the median of the frames. Unit: dB.
  double level_db;
} sdrkit_burst_finder_burst_row;

// A row of the list of repeating sources.
typedef struct sdrkit_burst_finder_repeating_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency of the source: its first burst. A frequency in Hz.
  double frequency;
  // The median interval between its bursts. A length of time in seconds.
  double period_s;
  // Its bursts in the history.
  int64_t bursts;
  // The RMS deviation of the agreeing intervals from the period, over the period. Unit: %.
  double spread_pct;
} sdrkit_burst_finder_repeating_row;

// The result fields of `hopper`. The readings of the hopper have no value until an evaluation finds one.
typedef struct sdrkit_hopper_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `hop_rate` holds a value.
  bool has_hop_rate;
  // The hop rate: one over the median start-to-start time. Unit: 1/s.
  double hop_rate;
  // True if `hop_period_ms` holds a value.
  bool has_hop_period_ms;
  // The median start-to-start time. Unit: ms.
  double hop_period_ms;
  // True if `dwell_ms` holds a value.
  bool has_dwell_ms;
  // The median dwell. Unit: ms.
  double dwell_ms;
  // True if `guard_ms` holds a value.
  bool has_guard_ms;
  // The median silence between dwells. Unit: ms.
  double guard_ms;
  // True if `channels` holds a value.
  bool has_channels;
  // The channels found.
  int64_t channels;
  // True if `spacing_khz` holds a value.
  bool has_spacing_khz;
  // The median distance of neighbouring channels. Unit: kHz.
  double spacing_khz;
  // True if `grid` holds a value.
  bool has_grid;
  // At least 90 % of the channels lie on a grid of the spacing.
  bool grid;
  // True if `span_khz` holds a value.
  bool has_span_khz;
  // The highest minus the lowest channel plus one spacing. Unit: kHz.
  double span_khz;
  // True if `lowest` holds a value.
  bool has_lowest;
  // The lowest channel. A frequency in Hz.
  double lowest;
  // True if `highest` holds a value.
  bool has_highest;
  // The highest channel. A frequency in Hz.
  double highest;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The mean dwell level over the frame median. Unit: dB.
  double level_db;
  // The dwells since the start.
  int64_t dwells;
  // The dwells kept for the evaluation.
  int64_t dwells_window;
  // The width of a bin. Unit: kHz.
  double bin_khz;
  // The frame step. Unit: us.
  double frame_us;
} sdrkit_hopper_fields;

// A row of the hopper log: an evaluation that found a hopper or a change of it.
typedef struct sdrkit_hopper_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the evaluation. A length of time in seconds.
  double time_s;
  // The hop rate. Unit: 1/s.
  double hop_rate;
  // The channels.
  int64_t channels;
  // The channel spacing. Unit: kHz.
  double spacing_khz;
  // The span. Unit: kHz.
  double span_khz;
  // The lowest channel. A frequency in Hz.
  double lowest;
  // The highest channel. A frequency in Hz.
  double highest;
  // The median dwell. Unit: ms.
  double dwell_ms;
  // The median guard. Unit: ms.
  double guard_ms;
  // The mean dwell level over the frame median. Unit: dB.
  double level_db;
} sdrkit_hopper_row;

// The result fields of `birdie`.
typedef struct sdrkit_birdie_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The persistent carriers listed now.
  int64_t carriers;
  // The snapshots taken.
  int64_t snapshots;
  // The time of a snapshot. A length of time in seconds.
  double snapshot_s;
  // The width of a bin. Unit: Hz.
  double bin_hz;
} sdrkit_birdie_fields;

// The options of `class_` of `sdrkit_birdie_carrier_row`.
typedef enum sdrkit_birdie_carrier_row_class_ {
  // Bare carrier
  SDRKIT_BIRDIE_CARRIER_ROW_CLASS__BARE = 0,
  // Modulated or wide
  SDRKIT_BIRDIE_CARRIER_ROW_CLASS__MODULATED = 1,
} sdrkit_birdie_carrier_row_class_;

// A row of the list of persistent carriers, the strongest first.
typedef struct sdrkit_birdie_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The smoothed frequency. A frequency in Hz.
  double frequency;
  // The mean level over the noise per bin. Unit: dB.
  double level_db;
  // The standard deviation of the level. Unit: dB.
  double level_sd_db;
  // The class. A value of `sdrkit_birdie_carrier_row_class_`.
  int32_t class_;
  // The least-squares drift of the frequency. Unit: Hz/min.
  double drift_hz_min;
  // The RMS of the frequency about the drift line. Unit: Hz.
  double scatter_hz;
  // The snapshots of the presence window in which it was seen.
  int64_t present;
  // The snapshots of the presence window.
  int64_t of;
} sdrkit_birdie_carrier_row;

// The options of `kind` of `sdrkit_birdie_event_row`.
typedef enum sdrkit_birdie_event_row_kind {
  // New
  SDRKIT_BIRDIE_EVENT_ROW_KIND_NEW = 0,
  // Returned
  SDRKIT_BIRDIE_EVENT_ROW_KIND_RETURNED = 1,
  // Gone
  SDRKIT_BIRDIE_EVENT_ROW_KIND_GONE = 2,
} sdrkit_birdie_event_row_kind;

// The options of `class_` of `sdrkit_birdie_event_row`.
typedef enum sdrkit_birdie_event_row_class_ {
  // Bare carrier
  SDRKIT_BIRDIE_EVENT_ROW_CLASS__BARE = 0,
  // Modulated or wide
  SDRKIT_BIRDIE_EVENT_ROW_CLASS__MODULATED = 1,
} sdrkit_birdie_event_row_class_;

// A row of the carrier log.
typedef struct sdrkit_birdie_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the row. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_birdie_event_row_kind`.
  int32_t kind;
  // The frequency. A frequency in Hz.
  double frequency;
  // The mean level over the noise. Unit: dB.
  double level_db;
  // The class. A value of `sdrkit_birdie_event_row_class_`.
  int32_t class_;
  // The drift. Unit: Hz/min.
  double drift_hz_min;
  // The scatter. Unit: Hz.
  double scatter_hz;
} sdrkit_birdie_event_row;

// The result fields of `obw`. The readings have no value while no signal is found.
typedef struct sdrkit_obw_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `obw_khz` holds a value.
  bool has_obw_khz;
  // The band that holds the power share, half the rest below and half above it. Unit: kHz.
  double obw_khz;
  // True if `obw_low` holds a value.
  bool has_obw_low;
  // The lower edge of the occupied band. A frequency in Hz.
  double obw_low;
  // True if `obw_high` holds a value.
  bool has_obw_high;
  // The upper edge of the occupied band. A frequency in Hz.
  double obw_high;
  // True if `bw3_khz` holds a value.
  bool has_bw3_khz;
  // The width at 3 dB under the peak of the smoothed spectrum. Unit: kHz.
  double bw3_khz;
  // True if `bw6_khz` holds a value.
  bool has_bw6_khz;
  // The width at 6 dB under the peak. Unit: kHz.
  double bw6_khz;
  // True if `bw20_khz` holds a value.
  bool has_bw20_khz;
  // The width at 20 dB under the peak. Unit: kHz.
  double bw20_khz;
  // True if `bw26_khz` holds a value.
  bool has_bw26_khz;
  // The width at 26 dB under the peak. Unit: kHz.
  double bw26_khz;
  // True if `centre_offset_hz` holds a value.
  bool has_centre_offset_hz;
  // The middle of the occupied band from the tuned frequency. Unit: Hz.
  double centre_offset_hz;
  // True if `power_db` holds a value.
  bool has_power_db;
  // The power between the edges over the floor, dB re full scale. Unit: dB.
  double power_db;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal power over the floor in the same bins. Unit: dB.
  double snr_db;
  // True if `adjacent_low_db` holds a value.
  bool has_adjacent_low_db;
  // The power of the band one adjacent offset below, under the signal; no value outside the spectrum. Unit: dB.
  double adjacent_low_db;
  // True if `adjacent_high_db` holds a value.
  bool has_adjacent_high_db;
  // The same above. Unit: dB.
  double adjacent_high_db;
  // True if `alternate_low_db` holds a value.
  bool has_alternate_low_db;
  // The power of the band two adjacent offsets below, under the signal. Unit: dB.
  double alternate_low_db;
  // True if `alternate_high_db` holds a value.
  bool has_alternate_high_db;
  // The same above. Unit: dB.
  double alternate_high_db;
  // A signal was found in the last evaluation.
  bool signal;
} sdrkit_obw_fields;

// A row of the bandwidth log.
typedef struct sdrkit_obw_bandwidth_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the evaluation. A length of time in seconds.
  double time_s;
  // The band that holds the power share, half the rest below and half above it. Unit: kHz.
  double obw_khz;
  // The lower edge of the occupied band. A frequency in Hz.
  double obw_low;
  // The upper edge of the occupied band. A frequency in Hz.
  double obw_high;
  // The width at 3 dB under the peak of the smoothed spectrum. Unit: kHz.
  double bw3_khz;
  // The width at 6 dB under the peak. Unit: kHz.
  double bw6_khz;
  // The width at 20 dB under the peak. Unit: kHz.
  double bw20_khz;
  // The width at 26 dB under the peak. Unit: kHz.
  double bw26_khz;
  // The middle of the occupied band from the tuned frequency. Unit: Hz.
  double centre_offset_hz;
  // The power between the edges over the floor, dB re full scale. Unit: dB.
  double power_db;
  // The signal power over the floor in the same bins. Unit: dB.
  double snr_db;
  // True if `adjacent_low_db` holds a value.
  bool has_adjacent_low_db;
  // The power of the band one adjacent offset below, under the signal; no value outside the spectrum. Unit: dB.
  double adjacent_low_db;
  // True if `adjacent_high_db` holds a value.
  bool has_adjacent_high_db;
  // The same above. Unit: dB.
  double adjacent_high_db;
  // True if `alternate_low_db` holds a value.
  bool has_alternate_low_db;
  // The power of the band two adjacent offsets below, under the signal. Unit: dB.
  double alternate_low_db;
  // True if `alternate_high_db` holds a value.
  bool has_alternate_high_db;
  // The same above. Unit: dB.
  double alternate_high_db;
} sdrkit_obw_bandwidth_row;

// The result fields of `ptt_log`.
typedef struct sdrkit_ptt_log_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The channel is keyed now.
  bool keyed;
  // True if `keyed_s` holds a value.
  bool has_keyed_s;
  // The length of the present transmission; no value while idle. A length of time in seconds.
  double keyed_s;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The floor of the channel, relative; no value before the first reading. Unit: dB.
  double floor_db;
  // The transmissions that ended.
  int64_t transmissions;
  // Those shorter than the kerchunk length.
  int64_t kerchunks;
  // True if `duty_pct` holds a value.
  bool has_duty_pct;
  // The keyed share of the time since the end of the warm-up, the present transmission included. Unit: %.
  double duty_pct;
  // True if `duty_10min_pct` holds a value.
  bool has_duty_10min_pct;
  // The keyed share of the last 10 minutes (or of the time measured, when shorter). Unit: %.
  double duty_10min_pct;
  // True if `mean_s` holds a value.
  bool has_mean_s;
  // The mean length of the transmissions that ended. A length of time in seconds.
  double mean_s;
  // True if `median_s` holds a value.
  bool has_median_s;
  // The median length. A length of time in seconds.
  double median_s;
  // True if `longest_s` holds a value.
  bool has_longest_s;
  // The longest. A length of time in seconds.
  double longest_s;
  // True if `per_hour` holds a value.
  bool has_per_hour;
  // Transmissions per hour of the time measured. Unit: 1/h.
  double per_hour;
  // The time measured since the end of the warm-up. A length of time in seconds.
  double elapsed_s;
} sdrkit_ptt_log_fields;

// The options of `kind` of `sdrkit_ptt_log_transmission_row`.
typedef enum sdrkit_ptt_log_transmission_row_kind {
  // Transmission
  SDRKIT_PTT_LOG_TRANSMISSION_ROW_KIND_TRANSMISSION = 0,
  // Kerchunk
  SDRKIT_PTT_LOG_TRANSMISSION_ROW_KIND_KERCHUNK = 1,
  // Stuck key
  SDRKIT_PTT_LOG_TRANSMISSION_ROW_KIND_STUCK_KEY = 2,
} sdrkit_ptt_log_transmission_row_kind;

// A row of the transmission log.
typedef struct sdrkit_ptt_log_transmission_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of the transmission, from 1.
  int64_t number;
  // The kind. A value of `sdrkit_ptt_log_transmission_row_kind`.
  int32_t kind;
  // The start, signal time. A length of time in seconds.
  double start_s;
  // The length (so far, for a stuck key). A length of time in seconds.
  double length_s;
  // The mean level over the floor of its keyed blocks. Unit: dB.
  double over_floor_db;
  // True if `gap_s` holds a value.
  bool has_gap_s;
  // The time from the end of the transmission before; no value for the first. A length of time in seconds.
  double gap_s;
  // The mean level of its keyed blocks, relative to full scale. Unit: dB.
  double level_db;
} sdrkit_ptt_log_transmission_row;

// The options of `class_` of `sdrkit_noise_locator_fields`.
typedef enum sdrkit_noise_locator_fields_class_ {
  // Twice the mains
  SDRKIT_NOISE_LOCATOR_FIELDS_CLASS__TWICE_MAINS = 0,
  // The mains
  SDRKIT_NOISE_LOCATOR_FIELDS_CLASS__MAINS = 1,
  // Other steady rate
  SDRKIT_NOISE_LOCATOR_FIELDS_CLASS__STEADY = 2,
  // None
  SDRKIT_NOISE_LOCATOR_FIELDS_CLASS__NONE = 3,
} sdrkit_noise_locator_fields_class_;

// The result fields of `noise_locator`.
typedef struct sdrkit_noise_locator_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `impulse_rate` holds a value.
  bool has_impulse_rate;
  // The impulses per second of the impulse window. Unit: 1/s.
  double impulse_rate;
  // True if `impulse_level_db` holds a value.
  bool has_impulse_level_db;
  // The median impulse level over the noise of the analysis band. Unit: dB.
  double impulse_level_db;
  // True if `impulse_spread_db` holds a value.
  bool has_impulse_spread_db;
  // The 75th minus the 25th percentile of the impulse levels. Unit: dB.
  double impulse_spread_db;
  // True if `repetition_hz` holds a value.
  bool has_repetition_hz;
  // The repetition rate of the present pattern; no value without one. Unit: Hz.
  double repetition_hz;
  // The class of the present pattern. A value of `sdrkit_noise_locator_fields_class_`.
  int32_t class_;
  // True if `mains_found_hz` holds a value.
  bool has_mains_found_hz;
  // The mains frequency of the class, when it names one. Unit: Hz.
  double mains_found_hz;
  // True if `strength_pct` holds a value.
  bool has_strength_pct;
  // The share of the impulses on the rate. Unit: %.
  double strength_pct;
  // The impulses since the start.
  int64_t impulses;
} sdrkit_noise_locator_fields;

// The options of `kind` of `sdrkit_noise_locator_pattern_row`.
typedef enum sdrkit_noise_locator_pattern_row_kind {
  // Found
  SDRKIT_NOISE_LOCATOR_PATTERN_ROW_KIND_FOUND = 0,
  // Gone
  SDRKIT_NOISE_LOCATOR_PATTERN_ROW_KIND_GONE = 1,
} sdrkit_noise_locator_pattern_row_kind;

// The options of `class_` of `sdrkit_noise_locator_pattern_row`.
typedef enum sdrkit_noise_locator_pattern_row_class_ {
  // Twice the mains
  SDRKIT_NOISE_LOCATOR_PATTERN_ROW_CLASS__TWICE_MAINS = 0,
  // The mains
  SDRKIT_NOISE_LOCATOR_PATTERN_ROW_CLASS__MAINS = 1,
  // Other steady rate
  SDRKIT_NOISE_LOCATOR_PATTERN_ROW_CLASS__STEADY = 2,
  // None
  SDRKIT_NOISE_LOCATOR_PATTERN_ROW_CLASS__NONE = 3,
} sdrkit_noise_locator_pattern_row_class_;

// A row of the pattern log.
typedef struct sdrkit_noise_locator_pattern_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the report. A length of time in seconds.
  double time_s;
  // Found or gone. A value of `sdrkit_noise_locator_pattern_row_kind`.
  int32_t kind;
  // The repetition rate. Unit: Hz.
  double repetition_hz;
  // The class of the rate. A value of `sdrkit_noise_locator_pattern_row_class_`.
  int32_t class_;
  // The impulses per second of the window. Unit: 1/s.
  double impulse_rate;
  // The median impulse level over the noise of the analysis band. Unit: dB.
  double level_db;
  // The share of the impulses on the rate. Unit: %.
  double strength_pct;
} sdrkit_noise_locator_pattern_row;

// The result fields of `freq_accuracy`. The readings of the carrier have no value while the carrier is absent.
typedef struct sdrkit_freq_accuracy_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The mean offset of the last 10 readings from the tuned frequency. A frequency in Hz.
  double offset_hz;
  // True if `scatter_hz` holds a value.
  bool has_scatter_hz;
  // The sample standard deviation of the last 10 readings. A frequency in Hz.
  double scatter_hz;
  // True if `error_ppm` holds a value.
  bool has_error_ppm;
  // The offset over the tuned frequency; no value without the tuned frequency. Unit: ppm.
  double error_ppm;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // The carrier to noise density of the last window. Unit: dB-Hz.
  double cn0_dbhz;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The least-squares slope of the last 120 readings; from 8 readings. Unit: Hz/s.
  double drift_hz_s;
  // True if `drift_ppm_h` holds a value.
  bool has_drift_ppm_h;
  // The drift over the tuned frequency. Unit: ppm/h.
  double drift_ppm_h;
  // The readings kept.
  int64_t readings;
  // True if `window_s` holds a value.
  bool has_window_s;
  // The length of one reading window. A length of time in seconds.
  double window_s;
  // True if `carrier` holds a value.
  bool has_carrier;
  // The carrier was present in the last window.
  bool carrier;
} sdrkit_freq_accuracy_fields;

// A row of the Allan deviation table: one averaging time.
typedef struct sdrkit_freq_accuracy_allan_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The averaging time. A length of time in seconds.
  double tau_s;
  // True if `adev` holds a value.
  bool has_adev;
  // The fractional Allan deviation; no value without the tuned frequency.
  double adev;
  // The Allan deviation of the offset. A frequency in Hz.
  double adev_hz;
  // The groups it is from.
  int64_t groups;
} sdrkit_freq_accuracy_allan_row;

// The options of `kind` of `sdrkit_freq_accuracy_event_row`.
typedef enum sdrkit_freq_accuracy_event_row_kind {
  // Frequency
  SDRKIT_FREQ_ACCURACY_EVENT_ROW_KIND_FREQUENCY = 0,
  // Step
  SDRKIT_FREQ_ACCURACY_EVENT_ROW_KIND_STEP = 1,
  // Lost
  SDRKIT_FREQ_ACCURACY_EVENT_ROW_KIND_LOST = 2,
  // Returned
  SDRKIT_FREQ_ACCURACY_EVENT_ROW_KIND_RETURNED = 3,
} sdrkit_freq_accuracy_event_row_kind;

// A row of the `freq_accuracy` log.
typedef struct sdrkit_freq_accuracy_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_freq_accuracy_event_row_kind`.
  int32_t kind;
  // The offset of the reading (for a loss, the last reading). A frequency in Hz.
  double offset_hz;
  // True if `step_hz` holds a value.
  bool has_step_hz;
  // The size of a step. A frequency in Hz.
  double step_hz;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // The carrier to noise density of the reading. Unit: dB-Hz.
  double cn0_dbhz;
} sdrkit_freq_accuracy_event_row;

// The result fields of `sinad`, from the last window. The readings have no value while no test tone is found.
typedef struct sdrkit_sinad_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `sinad_db` holds a value.
  bool has_sinad_db;
  // SINAD in the audio band, flat weighting. Unit: dB.
  double sinad_db;
  // True if `below_12db` holds a value.
  bool has_below_12db;
  // The SINAD is under the sensitivity mark (12 dB by default).
  bool below_12db;
  // True if `tone_hz` holds a value.
  bool has_tone_hz;
  // The test tone. A frequency in Hz.
  double tone_hz;
  // True if `deviation_khz` holds a value.
  bool has_deviation_khz;
  // The peak deviation of the tone. Unit: kHz.
  double deviation_khz;
  // True if `thd_pct` holds a value.
  bool has_thd_pct;
  // THD of harmonics 2 to 5. Unit: %.
  double thd_pct;
  // True if `thdn_pct` holds a value.
  bool has_thdn_pct;
  // THD and noise. Unit: %.
  double thdn_pct;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset: the mean of the discriminator audio. A frequency in Hz.
  double carrier_offset_hz;
  // The windows measured.
  int64_t windows;
} sdrkit_sinad_fields;

// The options of `crossing` of `sdrkit_sinad_event_row`.
typedef enum sdrkit_sinad_event_row_crossing {
  // None
  SDRKIT_SINAD_EVENT_ROW_CROSSING_NONE = 0,
  // Fell under the mark
  SDRKIT_SINAD_EVENT_ROW_CROSSING_FELL = 1,
  // Rose to the mark
  SDRKIT_SINAD_EVENT_ROW_CROSSING_ROSE = 2,
} sdrkit_sinad_event_row_crossing;

// A row of the `sinad` log.
typedef struct sdrkit_sinad_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // SINAD. Unit: dB.
  double sinad_db;
  // The change against the mark. A value of `sdrkit_sinad_event_row_crossing`.
  int32_t crossing;
  // The test tone. A frequency in Hz.
  double tone_hz;
  // The peak deviation. Unit: kHz.
  double deviation_khz;
  // THD. Unit: %.
  double thd_pct;
} sdrkit_sinad_event_row;

// The result fields of `nfm_meter`, from the last window. The voice readings have no value without a carrier, the CTCSS readings none without a tone.
typedef struct sdrkit_nfm_meter_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `voice_peak_khz` holds a value.
  bool has_voice_peak_khz;
  // The 99.9th percentile of the voice band deviation of the window. Unit: kHz.
  double voice_peak_khz;
  // True if `voice_peak_max_khz` holds a value.
  bool has_voice_peak_max_khz;
  // The largest voice peak so far. Unit: kHz.
  double voice_peak_max_khz;
  // Windows over the narrow channel limit.
  int64_t over_narrow;
  // Windows over the wide channel limit.
  int64_t over_wide;
  // Windows measured.
  int64_t windows;
  // True if `ctcss_hz` holds a value.
  bool has_ctcss_hz;
  // The CTCSS tone measured. A frequency in Hz.
  double ctcss_hz;
  // True if `ctcss_standard_hz` holds a value.
  bool has_ctcss_standard_hz;
  // The nearest standard tone. A frequency in Hz.
  double ctcss_standard_hz;
  // True if `ctcss_error_hz` holds a value.
  bool has_ctcss_error_hz;
  // Measured minus standard. A frequency in Hz.
  double ctcss_error_hz;
  // True if `ctcss_deviation_hz` holds a value.
  bool has_ctcss_deviation_hz;
  // The peak deviation of the CTCSS tone. A frequency in Hz.
  double ctcss_deviation_hz;
  // True if `ctcss_share_narrow_pct` holds a value.
  bool has_ctcss_share_narrow_pct;
  // The CTCSS deviation as a share of the narrow limit. Unit: %.
  double ctcss_share_narrow_pct;
  // True if `ctcss_share_wide_pct` holds a value.
  bool has_ctcss_share_wide_pct;
  // The CTCSS deviation as a share of the wide limit. Unit: %.
  double ctcss_share_wide_pct;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset: the mean of the discriminator audio. A frequency in Hz.
  double carrier_offset_hz;
} sdrkit_nfm_meter_fields;

// The options of `kind` of `sdrkit_nfm_meter_event_row`.
typedef enum sdrkit_nfm_meter_event_row_kind {
  // Tone
  SDRKIT_NFM_METER_EVENT_ROW_KIND_TONE = 0,
  // Tone gone
  SDRKIT_NFM_METER_EVENT_ROW_KIND_TONE_GONE = 1,
  // Over narrow
  SDRKIT_NFM_METER_EVENT_ROW_KIND_OVER_NARROW = 2,
  // Over wide
  SDRKIT_NFM_METER_EVENT_ROW_KIND_OVER_WIDE = 3,
} sdrkit_nfm_meter_event_row_kind;

// A row of the `nfm_meter` log.
typedef struct sdrkit_nfm_meter_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_nfm_meter_event_row_kind`.
  int32_t kind;
  // True if `ctcss_standard_hz` holds a value.
  bool has_ctcss_standard_hz;
  // The nearest standard tone. A frequency in Hz.
  double ctcss_standard_hz;
  // True if `ctcss_hz` holds a value.
  bool has_ctcss_hz;
  // The CTCSS tone measured. A frequency in Hz.
  double ctcss_hz;
  // True if `ctcss_deviation_hz` holds a value.
  bool has_ctcss_deviation_hz;
  // The CTCSS deviation. A frequency in Hz.
  double ctcss_deviation_hz;
  // True if `voice_peak_khz` holds a value.
  bool has_voice_peak_khz;
  // The voice peak of the window. Unit: kHz.
  double voice_peak_khz;
} sdrkit_nfm_meter_event_row;

// The result fields of `tx_stability`: the transmission now, and the last analysed transmission.
typedef struct sdrkit_tx_stability_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A transmission is on.
  bool transmitting;
  // True if `length_s` holds a value.
  bool has_length_s;
  // The length of the transmission. A length of time in seconds.
  double length_s;
  // True if `readings` holds a value.
  bool has_readings;
  // The frequency readings in it.
  int64_t readings;
  // True if `start_hz` holds a value.
  bool has_start_hz;
  // The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency. A frequency in Hz.
  double start_hz;
  // True if `final_hz` holds a value.
  bool has_final_hz;
  // The mean frequency of the last 0.4 s. A frequency in Hz.
  double final_hz;
  // True if `keyup_step_hz` holds a value.
  bool has_keyup_step_hz;
  // Start minus final. A frequency in Hz.
  double keyup_step_hz;
  // True if `settle_s` holds a value.
  bool has_settle_s;
  // The time of the last reading outside the settling band, plus 0.05 s. A length of time in seconds.
  double settle_s;
  // True if `settle_band_hz` holds a value.
  bool has_settle_band_hz;
  // The larger of 10 Hz and a tenth of the key-up step. A frequency in Hz.
  double settle_band_hz;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The least-squares slope of the readings of the second half. Unit: Hz/s.
  double drift_hz_s;
  // True if `mean_hz` holds a value.
  bool has_mean_hz;
  // The mean of all readings. A frequency in Hz.
  double mean_hz;
  // True if `mean_ppm` holds a value.
  bool has_mean_ppm;
  // The mean over the tuned frequency; no value without it. Unit: ppm.
  double mean_ppm;
  // True if `scatter_hz` holds a value.
  bool has_scatter_hz;
  // The RMS residual about the drift line. A frequency in Hz.
  double scatter_hz;
  // The transmissions seen, the short ones included.
  int64_t transmissions;
} sdrkit_tx_stability_fields;

// A row of the transmission table: one analysed transmission.
typedef struct sdrkit_tx_stability_transmission_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The key-up, signal time. A length of time in seconds.
  double time_s;
  // The length of the transmission. A length of time in seconds.
  double length_s;
  // The frequency readings in it.
  int64_t readings;
  // The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency. A frequency in Hz.
  double start_hz;
  // The mean frequency of the last 0.4 s. A frequency in Hz.
  double final_hz;
  // Start minus final. A frequency in Hz.
  double keyup_step_hz;
  // The time of the last reading outside the settling band, plus 0.05 s. A length of time in seconds.
  double settle_s;
  // The larger of 10 Hz and a tenth of the key-up step. A frequency in Hz.
  double settle_band_hz;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The least-squares slope of the readings of the second half. Unit: Hz/s.
  double drift_hz_s;
  // The mean of all readings. A frequency in Hz.
  double mean_hz;
  // True if `mean_ppm` holds a value.
  bool has_mean_ppm;
  // The mean over the tuned frequency; no value without it. Unit: ppm.
  double mean_ppm;
  // True if `scatter_hz` holds a value.
  bool has_scatter_hz;
  // The RMS residual about the drift line. A frequency in Hz.
  double scatter_hz;
} sdrkit_tx_stability_transmission_row;

// The options of `kind` of `sdrkit_ssb_test_fields`.
typedef enum sdrkit_ssb_test_fields_kind {
  // None
  SDRKIT_SSB_TEST_FIELDS_KIND_NONE = 0,
  // One tone
  SDRKIT_SSB_TEST_FIELDS_KIND_ONE_TONE = 1,
  // Two tones
  SDRKIT_SSB_TEST_FIELDS_KIND_TWO_TONES = 2,
} sdrkit_ssb_test_fields_kind;

// The options of `sideband` of `sdrkit_ssb_test_fields`.
typedef enum sdrkit_ssb_test_fields_sideband {
  // Upper
  SDRKIT_SSB_TEST_FIELDS_SIDEBAND_UPPER = 0,
  // Lower
  SDRKIT_SSB_TEST_FIELDS_SIDEBAND_LOWER = 1,
} sdrkit_ssb_test_fields_sideband;

// The result fields of `ssb_test`, from the last window. A level under the floor of the measurement is the floor with its flag set.
typedef struct sdrkit_ssb_test_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The test seen. A value of `sdrkit_ssb_test_fields_kind`.
  int32_t kind;
  // True if `sideband` holds a value.
  bool has_sideband;
  // The sideband of the tones. A value of `sdrkit_ssb_test_fields_sideband`.
  int32_t sideband;
  // True if `tone1_hz` holds a value.
  bool has_tone1_hz;
  // The audio frequency of the (lower) tone. A frequency in Hz.
  double tone1_hz;
  // True if `tone2_hz` holds a value.
  bool has_tone2_hz;
  // The audio frequency of the higher tone of two. A frequency in Hz.
  double tone2_hz;
  // True if `tone_level_diff_db` holds a value.
  bool has_tone_level_diff_db;
  // The stronger tone over the weaker. Unit: dB.
  double tone_level_diff_db;
  // True if `carrier_db` holds a value.
  bool has_carrier_db;
  // The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search: the leak is at least this far under the tone. Unit: dB.
  double carrier_db;
  // True if `carrier_under` holds a value.
  bool has_carrier_under;
  // No carrier is seen and the value is the floor of the carrier search, a bound.
  bool carrier_under;
  // True if `carrier_found` holds a value.
  bool has_carrier_found;
  // A carrier was seen; else it is assumed at the tuned frequency for the tones and products.
  bool carrier_found;
  // True if `opposite_db` holds a value.
  bool has_opposite_db;
  // The opposite sideband of the (lower) tone. Unit: dB.
  double opposite_db;
  // True if `opposite_under` holds a value.
  bool has_opposite_under;
  // The opposite sideband is under the floor.
  bool opposite_under;
  // True if `h2_db` holds a value.
  bool has_h2_db;
  // Harmonic 2 of the audio tone (one tone). Unit: dB.
  double h2_db;
  // True if `h3_db` holds a value.
  bool has_h3_db;
  // Harmonic 3 of the audio tone (one tone). Unit: dB.
  double h3_db;
  // True if `h4_db` holds a value.
  bool has_h4_db;
  // Harmonic 4 of the audio tone (one tone). Unit: dB.
  double h4_db;
  // True if `h5_db` holds a value.
  bool has_h5_db;
  // Harmonic 5 of the audio tone (one tone). Unit: dB.
  double h5_db;
  // True if `h2_under` holds a value.
  bool has_h2_under;
  // Harmonic 2 is under the floor.
  bool h2_under;
  // True if `h3_under` holds a value.
  bool has_h3_under;
  // Harmonic 3 is under the floor.
  bool h3_under;
  // True if `h4_under` holds a value.
  bool has_h4_under;
  // Harmonic 4 is under the floor.
  bool h4_under;
  // True if `h5_under` holds a value.
  bool has_h5_under;
  // Harmonic 5 is under the floor.
  bool h5_under;
  // True if `imd3_db` holds a value.
  bool has_imd3_db;
  // The stronger product of order 3 under the weaker tone (two tones). Unit: dB.
  double imd3_db;
  // True if `imd5_db` holds a value.
  bool has_imd5_db;
  // The stronger product of order 5 under the weaker tone (two tones). Unit: dB.
  double imd5_db;
  // True if `imd7_db` holds a value.
  bool has_imd7_db;
  // The stronger product of order 7 under the weaker tone (two tones). Unit: dB.
  double imd7_db;
  // True if `imd3_under` holds a value.
  bool has_imd3_under;
  // The products of order 3 are under the floor.
  bool imd3_under;
  // True if `imd5_under` holds a value.
  bool has_imd5_under;
  // The products of order 5 are under the floor.
  bool imd5_under;
  // True if `imd7_under` holds a value.
  bool has_imd7_under;
  // The products of order 7 are under the floor.
  bool imd7_under;
  // True if `imd3_pep_db` holds a value.
  bool has_imd3_pep_db;
  // The product of order 3 under the peak envelope power. Unit: dB.
  double imd3_pep_db;
  // True if `imd5_pep_db` holds a value.
  bool has_imd5_pep_db;
  // The product of order 5 under the peak envelope power. Unit: dB.
  double imd5_pep_db;
  // True if `imd7_pep_db` holds a value.
  bool has_imd7_pep_db;
  // The product of order 7 under the peak envelope power. Unit: dB.
  double imd7_pep_db;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The floor of this measurement under the tone: the noise of 7 bins. Unit: dB.
  double floor_db;
} sdrkit_ssb_test_fields;

// The options of `kind` of `sdrkit_ssb_test_event_row`.
typedef enum sdrkit_ssb_test_event_row_kind {
  // None
  SDRKIT_SSB_TEST_EVENT_ROW_KIND_NONE = 0,
  // One tone
  SDRKIT_SSB_TEST_EVENT_ROW_KIND_ONE_TONE = 1,
  // Two tones
  SDRKIT_SSB_TEST_EVENT_ROW_KIND_TWO_TONES = 2,
} sdrkit_ssb_test_event_row_kind;

// The options of `sideband` of `sdrkit_ssb_test_event_row`.
typedef enum sdrkit_ssb_test_event_row_sideband {
  // Upper
  SDRKIT_SSB_TEST_EVENT_ROW_SIDEBAND_UPPER = 0,
  // Lower
  SDRKIT_SSB_TEST_EVENT_ROW_SIDEBAND_LOWER = 1,
} sdrkit_ssb_test_event_row_sideband;

// A row of the `ssb_test` log.
typedef struct sdrkit_ssb_test_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The test seen. A value of `sdrkit_ssb_test_event_row_kind`.
  int32_t kind;
  // True if `sideband` holds a value.
  bool has_sideband;
  // The sideband of the tones. A value of `sdrkit_ssb_test_event_row_sideband`.
  int32_t sideband;
  // True if `tone1_hz` holds a value.
  bool has_tone1_hz;
  // The audio frequency of the (lower) tone. A frequency in Hz.
  double tone1_hz;
  // True if `tone2_hz` holds a value.
  bool has_tone2_hz;
  // The audio frequency of the higher tone of two. A frequency in Hz.
  double tone2_hz;
  // True if `tone_level_diff_db` holds a value.
  bool has_tone_level_diff_db;
  // The stronger tone over the weaker. Unit: dB.
  double tone_level_diff_db;
  // True if `carrier_db` holds a value.
  bool has_carrier_db;
  // The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search: the leak is at least this far under the tone. Unit: dB.
  double carrier_db;
  // True if `carrier_under` holds a value.
  bool has_carrier_under;
  // No carrier is seen and the value is the floor of the carrier search, a bound.
  bool carrier_under;
  // True if `carrier_found` holds a value.
  bool has_carrier_found;
  // A carrier was seen; else it is assumed at the tuned frequency for the tones and products.
  bool carrier_found;
  // True if `opposite_db` holds a value.
  bool has_opposite_db;
  // The opposite sideband of the (lower) tone. Unit: dB.
  double opposite_db;
  // True if `opposite_under` holds a value.
  bool has_opposite_under;
  // The opposite sideband is under the floor.
  bool opposite_under;
  // True if `h2_db` holds a value.
  bool has_h2_db;
  // Harmonic 2 of the audio tone (one tone). Unit: dB.
  double h2_db;
  // True if `h3_db` holds a value.
  bool has_h3_db;
  // Harmonic 3 of the audio tone (one tone). Unit: dB.
  double h3_db;
  // True if `h4_db` holds a value.
  bool has_h4_db;
  // Harmonic 4 of the audio tone (one tone). Unit: dB.
  double h4_db;
  // True if `h5_db` holds a value.
  bool has_h5_db;
  // Harmonic 5 of the audio tone (one tone). Unit: dB.
  double h5_db;
  // True if `h2_under` holds a value.
  bool has_h2_under;
  // Harmonic 2 is under the floor.
  bool h2_under;
  // True if `h3_under` holds a value.
  bool has_h3_under;
  // Harmonic 3 is under the floor.
  bool h3_under;
  // True if `h4_under` holds a value.
  bool has_h4_under;
  // Harmonic 4 is under the floor.
  bool h4_under;
  // True if `h5_under` holds a value.
  bool has_h5_under;
  // Harmonic 5 is under the floor.
  bool h5_under;
  // True if `imd3_db` holds a value.
  bool has_imd3_db;
  // The stronger product of order 3 under the weaker tone (two tones). Unit: dB.
  double imd3_db;
  // True if `imd5_db` holds a value.
  bool has_imd5_db;
  // The stronger product of order 5 under the weaker tone (two tones). Unit: dB.
  double imd5_db;
  // True if `imd7_db` holds a value.
  bool has_imd7_db;
  // The stronger product of order 7 under the weaker tone (two tones). Unit: dB.
  double imd7_db;
  // True if `imd3_under` holds a value.
  bool has_imd3_under;
  // The products of order 3 are under the floor.
  bool imd3_under;
  // True if `imd5_under` holds a value.
  bool has_imd5_under;
  // The products of order 5 are under the floor.
  bool imd5_under;
  // True if `imd7_under` holds a value.
  bool has_imd7_under;
  // The products of order 7 are under the floor.
  bool imd7_under;
  // True if `imd3_pep_db` holds a value.
  bool has_imd3_pep_db;
  // The product of order 3 under the peak envelope power. Unit: dB.
  double imd3_pep_db;
  // True if `imd5_pep_db` holds a value.
  bool has_imd5_pep_db;
  // The product of order 5 under the peak envelope power. Unit: dB.
  double imd5_pep_db;
  // True if `imd7_pep_db` holds a value.
  bool has_imd7_pep_db;
  // The product of order 7 under the peak envelope power. Unit: dB.
  double imd7_pep_db;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The floor of this measurement under the tone: the noise of 7 bins. Unit: dB.
  double floor_db;
} sdrkit_ssb_test_event_row;

// The result fields of `afsk_tuner`: the last analysed burst and the totals.
typedef struct sdrkit_afsk_tuner_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `mark_hz` holds a value.
  bool has_mark_hz;
  // The mark tone of the last burst. A frequency in Hz.
  double mark_hz;
  // True if `space_hz` holds a value.
  bool has_space_hz;
  // The space tone of the last burst. A frequency in Hz.
  double space_hz;
  // True if `mark_error_hz` holds a value.
  bool has_mark_error_hz;
  // The mark tone minus the set mark tone. A frequency in Hz.
  double mark_error_hz;
  // True if `space_error_hz` holds a value.
  bool has_space_error_hz;
  // The space tone minus the set space tone. A frequency in Hz.
  double space_error_hz;
  // True if `twist_db` holds a value.
  bool has_twist_db;
  // The twist of the last burst. Unit: dB.
  double twist_db;
  // True if `peak_deviation_khz` holds a value.
  bool has_peak_deviation_khz;
  // The peak deviation of the last burst. Unit: kHz.
  double peak_deviation_khz;
  // True if `length_s` holds a value.
  bool has_length_s;
  // The length of the last burst. A length of time in seconds.
  double length_s;
  // True if `gap_s` holds a value.
  bool has_gap_s;
  // The gap before the last burst. A length of time in seconds.
  double gap_s;
  // The bursts analysed.
  int64_t bursts;
  // True if `deviation_min_khz` holds a value.
  bool has_deviation_min_khz;
  // The least peak deviation over the bursts. Unit: kHz.
  double deviation_min_khz;
  // True if `deviation_max_khz` holds a value.
  bool has_deviation_max_khz;
  // The largest peak deviation over the bursts. Unit: kHz.
  double deviation_max_khz;
} sdrkit_afsk_tuner_fields;

// A row of the burst table.
typedef struct sdrkit_afsk_tuner_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The start of the burst, signal time. A length of time in seconds.
  double time_s;
  // The length of the burst. A length of time in seconds.
  double length_s;
  // The mark tone. A frequency in Hz.
  double mark_hz;
  // The space tone. A frequency in Hz.
  double space_hz;
  // The space-band power over the mark-band power. Unit: dB.
  double twist_db;
  // The 99.5th percentile of the band-limited audio. Unit: kHz.
  double peak_deviation_khz;
  // True if `gap_s` holds a value.
  bool has_gap_s;
  // The time since the end of the burst before; no value for the first. A length of time in seconds.
  double gap_s;
} sdrkit_afsk_tuner_burst_row;

// The result fields of `phase_noise`, from the average. They have no value before 4 blocks are averaged.
typedef struct sdrkit_phase_noise_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `l_10hz` holds a value.
  bool has_l_10hz;
  // Single-sideband phase noise at 10 Hz from the carrier. Unit: dBc/Hz.
  double l_10hz;
  // True if `l_100hz` holds a value.
  bool has_l_100hz;
  // Single-sideband phase noise at 100 Hz from the carrier. Unit: dBc/Hz.
  double l_100hz;
  // True if `l_1khz` holds a value.
  bool has_l_1khz;
  // Single-sideband phase noise at 1 kHz from the carrier. Unit: dBc/Hz.
  double l_1khz;
  // True if `l_10khz` holds a value.
  bool has_l_10khz;
  // Single-sideband phase noise at 10 kHz from the carrier. Unit: dBc/Hz.
  double l_10khz;
  // True if `l_100khz` holds a value.
  bool has_l_100khz;
  // Single-sideband phase noise at 100 kHz from the carrier. Unit: dBc/Hz.
  double l_100khz;
  // True if `jitter_mrad` holds a value.
  bool has_jitter_mrad;
  // The RMS phase jitter of the integration band. Unit: mrad.
  double jitter_mrad;
  // True if `jitter_ps` holds a value.
  bool has_jitter_ps;
  // The jitter as time at the tuned frequency; no value without it. Unit: ps.
  double jitter_ps;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset from the tuned frequency. A frequency in Hz.
  double carrier_offset_hz;
  // True if `cnr_db` holds a value.
  bool has_cnr_db;
  // The carrier over the noise in 200 kHz. Unit: dB.
  double cnr_db;
  // The blocks in the average.
  int64_t blocks;
  // True if `block_s` holds a value.
  bool has_block_s;
  // The length of a block. A length of time in seconds.
  double block_s;
  // True if `bin_hz` holds a value.
  bool has_bin_hz;
  // The width of a bin. A frequency in Hz.
  double bin_hz;
  // True if `enbw_bins` holds a value.
  bool has_enbw_bins;
  // The equivalent noise bandwidth of the window. Unit: bins.
  double enbw_bins;
} sdrkit_phase_noise_fields;

// A row of the spur table.
typedef struct sdrkit_phase_noise_spur_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset of the spur from the carrier, upper side. A frequency in Hz.
  double offset_hz;
  // The level of the spur. Unit: dBc.
  double level_dbc;
} sdrkit_phase_noise_spur_row;

// The options of `kind` of `sdrkit_phase_noise_event_row`.
typedef enum sdrkit_phase_noise_event_row_kind {
  // Reading
  SDRKIT_PHASE_NOISE_EVENT_ROW_KIND_READING = 0,
  // Lost
  SDRKIT_PHASE_NOISE_EVENT_ROW_KIND_LOST = 1,
} sdrkit_phase_noise_event_row_kind;

// A row of the `phase_noise` log.
typedef struct sdrkit_phase_noise_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_phase_noise_event_row_kind`.
  int32_t kind;
  // True if `l_10hz` holds a value.
  bool has_l_10hz;
  // Single-sideband phase noise at 10 Hz from the carrier. Unit: dBc/Hz.
  double l_10hz;
  // True if `l_100hz` holds a value.
  bool has_l_100hz;
  // Single-sideband phase noise at 100 Hz from the carrier. Unit: dBc/Hz.
  double l_100hz;
  // True if `l_1khz` holds a value.
  bool has_l_1khz;
  // Single-sideband phase noise at 1 kHz from the carrier. Unit: dBc/Hz.
  double l_1khz;
  // True if `l_10khz` holds a value.
  bool has_l_10khz;
  // Single-sideband phase noise at 10 kHz from the carrier. Unit: dBc/Hz.
  double l_10khz;
  // True if `l_100khz` holds a value.
  bool has_l_100khz;
  // Single-sideband phase noise at 100 kHz from the carrier. Unit: dBc/Hz.
  double l_100khz;
  // True if `jitter_mrad` holds a value.
  bool has_jitter_mrad;
  // The jitter. Unit: mrad.
  double jitter_mrad;
  // The spurs listed.
  int64_t spurs;
} sdrkit_phase_noise_event_row;

// The result fields of `cw_keying`: the evaluation of the last marks and spaces. The evaluation has no value before 10 marks and when no mark came in the last 8 s.
typedef struct sdrkit_cw_keying_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `wpm` holds a value.
  bool has_wpm;
  // The speed in words a minute of 50 units. Unit: wpm.
  double wpm;
  // True if `dit_ms` holds a value.
  bool has_dit_ms;
  // The mean dit. Unit: ms.
  double dit_ms;
  // True if `dah_ms` holds a value.
  bool has_dah_ms;
  // The mean dah; no value with fewer than 2 dahs. Unit: ms.
  double dah_ms;
  // True if `dah_ratio` holds a value.
  bool has_dah_ratio;
  // The mean dah over the mean dit.
  double dah_ratio;
  // True if `element_gap` holds a value.
  bool has_element_gap;
  // The mean gap inside a character, in dits. Unit: dits.
  double element_gap;
  // True if `char_gap` holds a value.
  bool has_char_gap;
  // The mean gap between characters, in dits. Unit: dits.
  double char_gap;
  // True if `word_gap` holds a value.
  bool has_word_gap;
  // The mean gap between words, in dits. Unit: dits.
  double word_gap;
  // True if `char_gaps` holds a value.
  bool has_char_gaps;
  // The character gaps counted.
  int64_t char_gaps;
  // True if `word_gaps` holds a value.
  bool has_word_gaps;
  // The word gaps counted.
  int64_t word_gaps;
  // True if `weight` holds a value.
  bool has_weight;
  // The dit length over the element gap length.
  double weight;
  // True if `dit_scatter_pct` holds a value.
  bool has_dit_scatter_pct;
  // The population standard deviation of the dits over their mean. Unit: %.
  double dit_scatter_pct;
  // True if `rise_ms` holds a value.
  bool has_rise_ms;
  // The mean 10 to 90 % rise of the marks; 1.5 ms is the least the envelope filter shows. Unit: ms.
  double rise_ms;
  // True if `fall_ms` holds a value.
  bool has_fall_ms;
  // The mean 90 to 10 % fall of the marks. Unit: ms.
  double fall_ms;
  // True if `chirp_hz` holds a value.
  bool has_chirp_hz;
  // The mean frequency of the start of a mark minus that of its second half. Unit: Hz.
  double chirp_hz;
  // True if `chirp_marks` holds a value.
  bool has_chirp_marks;
  // The marks the chirp is from.
  int64_t chirp_marks;
  // The marks in the window.
  int64_t marks;
  // The spaces in the window.
  int64_t spaces;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset from the tuned frequency. A frequency in Hz.
  double carrier_offset_hz;
} sdrkit_cw_keying_fields;

// A row of the `cw_keying` log.
typedef struct sdrkit_cw_keying_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The speed. Unit: wpm.
  double wpm;
  // The mean dit. Unit: ms.
  double dit_ms;
  // True if `dah_ratio` holds a value.
  bool has_dah_ratio;
  // The dah ratio.
  double dah_ratio;
  // True if `element_gap` holds a value.
  bool has_element_gap;
  // The element gap. Unit: dits.
  double element_gap;
  // True if `char_gap` holds a value.
  bool has_char_gap;
  // The character gap. Unit: dits.
  double char_gap;
  // True if `word_gap` holds a value.
  bool has_word_gap;
  // The word gap. Unit: dits.
  double word_gap;
  // True if `weight` holds a value.
  bool has_weight;
  // The weight.
  double weight;
  // The dit scatter. Unit: %.
  double dit_scatter_pct;
  // True if `rise_ms` holds a value.
  bool has_rise_ms;
  // The rise. Unit: ms.
  double rise_ms;
  // True if `fall_ms` holds a value.
  bool has_fall_ms;
  // The fall. Unit: ms.
  double fall_ms;
  // True if `chirp_hz` holds a value.
  bool has_chirp_hz;
  // The chirp. Unit: Hz.
  double chirp_hz;
} sdrkit_cw_keying_event_row;

// The options of `levels` of `sdrkit_fsk_analyser_fields`.
typedef enum sdrkit_fsk_analyser_fields_levels {
  // Two levels
  SDRKIT_FSK_ANALYSER_FIELDS_LEVELS_TWO_LEVELS = 0,
  // Four levels
  SDRKIT_FSK_ANALYSER_FIELDS_LEVELS_FOUR_LEVELS = 1,
} sdrkit_fsk_analyser_fields_levels;

// The result fields of `fsk_analyser`, from the last block. They have no value while no FSK signal is found.
typedef struct sdrkit_fsk_analyser_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `levels` holds a value.
  bool has_levels;
  // The number of levels. A value of `sdrkit_fsk_analyser_fields_levels`.
  int32_t levels;
  // True if `msk_like` holds a value.
  bool has_msk_like;
  // Two levels with a modulation index of 0.5.
  bool msk_like;
  // True if `symbol_rate_bd` holds a value.
  bool has_symbol_rate_bd;
  // The symbol rate. Unit: Bd.
  double symbol_rate_bd;
  // True if `shift_hz` holds a value.
  bool has_shift_hz;
  // Two levels: the distance of the tones; four: the mean distance of neighbours. A frequency in Hz.
  double shift_hz;
  // True if `outer_hz` holds a value.
  bool has_outer_hz;
  // The distance of the outer levels. A frequency in Hz.
  double outer_hz;
  // True if `mod_index` holds a value.
  bool has_mod_index;
  // The outer distance over the symbol rate.
  double mod_index;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The centre of the levels from the tuned frequency. A frequency in Hz.
  double centre_hz;
  // True if `spacing_ratio` holds a value.
  bool has_spacing_ratio;
  // Four levels: the mean outer spacing over the inner spacing.
  double spacing_ratio;
  // True if `spacing_equal` holds a value.
  bool has_spacing_equal;
  // Four levels: the spacing ratio is within 0.35 of 1.
  bool spacing_equal;
  // True if `eye_snr_db` holds a value.
  bool has_eye_snr_db;
  // The level spacing over the spread inside a level. Unit: dB.
  double eye_snr_db;
} sdrkit_fsk_analyser_fields;

// A row of the level table.
typedef struct sdrkit_fsk_analyser_level_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The place of the level from the lowest, from 0.
  int64_t index;
  // The frequency of the level from the tuned frequency. A frequency in Hz.
  double frequency_hz;
} sdrkit_fsk_analyser_level_row;

// The options of `levels` of `sdrkit_fsk_analyser_event_row`.
typedef enum sdrkit_fsk_analyser_event_row_levels {
  // Two levels
  SDRKIT_FSK_ANALYSER_EVENT_ROW_LEVELS_TWO_LEVELS = 0,
  // Four levels
  SDRKIT_FSK_ANALYSER_EVENT_ROW_LEVELS_FOUR_LEVELS = 1,
} sdrkit_fsk_analyser_event_row_levels;

// A row of the `fsk_analyser` log.
typedef struct sdrkit_fsk_analyser_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The number of levels. A value of `sdrkit_fsk_analyser_event_row_levels`.
  int32_t levels;
  // The symbol rate. Unit: Bd.
  double symbol_rate_bd;
  // The shift. A frequency in Hz.
  double shift_hz;
  // The modulation index.
  double mod_index;
  // The centre. A frequency in Hz.
  double centre_hz;
  // The eye SNR. Unit: dB.
  double eye_snr_db;
} sdrkit_fsk_analyser_event_row;

// The options of `modulation` of `sdrkit_mer_fields`.
typedef enum sdrkit_mer_fields_modulation {
  // BPSK
  SDRKIT_MER_FIELDS_MODULATION_BPSK = 0,
  // QPSK
  SDRKIT_MER_FIELDS_MODULATION_QPSK = 1,
  // 8-PSK
  SDRKIT_MER_FIELDS_MODULATION_PSK8 = 2,
} sdrkit_mer_fields_modulation;

// The result fields of `mer`, from the last block. The readings have no value without a lock.
typedef struct sdrkit_mer_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `locked` holds a value.
  bool has_locked;
  // A PSK carrier was found and measured in the last block.
  bool locked;
  // True if `modulation` holds a value.
  bool has_modulation;
  // The order. A value of `sdrkit_mer_fields_modulation`.
  int32_t modulation;
  // True if `mer_db` holds a value.
  bool has_mer_db;
  // The modulation error ratio after matched filtering, timing and phase recovery. Unit: dB.
  double mer_db;
  // True if `evm_pct` holds a value.
  bool has_evm_pct;
  // The RMS error vector, of the RMS amplitude. Unit: %.
  double evm_pct;
  // True if `symbol_rate_bd` holds a value.
  bool has_symbol_rate_bd;
  // The symbol rate. Unit: Bd.
  double symbol_rate_bd;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset from the tuned frequency. A frequency in Hz.
  double carrier_offset_hz;
  // True if `rolloff` holds a value.
  bool has_rolloff;
  // The roll-off of the best matched filter.
  double rolloff;
  // True if `symbols` holds a value.
  bool has_symbols;
  // The symbols measured in the last block.
  int64_t symbols;
} sdrkit_mer_fields;

// The options of `modulation` of `sdrkit_mer_event_row`.
typedef enum sdrkit_mer_event_row_modulation {
  // BPSK
  SDRKIT_MER_EVENT_ROW_MODULATION_BPSK = 0,
  // QPSK
  SDRKIT_MER_EVENT_ROW_MODULATION_QPSK = 1,
  // 8-PSK
  SDRKIT_MER_EVENT_ROW_MODULATION_PSK8 = 2,
} sdrkit_mer_event_row_modulation;

// A row of the `mer` log.
typedef struct sdrkit_mer_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The order. A value of `sdrkit_mer_event_row_modulation`.
  int32_t modulation;
  // MER. Unit: dB.
  double mer_db;
  // EVM. Unit: %.
  double evm_pct;
  // The symbol rate. Unit: Bd.
  double symbol_rate_bd;
  // The carrier offset. A frequency in Hz.
  double carrier_offset_hz;
  // The mean power of the block, relative to full scale. Unit: dB.
  double level_db;
} sdrkit_mer_event_row;

// The options of `reading` of `sdrkit_papr_fields`.
typedef enum sdrkit_papr_fields_reading {
  // Constant envelope
  SDRKIT_PAPR_FIELDS_READING_CONSTANT_ENVELOPE = 0,
  // Gaussian-like
  SDRKIT_PAPR_FIELDS_READING_GAUSSIAN_LIKE = 1,
  // Less peaky than Gaussian
  SDRKIT_PAPR_FIELDS_READING_LESS_PEAKY = 2,
  // Peakier than Gaussian
  SDRKIT_PAPR_FIELDS_READING_PEAKIER = 3,
} sdrkit_papr_fields_reading;

// The result fields of `papr`, from the last evaluation over the kept samples.
typedef struct sdrkit_papr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `mean_dbfs` holds a value.
  bool has_mean_dbfs;
  // The mean power, relative to full scale. Unit: dBFS.
  double mean_dbfs;
  // True if `peak_dbfs` holds a value.
  bool has_peak_dbfs;
  // The largest instantaneous power. Unit: dBFS.
  double peak_dbfs;
  // True if `crest_db` holds a value.
  bool has_crest_db;
  // The peak over the mean power. Unit: dB.
  double crest_db;
  // True if `ccdf_10_db` holds a value.
  bool has_ccdf_10_db;
  // The level over the mean exceeded by 10 % of the samples. Unit: dB.
  double ccdf_10_db;
  // True if `ccdf_1_db` holds a value.
  bool has_ccdf_1_db;
  // The level exceeded by 1 %. Unit: dB.
  double ccdf_1_db;
  // True if `ccdf_01_db` holds a value.
  bool has_ccdf_01_db;
  // The level exceeded by 0.1 %. Unit: dB.
  double ccdf_01_db;
  // True if `ccdf_001_db` holds a value.
  bool has_ccdf_001_db;
  // The level exceeded by 0.01 %; only with a million samples or more. Unit: dB.
  double ccdf_001_db;
  // True if `samples` holds a value.
  bool has_samples;
  // The samples in the statistics.
  int64_t samples;
  // True if `reading` holds a value.
  bool has_reading;
  // The shape against Gaussian noise. A value of `sdrkit_papr_fields_reading`.
  int32_t reading;
} sdrkit_papr_fields;

// A row of the `papr` log.
typedef struct sdrkit_papr_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The level at 10 %. Unit: dB.
  double ccdf_10_db;
  // The level at 1 %. Unit: dB.
  double ccdf_1_db;
  // The level at 0.1 %. Unit: dB.
  double ccdf_01_db;
  // True if `ccdf_001_db` holds a value.
  bool has_ccdf_001_db;
  // The level at 0.01 %. Unit: dB.
  double ccdf_001_db;
  // The crest factor. Unit: dB.
  double crest_db;
  // The mean power. Unit: dBFS.
  double mean_dbfs;
  // The samples in the statistics.
  int64_t samples;
} sdrkit_papr_event_row;

// The options of `verdict` of `sdrkit_input_health_fields`.
typedef enum sdrkit_input_health_fields_verdict {
  // Low
  SDRKIT_INPUT_HEALTH_FIELDS_VERDICT_LOW = 0,
  // Good
  SDRKIT_INPUT_HEALTH_FIELDS_VERDICT_GOOD = 1,
  // High
  SDRKIT_INPUT_HEALTH_FIELDS_VERDICT_HIGH = 2,
  // Overload
  SDRKIT_INPUT_HEALTH_FIELDS_VERDICT_OVERLOAD = 3,
} sdrkit_input_health_fields_verdict;

// The result fields of `input_health`, from the last block of raw samples. Levels are against a full scale of 1.0 per component.
typedef struct sdrkit_input_health_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `rms_dbfs` holds a value.
  bool has_rms_dbfs;
  // The RMS level, DC included. Unit: dBFS.
  double rms_dbfs;
  // True if `peak_dbfs` holds a value.
  bool has_peak_dbfs;
  // The largest of `|I|` and `|Q|`. Unit: dBFS.
  double peak_dbfs;
  // True if `crest_db` holds a value.
  bool has_crest_db;
  // The peak over the RMS level. Unit: dB.
  double crest_db;
  // True if `clipping_ppm` holds a value.
  bool has_clipping_ppm;
  // The samples with `|I|` or `|Q|` at the clip level or over it. Unit: ppm.
  double clipping_ppm;
  // True if `dc_i_pct` holds a value.
  bool has_dc_i_pct;
  // The mean of I. Unit: %.
  double dc_i_pct;
  // True if `dc_q_pct` holds a value.
  bool has_dc_q_pct;
  // The mean of Q. Unit: %.
  double dc_q_pct;
  // True if `dc_pct` holds a value.
  bool has_dc_pct;
  // The length of the mean vector. Unit: %.
  double dc_pct;
  // True if `dc_under_rms_db` holds a value.
  bool has_dc_under_rms_db;
  // The power of the mean vector under the RMS level. Unit: dB.
  double dc_under_rms_db;
  // True if `iq_gain_db` holds a value.
  bool has_iq_gain_db;
  // `10 log10(V_Q / V_I)`. Unit: dB.
  double iq_gain_db;
  // True if `iq_phase_deg` holds a value.
  bool has_iq_phase_deg;
  // `asin` of the correlation of I and Q. Unit: deg.
  double iq_phase_deg;
  // True if `image_rejection_db` holds a value.
  bool has_image_rejection_db;
  // The image rejection that the mismatch implies, at most 120 dB. Unit: dB.
  double image_rejection_db;
  // True if `verdict` holds a value.
  bool has_verdict;
  // The level verdict. A value of `sdrkit_input_health_fields_verdict`.
  int32_t verdict;
} sdrkit_input_health_fields;

// The options of `kind` of `sdrkit_input_health_event_row`.
typedef enum sdrkit_input_health_event_row_kind {
  // Verdict
  SDRKIT_INPUT_HEALTH_EVENT_ROW_KIND_VERDICT = 0,
  // Clipping
  SDRKIT_INPUT_HEALTH_EVENT_ROW_KIND_CLIPPING = 1,
} sdrkit_input_health_event_row_kind;

// The options of `verdict` of `sdrkit_input_health_event_row`.
typedef enum sdrkit_input_health_event_row_verdict {
  // Low
  SDRKIT_INPUT_HEALTH_EVENT_ROW_VERDICT_LOW = 0,
  // Good
  SDRKIT_INPUT_HEALTH_EVENT_ROW_VERDICT_GOOD = 1,
  // High
  SDRKIT_INPUT_HEALTH_EVENT_ROW_VERDICT_HIGH = 2,
  // Overload
  SDRKIT_INPUT_HEALTH_EVENT_ROW_VERDICT_OVERLOAD = 3,
} sdrkit_input_health_event_row_verdict;

// A row of the `input_health` log.
typedef struct sdrkit_input_health_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // What the row reports. A value of `sdrkit_input_health_event_row_kind`.
  int32_t kind;
  // The verdict. A value of `sdrkit_input_health_event_row_verdict`.
  int32_t verdict;
  // The RMS level. Unit: dBFS.
  double rms_dbfs;
  // The peak. Unit: dBFS.
  double peak_dbfs;
  // The samples near full scale. Unit: ppm.
  double clipping_ppm;
} sdrkit_input_health_event_row;

// The result fields of `noise_figure`. Levels are relative and uncalibrated.
typedef struct sdrkit_noise_figure_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `power_db` holds a value.
  bool has_power_db;
  // The level of the present plateau of the power. Unit: dB.
  double power_db;
  // True if `y_db` holds a value.
  bool has_y_db;
  // The Y-factor: the mean of the kept steps. Unit: dB.
  double y_db;
  // True if `y_lin` holds a value.
  bool has_y_lin;
  // The Y-factor as a ratio.
  double y_lin;
  // True if `y_spread_db` holds a value.
  bool has_y_spread_db;
  // The sample standard deviation of the kept steps. Unit: dB.
  double y_spread_db;
  // The steps seen.
  int64_t steps;
} sdrkit_noise_figure_fields;

// A row of the noise figure table: one source ENR of the setting.
typedef struct sdrkit_noise_figure_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The excess noise ratio of the source. Unit: dB.
  double enr_db;
  // True if `nf_db` holds a value.
  bool has_nf_db;
  // `ENR - 10 log10(Y - 1)`; no value when Y is 1 or less. Unit: dB.
  double nf_db;
  // The noise figure is negative: the ENR is lower than the step implies.
  bool negative;
} sdrkit_noise_figure_row;

// The options of `state` of `sdrkit_noise_figure_event_row`.
typedef enum sdrkit_noise_figure_event_row_state {
  // Source on
  SDRKIT_NOISE_FIGURE_EVENT_ROW_STATE_SOURCE_ON = 0,
  // Source off
  SDRKIT_NOISE_FIGURE_EVENT_ROW_STATE_SOURCE_OFF = 1,
} sdrkit_noise_figure_event_row_state;

// A row of the `noise_figure` log: one step.
typedef struct sdrkit_noise_figure_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The state of the source after the step. A value of `sdrkit_noise_figure_event_row_state`.
  int32_t state;
  // The plateau before. Unit: dB.
  double from_db;
  // The plateau after. Unit: dB.
  double to_db;
  // The size of the step. Unit: dB.
  double y_db;
  // The size as a ratio.
  double y_lin;
} sdrkit_noise_figure_event_row;

// The result fields of `kurtosis`, from the last evaluation.
typedef struct sdrkit_kurtosis_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `clean_pct` holds a value.
  bool has_clean_pct;
  // The share of the used bins whose SK is within the limits. Unit: %.
  double clean_pct;
  // The expected standard deviation of SK for Gaussian noise.
  double sigma;
  // True if `regions` holds a value.
  bool has_regions;
  // The flagged regions.
  int64_t regions;
  // The time of one evaluation. Unit: ms.
  double block_ms;
} sdrkit_kurtosis_fields;

// The options of `kind` of `sdrkit_kurtosis_region_row`.
typedef enum sdrkit_kurtosis_region_row_kind {
  // Pulsed
  SDRKIT_KURTOSIS_REGION_ROW_KIND_PULSED = 0,
  // Steady
  SDRKIT_KURTOSIS_REGION_ROW_KIND_STEADY = 1,
} sdrkit_kurtosis_region_row_kind;

// A flagged region of `kurtosis`.
typedef struct sdrkit_kurtosis_region_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The low edge. A frequency in Hz.
  double low;
  // The high edge. A frequency in Hz.
  double high;
  // The kind. A value of `sdrkit_kurtosis_region_row_kind`.
  int32_t kind;
  // The mean SK of its bins.
  double sk;
  // True if `full_power_pct` holds a value.
  bool has_full_power_pct;
  // The share of frames at full power, `1 / (SK + 1)`, for a pulsed region with SK over 1.5. Unit: %.
  double full_power_pct;
  // The mean power over the noise per bin. Unit: dB.
  double over_noise_db;
} sdrkit_kurtosis_region_row;

// The options of `kind` of `sdrkit_kurtosis_event_row`.
typedef enum sdrkit_kurtosis_event_row_kind {
  // Pulsed
  SDRKIT_KURTOSIS_EVENT_ROW_KIND_PULSED = 0,
  // Steady
  SDRKIT_KURTOSIS_EVENT_ROW_KIND_STEADY = 1,
  // Gone
  SDRKIT_KURTOSIS_EVENT_ROW_KIND_GONE = 2,
} sdrkit_kurtosis_event_row_kind;

// A row of the `kurtosis` log.
typedef struct sdrkit_kurtosis_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // What happened. A value of `sdrkit_kurtosis_event_row_kind`.
  int32_t kind;
  // The centre of the region. A frequency in Hz.
  double centre;
  // The low edge. A frequency in Hz.
  double low;
  // The high edge. A frequency in Hz.
  double high;
  // The mean SK.
  double sk;
  // The mean power over the noise. Unit: dB.
  double over_noise_db;
} sdrkit_kurtosis_event_row;

// The result fields of `glitch`.
typedef struct sdrkit_glitch_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `tone_hz` holds a value.
  bool has_tone_hz;
  // The steady tone whose phase is watched; no value while none is found. A frequency in Hz.
  double tone_hz;
  // Sample drop events.
  int64_t sample_drops;
  // Sample repeat events.
  int64_t sample_repeats;
  // Zero runs.
  int64_t zero_runs;
  // Repeated blocks.
  int64_t repeated_blocks;
  // Level steps.
  int64_t level_steps;
  // The time observed since the start or the last retune. A length of time in seconds.
  double elapsed_s;
} sdrkit_glitch_fields;

// The options of `kind` of `sdrkit_glitch_event_row`.
typedef enum sdrkit_glitch_event_row_kind {
  // Dropped samples
  SDRKIT_GLITCH_EVENT_ROW_KIND_DROPPED_SAMPLES = 0,
  // Repeated samples
  SDRKIT_GLITCH_EVENT_ROW_KIND_REPEATED_SAMPLES = 1,
  // Zero run
  SDRKIT_GLITCH_EVENT_ROW_KIND_ZERO_RUN = 2,
  // Repeated block
  SDRKIT_GLITCH_EVENT_ROW_KIND_REPEATED_BLOCK = 3,
  // Level step
  SDRKIT_GLITCH_EVENT_ROW_KIND_LEVEL_STEP = 4,
} sdrkit_glitch_event_row_kind;

// A row of the `glitch` log: one stream fault.
typedef struct sdrkit_glitch_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the fault. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_glitch_event_row_kind`.
  int32_t kind;
  // True if `samples` holds a value.
  bool has_samples;
  // The samples missing or repeated: the smallest count that fits the phase step.
  int64_t samples;
  // True if `phase_step_rad` holds a value.
  bool has_phase_step_rad;
  // The phase step of the tone. Unit: rad.
  double phase_step_rad;
  // True if `step_db` holds a value.
  bool has_step_db;
  // The size of a level step. Unit: dB.
  double step_db;
  // True if `from_db` holds a value.
  bool has_from_db;
  // The level before a step. Unit: dB.
  double from_db;
  // True if `to_db` holds a value.
  bool has_to_db;
  // The level after a step. Unit: dB.
  double to_db;
} sdrkit_glitch_event_row;

// The result fields of `intermod`, from the last evaluation.
typedef struct sdrkit_intermod_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `inputs` holds a value.
  bool has_inputs;
  // The strong signals used as parents.
  int64_t inputs;
  // True if `products` holds a value.
  bool has_products;
  // The peaks at product frequencies.
  int64_t products;
  // True if `products_follow` holds a value.
  bool has_products_follow;
  // Those that follow the law.
  int64_t products_follow;
  // True if `products_independent` holds a value.
  bool has_products_independent;
  // Those called independent.
  int64_t products_independent;
  // True if `products_unknown` holds a value.
  bool has_products_unknown;
  // Those unclear or that cannot be told.
  int64_t products_unknown;
  // The snapshots taken.
  int64_t snapshots;
} sdrkit_intermod_fields;

// A strong signal of `intermod`.
typedef struct sdrkit_intermod_input_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency (an offset from the centre when the tuned frequency is not known). A frequency in Hz.
  double frequency;
  // The mean level over the noise. Unit: dB.
  double level_db;
} sdrkit_intermod_input_row;

// The options of `kind` of `sdrkit_intermod_product_row`.
typedef enum sdrkit_intermod_product_row_kind {
  // 2a - b
  SDRKIT_INTERMOD_PRODUCT_ROW_KIND_2A_B = 0,
  // 2b - a
  SDRKIT_INTERMOD_PRODUCT_ROW_KIND_2B_A = 1,
  // b - a
  SDRKIT_INTERMOD_PRODUCT_ROW_KIND_B_A = 2,
} sdrkit_intermod_product_row_kind;

// The options of `verdict` of `sdrkit_intermod_product_row`.
typedef enum sdrkit_intermod_product_row_verdict {
  // Follows
  SDRKIT_INTERMOD_PRODUCT_ROW_VERDICT_FOLLOWS = 0,
  // Independent
  SDRKIT_INTERMOD_PRODUCT_ROW_VERDICT_INDEPENDENT = 1,
  // Unclear
  SDRKIT_INTERMOD_PRODUCT_ROW_VERDICT_UNCLEAR = 2,
  // Cannot tell
  SDRKIT_INTERMOD_PRODUCT_ROW_VERDICT_CANNOT_TELL = 3,
} sdrkit_intermod_product_row_verdict;

// A peak at a product frequency.
typedef struct sdrkit_intermod_product_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency of the peak. A frequency in Hz.
  double frequency;
  // The kind of product. A value of `sdrkit_intermod_product_row_kind`.
  int32_t kind;
  // The lower parent. A frequency in Hz.
  double parent_a;
  // The upper parent. A frequency in Hz.
  double parent_b;
  // The mean level under the weaker parent. Unit: dB.
  double below_db;
  // The verdict of the level fit. A value of `sdrkit_intermod_product_row_verdict`.
  int32_t verdict;
  // True if `slope` holds a value.
  bool has_slope;
  // The slope of its level against the law.
  double slope;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The correlation of its level with the law.
  double correlation;
  // The snapshots of the fit.
  int64_t points;
} sdrkit_intermod_product_row;

// The options of `kind` of `sdrkit_intermod_event_row`.
typedef enum sdrkit_intermod_event_row_kind {
  // 2a - b
  SDRKIT_INTERMOD_EVENT_ROW_KIND_2A_B = 0,
  // 2b - a
  SDRKIT_INTERMOD_EVENT_ROW_KIND_2B_A = 1,
  // b - a
  SDRKIT_INTERMOD_EVENT_ROW_KIND_B_A = 2,
} sdrkit_intermod_event_row_kind;

// The options of `verdict` of `sdrkit_intermod_event_row`.
typedef enum sdrkit_intermod_event_row_verdict {
  // Follows
  SDRKIT_INTERMOD_EVENT_ROW_VERDICT_FOLLOWS = 0,
  // Independent
  SDRKIT_INTERMOD_EVENT_ROW_VERDICT_INDEPENDENT = 1,
  // Unclear
  SDRKIT_INTERMOD_EVENT_ROW_VERDICT_UNCLEAR = 2,
  // Cannot tell
  SDRKIT_INTERMOD_EVENT_ROW_VERDICT_CANNOT_TELL = 3,
} sdrkit_intermod_event_row_verdict;

// A row of the `intermod` log.
typedef struct sdrkit_intermod_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The frequency of the peak. A frequency in Hz.
  double frequency;
  // The kind of product. A value of `sdrkit_intermod_event_row_kind`.
  int32_t kind;
  // The lower parent. A frequency in Hz.
  double parent_a;
  // The upper parent. A frequency in Hz.
  double parent_b;
  // The mean level under the weaker parent. Unit: dB.
  double below_db;
  // The verdict. A value of `sdrkit_intermod_event_row_verdict`.
  int32_t verdict;
  // True if `slope` holds a value.
  bool has_slope;
  // The slope.
  double slope;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The correlation.
  double correlation;
  // The snapshots of the fit.
  int64_t points;
} sdrkit_intermod_event_row;

// The result fields of `spur`, from the last evaluation. The findings hold for this radio at this gain, rate and tuning.
typedef struct sdrkit_spur_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dc_db` holds a value.
  bool has_dc_db;
  // The largest bin within 2 bins of 0 Hz over the noise. Unit: dB.
  double dc_db;
  // True if `dc_spike` holds a value.
  bool has_dc_spike;
  // The DC spike is over its threshold.
  bool dc_spike;
  // True if `signals` holds a value.
  bool has_signals;
  // The strong signals.
  int64_t signals;
  // True if `images` holds a value.
  bool has_images;
  // Images not called independent.
  int64_t images;
  // True if `harmonics` holds a value.
  bool has_harmonics;
  // Harmonics not called independent.
  int64_t harmonics;
  // True if `fixed_spurs` holds a value.
  bool has_fixed_spurs;
  // Fixed spurs.
  int64_t fixed_spurs;
  // The snapshots taken.
  int64_t snapshots;
} sdrkit_spur_fields;

// The options of `kind` of `sdrkit_spur_finding_row`.
typedef enum sdrkit_spur_finding_row_kind {
  // Image
  SDRKIT_SPUR_FINDING_ROW_KIND_IMAGE = 0,
  // 2nd harmonic
  SDRKIT_SPUR_FINDING_ROW_KIND_SECOND_HARMONIC = 1,
  // 3rd harmonic
  SDRKIT_SPUR_FINDING_ROW_KIND_THIRD_HARMONIC = 2,
  // Fixed spur
  SDRKIT_SPUR_FINDING_ROW_KIND_FIXED_SPUR = 3,
} sdrkit_spur_finding_row_kind;

// The options of `verdict` of `sdrkit_spur_finding_row`.
typedef enum sdrkit_spur_finding_row_verdict {
  // Follows
  SDRKIT_SPUR_FINDING_ROW_VERDICT_FOLLOWS = 0,
  // Independent
  SDRKIT_SPUR_FINDING_ROW_VERDICT_INDEPENDENT = 1,
  // Unclear
  SDRKIT_SPUR_FINDING_ROW_VERDICT_UNCLEAR = 2,
  // Cannot tell
  SDRKIT_SPUR_FINDING_ROW_VERDICT_CANNOT_TELL = 3,
  // Fixed
  SDRKIT_SPUR_FINDING_ROW_VERDICT_FIXED = 4,
} sdrkit_spur_finding_row_verdict;

// A finding of `spur`.
typedef struct sdrkit_spur_finding_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The kind. A value of `sdrkit_spur_finding_row_kind`.
  int32_t kind;
  // The frequency of the peak. A frequency in Hz.
  double frequency;
  // True if `from_signal` holds a value.
  bool has_from_signal;
  // The signal that it belongs to; none for a fixed spur. A frequency in Hz.
  double from_signal;
  // The mean level under the signal; for a fixed spur the mean level over the noise. Unit: dB.
  double below_db;
  // The verdict. A value of `sdrkit_spur_finding_row_verdict`.
  int32_t verdict;
  // True if `slope` holds a value.
  bool has_slope;
  // The slope of its level against the signal's.
  double slope;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The correlation.
  double correlation;
  // True if `image_rejection_db` holds a value.
  bool has_image_rejection_db;
  // The image rejection (images). Unit: dB.
  double image_rejection_db;
  // True if `gain_mismatch_db` holds a value.
  bool has_gain_mismatch_db;
  // The gain mismatch alone that gives this image. Unit: dB.
  double gain_mismatch_db;
  // True if `phase_mismatch_deg` holds a value.
  bool has_phase_mismatch_deg;
  // The phase mismatch alone that gives this image. Unit: deg.
  double phase_mismatch_deg;
} sdrkit_spur_finding_row;

// The options of `kind` of `sdrkit_spur_event_row`.
typedef enum sdrkit_spur_event_row_kind {
  // Image
  SDRKIT_SPUR_EVENT_ROW_KIND_IMAGE = 0,
  // 2nd harmonic
  SDRKIT_SPUR_EVENT_ROW_KIND_SECOND_HARMONIC = 1,
  // 3rd harmonic
  SDRKIT_SPUR_EVENT_ROW_KIND_THIRD_HARMONIC = 2,
  // Fixed spur
  SDRKIT_SPUR_EVENT_ROW_KIND_FIXED_SPUR = 3,
  // DC spike
  SDRKIT_SPUR_EVENT_ROW_KIND_DC = 4,
} sdrkit_spur_event_row_kind;

// The options of `verdict` of `sdrkit_spur_event_row`.
typedef enum sdrkit_spur_event_row_verdict {
  // Follows
  SDRKIT_SPUR_EVENT_ROW_VERDICT_FOLLOWS = 0,
  // Independent
  SDRKIT_SPUR_EVENT_ROW_VERDICT_INDEPENDENT = 1,
  // Unclear
  SDRKIT_SPUR_EVENT_ROW_VERDICT_UNCLEAR = 2,
  // Cannot tell
  SDRKIT_SPUR_EVENT_ROW_VERDICT_CANNOT_TELL = 3,
  // Fixed
  SDRKIT_SPUR_EVENT_ROW_VERDICT_FIXED = 4,
} sdrkit_spur_event_row_verdict;

// A row of the `spur` log.
typedef struct sdrkit_spur_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_spur_event_row_kind`.
  int32_t kind;
  // The frequency of the peak. A frequency in Hz.
  double frequency;
  // True if `from_signal` holds a value.
  bool has_from_signal;
  // The signal that it belongs to. A frequency in Hz.
  double from_signal;
  // True if `below_db` holds a value.
  bool has_below_db;
  // The mean level under the signal, or over the noise. Unit: dB.
  double below_db;
  // True if `verdict` holds a value.
  bool has_verdict;
  // The verdict. A value of `sdrkit_spur_event_row_verdict`.
  int32_t verdict;
  // True if `slope` holds a value.
  bool has_slope;
  // The slope.
  double slope;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The correlation.
  double correlation;
  // True if `image_rejection_db` holds a value.
  bool has_image_rejection_db;
  // The image rejection. Unit: dB.
  double image_rejection_db;
  // True if `gain_mismatch_db` holds a value.
  bool has_gain_mismatch_db;
  // The gain mismatch alone. Unit: dB.
  double gain_mismatch_db;
  // True if `phase_mismatch_deg` holds a value.
  bool has_phase_mismatch_deg;
  // The phase mismatch alone. Unit: deg.
  double phase_mismatch_deg;
  // True if `dc_db` holds a value.
  bool has_dc_db;
  // The DC spike over the noise (kind dc). Unit: dB.
  double dc_db;
} sdrkit_spur_event_row;

// The result fields of `fading`: the statistics of the envelope of one carrier over the window, and the carrier now.
typedef struct sdrkit_fading_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `k_db` holds a value.
  bool has_k_db;
  // The Rician K-factor; 40 dB for no fading, no value for K = 0 (Rayleigh). Unit: dB.
  double k_db;
  // True if `k` holds a value.
  bool has_k;
  // The K-factor as a ratio.
  double k;
  // True if `rayleigh` holds a value.
  bool has_rayleigh;
  // No steady component: K = 0.
  bool rayleigh;
  // True if `p1_db` holds a value.
  bool has_p1_db;
  // The level exceeded by 99 % of the time, against the mean power. Unit: dB.
  double p1_db;
  // True if `p10_db` holds a value.
  bool has_p10_db;
  // The level at 10 %. Unit: dB.
  double p10_db;
  // True if `p50_db` holds a value.
  bool has_p50_db;
  // The median level. Unit: dB.
  double p50_db;
  // True if `p90_db` holds a value.
  bool has_p90_db;
  // The level at 90 %. Unit: dB.
  double p90_db;
  // True if `p99_db` holds a value.
  bool has_p99_db;
  // The level at 99 %. Unit: dB.
  double p99_db;
  // True if `fade_depth_db` holds a value.
  bool has_fade_depth_db;
  // The median over the 1 % level. Unit: dB.
  double fade_depth_db;
  // True if `s4` holds a value.
  bool has_s4;
  // The scintillation index S4.
  double s4;
  // True if `fade_rate_hz` holds a value.
  bool has_fade_rate_hz;
  // Downward crossings of the fade threshold per second. Unit: 1/s.
  double fade_rate_hz;
  // True if `fade_length_ms` holds a value.
  bool has_fade_length_ms;
  // The average time under the fade threshold. Unit: ms.
  double fade_length_ms;
  // True if `doppler_hz` holds a value.
  bool has_doppler_hz;
  // The RMS Doppler spread. Unit: Hz.
  double doppler_hz;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // The carrier to noise density of the last block. Unit: dB-Hz.
  double cn0_dbhz;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier from the tuned frequency. A frequency in Hz.
  double carrier_offset_hz;
  // True if `window_s` holds a value.
  bool has_window_s;
  // The envelope kept. A length of time in seconds.
  double window_s;
} sdrkit_fading_fields;

// The options of `kind` of `sdrkit_fading_event_row`.
typedef enum sdrkit_fading_event_row_kind {
  // Found
  SDRKIT_FADING_EVENT_ROW_KIND_FOUND = 0,
  // Lost
  SDRKIT_FADING_EVENT_ROW_KIND_LOST = 1,
  // Fade
  SDRKIT_FADING_EVENT_ROW_KIND_FADE = 2,
  // Reading
  SDRKIT_FADING_EVENT_ROW_KIND_READING = 3,
} sdrkit_fading_event_row_kind;

// A row of the `fading` log.
typedef struct sdrkit_fading_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // What happened. A value of `sdrkit_fading_event_row_kind`.
  int32_t kind;
  // True if `depth_db` holds a value.
  bool has_depth_db;
  // The deepest level of a fade against the mean power. Unit: dB.
  double depth_db;
  // True if `length_ms` holds a value.
  bool has_length_ms;
  // The length of a fade. Unit: ms.
  double length_ms;
  // True if `k_db` holds a value.
  bool has_k_db;
  // The K-factor. Unit: dB.
  double k_db;
  // True if `p10_db` holds a value.
  bool has_p10_db;
  // The level at 10 %. Unit: dB.
  double p10_db;
  // True if `fade_rate_hz` holds a value.
  bool has_fade_rate_hz;
  // The fade rate. Unit: 1/s.
  double fade_rate_hz;
  // True if `fade_length_ms` holds a value.
  bool has_fade_length_ms;
  // The average fade length. Unit: ms.
  double fade_length_ms;
  // True if `doppler_hz` holds a value.
  bool has_doppler_hz;
  // The Doppler spread. Unit: Hz.
  double doppler_hz;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // The carrier to noise density. Unit: dB-Hz.
  double cn0_dbhz;
} sdrkit_fading_event_row;

// The options of `class_` of `sdrkit_beacon_monitor_fields`.
typedef enum sdrkit_beacon_monitor_fields_class_ {
  // None
  SDRKIT_BEACON_MONITOR_FIELDS_CLASS__NONE = 0,
  // Steady
  SDRKIT_BEACON_MONITOR_FIELDS_CLASS__STEADY = 1,
  // Fading
  SDRKIT_BEACON_MONITOR_FIELDS_CLASS__FADING = 2,
  // Aurora-like
  SDRKIT_BEACON_MONITOR_FIELDS_CLASS__AURORA_LIKE = 3,
} sdrkit_beacon_monitor_fields_class_;

// The result fields of `beacon_monitor`.
typedef struct sdrkit_beacon_monitor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The class of the path. A value of `sdrkit_beacon_monitor_fields_class_`.
  int32_t class_;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The line over the noise in a bin, last spectrum. Unit: dB.
  double level_db;
  // True if `spread_hz` holds a value.
  bool has_spread_hz;
  // The Doppler spread of the line (a Gaussian full width at half height), last spectrum. Unit: Hz.
  double spread_hz;
  // True if `swing_db` holds a value.
  bool has_swing_db;
  // The largest minus the smallest level of the last 12 s, at the last decision. Unit: dB.
  double swing_db;
  // True if `line_offset_hz` holds a value.
  bool has_line_offset_hz;
  // The line from the tuned frequency, last spectrum. A frequency in Hz.
  double line_offset_hz;
  // The class changes.
  int64_t changes;
} sdrkit_beacon_monitor_fields;

// The options of `from_class` of `sdrkit_beacon_monitor_event_row`.
typedef enum sdrkit_beacon_monitor_event_row_from_class {
  // None
  SDRKIT_BEACON_MONITOR_EVENT_ROW_FROM_CLASS_NONE = 0,
  // Steady
  SDRKIT_BEACON_MONITOR_EVENT_ROW_FROM_CLASS_STEADY = 1,
  // Fading
  SDRKIT_BEACON_MONITOR_EVENT_ROW_FROM_CLASS_FADING = 2,
  // Aurora-like
  SDRKIT_BEACON_MONITOR_EVENT_ROW_FROM_CLASS_AURORA_LIKE = 3,
} sdrkit_beacon_monitor_event_row_from_class;

// The options of `to_class` of `sdrkit_beacon_monitor_event_row`.
typedef enum sdrkit_beacon_monitor_event_row_to_class {
  // None
  SDRKIT_BEACON_MONITOR_EVENT_ROW_TO_CLASS_NONE = 0,
  // Steady
  SDRKIT_BEACON_MONITOR_EVENT_ROW_TO_CLASS_STEADY = 1,
  // Fading
  SDRKIT_BEACON_MONITOR_EVENT_ROW_TO_CLASS_FADING = 2,
  // Aurora-like
  SDRKIT_BEACON_MONITOR_EVENT_ROW_TO_CLASS_AURORA_LIKE = 3,
} sdrkit_beacon_monitor_event_row_to_class;

// A change of class of `beacon_monitor`.
typedef struct sdrkit_beacon_monitor_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. A length of time in seconds.
  double time_s;
  // The class before. A value of `sdrkit_beacon_monitor_event_row_from_class`.
  int32_t from_class;
  // The class after. A value of `sdrkit_beacon_monitor_event_row_to_class`.
  int32_t to_class;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The last level. Unit: dB.
  double level_db;
  // True if `spread_hz` holds a value.
  bool has_spread_hz;
  // The median spread of the decision. Unit: Hz.
  double spread_hz;
  // True if `swing_db` holds a value.
  bool has_swing_db;
  // The swing of the decision. Unit: dB.
  double swing_db;
} sdrkit_beacon_monitor_event_row;

// The options of `change` of `sdrkit_fm_monitor_pilot_event`.
typedef enum sdrkit_fm_monitor_pilot_event_change {
  // Present
  SDRKIT_FM_MONITOR_PILOT_EVENT_CHANGE_PRESENT = 0,
  // Absent
  SDRKIT_FM_MONITOR_PILOT_EVENT_CHANGE_ABSENT = 1,
  // Lost
  SDRKIT_FM_MONITOR_PILOT_EVENT_CHANGE_LOST = 2,
} sdrkit_fm_monitor_pilot_event_change;

// The pilot of an FM station was present or absent at the first measurement, came or went.
typedef struct sdrkit_fm_monitor_pilot_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // What changed. A value of `sdrkit_fm_monitor_pilot_event_change`.
  int32_t change;
  // The pilot of the second, peak equivalent. Unit: kHz.
  double pilot_khz;
} sdrkit_fm_monitor_pilot_event;

// The options of `change` of `sdrkit_fm_monitor_rds_event`.
typedef enum sdrkit_fm_monitor_rds_event_change {
  // Present
  SDRKIT_FM_MONITOR_RDS_EVENT_CHANGE_PRESENT = 0,
  // Lost
  SDRKIT_FM_MONITOR_RDS_EVENT_CHANGE_LOST = 1,
} sdrkit_fm_monitor_rds_event_change;

// RDS came or went.
typedef struct sdrkit_fm_monitor_rds_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // What changed. A value of `sdrkit_fm_monitor_rds_event_change`.
  int32_t change;
  // The RDS injection of the second, peak equivalent. Unit: kHz.
  double rds_khz;
} sdrkit_fm_monitor_rds_event;

// A subcarrier at 67 or 92 kHz came or went.
typedef struct sdrkit_fm_monitor_sca_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The subcarrier, 67 or 92. Unit: kHz.
  double subcarrier_khz;
  // The subcarrier came (true) or went.
  bool on;
  // True if `level_db` holds a value.
  bool has_level_db;
  // Its level over the floor in that second. Unit: dB.
  double level_db;
} sdrkit_fm_monitor_sca_event;

// A second whose peak deviation was over the threshold.
typedef struct sdrkit_fm_monitor_over_deviation_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The peak deviation of the second. Unit: kHz.
  double peak_deviation_khz;
  // The MPX power of the second. Unit: dBr.
  double mpx_power_dbr;
} sdrkit_fm_monitor_over_deviation_event;

// The options of `change` of `sdrkit_am_monitor_carrier_event`.
typedef enum sdrkit_am_monitor_carrier_event_change {
  // Present
  SDRKIT_AM_MONITOR_CARRIER_EVENT_CHANGE_PRESENT = 0,
  // Lost
  SDRKIT_AM_MONITOR_CARRIER_EVENT_CHANGE_LOST = 1,
  // Returned
  SDRKIT_AM_MONITOR_CARRIER_EVENT_CHANGE_RETURNED = 2,
} sdrkit_am_monitor_carrier_event_change;

// The carrier of an AM station was seen, was lost or came back.
typedef struct sdrkit_am_monitor_carrier_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // What changed. A value of `sdrkit_am_monitor_carrier_event_change`.
  int32_t change;
  // The carrier offset of the second. Unit: Hz.
  double carrier_offset_hz;
  // The carrier level of the second. Unit: dB.
  double carrier_db;
} sdrkit_am_monitor_carrier_event;

// A second of an AM station was over-modulated.
typedef struct sdrkit_am_monitor_over_modulation_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The share of the second at zero carrier. Unit: %.
  double over_mod_pct;
  // The positive peak of the second. Unit: %.
  double peak_pos_pct;
} sdrkit_am_monitor_over_modulation_event;

// A transmission of a channel ended.
typedef struct sdrkit_occupancy_activity_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The end of the transmission, signal time. A length of time in seconds.
  double time_s;
  // The centre of the channel. A frequency in Hz.
  double frequency;
  // The length of the transmission. A length of time in seconds.
  double length_s;
  // The highest channel power over the floor. Unit: dB.
  double peak_over_floor_db;
  // The highest channel power, relative, per bin. Unit: dB.
  double level_db;
} sdrkit_occupancy_activity_event;

// The options of `kind` of `sdrkit_noise_floor_event`.
typedef enum sdrkit_noise_floor_event_kind {
  // Noise rise
  SDRKIT_NOISE_FLOOR_EVENT_KIND_RISE = 0,
  // Noise back
  SDRKIT_NOISE_FLOOR_EVENT_KIND_BACK = 1,
  // Whole band
  SDRKIT_NOISE_FLOOR_EVENT_KIND_WHOLE_BAND = 2,
} sdrkit_noise_floor_event_kind;

// A row of the noise floor log was written.
typedef struct sdrkit_noise_floor_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the row. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_noise_floor_event_kind`.
  int32_t kind;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The centre of the sub-band; no value for the whole band. A frequency in Hz.
  double frequency;
  // True if `sub_band` holds a value.
  bool has_sub_band;
  // The sub-band, from 1; no value for the whole band.
  int64_t sub_band;
  // The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band. Unit: dB.
  double rise_db;
  // True if `baseline_db` holds a value.
  bool has_baseline_db;
  // The baseline of the sub-band. Unit: dB.
  double baseline_db;
  // True if `duration_s` holds a value.
  bool has_duration_s;
  // How long the rise lasted (rows of the fall back). A length of time in seconds.
  double duration_s;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The floor of the sub-band, or of the band, that second. Unit: dB.
  double level_db;
} sdrkit_noise_floor_event;

// The options of `class_` of `sdrkit_burst_finder_burst_found_event`.
typedef enum sdrkit_burst_finder_burst_found_event_class_ {
  // Narrow
  SDRKIT_BURST_FINDER_BURST_FOUND_EVENT_CLASS__NARROW = 0,
  // Wide
  SDRKIT_BURST_FINDER_BURST_FOUND_EVENT_CLASS__WIDE = 1,
} sdrkit_burst_finder_burst_found_event_class_;

// A burst was found.
typedef struct sdrkit_burst_finder_burst_found_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The start of the burst, signal time. A length of time in seconds.
  double time_s;
  // The median centre of the burst. A frequency in Hz.
  double frequency;
  // The length of the burst. Unit: ms.
  double length_ms;
  // The bandwidth class. A value of `sdrkit_burst_finder_burst_found_event_class_`.
  int32_t class_;
  // The median width, at least one bin. Unit: kHz.
  double width_khz;
  // The mean peak over the median of the frames. Unit: dB.
  double level_db;
} sdrkit_burst_finder_burst_found_event;

// A source was found repeating.
typedef struct sdrkit_burst_finder_burst_repeating_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The frequency of the source: its first burst. A frequency in Hz.
  double frequency;
  // The median interval between its bursts. A length of time in seconds.
  double period_s;
  // Its bursts in the history.
  int64_t bursts;
  // The RMS deviation of the agreeing intervals from the period, over the period. Unit: %.
  double spread_pct;
} sdrkit_burst_finder_burst_repeating_event;

// A hopper was found, or its rate or its channel count changed.
typedef struct sdrkit_hopper_changed_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the evaluation. A length of time in seconds.
  double time_s;
  // The hop rate. Unit: 1/s.
  double hop_rate;
  // The channels.
  int64_t channels;
  // The channel spacing. Unit: kHz.
  double spacing_khz;
  // The span. Unit: kHz.
  double span_khz;
  // The lowest channel. A frequency in Hz.
  double lowest;
  // The highest channel. A frequency in Hz.
  double highest;
  // The median dwell. Unit: ms.
  double dwell_ms;
  // The median guard. Unit: ms.
  double guard_ms;
  // The mean dwell level over the frame median. Unit: dB.
  double level_db;
} sdrkit_hopper_changed_event;

// The options of `kind` of `sdrkit_birdie_event`.
typedef enum sdrkit_birdie_event_kind {
  // New
  SDRKIT_BIRDIE_EVENT_KIND_NEW = 0,
  // Returned
  SDRKIT_BIRDIE_EVENT_KIND_RETURNED = 1,
  // Gone
  SDRKIT_BIRDIE_EVENT_KIND_GONE = 2,
} sdrkit_birdie_event_kind;

// The options of `class_` of `sdrkit_birdie_event`.
typedef enum sdrkit_birdie_event_class_ {
  // Bare carrier
  SDRKIT_BIRDIE_EVENT_CLASS__BARE = 0,
  // Modulated or wide
  SDRKIT_BIRDIE_EVENT_CLASS__MODULATED = 1,
} sdrkit_birdie_event_class_;

// A persistent carrier was listed, came back or went.
typedef struct sdrkit_birdie_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the row. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_birdie_event_kind`.
  int32_t kind;
  // The frequency. A frequency in Hz.
  double frequency;
  // The mean level over the noise. Unit: dB.
  double level_db;
  // The class. A value of `sdrkit_birdie_event_class_`.
  int32_t class_;
  // The drift. Unit: Hz/min.
  double drift_hz_min;
  // The scatter. Unit: Hz.
  double scatter_hz;
} sdrkit_birdie_event;

// An evaluation was logged.
typedef struct sdrkit_obw_reading_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the evaluation. A length of time in seconds.
  double time_s;
  // The band that holds the power share, half the rest below and half above it. Unit: kHz.
  double obw_khz;
  // The lower edge of the occupied band. A frequency in Hz.
  double obw_low;
  // The upper edge of the occupied band. A frequency in Hz.
  double obw_high;
  // The width at 3 dB under the peak of the smoothed spectrum. Unit: kHz.
  double bw3_khz;
  // The width at 6 dB under the peak. Unit: kHz.
  double bw6_khz;
  // The width at 20 dB under the peak. Unit: kHz.
  double bw20_khz;
  // The width at 26 dB under the peak. Unit: kHz.
  double bw26_khz;
  // The middle of the occupied band from the tuned frequency. Unit: Hz.
  double centre_offset_hz;
  // The power between the edges over the floor, dB re full scale. Unit: dB.
  double power_db;
  // The signal power over the floor in the same bins. Unit: dB.
  double snr_db;
  // True if `adjacent_low_db` holds a value.
  bool has_adjacent_low_db;
  // The power of the band one adjacent offset below, under the signal; no value outside the spectrum. Unit: dB.
  double adjacent_low_db;
  // True if `adjacent_high_db` holds a value.
  bool has_adjacent_high_db;
  // The same above. Unit: dB.
  double adjacent_high_db;
  // True if `alternate_low_db` holds a value.
  bool has_alternate_low_db;
  // The power of the band two adjacent offsets below, under the signal. Unit: dB.
  double alternate_low_db;
  // True if `alternate_high_db` holds a value.
  bool has_alternate_high_db;
  // The same above. Unit: dB.
  double alternate_high_db;
} sdrkit_obw_reading_event;

// The options of `kind` of `sdrkit_ptt_log_ptt_transmission_event`.
typedef enum sdrkit_ptt_log_ptt_transmission_event_kind {
  // Transmission
  SDRKIT_PTT_LOG_PTT_TRANSMISSION_EVENT_KIND_TRANSMISSION = 0,
  // Kerchunk
  SDRKIT_PTT_LOG_PTT_TRANSMISSION_EVENT_KIND_KERCHUNK = 1,
  // Stuck key
  SDRKIT_PTT_LOG_PTT_TRANSMISSION_EVENT_KIND_STUCK_KEY = 2,
} sdrkit_ptt_log_ptt_transmission_event_kind;

// A transmission ended, or a stuck key was found.
typedef struct sdrkit_ptt_log_ptt_transmission_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the transmission, from 1.
  int64_t number;
  // The kind. A value of `sdrkit_ptt_log_ptt_transmission_event_kind`.
  int32_t kind;
  // The start, signal time. A length of time in seconds.
  double start_s;
  // The length (so far, for a stuck key). A length of time in seconds.
  double length_s;
  // The mean level over the floor of its keyed blocks. Unit: dB.
  double over_floor_db;
  // True if `gap_s` holds a value.
  bool has_gap_s;
  // The time from the end of the transmission before; no value for the first. A length of time in seconds.
  double gap_s;
  // The mean level of its keyed blocks, relative to full scale. Unit: dB.
  double level_db;
} sdrkit_ptt_log_ptt_transmission_event;

// The options of `kind` of `sdrkit_noise_locator_pattern_event`.
typedef enum sdrkit_noise_locator_pattern_event_kind {
  // Found
  SDRKIT_NOISE_LOCATOR_PATTERN_EVENT_KIND_FOUND = 0,
  // Gone
  SDRKIT_NOISE_LOCATOR_PATTERN_EVENT_KIND_GONE = 1,
} sdrkit_noise_locator_pattern_event_kind;

// The options of `class_` of `sdrkit_noise_locator_pattern_event`.
typedef enum sdrkit_noise_locator_pattern_event_class_ {
  // Twice the mains
  SDRKIT_NOISE_LOCATOR_PATTERN_EVENT_CLASS__TWICE_MAINS = 0,
  // The mains
  SDRKIT_NOISE_LOCATOR_PATTERN_EVENT_CLASS__MAINS = 1,
  // Other steady rate
  SDRKIT_NOISE_LOCATOR_PATTERN_EVENT_CLASS__STEADY = 2,
  // None
  SDRKIT_NOISE_LOCATOR_PATTERN_EVENT_CLASS__NONE = 3,
} sdrkit_noise_locator_pattern_event_class_;

// A pattern was found or went.
typedef struct sdrkit_noise_locator_pattern_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the report. A length of time in seconds.
  double time_s;
  // Found or gone. A value of `sdrkit_noise_locator_pattern_event_kind`.
  int32_t kind;
  // The repetition rate. Unit: Hz.
  double repetition_hz;
  // The class of the rate. A value of `sdrkit_noise_locator_pattern_event_class_`.
  int32_t class_;
  // The impulses per second of the window. Unit: 1/s.
  double impulse_rate;
  // The median impulse level over the noise of the analysis band. Unit: dB.
  double level_db;
  // The share of the impulses on the rate. Unit: %.
  double strength_pct;
} sdrkit_noise_locator_pattern_event;

// The options of `kind` of `sdrkit_freq_accuracy_event`.
typedef enum sdrkit_freq_accuracy_event_kind {
  // Frequency
  SDRKIT_FREQ_ACCURACY_EVENT_KIND_FREQUENCY = 0,
  // Step
  SDRKIT_FREQ_ACCURACY_EVENT_KIND_STEP = 1,
  // Lost
  SDRKIT_FREQ_ACCURACY_EVENT_KIND_LOST = 2,
  // Returned
  SDRKIT_FREQ_ACCURACY_EVENT_KIND_RETURNED = 3,
} sdrkit_freq_accuracy_event_kind;

// A row of the `freq_accuracy` log was written.
typedef struct sdrkit_freq_accuracy_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_freq_accuracy_event_kind`.
  int32_t kind;
  // The offset of the reading (for a loss, the last reading). A frequency in Hz.
  double offset_hz;
  // True if `step_hz` holds a value.
  bool has_step_hz;
  // The size of a step. A frequency in Hz.
  double step_hz;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // The carrier to noise density of the reading. Unit: dB-Hz.
  double cn0_dbhz;
} sdrkit_freq_accuracy_event;

// The options of `crossing` of `sdrkit_sinad_crossing_event`.
typedef enum sdrkit_sinad_crossing_event_crossing {
  // None
  SDRKIT_SINAD_CROSSING_EVENT_CROSSING_NONE = 0,
  // Fell under the mark
  SDRKIT_SINAD_CROSSING_EVENT_CROSSING_FELL = 1,
  // Rose to the mark
  SDRKIT_SINAD_CROSSING_EVENT_CROSSING_ROSE = 2,
} sdrkit_sinad_crossing_event_crossing;

// The SINAD crossed the sensitivity mark.
typedef struct sdrkit_sinad_crossing_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time. A length of time in seconds.
  double time_s;
  // SINAD. Unit: dB.
  double sinad_db;
  // The change against the mark. A value of `sdrkit_sinad_crossing_event_crossing`.
  int32_t crossing;
  // The test tone. A frequency in Hz.
  double tone_hz;
  // The peak deviation. Unit: kHz.
  double deviation_khz;
  // THD. Unit: %.
  double thd_pct;
} sdrkit_sinad_crossing_event;

// A transmission was analysed.
typedef struct sdrkit_tx_stability_transmission_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The key-up, signal time. A length of time in seconds.
  double time_s;
  // The length of the transmission. A length of time in seconds.
  double length_s;
  // The frequency readings in it.
  int64_t readings;
  // The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency. A frequency in Hz.
  double start_hz;
  // The mean frequency of the last 0.4 s. A frequency in Hz.
  double final_hz;
  // Start minus final. A frequency in Hz.
  double keyup_step_hz;
  // The time of the last reading outside the settling band, plus 0.05 s. A length of time in seconds.
  double settle_s;
  // The larger of 10 Hz and a tenth of the key-up step. A frequency in Hz.
  double settle_band_hz;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The least-squares slope of the readings of the second half. Unit: Hz/s.
  double drift_hz_s;
  // The mean of all readings. A frequency in Hz.
  double mean_hz;
  // True if `mean_ppm` holds a value.
  bool has_mean_ppm;
  // The mean over the tuned frequency; no value without it. Unit: ppm.
  double mean_ppm;
  // True if `scatter_hz` holds a value.
  bool has_scatter_hz;
  // The RMS residual about the drift line. A frequency in Hz.
  double scatter_hz;
} sdrkit_tx_stability_transmission_event;

// A burst was analysed.
typedef struct sdrkit_afsk_tuner_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The start of the burst, signal time. A length of time in seconds.
  double time_s;
  // The length of the burst. A length of time in seconds.
  double length_s;
  // The mark tone. A frequency in Hz.
  double mark_hz;
  // The space tone. A frequency in Hz.
  double space_hz;
  // The space-band power over the mark-band power. Unit: dB.
  double twist_db;
  // The 99.5th percentile of the band-limited audio. Unit: kHz.
  double peak_deviation_khz;
  // True if `gap_s` holds a value.
  bool has_gap_s;
  // The time since the end of the burst before; no value for the first. A length of time in seconds.
  double gap_s;
} sdrkit_afsk_tuner_burst_event;

// The options of `kind` of `sdrkit_glitch_event`.
typedef enum sdrkit_glitch_event_kind {
  // Dropped samples
  SDRKIT_GLITCH_EVENT_KIND_DROPPED_SAMPLES = 0,
  // Repeated samples
  SDRKIT_GLITCH_EVENT_KIND_REPEATED_SAMPLES = 1,
  // Zero run
  SDRKIT_GLITCH_EVENT_KIND_ZERO_RUN = 2,
  // Repeated block
  SDRKIT_GLITCH_EVENT_KIND_REPEATED_BLOCK = 3,
  // Level step
  SDRKIT_GLITCH_EVENT_KIND_LEVEL_STEP = 4,
} sdrkit_glitch_event_kind;

// A stream fault of `glitch`.
typedef struct sdrkit_glitch_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the fault. A length of time in seconds.
  double time_s;
  // The kind. A value of `sdrkit_glitch_event_kind`.
  int32_t kind;
  // True if `samples` holds a value.
  bool has_samples;
  // The samples missing or repeated.
  int64_t samples;
  // True if `phase_step_rad` holds a value.
  bool has_phase_step_rad;
  // The phase step of the tone. Unit: rad.
  double phase_step_rad;
  // True if `step_db` holds a value.
  bool has_step_db;
  // The size of a level step. Unit: dB.
  double step_db;
  // True if `from_db` holds a value.
  bool has_from_db;
  // The level before a step. Unit: dB.
  double from_db;
  // True if `to_db` holds a value.
  bool has_to_db;
  // The level after a step. Unit: dB.
  double to_db;
} sdrkit_glitch_event;

// The options of `kind` of `sdrkit_intermod_product_event`.
typedef enum sdrkit_intermod_product_event_kind {
  // 2a - b
  SDRKIT_INTERMOD_PRODUCT_EVENT_KIND_2A_B = 0,
  // 2b - a
  SDRKIT_INTERMOD_PRODUCT_EVENT_KIND_2B_A = 1,
  // b - a
  SDRKIT_INTERMOD_PRODUCT_EVENT_KIND_B_A = 2,
} sdrkit_intermod_product_event_kind;

// The options of `verdict` of `sdrkit_intermod_product_event`.
typedef enum sdrkit_intermod_product_event_verdict {
  // Follows
  SDRKIT_INTERMOD_PRODUCT_EVENT_VERDICT_FOLLOWS = 0,
  // Independent
  SDRKIT_INTERMOD_PRODUCT_EVENT_VERDICT_INDEPENDENT = 1,
  // Unclear
  SDRKIT_INTERMOD_PRODUCT_EVENT_VERDICT_UNCLEAR = 2,
  // Cannot tell
  SDRKIT_INTERMOD_PRODUCT_EVENT_VERDICT_CANNOT_TELL = 3,
} sdrkit_intermod_product_event_verdict;

// A verdict of a product that held for two evaluations.
typedef struct sdrkit_intermod_product_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time. A length of time in seconds.
  double time_s;
  // The frequency of the peak. A frequency in Hz.
  double frequency;
  // The kind of product. A value of `sdrkit_intermod_product_event_kind`.
  int32_t kind;
  // The lower parent. A frequency in Hz.
  double parent_a;
  // The upper parent. A frequency in Hz.
  double parent_b;
  // The mean level under the weaker parent. Unit: dB.
  double below_db;
  // The verdict. A value of `sdrkit_intermod_product_event_verdict`.
  int32_t verdict;
  // True if `slope` holds a value.
  bool has_slope;
  // The slope.
  double slope;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The correlation.
  double correlation;
  // The snapshots of the fit.
  int64_t points;
} sdrkit_intermod_product_event;

// The options of `from_class` of `sdrkit_beacon_monitor_change_event`.
typedef enum sdrkit_beacon_monitor_change_event_from_class {
  // None
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_FROM_CLASS_NONE = 0,
  // Steady
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_FROM_CLASS_STEADY = 1,
  // Fading
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_FROM_CLASS_FADING = 2,
  // Aurora-like
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_FROM_CLASS_AURORA_LIKE = 3,
} sdrkit_beacon_monitor_change_event_from_class;

// The options of `to_class` of `sdrkit_beacon_monitor_change_event`.
typedef enum sdrkit_beacon_monitor_change_event_to_class {
  // None
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_TO_CLASS_NONE = 0,
  // Steady
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_TO_CLASS_STEADY = 1,
  // Fading
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_TO_CLASS_FADING = 2,
  // Aurora-like
  SDRKIT_BEACON_MONITOR_CHANGE_EVENT_TO_CLASS_AURORA_LIKE = 3,
} sdrkit_beacon_monitor_change_event_to_class;

// A change of class of the path.
typedef struct sdrkit_beacon_monitor_change_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time. A length of time in seconds.
  double time_s;
  // The class before. A value of `sdrkit_beacon_monitor_change_event_from_class`.
  int32_t from_class;
  // The class after. A value of `sdrkit_beacon_monitor_change_event_to_class`.
  int32_t to_class;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The last level. Unit: dB.
  double level_db;
  // True if `spread_hz` holds a value.
  bool has_spread_hz;
  // The median spread of the decision. Unit: Hz.
  double spread_hz;
  // True if `swing_db` holds a value.
  bool has_swing_db;
  // The swing of the decision. Unit: dB.
  double swing_db;
} sdrkit_beacon_monitor_change_event;

// Fills `rows` with the changed rows of the table "log" of the stack "fm_monitor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `identity` names a row.
static inline int32_t sdrkit_fm_monitor_log_rows(SDRKIT_HANDLE handle,
                                                sdrkit_fm_monitor_log_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.FmMonitorLogRow", "log", rows, sizeof(sdrkit_fm_monitor_log_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fm_monitor.pilot"
// (Pilot). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fm_monitor_pilot_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fm_monitor_pilot_event *event) {
  return sdrkit_typed_event(handle, n, "fm_monitor.pilot", "meter.FmMonitorPilotEvent", event, sizeof(sdrkit_fm_monitor_pilot_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fm_monitor.rds"
// (RDS). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fm_monitor_rds_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fm_monitor_rds_event *event) {
  return sdrkit_typed_event(handle, n, "fm_monitor.rds", "meter.FmMonitorRdsEvent", event, sizeof(sdrkit_fm_monitor_rds_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fm_monitor.sca"
// (Subcarrier). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fm_monitor_sca_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fm_monitor_sca_event *event) {
  return sdrkit_typed_event(handle, n, "fm_monitor.sca", "meter.FmMonitorScaEvent", event, sizeof(sdrkit_fm_monitor_sca_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fm_monitor.over_deviation"
// (Over-deviation). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fm_monitor_over_deviation_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fm_monitor_over_deviation_event *event) {
  return sdrkit_typed_event(handle, n, "fm_monitor.over_deviation", "meter.FmMonitorOverDeviationEvent", event, sizeof(sdrkit_fm_monitor_over_deviation_event));
}

// Runs the command "clear" (Clear) of the stack "fm_monitor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fm_monitor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fm_monitor" are the sdrkit_fm_monitor_gen_*() functions
// after the settings of its receiver.

// The setting "over_deviation_khz" (Over-deviation threshold) of the stack "fm_monitor". Unit: kHz. A second whose peak deviation is over this is counted and logged.
static inline int32_t sdrkit_fm_monitor_get_over_deviation_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "over_deviation_khz", value);
}
static inline int32_t sdrkit_fm_monitor_set_over_deviation_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "over_deviation_khz", value);
}

// The setting "deviation_limit" (Peak deviation limit) of the stack "fm_monitor". A value of `sdrkit_fm_monitor_deviation_limit`. The maximum peak deviation of ITU-R BS.450; the pilot is in its usual range at 8 to 10 % of it.
static inline int32_t sdrkit_fm_monitor_get_deviation_limit(SDRKIT_HANDLE handle, sdrkit_fm_monitor_deviation_limit *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deviation_limit", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_monitor_deviation_limit)index;
  return code;
}
static inline int32_t sdrkit_fm_monitor_set_deviation_limit(SDRKIT_HANDLE handle, sdrkit_fm_monitor_deviation_limit value) {
  return sdrkit_typed_setting_set_enum(handle, "deviation_limit", (int32_t)value);
}

// The setting "pilot_margin_khz" (Pilot range margin) of the stack "fm_monitor". Unit: kHz. The reading may lie this far outside 8 to 10 % of the limit and still count as in the usual range.
static inline int32_t sdrkit_fm_monitor_get_pilot_margin_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pilot_margin_khz", value);
}
static inline int32_t sdrkit_fm_monitor_set_pilot_margin_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pilot_margin_khz", value);
}

// The setting "gen_audio_khz" (Audio tone deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_audio_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_audio_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_audio_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_audio_khz", value);
}

// The setting "gen_tone2_khz" (Second tone deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_tone2_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone2_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_tone2_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone2_khz", value);
}

// The setting "gen_pilot_khz" (Pilot deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_pilot_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pilot_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_pilot_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pilot_khz", value);
}

// The setting "gen_stereo_khz" (L-R tone deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_stereo_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_stereo_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_stereo_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_stereo_khz", value);
}

// The setting "gen_rds_khz" (RDS deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_rds_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rds_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_rds_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rds_khz", value);
}

// The setting "gen_sca67_khz" (67 kHz subcarrier deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_sca67_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_sca67_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_sca67_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_sca67_khz", value);
}

// The setting "gen_sca92_khz" (92 kHz subcarrier deviation) of the test signal generator of the stack "fm_monitor". Unit: kHz.
static inline int32_t sdrkit_fm_monitor_gen_get_sca92_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_sca92_khz", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_sca92_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_sca92_khz", value);
}

// The setting "gen_scale" (Multiplex scale) of the test signal generator of the stack "fm_monitor".
static inline int32_t sdrkit_fm_monitor_gen_get_scale(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_scale", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_scale(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_scale", value);
}

// The setting "gen_pilot_from_s" (Pilot from) of the test signal generator of the stack "fm_monitor". Unit: s.
static inline int32_t sdrkit_fm_monitor_gen_get_pilot_from_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pilot_from_s", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_pilot_from_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pilot_from_s", value);
}

// The setting "gen_pilot_until_s" (Pilot until) of the test signal generator of the stack "fm_monitor". Unit: s. A negative value keeps the pilot on.
static inline int32_t sdrkit_fm_monitor_gen_get_pilot_until_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pilot_until_s", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_pilot_until_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pilot_until_s", value);
}

// The setting "gen_rds_from_s" (RDS from) of the test signal generator of the stack "fm_monitor". Unit: s.
static inline int32_t sdrkit_fm_monitor_gen_get_rds_from_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rds_from_s", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_rds_from_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rds_from_s", value);
}

// The setting "gen_rds_until_s" (RDS until) of the test signal generator of the stack "fm_monitor". Unit: s. A negative value keeps RDS on.
static inline int32_t sdrkit_fm_monitor_gen_get_rds_until_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rds_until_s", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_rds_until_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rds_until_s", value);
}

// The setting "gen_sca_from_s" (Subcarriers from) of the test signal generator of the stack "fm_monitor". Unit: s.
static inline int32_t sdrkit_fm_monitor_gen_get_sca_from_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_sca_from_s", value);
}
static inline int32_t sdrkit_fm_monitor_gen_set_sca_from_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_sca_from_s", value);
}

// Fills `rows` with the changed rows of the table "log" of the stack "am_monitor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `identity` names a row.
static inline int32_t sdrkit_am_monitor_log_rows(SDRKIT_HANDLE handle,
                                                sdrkit_am_monitor_log_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.AmMonitorLogRow", "log", rows, sizeof(sdrkit_am_monitor_log_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "am_monitor.carrier"
// (Carrier). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_am_monitor_carrier_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_am_monitor_carrier_event *event) {
  return sdrkit_typed_event(handle, n, "am_monitor.carrier", "meter.AmMonitorCarrierEvent", event, sizeof(sdrkit_am_monitor_carrier_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "am_monitor.over_modulation"
// (Over-modulation). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_am_monitor_over_modulation_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_am_monitor_over_modulation_event *event) {
  return sdrkit_typed_event(handle, n, "am_monitor.over_modulation", "meter.AmMonitorOverModulationEvent", event, sizeof(sdrkit_am_monitor_over_modulation_event));
}

// Runs the command "clear" (Clear) of the stack "am_monitor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_am_monitor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "am_monitor" are the sdrkit_am_monitor_gen_*() functions
// after the settings of its receiver.

// The setting "over_mod_share_pct" (Over-modulation: share of a second at zero carrier) of the stack "am_monitor". Unit: %. A second with more of its blocks at zero carrier than this is counted and logged.
static inline int32_t sdrkit_am_monitor_get_over_mod_share_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "over_mod_share_pct", value);
}
static inline int32_t sdrkit_am_monitor_set_over_mod_share_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "over_mod_share_pct", value);
}

// The setting "carrier_loss_db" (Carrier lost below the best level by) of the stack "am_monitor". Unit: dB. The carrier counts as lost when its level falls this far below the best level of about the last minute.
static inline int32_t sdrkit_am_monitor_get_carrier_loss_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "carrier_loss_db", value);
}
static inline int32_t sdrkit_am_monitor_set_carrier_loss_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "carrier_loss_db", value);
}

// The setting "gen_depth1" (First tone depth) of the test signal generator of the stack "am_monitor". The peak of the first tone as a share of the carrier.
static inline int32_t sdrkit_am_monitor_gen_get_depth1(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth1", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_depth1(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth1", value);
}

// The setting "gen_depth2" (Second tone depth) of the test signal generator of the stack "am_monitor". The peak of the second tone as a share of the carrier.
static inline int32_t sdrkit_am_monitor_gen_get_depth2(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth2", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_depth2(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth2", value);
}

// The setting "gen_tone1_hz" (First tone) of the test signal generator of the stack "am_monitor". Unit: Hz.
static inline int32_t sdrkit_am_monitor_gen_get_tone1_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone1_hz", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_tone1_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone1_hz", value);
}

// The setting "gen_tone2_hz" (Second tone) of the test signal generator of the stack "am_monitor". Unit: Hz.
static inline int32_t sdrkit_am_monitor_gen_get_tone2_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone2_hz", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_tone2_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone2_hz", value);
}

// The setting "gen_asymmetry" (Asymmetry) of the test signal generator of the stack "am_monitor". The positive half of the modulation is raised by this share of itself.
static inline int32_t sdrkit_am_monitor_gen_get_asymmetry(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_asymmetry", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_asymmetry(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_asymmetry", value);
}

// The setting "gen_cut_from_s" (Carrier cut from) of the test signal generator of the stack "am_monitor". Unit: s. The carrier is off from this time; a negative value is no cut.
static inline int32_t sdrkit_am_monitor_gen_get_cut_from_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cut_from_s", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_cut_from_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cut_from_s", value);
}

// The setting "gen_cut_until_s" (Carrier cut until) of the test signal generator of the stack "am_monitor". Unit: s. The carrier is back from this time.
static inline int32_t sdrkit_am_monitor_gen_get_cut_until_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cut_until_s", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_cut_until_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cut_until_s", value);
}

// The setting "gen_clip" (Clip at zero carrier) of the test signal generator of the stack "am_monitor".
static inline int32_t sdrkit_am_monitor_gen_get_clip(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_clip", value);
}
static inline int32_t sdrkit_am_monitor_gen_set_clip(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_clip", value);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "occupancy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_occupancy_channels_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_occupancy_channel_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.OccupancyChannelRow", "channels", rows, sizeof(sdrkit_occupancy_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "activity" of the stack "occupancy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_occupancy_activity_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_occupancy_activity_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.OccupancyActivityRow", "activity", rows, sizeof(sdrkit_occupancy_activity_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "occupancy.activity"
// (Activity). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_occupancy_activity_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_occupancy_activity_event *event) {
  return sdrkit_typed_event(handle, n, "occupancy.activity", "meter.OccupancyActivityEvent", event, sizeof(sdrkit_occupancy_activity_event));
}

// Runs the command "clear" (Clear) of the stack "occupancy" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_occupancy_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "occupancy" on the session of `handle`. Learns the floor again and drops the channel states; the tables stay.
static inline int32_t sdrkit_occupancy_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "occupancy" are the sdrkit_occupancy_gen_*() functions
// after the settings of its receiver.

// The setting "grid_khz" (Channel width) of the stack "occupancy". Unit: kHz. The width of each channel of the grid, rounded to whole FFT bins (at least 2).
static inline int32_t sdrkit_occupancy_get_grid_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "grid_khz", value);
}
static inline int32_t sdrkit_occupancy_set_grid_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "grid_khz", value);
}

// The setting "on_db" (Busy above (over the floor)) of the stack "occupancy". Unit: dB.
static inline int32_t sdrkit_occupancy_get_on_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "on_db", value);
}
static inline int32_t sdrkit_occupancy_set_on_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "on_db", value);
}

// The setting "off_db" (Idle below (over the floor)) of the stack "occupancy". Unit: dB.
static inline int32_t sdrkit_occupancy_get_off_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "off_db", value);
}
static inline int32_t sdrkit_occupancy_set_off_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "off_db", value);
}

// The setting "floor_share" (Floor quantile of the channels) of the stack "occupancy". The floor is the channel power at this quantile of the sorted channels.
static inline int32_t sdrkit_occupancy_get_floor_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "floor_share", value);
}
static inline int32_t sdrkit_occupancy_set_floor_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "floor_share", value);
}

// The setting "min_log_s" (Shortest transmission listed) of the stack "occupancy". Unit: s.
static inline int32_t sdrkit_occupancy_get_min_log_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_log_s", value);
}
static inline int32_t sdrkit_occupancy_set_min_log_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_log_s", value);
}

// The setting "gen_tones" (Carriers) of the test signal generator of the stack "occupancy". Carriers separated by ';', each 'offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s, from s, to s'; the values after the amplitude can be left out.
static inline int32_t sdrkit_occupancy_gen_get_tones(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tones", value);
}
static inline int32_t sdrkit_occupancy_gen_set_tones(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tones", value);
}

// Fills `rows` with the changed rows of the table "sub_bands" of the stack "noise_floor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noise_floor_sub_bands_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_noise_floor_sub_band_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.NoiseFloorSubBandRow", "sub_bands", rows, sizeof(sdrkit_noise_floor_sub_band_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "noise_events" of the stack "noise_floor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noise_floor_noise_events_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_noise_floor_event_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.NoiseFloorEventRow", "noise_events", rows, sizeof(sdrkit_noise_floor_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "noise_floor.event"
// (Noise event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_noise_floor_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_noise_floor_event *event) {
  return sdrkit_typed_event(handle, n, "noise_floor.event", "meter.NoiseFloorEvent", event, sizeof(sdrkit_noise_floor_event));
}

// Runs the command "clear" (Clear) of the stack "noise_floor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_noise_floor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "noise_floor" on the session of `handle`. Drops the histories of the sub-bands and their rise states; the tables stay.
static inline int32_t sdrkit_noise_floor_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "noise_floor" are the sdrkit_noise_floor_gen_*() functions
// after the settings of its receiver.

// The setting "sub_bands" (Sub-bands) of the stack "noise_floor".
static inline int32_t sdrkit_noise_floor_get_sub_bands(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sub_bands", value);
}
static inline int32_t sdrkit_noise_floor_set_sub_bands(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sub_bands", value);
}

// The setting "rise_db" (Rise over the baseline) of the stack "noise_floor". Unit: dB.
static inline int32_t sdrkit_noise_floor_get_rise_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "rise_db", value);
}
static inline int32_t sdrkit_noise_floor_set_rise_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "rise_db", value);
}

// The setting "hold_s" (Rise must hold for) of the stack "noise_floor". Unit: s.
static inline int32_t sdrkit_noise_floor_get_hold_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hold_s", value);
}
static inline int32_t sdrkit_noise_floor_set_hold_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hold_s", value);
}

// The setting "baseline_min" (Baseline window) of the stack "noise_floor". Unit: min.
static inline int32_t sdrkit_noise_floor_get_baseline_min(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "baseline_min", value);
}
static inline int32_t sdrkit_noise_floor_set_baseline_min(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "baseline_min", value);
}

// The setting "whole_band_share" (Sub-bands that make a whole-band change) of the stack "noise_floor".
static inline int32_t sdrkit_noise_floor_get_whole_band_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "whole_band_share", value);
}
static inline int32_t sdrkit_noise_floor_set_whole_band_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "whole_band_share", value);
}

// The setting "gen_extra_hz" (Added noise centre) of the test signal generator of the stack "noise_floor". Unit: Hz.
static inline int32_t sdrkit_noise_floor_gen_get_extra_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_hz", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_extra_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_hz", value);
}

// The setting "gen_extra_bw_hz" (Added noise width) of the test signal generator of the stack "noise_floor". Unit: Hz. Twice the cut-off of the one-pole low-pass that shapes the added noise.
static inline int32_t sdrkit_noise_floor_gen_get_extra_bw_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_bw_hz", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_extra_bw_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_bw_hz", value);
}

// The setting "gen_extra_power" (Added noise power) of the test signal generator of the stack "noise_floor".
static inline int32_t sdrkit_noise_floor_gen_get_extra_power(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_power", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_extra_power(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_power", value);
}

// The setting "gen_extra_from_s" (Added noise from) of the test signal generator of the stack "noise_floor". Unit: s.
static inline int32_t sdrkit_noise_floor_gen_get_extra_from_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_from_s", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_extra_from_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_from_s", value);
}

// The setting "gen_extra_to_s" (Added noise until) of the test signal generator of the stack "noise_floor". Unit: s.
static inline int32_t sdrkit_noise_floor_gen_get_extra_to_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_to_s", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_extra_to_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_to_s", value);
}

// The setting "gen_tone_hz" (Carrier offset) of the test signal generator of the stack "noise_floor". Unit: Hz.
static inline int32_t sdrkit_noise_floor_gen_get_tone_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_tone_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value);
}

// The setting "gen_tone_amp" (Carrier amplitude) of the test signal generator of the stack "noise_floor".
static inline int32_t sdrkit_noise_floor_gen_get_tone_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_amp", value);
}
static inline int32_t sdrkit_noise_floor_gen_set_tone_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_amp", value);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "burst_finder" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_burst_finder_bursts_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_burst_finder_burst_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.BurstFinderBurstRow", "bursts", rows, sizeof(sdrkit_burst_finder_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "repeating" of the stack "burst_finder" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_burst_finder_repeating_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_burst_finder_repeating_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.BurstFinderRepeatingRow", "repeating", rows, sizeof(sdrkit_burst_finder_repeating_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "burst.found"
// (Burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_burst_finder_burst_found_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_burst_finder_burst_found_event *event) {
  return sdrkit_typed_event(handle, n, "burst.found", "meter.BurstFoundEvent", event, sizeof(sdrkit_burst_finder_burst_found_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "burst.repeating"
// (Repeating source). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_burst_finder_burst_repeating_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_burst_finder_burst_repeating_event *event) {
  return sdrkit_typed_event(handle, n, "burst.repeating", "meter.BurstRepeatingEvent", event, sizeof(sdrkit_burst_finder_burst_repeating_event));
}

// Runs the command "clear" (Clear) of the stack "burst_finder" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_burst_finder_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "burst_finder" on the session of `handle`. Drops the bursts in progress; the tables stay.
static inline int32_t sdrkit_burst_finder_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "burst_finder" are the sdrkit_burst_finder_gen_*() functions
// after the settings of its receiver.

// The setting "hot_db" (Hot bin over the frame median) of the stack "burst_finder". Unit: dB.
static inline int32_t sdrkit_burst_finder_get_hot_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hot_db", value);
}
static inline int32_t sdrkit_burst_finder_set_hot_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hot_db", value);
}

// The setting "max_burst_s" (Longest burst) of the stack "burst_finder". Unit: s.
static inline int32_t sdrkit_burst_finder_get_max_burst_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_burst_s", value);
}
static inline int32_t sdrkit_burst_finder_set_max_burst_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_burst_s", value);
}

// The setting "repeat_tolerance_pct" (Interval agreement) of the stack "burst_finder". Unit: %.
static inline int32_t sdrkit_burst_finder_get_repeat_tolerance_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "repeat_tolerance_pct", value);
}
static inline int32_t sdrkit_burst_finder_set_repeat_tolerance_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "repeat_tolerance_pct", value);
}

// The setting "fft_size" (Frame length) of the stack "burst_finder". Points of a frame: 128, 256, 512 or 1024 (rounded to a power of two). Frames step by half of it.
static inline int32_t sdrkit_burst_finder_get_fft_size(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "fft_size", value);
}
static inline int32_t sdrkit_burst_finder_set_fft_size(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "fft_size", value);
}

// The setting "gen_bursts" (Bursts) of the test signal generator of the stack "burst_finder". Bursts separated by ';', each 'start s, length s, offset Hz, width Hz, amplitude' and for a train 'count, every s, jitter' (a share of the interval).
static inline int32_t sdrkit_burst_finder_gen_get_bursts(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_bursts", value);
}
static inline int32_t sdrkit_burst_finder_gen_set_bursts(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_bursts", value);
}

// Fills `rows` with the changed rows of the table "hoppers" of the stack "hopper" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hopper_hoppers_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hopper_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.HopperRow", "hoppers", rows, sizeof(sdrkit_hopper_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hopper.changed"
// (Hopper). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hopper_changed_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hopper_changed_event *event) {
  return sdrkit_typed_event(handle, n, "hopper.changed", "meter.HopperChangedEvent", event, sizeof(sdrkit_hopper_changed_event));
}

// Runs the command "clear" (Clear) of the stack "hopper" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_hopper_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "hopper" on the session of `handle`. Drops the dwells and the readings; the table stays.
static inline int32_t sdrkit_hopper_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hopper" are the sdrkit_hopper_gen_*() functions
// after the settings of its receiver.

// The setting "present_db" (Strongest bin over the frame median) of the stack "hopper". Unit: dB.
static inline int32_t sdrkit_hopper_get_present_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "present_db", value);
}
static inline int32_t sdrkit_hopper_set_present_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "present_db", value);
}

// The setting "min_dwells" (Dwells before a report) of the stack "hopper".
static inline int32_t sdrkit_hopper_get_min_dwells(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "min_dwells", value);
}
static inline int32_t sdrkit_hopper_set_min_dwells(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "min_dwells", value);
}

// The setting "min_channels" (Fewest channels) of the stack "hopper".
static inline int32_t sdrkit_hopper_get_min_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "min_channels", value);
}
static inline int32_t sdrkit_hopper_set_min_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "min_channels", value);
}

// The setting "gen_channels" (Channels) of the test signal generator of the stack "hopper".
static inline int32_t sdrkit_hopper_gen_get_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_channels", value);
}
static inline int32_t sdrkit_hopper_gen_set_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_channels", value);
}

// The setting "gen_spacing_hz" (Channel spacing) of the test signal generator of the stack "hopper". Unit: Hz.
static inline int32_t sdrkit_hopper_gen_get_spacing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spacing_hz", value);
}
static inline int32_t sdrkit_hopper_gen_set_spacing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spacing_hz", value);
}

// The setting "gen_dwell_s" (Dwell) of the test signal generator of the stack "hopper". Unit: s.
static inline int32_t sdrkit_hopper_gen_get_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_dwell_s", value);
}
static inline int32_t sdrkit_hopper_gen_set_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_dwell_s", value);
}

// The setting "gen_guard_s" (Guard) of the test signal generator of the stack "hopper". Unit: s.
static inline int32_t sdrkit_hopper_gen_get_guard_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_guard_s", value);
}
static inline int32_t sdrkit_hopper_gen_set_guard_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_guard_s", value);
}

// The setting "gen_jitter" (Hop clock jitter) of the test signal generator of the stack "hopper".
static inline int32_t sdrkit_hopper_gen_get_jitter(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_jitter", value);
}
static inline int32_t sdrkit_hopper_gen_set_jitter(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_jitter", value);
}

// The setting "gen_fsk_hz" (FSK deviation) of the test signal generator of the stack "hopper". Unit: Hz.
static inline int32_t sdrkit_hopper_gen_get_fsk_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fsk_hz", value);
}
static inline int32_t sdrkit_hopper_gen_set_fsk_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fsk_hz", value);
}

// The setting "gen_fsk_bps" (FSK bit rate) of the test signal generator of the stack "hopper". Unit: bit/s.
static inline int32_t sdrkit_hopper_gen_get_fsk_bps(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fsk_bps", value);
}
static inline int32_t sdrkit_hopper_gen_set_fsk_bps(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fsk_bps", value);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "birdie" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_db`, highest first.
static inline int32_t sdrkit_birdie_carriers_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_birdie_carrier_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.BirdieCarrierRow", "carriers", rows, sizeof(sdrkit_birdie_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carrier_events" of the stack "birdie" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_birdie_carrier_events_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_birdie_event_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.BirdieEventRow", "carrier_events", rows, sizeof(sdrkit_birdie_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "birdie.event"
// (Carrier event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_birdie_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_birdie_event *event) {
  return sdrkit_typed_event(handle, n, "birdie.event", "meter.BirdieEvent", event, sizeof(sdrkit_birdie_event));
}

// Runs the command "clear" (Clear) of the stack "birdie" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_birdie_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "birdie" on the session of `handle`. Drops the tracks and the list; the log stays.
static inline int32_t sdrkit_birdie_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "birdie" are the sdrkit_birdie_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Peak over the noise) of the stack "birdie". Unit: dB.
static inline int32_t sdrkit_birdie_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_birdie_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "presence_pct" (Present in at least) of the stack "birdie". Unit: %.
static inline int32_t sdrkit_birdie_get_presence_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "presence_pct", value);
}
static inline int32_t sdrkit_birdie_set_presence_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "presence_pct", value);
}

// The setting "window_s" (Presence window) of the stack "birdie". Unit: s.
static inline int32_t sdrkit_birdie_get_window_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "window_s", value);
}
static inline int32_t sdrkit_birdie_set_window_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "window_s", value);
}

// The setting "bare_share" (Bare carrier when the narrowness is at least) of the stack "birdie".
static inline int32_t sdrkit_birdie_get_bare_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "bare_share", value);
}
static inline int32_t sdrkit_birdie_set_bare_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "bare_share", value);
}

// The setting "gen_tones" (Carriers) of the test signal generator of the stack "birdie". Carriers separated by ';', each 'offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s, from s, to s'; the values after the amplitude can be left out.
static inline int32_t sdrkit_birdie_gen_get_tones(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tones", value);
}
static inline int32_t sdrkit_birdie_gen_set_tones(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tones", value);
}

// The setting "gen_fm_hz" (Modulated carrier offset) of the test signal generator of the stack "birdie". Unit: Hz.
static inline int32_t sdrkit_birdie_gen_get_fm_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fm_hz", value);
}
static inline int32_t sdrkit_birdie_gen_set_fm_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fm_hz", value);
}

// The setting "gen_fm_amp" (Modulated carrier amplitude) of the test signal generator of the stack "birdie".
static inline int32_t sdrkit_birdie_gen_get_fm_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fm_amp", value);
}
static inline int32_t sdrkit_birdie_gen_set_fm_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fm_amp", value);
}

// The setting "gen_fm_dev_hz" (Modulated carrier deviation) of the test signal generator of the stack "birdie". Unit: Hz. The RMS deviation; the modulation is white noise through a one-pole low-pass of 300 Hz.
static inline int32_t sdrkit_birdie_gen_get_fm_dev_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fm_dev_hz", value);
}
static inline int32_t sdrkit_birdie_gen_set_fm_dev_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fm_dev_hz", value);
}

// Fills `rows` with the changed rows of the table "bandwidths" of the stack "obw" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_obw_bandwidths_rows(SDRKIT_HANDLE handle,
                                                sdrkit_obw_bandwidth_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.ObwBandwidthRow", "bandwidths", rows, sizeof(sdrkit_obw_bandwidth_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "obw.reading"
// (Bandwidth reading). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_obw_reading_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_obw_reading_event *event) {
  return sdrkit_typed_event(handle, n, "obw.reading", "meter.ObwReadingEvent", event, sizeof(sdrkit_obw_reading_event));
}

// Runs the command "clear" (Clear) of the stack "obw" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_obw_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "obw" on the session of `handle`. Starts the average again; the table stays.
static inline int32_t sdrkit_obw_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "obw" are the sdrkit_obw_gen_*() functions
// after the settings of its receiver.

// The setting "power_pct" (Occupied power share) of the stack "obw". Unit: %.
static inline int32_t sdrkit_obw_get_power_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "power_pct", value);
}
static inline int32_t sdrkit_obw_set_power_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "power_pct", value);
}

// The setting "x_db" (The x of the x dB bandwidth in the headline) of the stack "obw". Unit: dB.
static inline int32_t sdrkit_obw_get_x_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "x_db", value);
}
static inline int32_t sdrkit_obw_set_x_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "x_db", value);
}

// The setting "average_s" (Averaging time) of the stack "obw". Unit: s.
static inline int32_t sdrkit_obw_get_average_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "average_s", value);
}
static inline int32_t sdrkit_obw_set_average_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "average_s", value);
}

// The setting "adjacent_offset" (Adjacent band centre, in bandwidths) of the stack "obw".
static inline int32_t sdrkit_obw_get_adjacent_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "adjacent_offset", value);
}
static inline int32_t sdrkit_obw_set_adjacent_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "adjacent_offset", value);
}

// The setting "gen_symbol_rate" (Symbol rate) of the test signal generator of the stack "obw". Unit: Bd.
static inline int32_t sdrkit_obw_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_obw_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_rolloff" (Roll-off) of the test signal generator of the stack "obw".
static inline int32_t sdrkit_obw_gen_get_rolloff(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rolloff", value);
}
static inline int32_t sdrkit_obw_gen_set_rolloff(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value);
}

// The setting "gen_psk" (PSK points) of the test signal generator of the stack "obw".
static inline int32_t sdrkit_obw_gen_get_psk(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_psk", value);
}
static inline int32_t sdrkit_obw_gen_set_psk(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_psk", value);
}

// The setting "gen_neighbour" (Neighbour) of the test signal generator of the stack "obw".
static inline int32_t sdrkit_obw_gen_get_neighbour(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_neighbour", value);
}
static inline int32_t sdrkit_obw_gen_set_neighbour(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_neighbour", value);
}

// The setting "gen_neighbour_db" (Neighbour level) of the test signal generator of the stack "obw". Unit: dB.
static inline int32_t sdrkit_obw_gen_get_neighbour_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_neighbour_db", value);
}
static inline int32_t sdrkit_obw_gen_set_neighbour_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_neighbour_db", value);
}

// The setting "gen_neighbour_offset" (Neighbour offset) of the test signal generator of the stack "obw".
static inline int32_t sdrkit_obw_gen_get_neighbour_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_neighbour_offset", value);
}
static inline int32_t sdrkit_obw_gen_set_neighbour_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_neighbour_offset", value);
}

// Fills `rows` with the changed rows of the table "transmissions" of the stack "ptt_log" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ptt_log_transmissions_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_ptt_log_transmission_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.PttLogTransmissionRow", "transmissions", rows, sizeof(sdrkit_ptt_log_transmission_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ptt.transmission"
// (Transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ptt_log_ptt_transmission_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ptt_log_ptt_transmission_event *event) {
  return sdrkit_typed_event(handle, n, "ptt.transmission", "meter.PttTransmissionEvent", event, sizeof(sdrkit_ptt_log_ptt_transmission_event));
}

// Runs the command "clear" (Clear) of the stack "ptt_log" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ptt_log_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "ptt_log" on the session of `handle`. Learns the floor again and drops a transmission in progress; the table stays.
static inline int32_t sdrkit_ptt_log_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ptt_log" are the sdrkit_ptt_log_gen_*() functions
// after the settings of its receiver.

// The setting "channel_khz" (Channel width) of the stack "ptt_log". Unit: kHz. Twice the pass edge of the channel filter.
static inline int32_t sdrkit_ptt_log_get_channel_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_khz", value);
}
static inline int32_t sdrkit_ptt_log_set_channel_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_khz", value);
}

// The setting "on_db" (Keyed above (over the floor)) of the stack "ptt_log". Unit: dB.
static inline int32_t sdrkit_ptt_log_get_on_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "on_db", value);
}
static inline int32_t sdrkit_ptt_log_set_on_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "on_db", value);
}

// The setting "off_db" (Released below (over the floor)) of the stack "ptt_log". Unit: dB.
static inline int32_t sdrkit_ptt_log_get_off_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "off_db", value);
}
static inline int32_t sdrkit_ptt_log_set_off_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "off_db", value);
}

// The setting "hang_s" (Release after) of the stack "ptt_log". Unit: s.
static inline int32_t sdrkit_ptt_log_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_ptt_log_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "kerchunk_s" (Kerchunk shorter than) of the stack "ptt_log". Unit: s.
static inline int32_t sdrkit_ptt_log_get_kerchunk_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "kerchunk_s", value);
}
static inline int32_t sdrkit_ptt_log_set_kerchunk_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "kerchunk_s", value);
}

// The setting "stuck_s" (Stuck key after) of the stack "ptt_log". Unit: s.
static inline int32_t sdrkit_ptt_log_get_stuck_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "stuck_s", value);
}
static inline int32_t sdrkit_ptt_log_set_stuck_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "stuck_s", value);
}

// The setting "gen_keyed" (Transmissions) of the test signal generator of the stack "ptt_log". Transmissions separated by ';', each 'from s, to s'.
static inline int32_t sdrkit_ptt_log_gen_get_keyed(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_keyed", value);
}
static inline int32_t sdrkit_ptt_log_gen_set_keyed(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_keyed", value);
}

// The setting "gen_fades" (Fades) of the test signal generator of the stack "ptt_log". Fades separated by ';', each 'from s, to s': the carrier is off.
static inline int32_t sdrkit_ptt_log_gen_get_fades(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_fades", value);
}
static inline int32_t sdrkit_ptt_log_gen_set_fades(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_fades", value);
}

// The setting "gen_tone_hz" (Carrier offset) of the test signal generator of the stack "ptt_log". Unit: Hz.
static inline int32_t sdrkit_ptt_log_gen_get_tone_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", value);
}
static inline int32_t sdrkit_ptt_log_gen_set_tone_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value);
}

// Fills `rows` with the changed rows of the table "patterns" of the stack "noise_locator" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noise_locator_patterns_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_noise_locator_pattern_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.NoiseLocatorPatternRow", "patterns", rows, sizeof(sdrkit_noise_locator_pattern_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "noise_locator.pattern"
// (Pattern). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_noise_locator_pattern_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_noise_locator_pattern_event *event) {
  return sdrkit_typed_event(handle, n, "noise_locator.pattern", "meter.NoiseLocatorPatternEvent", event, sizeof(sdrkit_noise_locator_pattern_event));
}

// Runs the command "clear" (Clear) of the stack "noise_locator" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_noise_locator_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "noise_locator" on the session of `handle`. Learns the noise again and drops the impulses and the pattern; the table stays.
static inline int32_t sdrkit_noise_locator_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "noise_locator" are the sdrkit_noise_locator_gen_*() functions
// after the settings of its receiver.

// The setting "mains_hz" (Mains frequencies) of the stack "noise_locator". Unit: Hz. The mains frequencies tested, separated by commas; a rate near one or twice one names a power-line class.
static inline int32_t sdrkit_noise_locator_get_mains_hz(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "mains_hz", value);
}
static inline int32_t sdrkit_noise_locator_set_mains_hz(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "mains_hz", value);
}

// The setting "sigmas" (Impulse threshold over the noise) of the stack "noise_locator". Unit: sigma.
static inline int32_t sdrkit_noise_locator_get_sigmas(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "sigmas", value);
}
static inline int32_t sdrkit_noise_locator_set_sigmas(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "sigmas", value);
}

// The setting "window_s" (Impulse window) of the stack "noise_locator". Unit: s.
static inline int32_t sdrkit_noise_locator_get_window_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "window_s", value);
}
static inline int32_t sdrkit_noise_locator_set_window_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "window_s", value);
}

// The setting "analysis_khz" (Analysis band) of the stack "noise_locator". Unit: kHz. Twice the pass edge of the filter before the impulse detector.
static inline int32_t sdrkit_noise_locator_get_analysis_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "analysis_khz", value);
}
static inline int32_t sdrkit_noise_locator_set_analysis_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "analysis_khz", value);
}

// The setting "gen_rate_hz" (Impulse rate) of the test signal generator of the stack "noise_locator". Unit: Hz.
static inline int32_t sdrkit_noise_locator_gen_get_rate_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rate_hz", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_rate_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rate_hz", value);
}

// The setting "gen_jitter_us" (Timing jitter) of the test signal generator of the stack "noise_locator". Unit: us.
static inline int32_t sdrkit_noise_locator_gen_get_jitter_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_jitter_us", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_jitter_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_jitter_us", value);
}

// The setting "gen_width_us" (Impulse width) of the test signal generator of the stack "noise_locator". Unit: us.
static inline int32_t sdrkit_noise_locator_gen_get_width_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_width_us", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_width_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_width_us", value);
}

// The setting "gen_peak_db" (Impulse peak) of the test signal generator of the stack "noise_locator". Unit: dB.
static inline int32_t sdrkit_noise_locator_gen_get_peak_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_peak_db", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_peak_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_peak_db", value);
}

// The setting "gen_spread_db" (Peak spread) of the test signal generator of the stack "noise_locator". Unit: dB.
static inline int32_t sdrkit_noise_locator_gen_get_spread_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spread_db", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_spread_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spread_db", value);
}

// The setting "gen_drop" (Missing share) of the test signal generator of the stack "noise_locator".
static inline int32_t sdrkit_noise_locator_gen_get_drop(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_drop", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_drop(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_drop", value);
}

// The setting "gen_random_rate" (Random impulse rate) of the test signal generator of the stack "noise_locator". Unit: Hz.
static inline int32_t sdrkit_noise_locator_gen_get_random_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_random_rate", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_random_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_random_rate", value);
}

// The setting "gen_random_db" (Random impulse peak) of the test signal generator of the stack "noise_locator". Unit: dB.
static inline int32_t sdrkit_noise_locator_gen_get_random_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_random_db", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_random_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_random_db", value);
}

// The setting "gen_rate_drift_hz_s" (Rate drift) of the test signal generator of the stack "noise_locator". Unit: Hz/s.
static inline int32_t sdrkit_noise_locator_gen_get_rate_drift_hz_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rate_drift_hz_s", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_rate_drift_hz_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rate_drift_hz_s", value);
}

// The setting "gen_noise_sigma" (Reference noise per quadrature) of the test signal generator of the stack "noise_locator".
static inline int32_t sdrkit_noise_locator_gen_get_noise_sigma(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_noise_sigma", value);
}
static inline int32_t sdrkit_noise_locator_gen_set_noise_sigma(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_noise_sigma", value);
}

// Fills `rows` with the changed rows of the table "allan" of the stack "freq_accuracy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_freq_accuracy_allan_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_freq_accuracy_allan_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.FreqAccuracyAllanRow", "allan", rows, sizeof(sdrkit_freq_accuracy_allan_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "freq_accuracy" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_freq_accuracy_events_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_freq_accuracy_event_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.FreqAccuracyEventRow", "events", rows, sizeof(sdrkit_freq_accuracy_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "freq_accuracy.event"
// (Event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_freq_accuracy_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_freq_accuracy_event *event) {
  return sdrkit_typed_event(handle, n, "freq_accuracy.event", "meter.FreqAccuracyEvent", event, sizeof(sdrkit_freq_accuracy_event));
}

// Runs the command "clear" (Clear) of the stack "freq_accuracy" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_freq_accuracy_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "freq_accuracy" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_freq_accuracy_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "freq_accuracy" are the sdrkit_freq_accuracy_gen_*() functions
// after the settings of its receiver.

// The setting "search_hz" (Search half width) of the stack "freq_accuracy". Unit: Hz.
static inline int32_t sdrkit_freq_accuracy_get_search_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_hz", value);
}
static inline int32_t sdrkit_freq_accuracy_set_search_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_hz", value);
}

// The setting "present_db" (Carrier over the noise per bin) of the stack "freq_accuracy". Unit: dB.
static inline int32_t sdrkit_freq_accuracy_get_present_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "present_db", value);
}
static inline int32_t sdrkit_freq_accuracy_set_present_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "present_db", value);
}

// The setting "step_sigmas" (Step threshold in scatter widths) of the stack "freq_accuracy".
static inline int32_t sdrkit_freq_accuracy_get_step_sigmas(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "step_sigmas", value);
}
static inline int32_t sdrkit_freq_accuracy_set_step_sigmas(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "step_sigmas", value);
}

// The setting "step_min_hz" (Smallest step) of the stack "freq_accuracy". Unit: Hz.
static inline int32_t sdrkit_freq_accuracy_get_step_min_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "step_min_hz", value);
}
static inline int32_t sdrkit_freq_accuracy_set_step_min_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "step_min_hz", value);
}

// The setting "row_every_s" (Frequency row every) of the stack "freq_accuracy". Unit: s.
static inline int32_t sdrkit_freq_accuracy_get_row_every_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "row_every_s", value);
}
static inline int32_t sdrkit_freq_accuracy_set_row_every_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "row_every_s", value);
}

// The setting "gen_steps" (Steps) of the test signal generator of the stack "freq_accuracy". Steps separated by ';', each 'time s, frequency Hz': from that time the carrier is that far from its start.
static inline int32_t sdrkit_freq_accuracy_gen_get_steps(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_steps", value);
}
static inline int32_t sdrkit_freq_accuracy_gen_set_steps(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_steps", value);
}

// The setting "gen_gaps" (Gaps) of the test signal generator of the stack "freq_accuracy". Stretches separated by ';', each 'from s, to s': the carrier is off.
static inline int32_t sdrkit_freq_accuracy_gen_get_gaps(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_gaps", value);
}
static inline int32_t sdrkit_freq_accuracy_gen_set_gaps(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_gaps", value);
}

// The setting "gen_fm_noise_hz" (Frequency noise) of the test signal generator of the stack "freq_accuracy". Unit: Hz. Standard deviation of a frequency held constant over each hold; 0 for none.
static inline int32_t sdrkit_freq_accuracy_gen_get_fm_noise_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fm_noise_hz", value);
}
static inline int32_t sdrkit_freq_accuracy_gen_set_fm_noise_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fm_noise_hz", value);
}

// The setting "gen_fm_hold_s" (Frequency noise hold) of the test signal generator of the stack "freq_accuracy". Unit: s.
static inline int32_t sdrkit_freq_accuracy_gen_get_fm_hold_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fm_hold_s", value);
}
static inline int32_t sdrkit_freq_accuracy_gen_set_fm_hold_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fm_hold_s", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "sinad" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sinad_events_rows(SDRKIT_HANDLE handle,
                                              sdrkit_sinad_event_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.SinadEventRow", "events", rows, sizeof(sdrkit_sinad_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sinad.crossing"
// (Crossing). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sinad_crossing_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sinad_crossing_event *event) {
  return sdrkit_typed_event(handle, n, "sinad.crossing", "meter.SinadCrossingEvent", event, sizeof(sdrkit_sinad_crossing_event));
}

// Runs the command "clear" (Clear) of the stack "sinad" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_sinad_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "sinad" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_sinad_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sinad" are the sdrkit_sinad_gen_*() functions
// after the settings of its receiver.

// The setting "band_lo_hz" (Audio band, low edge) of the stack "sinad". Unit: Hz.
static inline int32_t sdrkit_sinad_get_band_lo_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "band_lo_hz", value);
}
static inline int32_t sdrkit_sinad_set_band_lo_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "band_lo_hz", value);
}

// The setting "band_hi_hz" (Audio band, high edge) of the stack "sinad". Unit: Hz.
static inline int32_t sdrkit_sinad_get_band_hi_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "band_hi_hz", value);
}
static inline int32_t sdrkit_sinad_set_band_hi_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "band_hi_hz", value);
}

// The setting "sinad_mark_db" (Sensitivity mark) of the stack "sinad". Unit: dB. The SINAD of the sensitivity figure: 12 dB in TIA-603.
static inline int32_t sdrkit_sinad_get_sinad_mark_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "sinad_mark_db", value);
}
static inline int32_t sdrkit_sinad_set_sinad_mark_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "sinad_mark_db", value);
}

// The setting "tone_min_db" (Tone over the band median) of the stack "sinad". Unit: dB.
static inline int32_t sdrkit_sinad_get_tone_min_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tone_min_db", value);
}
static inline int32_t sdrkit_sinad_set_tone_min_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tone_min_db", value);
}

// The setting "gen_tone_hz" (Test tone) of the test signal generator of the stack "sinad". Unit: Hz.
static inline int32_t sdrkit_sinad_gen_get_tone_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", value);
}
static inline int32_t sdrkit_sinad_gen_set_tone_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value);
}

// The setting "gen_deviation_hz" (Deviation) of the test signal generator of the stack "sinad". Unit: Hz.
static inline int32_t sdrkit_sinad_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_sinad_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// The setting "gen_h2" (Second harmonic) of the test signal generator of the stack "sinad".
static inline int32_t sdrkit_sinad_gen_get_h2(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_h2", value);
}
static inline int32_t sdrkit_sinad_gen_set_h2(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_h2", value);
}

// The setting "gen_h3" (Third harmonic) of the test signal generator of the stack "sinad".
static inline int32_t sdrkit_sinad_gen_get_h3(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_h3", value);
}
static inline int32_t sdrkit_sinad_gen_set_h3(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_h3", value);
}

// The setting "gen_deviation_after" (Later deviations) of the test signal generator of the stack "sinad". Changes separated by ';', each 'time s, deviation Hz'.
static inline int32_t sdrkit_sinad_gen_get_deviation_after(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_deviation_after", value);
}
static inline int32_t sdrkit_sinad_gen_set_deviation_after(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_deviation_after", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "nfm_meter" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nfm_meter_events_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_nfm_meter_event_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.NfmMeterEventRow", "events", rows, sizeof(sdrkit_nfm_meter_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "nfm_meter" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_nfm_meter_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "nfm_meter" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_nfm_meter_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "nfm_meter" are the sdrkit_nfm_meter_gen_*() functions
// after the settings of its receiver.

// The setting "limit_narrow_hz" (Narrow channel deviation limit) of the stack "nfm_meter". Unit: Hz. A regional limit for 12.5 kHz channels.
static inline int32_t sdrkit_nfm_meter_get_limit_narrow_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "limit_narrow_hz", value);
}
static inline int32_t sdrkit_nfm_meter_set_limit_narrow_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "limit_narrow_hz", value);
}

// The setting "limit_wide_hz" (Wide channel deviation limit) of the stack "nfm_meter". Unit: Hz. A regional limit for 25 kHz channels.
static inline int32_t sdrkit_nfm_meter_get_limit_wide_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "limit_wide_hz", value);
}
static inline int32_t sdrkit_nfm_meter_set_limit_wide_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "limit_wide_hz", value);
}

// The setting "peak_percentile" (Voice peak percentile) of the stack "nfm_meter". Unit: %.
static inline int32_t sdrkit_nfm_meter_get_peak_percentile(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "peak_percentile", value);
}
static inline int32_t sdrkit_nfm_meter_set_peak_percentile(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "peak_percentile", value);
}

// The setting "ctcss_min_db" (CTCSS over its neighbourhood) of the stack "nfm_meter". Unit: dB.
static inline int32_t sdrkit_nfm_meter_get_ctcss_min_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ctcss_min_db", value);
}
static inline int32_t sdrkit_nfm_meter_set_ctcss_min_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ctcss_min_db", value);
}

// The setting "gen_voice_hz" (Voice scale) of the test signal generator of the stack "nfm_meter". Unit: Hz.
static inline int32_t sdrkit_nfm_meter_gen_get_voice_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_voice_hz", value);
}
static inline int32_t sdrkit_nfm_meter_gen_set_voice_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_voice_hz", value);
}

// The setting "gen_ctcss_hz" (CTCSS tone) of the test signal generator of the stack "nfm_meter". Unit: Hz.
static inline int32_t sdrkit_nfm_meter_gen_get_ctcss_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ctcss_hz", value);
}
static inline int32_t sdrkit_nfm_meter_gen_set_ctcss_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ctcss_hz", value);
}

// The setting "gen_ctcss_dev_hz" (CTCSS deviation) of the test signal generator of the stack "nfm_meter". Unit: Hz.
static inline int32_t sdrkit_nfm_meter_gen_get_ctcss_dev_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ctcss_dev_hz", value);
}
static inline int32_t sdrkit_nfm_meter_gen_set_ctcss_dev_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ctcss_dev_hz", value);
}

// The setting "gen_ctcss_off_s" (CTCSS off at) of the test signal generator of the stack "nfm_meter". Unit: s.
static inline int32_t sdrkit_nfm_meter_gen_get_ctcss_off_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ctcss_off_s", value);
}
static inline int32_t sdrkit_nfm_meter_gen_set_ctcss_off_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ctcss_off_s", value);
}

// Fills `rows` with the changed rows of the table "transmissions" of the stack "tx_stability" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tx_stability_transmissions_rows(SDRKIT_HANDLE handle,
                                                            sdrkit_tx_stability_transmission_row *rows,
                                                            uint32_t capacity,
                                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.TxStabilityTransmissionRow", "transmissions", rows, sizeof(sdrkit_tx_stability_transmission_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tx_stability.transmission"
// (Transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tx_stability_transmission_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tx_stability_transmission_event *event) {
  return sdrkit_typed_event(handle, n, "tx_stability.transmission", "meter.TxStabilityTransmissionEvent", event, sizeof(sdrkit_tx_stability_transmission_event));
}

// Runs the command "clear" (Clear) of the stack "tx_stability" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_tx_stability_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "tx_stability" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_tx_stability_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "tx_stability" are the sdrkit_tx_stability_gen_*() functions
// after the settings of its receiver.

// The setting "on_db" (Key-up over the floor) of the stack "tx_stability". Unit: dB.
static inline int32_t sdrkit_tx_stability_get_on_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "on_db", value);
}
static inline int32_t sdrkit_tx_stability_set_on_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "on_db", value);
}

// The setting "off_db" (Key-down under) of the stack "tx_stability". Unit: dB.
static inline int32_t sdrkit_tx_stability_get_off_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "off_db", value);
}
static inline int32_t sdrkit_tx_stability_set_off_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "off_db", value);
}

// The setting "hang_s" (Hang before key-down) of the stack "tx_stability". Unit: s.
static inline int32_t sdrkit_tx_stability_get_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang_s", value);
}
static inline int32_t sdrkit_tx_stability_set_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang_s", value);
}

// The setting "min_tx_s" (Shortest transmission analysed) of the stack "tx_stability". Unit: s.
static inline int32_t sdrkit_tx_stability_get_min_tx_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_tx_s", value);
}
static inline int32_t sdrkit_tx_stability_set_min_tx_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_tx_s", value);
}

// The setting "reading_s" (Least reading length) of the stack "tx_stability". Unit: s.
static inline int32_t sdrkit_tx_stability_get_reading_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "reading_s", value);
}
static inline int32_t sdrkit_tx_stability_set_reading_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "reading_s", value);
}

// The setting "gen_keyed" (Transmissions) of the test signal generator of the stack "tx_stability". Transmissions separated by ';', each 'from s, to s'.
static inline int32_t sdrkit_tx_stability_gen_get_keyed(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_keyed", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_keyed(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_keyed", value);
}

// The setting "gen_final_hz" (Final frequency) of the test signal generator of the stack "tx_stability". Unit: Hz.
static inline int32_t sdrkit_tx_stability_gen_get_final_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_final_hz", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_final_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_final_hz", value);
}

// The setting "gen_start_offset_hz" (Start offset) of the test signal generator of the stack "tx_stability". Unit: Hz.
static inline int32_t sdrkit_tx_stability_gen_get_start_offset_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_start_offset_hz", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_start_offset_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_start_offset_hz", value);
}

// The setting "gen_tau_s" (Settling time constant) of the test signal generator of the stack "tx_stability". Unit: s.
static inline int32_t sdrkit_tx_stability_gen_get_tau_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tau_s", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_tau_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tau_s", value);
}

// The setting "gen_tx_drift_hz_s" (Drift in a transmission) of the test signal generator of the stack "tx_stability". Unit: Hz/s.
static inline int32_t sdrkit_tx_stability_gen_get_tx_drift_hz_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tx_drift_hz_s", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_tx_drift_hz_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tx_drift_hz_s", value);
}

// The setting "gen_mod_hz" (1 kHz tone deviation) of the test signal generator of the stack "tx_stability". Unit: Hz.
static inline int32_t sdrkit_tx_stability_gen_get_mod_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_mod_hz", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_mod_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_mod_hz", value);
}

// The setting "gen_ramp_s" (Key ramp) of the test signal generator of the stack "tx_stability". Unit: s.
static inline int32_t sdrkit_tx_stability_gen_get_ramp_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ramp_s", value);
}
static inline int32_t sdrkit_tx_stability_gen_set_ramp_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ramp_s", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "ssb_test" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ssb_test_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_ssb_test_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.SsbTestEventRow", "events", rows, sizeof(sdrkit_ssb_test_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ssb_test" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ssb_test_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "ssb_test" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_ssb_test_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ssb_test" are the sdrkit_ssb_test_gen_*() functions
// after the settings of its receiver.

// The setting "tone_min_hz" (Tone search, low edge) of the stack "ssb_test". Unit: Hz.
static inline int32_t sdrkit_ssb_test_get_tone_min_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tone_min_hz", value);
}
static inline int32_t sdrkit_ssb_test_set_tone_min_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tone_min_hz", value);
}

// The setting "tone_max_hz" (Tone search, high edge) of the stack "ssb_test". Unit: Hz.
static inline int32_t sdrkit_ssb_test_get_tone_max_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tone_max_hz", value);
}
static inline int32_t sdrkit_ssb_test_set_tone_max_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tone_max_hz", value);
}

// The setting "two_tone_db" (Second tone within) of the stack "ssb_test". Unit: dB.
static inline int32_t sdrkit_ssb_test_get_two_tone_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "two_tone_db", value);
}
static inline int32_t sdrkit_ssb_test_set_two_tone_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "two_tone_db", value);
}

// The setting "pep_offset_db" (PEP over one tone) of the stack "ssb_test". Unit: dB. 6.02 dB for two equal tones.
static inline int32_t sdrkit_ssb_test_get_pep_offset_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pep_offset_db", value);
}
static inline int32_t sdrkit_ssb_test_set_pep_offset_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pep_offset_db", value);
}

// The setting "gen_tones" (Tones) of the test signal generator of the stack "ssb_test". Tones separated by ';', each 'audio Hz, dBc'; a negative audio frequency is on the opposite sideband.
static inline int32_t sdrkit_ssb_test_gen_get_tones(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tones", value);
}
static inline int32_t sdrkit_ssb_test_gen_set_tones(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tones", value);
}

// The setting "gen_sideband" (Sideband) of the test signal generator of the stack "ssb_test". A value of `sdrkit_ssb_test_gen_sideband`.
static inline int32_t sdrkit_ssb_test_gen_get_sideband(SDRKIT_HANDLE handle, sdrkit_ssb_test_gen_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ssb_test_gen_sideband)index;
  return code;
}
static inline int32_t sdrkit_ssb_test_gen_set_sideband(SDRKIT_HANDLE handle, sdrkit_ssb_test_gen_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_sideband", (int32_t)value);
}

// The setting "gen_carrier_dbc" (Carrier leak) of the test signal generator of the stack "ssb_test". Unit: dBc.
static inline int32_t sdrkit_ssb_test_gen_get_carrier_dbc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_carrier_dbc", value);
}
static inline int32_t sdrkit_ssb_test_gen_set_carrier_dbc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_carrier_dbc", value);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "afsk_tuner" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_afsk_tuner_bursts_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_afsk_tuner_burst_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.AfskTunerBurstRow", "bursts", rows, sizeof(sdrkit_afsk_tuner_burst_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "afsk_tuner.burst"
// (Burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_afsk_tuner_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_afsk_tuner_burst_event *event) {
  return sdrkit_typed_event(handle, n, "afsk_tuner.burst", "meter.AfskTunerBurstEvent", event, sizeof(sdrkit_afsk_tuner_burst_event));
}

// Runs the command "clear" (Clear) of the stack "afsk_tuner" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_afsk_tuner_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "afsk_tuner" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_afsk_tuner_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "afsk_tuner" are the sdrkit_afsk_tuner_gen_*() functions
// after the settings of its receiver.

// The setting "mark_hz" (Mark tone) of the stack "afsk_tuner". Unit: Hz. 1200 Hz in Bell 202.
static inline int32_t sdrkit_afsk_tuner_get_mark_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "mark_hz", value);
}
static inline int32_t sdrkit_afsk_tuner_set_mark_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "mark_hz", value);
}

// The setting "space_hz" (Space tone) of the stack "afsk_tuner". Unit: Hz. 2200 Hz in Bell 202.
static inline int32_t sdrkit_afsk_tuner_get_space_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "space_hz", value);
}
static inline int32_t sdrkit_afsk_tuner_set_space_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "space_hz", value);
}

// The setting "on_ratio" (Burst on over the floor) of the stack "afsk_tuner".
static inline int32_t sdrkit_afsk_tuner_get_on_ratio(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "on_ratio", value);
}
static inline int32_t sdrkit_afsk_tuner_set_on_ratio(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "on_ratio", value);
}

// The setting "off_ratio" (Burst off over the floor) of the stack "afsk_tuner".
static inline int32_t sdrkit_afsk_tuner_get_off_ratio(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "off_ratio", value);
}
static inline int32_t sdrkit_afsk_tuner_set_off_ratio(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "off_ratio", value);
}

// The setting "min_burst_s" (Shortest burst analysed) of the stack "afsk_tuner". Unit: s.
static inline int32_t sdrkit_afsk_tuner_get_min_burst_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_burst_s", value);
}
static inline int32_t sdrkit_afsk_tuner_set_min_burst_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_burst_s", value);
}

// The setting "gen_deviation_hz" (Deviation) of the test signal generator of the stack "afsk_tuner". Unit: Hz.
static inline int32_t sdrkit_afsk_tuner_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_afsk_tuner_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// The setting "gen_twist_db" (Twist, space over mark) of the test signal generator of the stack "afsk_tuner". Unit: dB.
static inline int32_t sdrkit_afsk_tuner_gen_get_twist_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_twist_db", value);
}
static inline int32_t sdrkit_afsk_tuner_gen_set_twist_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_twist_db", value);
}

// The setting "gen_baud" (Bit rate) of the test signal generator of the stack "afsk_tuner". Unit: Bd.
static inline int32_t sdrkit_afsk_tuner_gen_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud", value);
}
static inline int32_t sdrkit_afsk_tuner_gen_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud", value);
}

// The setting "gen_period_s" (Burst period) of the test signal generator of the stack "afsk_tuner". Unit: s.
static inline int32_t sdrkit_afsk_tuner_gen_get_period_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_period_s", value);
}
static inline int32_t sdrkit_afsk_tuner_gen_set_period_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_period_s", value);
}

// The setting "gen_burst_s" (Burst length) of the test signal generator of the stack "afsk_tuner". Unit: s.
static inline int32_t sdrkit_afsk_tuner_gen_get_burst_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_burst_s", value);
}
static inline int32_t sdrkit_afsk_tuner_gen_set_burst_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_burst_s", value);
}

// The setting "gen_first_s" (First burst at) of the test signal generator of the stack "afsk_tuner". Unit: s.
static inline int32_t sdrkit_afsk_tuner_gen_get_first_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_first_s", value);
}
static inline int32_t sdrkit_afsk_tuner_gen_set_first_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_first_s", value);
}

// Fills `rows` with the changed rows of the table "spurs" of the stack "phase_noise" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_dbc`, highest first.
static inline int32_t sdrkit_phase_noise_spurs_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_phase_noise_spur_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.PhaseNoiseSpurRow", "spurs", rows, sizeof(sdrkit_phase_noise_spur_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "phase_noise" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_phase_noise_events_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_phase_noise_event_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.PhaseNoiseEventRow", "events", rows, sizeof(sdrkit_phase_noise_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "phase_noise" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_phase_noise_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "phase_noise" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_phase_noise_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "phase_noise" are the sdrkit_phase_noise_gen_*() functions
// after the settings of its receiver.

// The setting "average_blocks" (Blocks averaged) of the stack "phase_noise".
static inline int32_t sdrkit_phase_noise_get_average_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "average_blocks", value);
}
static inline int32_t sdrkit_phase_noise_set_average_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "average_blocks", value);
}

// The setting "spur_ratio_db" (Spur over its neighbourhood) of the stack "phase_noise". Unit: dB.
static inline int32_t sdrkit_phase_noise_get_spur_ratio_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "spur_ratio_db", value);
}
static inline int32_t sdrkit_phase_noise_set_spur_ratio_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "spur_ratio_db", value);
}

// The setting "jitter_lo_hz" (Jitter band, low edge) of the stack "phase_noise". Unit: Hz.
static inline int32_t sdrkit_phase_noise_get_jitter_lo_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jitter_lo_hz", value);
}
static inline int32_t sdrkit_phase_noise_set_jitter_lo_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jitter_lo_hz", value);
}

// The setting "jitter_hi_hz" (Jitter band, high edge) of the stack "phase_noise". Unit: Hz.
static inline int32_t sdrkit_phase_noise_get_jitter_hi_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "jitter_hi_hz", value);
}
static inline int32_t sdrkit_phase_noise_set_jitter_hi_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "jitter_hi_hz", value);
}

// The setting "gen_linewidth_hz" (Linewidth) of the test signal generator of the stack "phase_noise". Unit: Hz.
static inline int32_t sdrkit_phase_noise_gen_get_linewidth_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_linewidth_hz", value);
}
static inline int32_t sdrkit_phase_noise_gen_set_linewidth_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_linewidth_hz", value);
}

// The setting "gen_floor_dbc" (White phase floor) of the test signal generator of the stack "phase_noise". Unit: dBc/Hz.
static inline int32_t sdrkit_phase_noise_gen_get_floor_dbc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_floor_dbc", value);
}
static inline int32_t sdrkit_phase_noise_gen_set_floor_dbc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_floor_dbc", value);
}

// The setting "gen_spur_hz" (Spur offset) of the test signal generator of the stack "phase_noise". Unit: Hz.
static inline int32_t sdrkit_phase_noise_gen_get_spur_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spur_hz", value);
}
static inline int32_t sdrkit_phase_noise_gen_set_spur_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spur_hz", value);
}

// The setting "gen_spur_dbc" (Spur level) of the test signal generator of the stack "phase_noise". Unit: dBc.
static inline int32_t sdrkit_phase_noise_gen_get_spur_dbc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spur_dbc", value);
}
static inline int32_t sdrkit_phase_noise_gen_set_spur_dbc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spur_dbc", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "cw_keying" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cw_keying_events_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_cw_keying_event_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.CwKeyingEventRow", "events", rows, sizeof(sdrkit_cw_keying_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "cw_keying" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cw_keying_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "cw_keying" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_cw_keying_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cw_keying" are the sdrkit_cw_keying_gen_*() functions
// after the settings of its receiver.

// The setting "search_hz" (Carrier search half width) of the stack "cw_keying". Unit: Hz.
static inline int32_t sdrkit_cw_keying_get_search_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_hz", value);
}
static inline int32_t sdrkit_cw_keying_set_search_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_hz", value);
}

// The setting "on_share" (Key-down above (of the peak)) of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_get_on_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "on_share", value);
}
static inline int32_t sdrkit_cw_keying_set_on_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "on_share", value);
}

// The setting "off_share" (Key-up under (of the peak)) of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_get_off_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "off_share", value);
}
static inline int32_t sdrkit_cw_keying_set_off_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "off_share", value);
}

// The setting "elements" (Elements kept) of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_get_elements(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "elements", value);
}
static inline int32_t sdrkit_cw_keying_set_elements(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "elements", value);
}

// The setting "min_chirp_mark_s" (Shortest mark for the chirp) of the stack "cw_keying". Unit: s.
static inline int32_t sdrkit_cw_keying_get_min_chirp_mark_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_chirp_mark_s", value);
}
static inline int32_t sdrkit_cw_keying_set_min_chirp_mark_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_chirp_mark_s", value);
}

// The setting "gen_text" (Text) of the test signal generator of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// The setting "gen_wpm" (Speed) of the test signal generator of the stack "cw_keying". Unit: wpm.
static inline int32_t sdrkit_cw_keying_gen_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm", value);
}

// The setting "gen_dah_ratio" (Dah ratio) of the test signal generator of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_gen_get_dah_ratio(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_dah_ratio", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_dah_ratio(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_dah_ratio", value);
}

// The setting "gen_weight" (Weight) of the test signal generator of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_gen_get_weight(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_weight", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_weight(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_weight", value);
}

// The setting "gen_char_gap" (Character gap) of the test signal generator of the stack "cw_keying". Unit: units.
static inline int32_t sdrkit_cw_keying_gen_get_char_gap(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_char_gap", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_char_gap(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_char_gap", value);
}

// The setting "gen_word_gap" (Word gap) of the test signal generator of the stack "cw_keying". Unit: units.
static inline int32_t sdrkit_cw_keying_gen_get_word_gap(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_word_gap", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_word_gap(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_word_gap", value);
}

// The setting "gen_jitter" (Mark jitter) of the test signal generator of the stack "cw_keying".
static inline int32_t sdrkit_cw_keying_gen_get_jitter(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_jitter", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_jitter(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_jitter", value);
}

// The setting "gen_ramp_s" (Edge length) of the test signal generator of the stack "cw_keying". Unit: s.
static inline int32_t sdrkit_cw_keying_gen_get_ramp_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ramp_s", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_ramp_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ramp_s", value);
}

// The setting "gen_chirp_hz" (Chirp) of the test signal generator of the stack "cw_keying". Unit: Hz.
static inline int32_t sdrkit_cw_keying_gen_get_chirp_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_chirp_hz", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_chirp_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_chirp_hz", value);
}

// The setting "gen_chirp_tau_s" (Chirp time constant) of the test signal generator of the stack "cw_keying". Unit: s.
static inline int32_t sdrkit_cw_keying_gen_get_chirp_tau_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_chirp_tau_s", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_chirp_tau_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_chirp_tau_s", value);
}

// The setting "gen_fade_db" (Level swing at 0.2 Hz) of the test signal generator of the stack "cw_keying". Unit: dB.
static inline int32_t sdrkit_cw_keying_gen_get_fade_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fade_db", value);
}
static inline int32_t sdrkit_cw_keying_gen_set_fade_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fade_db", value);
}

// Fills `rows` with the changed rows of the table "level_freqs" of the stack "fsk_analyser" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsk_analyser_level_freqs_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_fsk_analyser_level_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.FskAnalyserLevelRow", "level_freqs", rows, sizeof(sdrkit_fsk_analyser_level_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "fsk_analyser" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fsk_analyser_events_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_fsk_analyser_event_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.FskAnalyserEventRow", "events", rows, sizeof(sdrkit_fsk_analyser_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "fsk_analyser" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fsk_analyser_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "fsk_analyser" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_fsk_analyser_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fsk_analyser" are the sdrkit_fsk_analyser_gen_*() functions
// after the settings of its receiver.

// The setting "block_s" (Block length) of the stack "fsk_analyser". Unit: s.
static inline int32_t sdrkit_fsk_analyser_get_block_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "block_s", value);
}
static inline int32_t sdrkit_fsk_analyser_set_block_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "block_s", value);
}

// The setting "min_baud" (Lowest symbol rate) of the stack "fsk_analyser". Unit: Bd.
static inline int32_t sdrkit_fsk_analyser_get_min_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_baud", value);
}
static inline int32_t sdrkit_fsk_analyser_set_min_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_baud", value);
}

// The setting "min_eye_db" (Lowest eye SNR shown) of the stack "fsk_analyser". Unit: dB.
static inline int32_t sdrkit_fsk_analyser_get_min_eye_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_eye_db", value);
}
static inline int32_t sdrkit_fsk_analyser_set_min_eye_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_eye_db", value);
}

// The setting "gen_levels" (Levels) of the test signal generator of the stack "fsk_analyser".
static inline int32_t sdrkit_fsk_analyser_gen_get_levels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_levels", value);
}
static inline int32_t sdrkit_fsk_analyser_gen_set_levels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_levels", value);
}

// The setting "gen_baud" (Symbol rate) of the test signal generator of the stack "fsk_analyser". Unit: Bd.
static inline int32_t sdrkit_fsk_analyser_gen_get_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_baud", value);
}
static inline int32_t sdrkit_fsk_analyser_gen_set_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_baud", value);
}

// The setting "gen_deviation_hz" (Deviation) of the test signal generator of the stack "fsk_analyser". Unit: Hz.
static inline int32_t sdrkit_fsk_analyser_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_fsk_analyser_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// The setting "gen_bt" (Gaussian BT) of the test signal generator of the stack "fsk_analyser".
static inline int32_t sdrkit_fsk_analyser_gen_get_bt(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bt", value);
}
static inline int32_t sdrkit_fsk_analyser_gen_set_bt(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bt", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "mer" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_mer_events_rows(SDRKIT_HANDLE handle,
                                            sdrkit_mer_event_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.MerEventRow", "events", rows, sizeof(sdrkit_mer_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "mer" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_mer_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "mer" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_mer_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "mer" are the sdrkit_mer_gen_*() functions
// after the settings of its receiver.

// The setting "block_s" (Block length) of the stack "mer". Unit: s.
static inline int32_t sdrkit_mer_get_block_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "block_s", value);
}
static inline int32_t sdrkit_mer_set_block_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "block_s", value);
}

// The setting "min_baud" (Lowest symbol rate) of the stack "mer". Unit: Bd.
static inline int32_t sdrkit_mer_get_min_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_baud", value);
}
static inline int32_t sdrkit_mer_set_min_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_baud", value);
}

// The setting "min_mer_db" (Lowest MER shown) of the stack "mer". Unit: dB.
static inline int32_t sdrkit_mer_get_min_mer_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_mer_db", value);
}
static inline int32_t sdrkit_mer_set_min_mer_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_mer_db", value);
}

// The setting "rolloffs" (Roll-offs tried) of the stack "mer". Roll-offs separated by ',', each 0.1 to 1.0.
static inline int32_t sdrkit_mer_get_rolloffs(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "rolloffs", value);
}
static inline int32_t sdrkit_mer_set_rolloffs(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "rolloffs", value);
}

// The setting "gen_psk" (Phases) of the test signal generator of the stack "mer".
static inline int32_t sdrkit_mer_gen_get_psk(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_psk", value);
}
static inline int32_t sdrkit_mer_gen_set_psk(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_psk", value);
}

// The setting "gen_symbol_rate" (Symbol rate) of the test signal generator of the stack "mer". Unit: Bd.
static inline int32_t sdrkit_mer_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_mer_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_rolloff" (Roll-off) of the test signal generator of the stack "mer".
static inline int32_t sdrkit_mer_gen_get_rolloff(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rolloff", value);
}
static inline int32_t sdrkit_mer_gen_set_rolloff(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value);
}

// The setting "gen_rotated" (Points turned by pi / M) of the test signal generator of the stack "mer".
static inline int32_t sdrkit_mer_gen_get_rotated(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_rotated", value);
}
static inline int32_t sdrkit_mer_gen_set_rotated(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_rotated", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "papr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_papr_events_rows(SDRKIT_HANDLE handle,
                                             sdrkit_papr_event_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.PaprEventRow", "events", rows, sizeof(sdrkit_papr_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "papr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_papr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "papr" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_papr_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "papr" are the sdrkit_papr_gen_*() functions
// after the settings of its receiver.

// The setting "keep_samples" (Samples in the statistics) of the stack "papr".
static inline int32_t sdrkit_papr_get_keep_samples(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "keep_samples", value);
}
static inline int32_t sdrkit_papr_set_keep_samples(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "keep_samples", value);
}

// The setting "every_blocks" (Evaluate every (blocks of 16384)) of the stack "papr".
static inline int32_t sdrkit_papr_get_every_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "every_blocks", value);
}
static inline int32_t sdrkit_papr_set_every_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "every_blocks", value);
}

// The setting "gen_kind" (Signal) of the test signal generator of the stack "papr". A value of `sdrkit_papr_gen_kind`.
static inline int32_t sdrkit_papr_gen_get_kind(SDRKIT_HANDLE handle, sdrkit_papr_gen_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_papr_gen_kind)index;
  return code;
}
static inline int32_t sdrkit_papr_gen_set_kind(SDRKIT_HANDLE handle, sdrkit_papr_gen_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_kind", (int32_t)value);
}

// The setting "gen_power" (Power) of the test signal generator of the stack "papr".
static inline int32_t sdrkit_papr_gen_get_power(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_power", value);
}
static inline int32_t sdrkit_papr_gen_set_power(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_power", value);
}

// The setting "gen_clip" (Magnitude limit) of the test signal generator of the stack "papr".
static inline int32_t sdrkit_papr_gen_get_clip(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_clip", value);
}
static inline int32_t sdrkit_papr_gen_set_clip(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_clip", value);
}

// The setting "gen_impulse_every" (Impulse every) of the test signal generator of the stack "papr". Unit: samples.
static inline int32_t sdrkit_papr_gen_get_impulse_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_impulse_every", value);
}
static inline int32_t sdrkit_papr_gen_set_impulse_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_impulse_every", value);
}

// The setting "gen_impulse_amp" (Impulse size) of the test signal generator of the stack "papr".
static inline int32_t sdrkit_papr_gen_get_impulse_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_impulse_amp", value);
}
static inline int32_t sdrkit_papr_gen_set_impulse_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_impulse_amp", value);
}

// The setting "gen_symbol_rate" (Symbol rate) of the test signal generator of the stack "papr". Unit: Bd.
static inline int32_t sdrkit_papr_gen_get_symbol_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", value);
}
static inline int32_t sdrkit_papr_gen_set_symbol_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value);
}

// The setting "gen_rolloff" (Roll-off) of the test signal generator of the stack "papr".
static inline int32_t sdrkit_papr_gen_get_rolloff(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rolloff", value);
}
static inline int32_t sdrkit_papr_gen_set_rolloff(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rolloff", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "input_health" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_input_health_events_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_input_health_event_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.InputHealthEventRow", "events", rows, sizeof(sdrkit_input_health_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "input_health" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_input_health_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "input_health" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_input_health_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "input_health" are the sdrkit_input_health_gen_*() functions
// after the settings of its receiver.

// The setting "block_s" (Reading every) of the stack "input_health". Unit: s.
static inline int32_t sdrkit_input_health_get_block_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "block_s", value);
}
static inline int32_t sdrkit_input_health_set_block_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "block_s", value);
}

// The setting "clip_level" (Clip at (of full scale)) of the stack "input_health".
static inline int32_t sdrkit_input_health_get_clip_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clip_level", value);
}
static inline int32_t sdrkit_input_health_set_clip_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clip_level", value);
}

// The setting "overload_ppm" (Overload above) of the stack "input_health". Unit: ppm.
static inline int32_t sdrkit_input_health_get_overload_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "overload_ppm", value);
}
static inline int32_t sdrkit_input_health_set_overload_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "overload_ppm", value);
}

// The setting "high_dbfs" (High above) of the stack "input_health". Unit: dBFS.
static inline int32_t sdrkit_input_health_get_high_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "high_dbfs", value);
}
static inline int32_t sdrkit_input_health_set_high_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "high_dbfs", value);
}

// The setting "low_dbfs" (Low under) of the stack "input_health". Unit: dBFS.
static inline int32_t sdrkit_input_health_get_low_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "low_dbfs", value);
}
static inline int32_t sdrkit_input_health_set_low_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "low_dbfs", value);
}

// The setting "gen_power" (Noise power) of the test signal generator of the stack "input_health".
static inline int32_t sdrkit_input_health_gen_get_power(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_power", value);
}
static inline int32_t sdrkit_input_health_gen_set_power(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_power", value);
}

// The setting "gen_tones" (Tones) of the test signal generator of the stack "input_health". Entries separated by ';', each 'offset Hz, amplitude' with the amplitude in units of the square root of the noise power.
static inline int32_t sdrkit_input_health_gen_get_tones(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tones", value);
}
static inline int32_t sdrkit_input_health_gen_set_tones(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tones", value);
}

// The setting "gen_clip" (Clip at) of the test signal generator of the stack "input_health".
static inline int32_t sdrkit_input_health_gen_get_clip(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_clip", value);
}
static inline int32_t sdrkit_input_health_gen_set_clip(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_clip", value);
}

// The setting "gen_scale" (Gain after the clip) of the test signal generator of the stack "input_health".
static inline int32_t sdrkit_input_health_gen_get_scale(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_scale", value);
}
static inline int32_t sdrkit_input_health_gen_set_scale(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_scale", value);
}

// Fills `rows` with the changed rows of the table "noise_figure" of the stack "noise_figure" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noise_figure_rows(SDRKIT_HANDLE handle,
                                              sdrkit_noise_figure_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.NoiseFigureRow", "noise_figure", rows, sizeof(sdrkit_noise_figure_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "noise_figure" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_noise_figure_events_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_noise_figure_event_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.NoiseFigureEventRow", "events", rows, sizeof(sdrkit_noise_figure_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "noise_figure" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_noise_figure_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "noise_figure" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_noise_figure_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "noise_figure" are the sdrkit_noise_figure_gen_*() functions
// after the settings of its receiver.

// The setting "enr_db" (Source ENR values) of the stack "noise_figure". Unit: dB. Values from 0 to 40 dB separated by ','.
static inline int32_t sdrkit_noise_figure_get_enr_db(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "enr_db", value);
}
static inline int32_t sdrkit_noise_figure_set_enr_db(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "enr_db", value);
}

// The setting "agree_db" (A window joins a plateau within) of the stack "noise_figure". Unit: dB.
static inline int32_t sdrkit_noise_figure_get_agree_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "agree_db", value);
}
static inline int32_t sdrkit_noise_figure_set_agree_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "agree_db", value);
}

// The setting "step_db" (Smallest step) of the stack "noise_figure". Unit: dB.
static inline int32_t sdrkit_noise_figure_get_step_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "step_db", value);
}
static inline int32_t sdrkit_noise_figure_set_step_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "step_db", value);
}

// The setting "steps_kept" (Steps averaged) of the stack "noise_figure".
static inline int32_t sdrkit_noise_figure_get_steps_kept(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "steps_kept", value);
}
static inline int32_t sdrkit_noise_figure_set_steps_kept(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "steps_kept", value);
}

// The setting "gen_y_db" (Y-factor) of the test signal generator of the stack "noise_figure". Unit: dB.
static inline int32_t sdrkit_noise_figure_gen_get_y_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_y_db", value);
}
static inline int32_t sdrkit_noise_figure_gen_set_y_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_y_db", value);
}

// The setting "gen_period_s" (Source toggles every) of the test signal generator of the stack "noise_figure". Unit: s.
static inline int32_t sdrkit_noise_figure_gen_get_period_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_period_s", value);
}
static inline int32_t sdrkit_noise_figure_gen_set_period_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_period_s", value);
}

// The setting "gen_drift_db_s" (Gain drift) of the test signal generator of the stack "noise_figure". Unit: dB/s.
static inline int32_t sdrkit_noise_figure_gen_get_drift_db_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_drift_db_s", value);
}
static inline int32_t sdrkit_noise_figure_gen_set_drift_db_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_drift_db_s", value);
}

// The setting "gen_p0" (Receiver noise) of the test signal generator of the stack "noise_figure".
static inline int32_t sdrkit_noise_figure_gen_get_p0(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_p0", value);
}
static inline int32_t sdrkit_noise_figure_gen_set_p0(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_p0", value);
}

// Fills `rows` with the changed rows of the table "regions" of the stack "kurtosis" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_kurtosis_regions_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_kurtosis_region_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.KurtosisRegionRow", "regions", rows, sizeof(sdrkit_kurtosis_region_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "kurtosis" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_kurtosis_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_kurtosis_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.KurtosisEventRow", "events", rows, sizeof(sdrkit_kurtosis_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "kurtosis" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_kurtosis_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "kurtosis" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_kurtosis_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "kurtosis" are the sdrkit_kurtosis_gen_*() functions
// after the settings of its receiver.

// The setting "fft" (Frame length) of the stack "kurtosis". Unit: points.
static inline int32_t sdrkit_kurtosis_get_fft(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "fft", value);
}
static inline int32_t sdrkit_kurtosis_set_fft(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "fft", value);
}

// The setting "frames" (Frames per estimate) of the stack "kurtosis".
static inline int32_t sdrkit_kurtosis_get_frames(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "frames", value);
}
static inline int32_t sdrkit_kurtosis_set_frames(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "frames", value);
}

// The setting "blocks" (Estimates averaged) of the stack "kurtosis".
static inline int32_t sdrkit_kurtosis_get_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "blocks", value);
}
static inline int32_t sdrkit_kurtosis_set_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "blocks", value);
}

// The setting "sigmas" (Flag threshold) of the stack "kurtosis". Unit: sigma.
static inline int32_t sdrkit_kurtosis_get_sigmas(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "sigmas", value);
}
static inline int32_t sdrkit_kurtosis_set_sigmas(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "sigmas", value);
}

// The setting "gen_tones" (Tones) of the test signal generator of the stack "kurtosis". Entries separated by ';', each 'offset Hz, amplitude, period s, duty'; a period of 0 is always on.
static inline int32_t sdrkit_kurtosis_gen_get_tones(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tones", value);
}
static inline int32_t sdrkit_kurtosis_gen_set_tones(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tones", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "glitch" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_glitch_events_rows(SDRKIT_HANDLE handle,
                                               sdrkit_glitch_event_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.GlitchEventRow", "events", rows, sizeof(sdrkit_glitch_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "glitch.event"
// (Stream fault). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_glitch_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_glitch_event *event) {
  return sdrkit_typed_event(handle, n, "glitch.event", "meter.GlitchEvent", event, sizeof(sdrkit_glitch_event));
}

// Runs the command "clear" (Clear) of the stack "glitch" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_glitch_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "glitch" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_glitch_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "glitch" are the sdrkit_glitch_gen_*() functions
// after the settings of its receiver.

// The setting "zero_run" (Shortest zero run) of the stack "glitch". Unit: samples.
static inline int32_t sdrkit_glitch_get_zero_run(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "zero_run", value);
}
static inline int32_t sdrkit_glitch_set_zero_run(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "zero_run", value);
}

// The setting "block" (Repeated block length) of the stack "glitch". Unit: samples.
static inline int32_t sdrkit_glitch_get_block(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "block", value);
}
static inline int32_t sdrkit_glitch_set_block(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "block", value);
}

// The setting "step_db" (Level step) of the stack "glitch". Unit: dB.
static inline int32_t sdrkit_glitch_get_step_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "step_db", value);
}
static inline int32_t sdrkit_glitch_set_step_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "step_db", value);
}

// The setting "tone_db" (Tone over the noise per sample) of the stack "glitch". Unit: dB.
static inline int32_t sdrkit_glitch_get_tone_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tone_db", value);
}
static inline int32_t sdrkit_glitch_set_tone_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tone_db", value);
}

// The setting "gen_tone_hz" (Tone) of the test signal generator of the stack "glitch". Unit: Hz.
static inline int32_t sdrkit_glitch_gen_get_tone_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_hz", value);
}
static inline int32_t sdrkit_glitch_gen_set_tone_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_hz", value);
}

// The setting "gen_tone_amp" (Tone amplitude) of the test signal generator of the stack "glitch".
static inline int32_t sdrkit_glitch_gen_get_tone_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_amp", value);
}
static inline int32_t sdrkit_glitch_gen_set_tone_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_amp", value);
}

// The setting "gen_drops" (Drops and repeats) of the test signal generator of the stack "glitch". Entries separated by ';', each 'time s, samples'; negative samples are repeated.
static inline int32_t sdrkit_glitch_gen_get_drops(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_drops", value);
}
static inline int32_t sdrkit_glitch_gen_set_drops(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_drops", value);
}

// The setting "gen_zero_runs" (Zero runs) of the test signal generator of the stack "glitch". Entries separated by ';', each 'time s, samples'.
static inline int32_t sdrkit_glitch_gen_get_zero_runs(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_zero_runs", value);
}
static inline int32_t sdrkit_glitch_gen_set_zero_runs(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_zero_runs", value);
}

// The setting "gen_repeat_blocks" (Repeated blocks) of the test signal generator of the stack "glitch". Times in s separated by ';': the 256 samples after each time plus 256 are those from the time.
static inline int32_t sdrkit_glitch_gen_get_repeat_blocks(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_repeat_blocks", value);
}
static inline int32_t sdrkit_glitch_gen_set_repeat_blocks(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_repeat_blocks", value);
}

// The setting "gen_gain_steps" (Gain steps) of the test signal generator of the stack "glitch". Entries separated by ';', each 'time s, dB': the gain from that time on.
static inline int32_t sdrkit_glitch_gen_get_gain_steps(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_gain_steps", value);
}
static inline int32_t sdrkit_glitch_gen_set_gain_steps(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_gain_steps", value);
}

// Fills `rows` with the changed rows of the table "inputs" of the stack "intermod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_intermod_inputs_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_intermod_input_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.IntermodInputRow", "inputs", rows, sizeof(sdrkit_intermod_input_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "products" of the stack "intermod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_intermod_products_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_intermod_product_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.IntermodProductRow", "products", rows, sizeof(sdrkit_intermod_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "intermod" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_intermod_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_intermod_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.IntermodEventRow", "events", rows, sizeof(sdrkit_intermod_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "intermod.product"
// (Product). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_intermod_product_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_intermod_product_event *event) {
  return sdrkit_typed_event(handle, n, "intermod.product", "meter.IntermodProductEvent", event, sizeof(sdrkit_intermod_product_event));
}

// Runs the command "clear" (Clear) of the stack "intermod" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_intermod_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "intermod" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_intermod_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "intermod" are the sdrkit_intermod_gen_*() functions
// after the settings of its receiver.

// The setting "snapshot_s" (Snapshot length) of the stack "intermod". Unit: s.
static inline int32_t sdrkit_intermod_get_snapshot_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "snapshot_s", value);
}
static inline int32_t sdrkit_intermod_set_snapshot_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "snapshot_s", value);
}

// The setting "peak_db" (Peak over the noise) of the stack "intermod". Unit: dB.
static inline int32_t sdrkit_intermod_get_peak_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "peak_db", value);
}
static inline int32_t sdrkit_intermod_set_peak_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "peak_db", value);
}

// The setting "input_db" (Strong signal over the noise (mean)) of the stack "intermod". Unit: dB.
static inline int32_t sdrkit_intermod_get_input_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "input_db", value);
}
static inline int32_t sdrkit_intermod_set_input_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "input_db", value);
}

// The setting "max_inputs" (Strong signals used) of the stack "intermod".
static inline int32_t sdrkit_intermod_get_max_inputs(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_inputs", value);
}
static inline int32_t sdrkit_intermod_set_max_inputs(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_inputs", value);
}

// The setting "second_order" (Also f2 - f1) of the stack "intermod".
static inline int32_t sdrkit_intermod_get_second_order(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "second_order", value);
}
static inline int32_t sdrkit_intermod_set_second_order(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "second_order", value);
}

// The setting "gen_f1_hz" (First tone) of the test signal generator of the stack "intermod". Unit: Hz.
static inline int32_t sdrkit_intermod_gen_get_f1_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_f1_hz", value);
}
static inline int32_t sdrkit_intermod_gen_set_f1_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_f1_hz", value);
}

// The setting "gen_f2_hz" (Second tone) of the test signal generator of the stack "intermod". Unit: Hz.
static inline int32_t sdrkit_intermod_gen_get_f2_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_f2_hz", value);
}
static inline int32_t sdrkit_intermod_gen_set_f2_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_f2_hz", value);
}

// The setting "gen_amp" (Tone amplitude) of the test signal generator of the stack "intermod".
static inline int32_t sdrkit_intermod_gen_get_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_amp", value);
}
static inline int32_t sdrkit_intermod_gen_set_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_amp", value);
}

// The setting "gen_swing_db" (Level swing) of the test signal generator of the stack "intermod". Unit: dB.
static inline int32_t sdrkit_intermod_gen_get_swing_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_swing_db", value);
}
static inline int32_t sdrkit_intermod_gen_set_swing_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_swing_db", value);
}

// The setting "gen_swing_hz" (Swing rates) of the test signal generator of the stack "intermod". Unit: Hz. The rates of the two swings separated by ','.
static inline int32_t sdrkit_intermod_gen_get_swing_hz(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_swing_hz", value);
}
static inline int32_t sdrkit_intermod_gen_set_swing_hz(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_swing_hz", value);
}

// The setting "gen_k" (Cubic term of the receiver) of the test signal generator of the stack "intermod".
static inline int32_t sdrkit_intermod_gen_get_k(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_k", value);
}
static inline int32_t sdrkit_intermod_gen_set_k(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_k", value);
}

// The setting "gen_extra_hz" (Independent tone) of the test signal generator of the stack "intermod". Unit: Hz.
static inline int32_t sdrkit_intermod_gen_get_extra_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_hz", value);
}
static inline int32_t sdrkit_intermod_gen_set_extra_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_hz", value);
}

// The setting "gen_extra_amp" (Independent tone amplitude) of the test signal generator of the stack "intermod".
static inline int32_t sdrkit_intermod_gen_get_extra_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_extra_amp", value);
}
static inline int32_t sdrkit_intermod_gen_set_extra_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_extra_amp", value);
}

// Fills `rows` with the changed rows of the table "findings" of the stack "spur" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_spur_findings_rows(SDRKIT_HANDLE handle,
                                               sdrkit_spur_finding_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.SpurFindingRow", "findings", rows, sizeof(sdrkit_spur_finding_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "spur" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_spur_events_rows(SDRKIT_HANDLE handle,
                                             sdrkit_spur_event_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.SpurEventRow", "events", rows, sizeof(sdrkit_spur_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "spur" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_spur_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "spur" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_spur_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "spur" are the sdrkit_spur_gen_*() functions
// after the settings of its receiver.

// The setting "peak_db" (Peak over the noise) of the stack "spur". Unit: dB.
static inline int32_t sdrkit_spur_get_peak_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "peak_db", value);
}
static inline int32_t sdrkit_spur_set_peak_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "peak_db", value);
}

// The setting "dc_db" (DC spike over the noise) of the stack "spur". Unit: dB.
static inline int32_t sdrkit_spur_get_dc_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dc_db", value);
}
static inline int32_t sdrkit_spur_set_dc_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dc_db", value);
}

// The setting "spur_grid" (Fixed spur grid as a fraction of the rate) of the stack "spur". Unit: 1/n.
static inline int32_t sdrkit_spur_get_spur_grid(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "spur_grid", value);
}
static inline int32_t sdrkit_spur_set_spur_grid(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "spur_grid", value);
}

// The setting "max_signals" (Strong signals used) of the stack "spur".
static inline int32_t sdrkit_spur_get_max_signals(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_signals", value);
}
static inline int32_t sdrkit_spur_set_max_signals(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_signals", value);
}

// The setting "gen_signals" (Signals) of the test signal generator of the stack "spur". Entries separated by ';', each 'offset Hz, amplitude'.
static inline int32_t sdrkit_spur_gen_get_signals(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_signals", value);
}
static inline int32_t sdrkit_spur_gen_set_signals(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_signals", value);
}

// The setting "gen_swing_db" (Level swing) of the test signal generator of the stack "spur". Unit: dB.
static inline int32_t sdrkit_spur_gen_get_swing_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_swing_db", value);
}
static inline int32_t sdrkit_spur_gen_set_swing_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_swing_db", value);
}

// The setting "gen_image_db" (Image rejection) of the test signal generator of the stack "spur". Unit: dB.
static inline int32_t sdrkit_spur_gen_get_image_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_image_db", value);
}
static inline int32_t sdrkit_spur_gen_set_image_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_image_db", value);
}

// The setting "gen_image_phase" (Image phase) of the test signal generator of the stack "spur". Unit: rad.
static inline int32_t sdrkit_spur_gen_get_image_phase(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_image_phase", value);
}
static inline int32_t sdrkit_spur_gen_set_image_phase(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_image_phase", value);
}

// The setting "gen_dc" (DC offset) of the test signal generator of the stack "spur".
static inline int32_t sdrkit_spur_gen_get_dc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_dc", value);
}
static inline int32_t sdrkit_spur_gen_set_dc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_dc", value);
}

// The setting "gen_h2" (Second-order term) of the test signal generator of the stack "spur".
static inline int32_t sdrkit_spur_gen_get_h2(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_h2", value);
}
static inline int32_t sdrkit_spur_gen_set_h2(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_h2", value);
}

// The setting "gen_h3" (Third-order term) of the test signal generator of the stack "spur".
static inline int32_t sdrkit_spur_gen_get_h3(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_h3", value);
}
static inline int32_t sdrkit_spur_gen_set_h3(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_h3", value);
}

// The setting "gen_spur_hz" (Fixed tone) of the test signal generator of the stack "spur". Unit: Hz.
static inline int32_t sdrkit_spur_gen_get_spur_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spur_hz", value);
}
static inline int32_t sdrkit_spur_gen_set_spur_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spur_hz", value);
}

// The setting "gen_spur_amp" (Fixed tone amplitude) of the test signal generator of the stack "spur".
static inline int32_t sdrkit_spur_gen_get_spur_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spur_amp", value);
}
static inline int32_t sdrkit_spur_gen_set_spur_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spur_amp", value);
}

// The setting "gen_real_hz" (Real signal) of the test signal generator of the stack "spur". Unit: Hz.
static inline int32_t sdrkit_spur_gen_get_real_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_real_hz", value);
}
static inline int32_t sdrkit_spur_gen_set_real_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_real_hz", value);
}

// The setting "gen_real_amp" (Real signal amplitude) of the test signal generator of the stack "spur".
static inline int32_t sdrkit_spur_gen_get_real_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_real_amp", value);
}
static inline int32_t sdrkit_spur_gen_set_real_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_real_amp", value);
}

// The setting "gen_shift_hz" (Shift of the signals) of the test signal generator of the stack "spur". Unit: Hz.
static inline int32_t sdrkit_spur_gen_get_shift_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_shift_hz", value);
}
static inline int32_t sdrkit_spur_gen_set_shift_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_shift_hz", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "fading" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fading_events_rows(SDRKIT_HANDLE handle,
                                               sdrkit_fading_event_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.FadingEventRow", "events", rows, sizeof(sdrkit_fading_event_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "fading" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_fading_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "fading" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_fading_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fading" are the sdrkit_fading_gen_*() functions
// after the settings of its receiver.

// The setting "window_s" (Statistics window) of the stack "fading". Unit: s.
static inline int32_t sdrkit_fading_get_window_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "window_s", value);
}
static inline int32_t sdrkit_fading_set_window_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "window_s", value);
}

// The setting "first_s" (First reading after) of the stack "fading". Unit: s.
static inline int32_t sdrkit_fading_get_first_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "first_s", value);
}
static inline int32_t sdrkit_fading_set_first_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "first_s", value);
}

// The setting "fade_db" (Fade threshold under the mean) of the stack "fading". Unit: dB.
static inline int32_t sdrkit_fading_get_fade_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fade_db", value);
}
static inline int32_t sdrkit_fading_set_fade_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fade_db", value);
}

// The setting "log_depth_db" (Fade logged deeper than) of the stack "fading". Unit: dB.
static inline int32_t sdrkit_fading_get_log_depth_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "log_depth_db", value);
}
static inline int32_t sdrkit_fading_set_log_depth_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "log_depth_db", value);
}

// The setting "log_length_s" (Fade logged longer than) of the stack "fading". Unit: s.
static inline int32_t sdrkit_fading_get_log_length_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "log_length_s", value);
}
static inline int32_t sdrkit_fading_set_log_length_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "log_length_s", value);
}

// The setting "gen_fd_hz" (Largest Doppler) of the test signal generator of the stack "fading". Unit: Hz.
static inline int32_t sdrkit_fading_gen_get_fd_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fd_hz", value);
}
static inline int32_t sdrkit_fading_gen_set_fd_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fd_hz", value);
}

// The setting "gen_k" (Rice factor) of the test signal generator of the stack "fading". Direct over scattered power as a ratio; 0 is Rayleigh fading.
static inline int32_t sdrkit_fading_gen_get_k(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_k", value);
}
static inline int32_t sdrkit_fading_gen_set_k(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_k", value);
}

// The setting "gen_off" (No carrier) of the test signal generator of the stack "fading". Spans separated by ';', each 'from s, to s'.
static inline int32_t sdrkit_fading_gen_get_off(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_off", value);
}
static inline int32_t sdrkit_fading_gen_set_off(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_off", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "beacon_monitor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_beacon_monitor_events_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_beacon_monitor_event_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "meter.BeaconMonitorEventRow", "events", rows, sizeof(sdrkit_beacon_monitor_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "beacon_monitor.change"
// (Path change). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_beacon_monitor_change_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_beacon_monitor_change_event *event) {
  return sdrkit_typed_event(handle, n, "beacon_monitor.change", "meter.BeaconMonitorChangeEvent", event, sizeof(sdrkit_beacon_monitor_change_event));
}

// Runs the command "clear" (Clear) of the stack "beacon_monitor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_beacon_monitor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "beacon_monitor" on the session of `handle`. Drops the trackers, averages and floors, as after a retune; the tables stay.
static inline int32_t sdrkit_beacon_monitor_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "beacon_monitor" are the sdrkit_beacon_monitor_gen_*() functions
// after the settings of its receiver.

// The setting "present_db" (Line over the noise) of the stack "beacon_monitor". Unit: dB.
static inline int32_t sdrkit_beacon_monitor_get_present_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "present_db", value);
}
static inline int32_t sdrkit_beacon_monitor_set_present_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "present_db", value);
}

// The setting "aurora_hz" (Aurora-like above) of the stack "beacon_monitor". Unit: Hz.
static inline int32_t sdrkit_beacon_monitor_get_aurora_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "aurora_hz", value);
}
static inline int32_t sdrkit_beacon_monitor_set_aurora_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "aurora_hz", value);
}

// The setting "fading_db" (Fading swing above) of the stack "beacon_monitor". Unit: dB.
static inline int32_t sdrkit_beacon_monitor_get_fading_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fading_db", value);
}
static inline int32_t sdrkit_beacon_monitor_set_fading_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fading_db", value);
}

// The setting "spectra" (Spectra per decision) of the stack "beacon_monitor".
static inline int32_t sdrkit_beacon_monitor_get_spectra(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "spectra", value);
}
static inline int32_t sdrkit_beacon_monitor_set_spectra(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "spectra", value);
}

// The setting "gen_fade_hz" (Fading rate) of the test signal generator of the stack "beacon_monitor". Unit: Hz.
static inline int32_t sdrkit_beacon_monitor_gen_get_fade_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fade_hz", value);
}
static inline int32_t sdrkit_beacon_monitor_gen_set_fade_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fade_hz", value);
}

// The setting "gen_fade_depth_db" (Fading depth) of the test signal generator of the stack "beacon_monitor". Unit: dB.
static inline int32_t sdrkit_beacon_monitor_gen_get_fade_depth_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fade_depth_db", value);
}
static inline int32_t sdrkit_beacon_monitor_gen_set_fade_depth_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fade_depth_db", value);
}

// The setting "gen_spread_hz" (Aurora spread) of the test signal generator of the stack "beacon_monitor". Unit: Hz.
static inline int32_t sdrkit_beacon_monitor_gen_get_spread_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spread_hz", value);
}
static inline int32_t sdrkit_beacon_monitor_gen_set_spread_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spread_hz", value);
}

// The setting "gen_off" (No beacon) of the test signal generator of the stack "beacon_monitor". Spans separated by ';', each 'from s, to s'.
static inline int32_t sdrkit_beacon_monitor_gen_get_off(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_off", value);
}
static inline int32_t sdrkit_beacon_monitor_gen_set_off(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_off", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fm_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_fm_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.FmMonitorFields", fields, sizeof(sdrkit_fm_monitor_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_meter_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_meter_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.DecoderFields", fields, sizeof(sdrkit_meter_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_am_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_am_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.AmMonitorFields", fields, sizeof(sdrkit_am_monitor_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_meter_watch_fields_read(SDRKIT_HANDLE handle, sdrkit_meter_watch_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.WatchFields", fields, sizeof(sdrkit_meter_watch_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_occupancy_fields_read(SDRKIT_HANDLE handle, sdrkit_occupancy_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.OccupancyFields", fields, sizeof(sdrkit_occupancy_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_noise_floor_fields_read(SDRKIT_HANDLE handle, sdrkit_noise_floor_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.NoiseFloorFields", fields, sizeof(sdrkit_noise_floor_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_burst_finder_fields_read(SDRKIT_HANDLE handle, sdrkit_burst_finder_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.BurstFinderFields", fields, sizeof(sdrkit_burst_finder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hopper_fields_read(SDRKIT_HANDLE handle, sdrkit_hopper_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.HopperFields", fields, sizeof(sdrkit_hopper_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_birdie_fields_read(SDRKIT_HANDLE handle, sdrkit_birdie_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.BirdieFields", fields, sizeof(sdrkit_birdie_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_obw_fields_read(SDRKIT_HANDLE handle, sdrkit_obw_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.ObwFields", fields, sizeof(sdrkit_obw_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ptt_log_fields_read(SDRKIT_HANDLE handle, sdrkit_ptt_log_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.PttLogFields", fields, sizeof(sdrkit_ptt_log_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_noise_locator_fields_read(SDRKIT_HANDLE handle, sdrkit_noise_locator_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.NoiseLocatorFields", fields, sizeof(sdrkit_noise_locator_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_freq_accuracy_fields_read(SDRKIT_HANDLE handle, sdrkit_freq_accuracy_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.FreqAccuracyFields", fields, sizeof(sdrkit_freq_accuracy_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sinad_fields_read(SDRKIT_HANDLE handle, sdrkit_sinad_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.SinadFields", fields, sizeof(sdrkit_sinad_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nfm_meter_fields_read(SDRKIT_HANDLE handle, sdrkit_nfm_meter_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.NfmMeterFields", fields, sizeof(sdrkit_nfm_meter_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tx_stability_fields_read(SDRKIT_HANDLE handle, sdrkit_tx_stability_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.TxStabilityFields", fields, sizeof(sdrkit_tx_stability_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ssb_test_fields_read(SDRKIT_HANDLE handle, sdrkit_ssb_test_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.SsbTestFields", fields, sizeof(sdrkit_ssb_test_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_afsk_tuner_fields_read(SDRKIT_HANDLE handle, sdrkit_afsk_tuner_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.AfskTunerFields", fields, sizeof(sdrkit_afsk_tuner_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_phase_noise_fields_read(SDRKIT_HANDLE handle, sdrkit_phase_noise_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.PhaseNoiseFields", fields, sizeof(sdrkit_phase_noise_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cw_keying_fields_read(SDRKIT_HANDLE handle, sdrkit_cw_keying_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.CwKeyingFields", fields, sizeof(sdrkit_cw_keying_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fsk_analyser_fields_read(SDRKIT_HANDLE handle, sdrkit_fsk_analyser_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.FskAnalyserFields", fields, sizeof(sdrkit_fsk_analyser_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mer_fields_read(SDRKIT_HANDLE handle, sdrkit_mer_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.MerFields", fields, sizeof(sdrkit_mer_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_papr_fields_read(SDRKIT_HANDLE handle, sdrkit_papr_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.PaprFields", fields, sizeof(sdrkit_papr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_input_health_fields_read(SDRKIT_HANDLE handle, sdrkit_input_health_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.InputHealthFields", fields, sizeof(sdrkit_input_health_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_noise_figure_fields_read(SDRKIT_HANDLE handle, sdrkit_noise_figure_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.NoiseFigureFields", fields, sizeof(sdrkit_noise_figure_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_kurtosis_fields_read(SDRKIT_HANDLE handle, sdrkit_kurtosis_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.KurtosisFields", fields, sizeof(sdrkit_kurtosis_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_glitch_fields_read(SDRKIT_HANDLE handle, sdrkit_glitch_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.GlitchFields", fields, sizeof(sdrkit_glitch_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_intermod_fields_read(SDRKIT_HANDLE handle, sdrkit_intermod_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.IntermodFields", fields, sizeof(sdrkit_intermod_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_spur_fields_read(SDRKIT_HANDLE handle, sdrkit_spur_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.SpurFields", fields, sizeof(sdrkit_spur_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fading_fields_read(SDRKIT_HANDLE handle, sdrkit_fading_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.FadingFields", fields, sizeof(sdrkit_fading_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_beacon_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_beacon_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "meter.BeaconMonitorFields", fields, sizeof(sdrkit_beacon_monitor_fields));
}

#endif
