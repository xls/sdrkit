/*
 * sdrkit: the typed interface of the stack family "dab", version 1.1. MIT licence.
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

#ifndef SDRKIT_DAB_H
#define SDRKIT_DAB_H

#include "../sdrkit.h"
#include "mpegts.h"
#include "bcdata.h"

// The version of the family that this header describes.
#define SDRKIT_DAB_VERSION_MAJOR 1
#define SDRKIT_DAB_VERSION_MINOR 1

// The stack "dab": its id, and the keys of its event types.
#define SDRKIT_STACK_DAB "dab"
#define SDRKIT_EVENT_DAB_ENSEMBLE "dab.ensemble"
#define SDRKIT_EVENT_DAB_SERVICES "dab.services"
#define SDRKIT_EVENT_DAB_ANNOUNCEMENT_STARTED "dab.announcement_started"
#define SDRKIT_EVENT_DAB_ANNOUNCEMENT_ENDED "dab.announcement_ended"
#define SDRKIT_EVENT_DAB_SYNC_LOST "dab.sync_lost"
#define SDRKIT_EVENT_DAB_LABEL "dab.label"
#define SDRKIT_EVENT_DAB_SLIDE "dab.slide"

// The options of the setting "detection" of the stack "dab".
typedef enum sdrkit_dab_detection {
  // Automatic
  SDRKIT_DAB_DETECTION_AUTO = 0,
  // Differential
  SDRKIT_DAB_DETECTION_DIFFERENTIAL = 1,
} sdrkit_dab_detection;

// The options of the setting "aac_packaging" of the stack "dab".
typedef enum sdrkit_dab_aac_packaging {
  // Raw access units
  SDRKIT_DAB_AAC_PACKAGING_RAW = 0,
  // LATM
  SDRKIT_DAB_AAC_PACKAGING_LATM = 1,
} sdrkit_dab_aac_packaging;

// The measurements of the signal. No SNR before the lock, and no offset and clock error while the receiver searches.
typedef struct sdrkit_dab_signal_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR measure. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset, without the centring shift. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The sample clock error. Unit: ppm.
  double clock_ppm;
  // The input level. Unit: dBFS.
  double level_dbfs;
} sdrkit_dab_signal_fields;

// The ensemble. No value for an item that the ensemble has not signalled yet.
typedef struct sdrkit_dab_ensemble_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `ensemble_label` holds a value.
  bool has_ensemble_label;
  // The ensemble label.
  sdrkit_string ensemble_label;
  // True if `ensemble_short_label` holds a value.
  bool has_ensemble_short_label;
  // The ensemble short label.
  sdrkit_string ensemble_short_label;
  // True if `eid` holds a value.
  bool has_eid;
  // The ensemble identifier (EId).
  int64_t eid;
  // True if `ecc` holds a value.
  bool has_ecc;
  // The extended country code (FIG 0/9).
  int64_t ecc;
  // True if `country` holds a value.
  bool has_country;
  // The country id (top 4 bits of the EId).
  int64_t country;
  // True if `utc` holds a value.
  bool has_utc;
  // The ensemble time. Seconds since 1970 (UTC).
  double utc;
  // True if `local_time_offset` holds a value.
  bool has_local_time_offset;
  // The local time offset (FIG 0/9). A length of time in seconds.
  double local_time_offset;
  // Every label and the service organisation are received.
  bool ensemble_valid;
  // Services in the ensemble.
  int64_t service_count;
} sdrkit_dab_ensemble_fields;

// The options of `detection` of `sdrkit_dab_sync_fields`.
typedef enum sdrkit_dab_sync_fields_detection {
  // Coherent
  SDRKIT_DAB_SYNC_FIELDS_DETECTION_COHERENT = 0,
  // Differential
  SDRKIT_DAB_SYNC_FIELDS_DETECTION_DIFFERENTIAL = 1,
} sdrkit_dab_sync_fields_detection;

// The synchronisation, the FIC and the front end.
typedef struct sdrkit_dab_sync_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The last synchronisation symbol (PRS) was found.
  bool sync;
  // True if `prs_peak` holds a value.
  bool has_prs_peak;
  // The PRS correlation peak over its median. No value while the receiver searches.
  double prs_peak;
  // Frames demodulated.
  int64_t frames;
  // FIBs with a good CRC.
  int64_t fib_ok;
  // FIBs with a bad CRC.
  int64_t fib_bad;
  // Good FIBs in the last frame.
  int64_t fib_ok_last_frame;
  // The share of the FIC that carries FIGs. Unit: %.
  double fic_fill_pct;
  // The quiet time of the null symbol. A length of time in seconds.
  double quiet;
  // Sync losses.
  int64_t sync_losses;
  // Relocks.
  int64_t relocks;
  // CIFs erased after lost samples.
  int64_t erased_cifs;
  // Impulses blanked.
  int64_t impulses_blanked;
  // The spectrum was mirrored.
  bool mirrored;
  // Signal time. A length of time in seconds.
  double signal_time;
  // True if `detection` holds a value.
  bool has_detection;
  // The detection in use. No value while the receiver searches. A value of `sdrkit_dab_sync_fields_detection`.
  int32_t detection;
} sdrkit_dab_sync_fields;

// The options of `selected_kind` of `sdrkit_dab_playing_fields`.
typedef enum sdrkit_dab_playing_fields_selected_kind {
  // DAB+
  SDRKIT_DAB_PLAYING_FIELDS_SELECTED_KIND_DAB_PLUS = 0,
  // DAB
  SDRKIT_DAB_PLAYING_FIELDS_SELECTED_KIND_DAB = 1,
  // DMB
  SDRKIT_DAB_PLAYING_FIELDS_SELECTED_KIND_DMB = 2,
  // Data
  SDRKIT_DAB_PLAYING_FIELDS_SELECTED_KIND_DATA = 3,
} sdrkit_dab_playing_fields_selected_kind;

// The options of `playing` of `sdrkit_dab_playing_fields`.
typedef enum sdrkit_dab_playing_fields_playing {
  // No service selected
  SDRKIT_DAB_PLAYING_FIELDS_PLAYING_NOTHING = 0,
  // Scrambled
  SDRKIT_DAB_PLAYING_FIELDS_PLAYING_SCRAMBLED = 1,
  // Waiting for the sub-channel
  SDRKIT_DAB_PLAYING_FIELDS_PLAYING_WAITING = 2,
  // No audio yet
  SDRKIT_DAB_PLAYING_FIELDS_PLAYING_NO_AUDIO = 3,
  // Decoding
  SDRKIT_DAB_PLAYING_FIELDS_PLAYING_DECODING = 4,
} sdrkit_dab_playing_fields_playing;

// The service playing. No value while no service plays.
typedef struct sdrkit_dab_playing_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `selected_service` holds a value.
  bool has_selected_service;
  // The row key of the service playing in [`SERVICES`].
  sdrkit_string selected_service;
  // True if `selected_subchannel` holds a value.
  bool has_selected_subchannel;
  // The sub-channel playing.
  int64_t selected_subchannel;
  // True if `selected_label` holds a value.
  bool has_selected_label;
  // The label of the service playing.
  sdrkit_string selected_label;
  // True if `selected_kind` holds a value.
  bool has_selected_kind;
  // The kind of the service playing. A value of `sdrkit_dab_playing_fields_selected_kind`.
  int32_t selected_kind;
  // True if `selected_bitrate_kbps` holds a value.
  bool has_selected_bitrate_kbps;
  // The bit rate playing. Unit: kbit/s.
  int64_t selected_bitrate_kbps;
  // True if `selected_protection` holds a value.
  bool has_selected_protection;
  // The protection playing ("EEP 3-A", "UEP 12").
  sdrkit_string selected_protection;
  // What the audio of the service playing does. A value of `sdrkit_dab_playing_fields_playing`.
  int32_t playing;
} sdrkit_dab_playing_fields;

// The options of `audio_codec` of `sdrkit_dab_audio_fields`.
typedef enum sdrkit_dab_audio_fields_audio_codec {
  // AAC-LC
  SDRKIT_DAB_AUDIO_FIELDS_AUDIO_CODEC_AAC = 0,
  // HE-AAC
  SDRKIT_DAB_AUDIO_FIELDS_AUDIO_CODEC_HE_AAC = 1,
  // HE-AAC v2
  SDRKIT_DAB_AUDIO_FIELDS_AUDIO_CODEC_HE_AAC_V2 = 2,
  // MPEG Layer II
  SDRKIT_DAB_AUDIO_FIELDS_AUDIO_CODEC_MP2 = 3,
} sdrkit_dab_audio_fields_audio_codec;

// The audio of the service playing. No codec, rate, channels, SBR or PS before the audio configuration is known, and no concealed share before the first access unit.
typedef struct sdrkit_dab_audio_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `audio_codec` holds a value.
  bool has_audio_codec;
  // The audio codec. A value of `sdrkit_dab_audio_fields_audio_codec`.
  int32_t audio_codec;
  // True if `audio_rate` holds a value.
  bool has_audio_rate;
  // The audio sample rate. A frequency in Hz.
  double audio_rate;
  // True if `audio_channels` holds a value.
  bool has_audio_channels;
  // Audio channels.
  int64_t audio_channels;
  // True if `audio_sbr` holds a value.
  bool has_audio_sbr;
  // SBR in use.
  bool audio_sbr;
  // True if `audio_ps` holds a value.
  bool has_audio_ps;
  // Parametric stereo in use.
  bool audio_ps;
  // True if `audio_units_per_superframe` holds a value.
  bool has_audio_units_per_superframe;
  // Access units in a DAB+ superframe.
  int64_t audio_units_per_superframe;
  // Logical frames of the sub-channel playing.
  int64_t audio_logical_frames;
  // Good access units.
  int64_t audio_units_ok;
  // Bad access units.
  int64_t audio_units_bad;
  // Good superframes.
  int64_t audio_superframes_ok;
  // Bad superframes.
  int64_t audio_superframes_bad;
  // Reed-Solomon bytes corrected.
  int64_t audio_rs_corrected;
  // Reed-Solomon words beyond repair.
  int64_t audio_rs_uncorrectable;
  // Layer II frames.
  int64_t audio_mp2_frames;
  // Bad Layer II frames.
  int64_t audio_mp2_bad;
  // True if `audio_concealed_pct` holds a value.
  bool has_audio_concealed_pct;
  // The share of access units concealed. Unit: %.
  double audio_concealed_pct;
} sdrkit_dab_audio_fields;

// The dynamic label and the slide of the service playing. No value before the first label or slide.
typedef struct sdrkit_dab_pad_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `label_charset` holds a value.
  bool has_label_charset;
  // The character set code of the dynamic label (TS 101 756 table 1).
  int64_t label_charset;
  // True if `slide_sequence` holds a value.
  bool has_slide_sequence;
  // The slide number: a new number is a new picture.
  int64_t slide_sequence;
  // True if `slide_width` holds a value.
  bool has_slide_width;
  // The slide width. Unit: px.
  int64_t slide_width;
  // True if `slide_height` holds a value.
  bool has_slide_height;
  // The slide height. Unit: px.
  int64_t slide_height;
  // True if `slide_bytes` holds a value.
  bool has_slide_bytes;
  // The slide size. Unit: bytes.
  int64_t slide_bytes;
} sdrkit_dab_pad_fields;

// When things were first reported, after the start. No value before it happened.
typedef struct sdrkit_dab_timing_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `timing_lock` holds a value.
  bool has_timing_lock;
  // Lock. A length of time in seconds.
  double timing_lock;
  // True if `timing_first_fib` holds a value.
  bool has_timing_first_fib;
  // The first good FIB. A length of time in seconds.
  double timing_first_fib;
  // True if `timing_ensemble_valid` holds a value.
  bool has_timing_ensemble_valid;
  // The ensemble complete. A length of time in seconds.
  double timing_ensemble_valid;
  // True if `timing_all_labels` holds a value.
  bool has_timing_all_labels;
  // Every label known. A length of time in seconds.
  double timing_all_labels;
  // True if `timing_first_access_unit` holds a value.
  bool has_timing_first_access_unit;
  // The first audio of the service playing. A length of time in seconds.
  double timing_first_access_unit;
  // True if `timing_label` holds a value.
  bool has_timing_label;
  // The first label. A length of time in seconds.
  double timing_label;
  // True if `timing_slide` holds a value.
  bool has_timing_slide;
  // The first slide. A length of time in seconds.
  double timing_slide;
} sdrkit_dab_timing_fields;

// The announcement that applies to the service playing. No value while none applies.
typedef struct sdrkit_dab_announcement_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `announcement_cluster` holds a value.
  bool has_announcement_cluster;
  // The cluster of the announcement.
  int64_t announcement_cluster;
  // True if `announcement_type` holds a value.
  bool has_announcement_type;
  // The type of the announcement (TS 101 756 table 14: 0 alarm, 1 road traffic ...).
  int64_t announcement_type;
  // True if `announcement_name` holds a value.
  bool has_announcement_name;
  // The name of that type.
  sdrkit_string announcement_name;
  // True if `announcement_subchannel` holds a value.
  bool has_announcement_subchannel;
  // The sub-channel of the announcement.
  int64_t announcement_subchannel;
} sdrkit_dab_announcement_fields;

// The DMB video sub-channel. No value before the DMB decoder reports, and no size before a sequence parameter set.
typedef struct sdrkit_dab_dmb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dmb_ts_packets` holds a value.
  bool has_dmb_ts_packets;
  // DMB transport stream packets.
  int64_t dmb_ts_packets;
  // True if `dmb_rs_corrected` holds a value.
  bool has_dmb_rs_corrected;
  // DMB Reed-Solomon words corrected.
  int64_t dmb_rs_corrected;
  // True if `dmb_rs_failed` holds a value.
  bool has_dmb_rs_failed;
  // DMB Reed-Solomon words beyond repair.
  int64_t dmb_rs_failed;
  // True if `dmb_video_units` holds a value.
  bool has_dmb_video_units;
  // DMB video access units.
  int64_t dmb_video_units;
  // True if `dmb_audio_units` holds a value.
  bool has_dmb_audio_units;
  // DMB audio access units.
  int64_t dmb_audio_units;
  // True if `dmb_video_width` holds a value.
  bool has_dmb_video_width;
  // The width of the DMB video, from its sequence parameter set. Unit: px.
  int64_t dmb_video_width;
  // True if `dmb_video_height` holds a value.
  bool has_dmb_video_height;
  // The height of the DMB video. Unit: px.
  int64_t dmb_video_height;
} sdrkit_dab_dmb_fields;

// The counts of the data decoders. No value before a decoder reports.
typedef struct sdrkit_dab_data_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // FIG records kept for the analyser.
  int64_t fic_records;
  // True if `spi_objects` holds a value.
  bool has_spi_objects;
  // Guide objects decoded.
  int64_t spi_objects;
  // True if `journaline_received` holds a value.
  bool has_journaline_received;
  // Journaline objects received.
  int64_t journaline_received;
  // True if `tii_analysed` holds a value.
  bool has_tii_analysed;
  // Null symbols analysed for transmitter identification.
  int64_t tii_analysed;
} sdrkit_dab_data_fields;

// The options of `health_sync` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_sync {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_SYNC_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_SYNC_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_SYNC_GREEN = 2,
} sdrkit_dab_health_fields_health_sync;

// The options of `health_prs` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_prs {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_PRS_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_PRS_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_PRS_GREEN = 2,
} sdrkit_dab_health_fields_health_prs;

// The options of `health_fic` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_fic {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_FIC_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_FIC_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_FIC_GREEN = 2,
} sdrkit_dab_health_fields_health_fic;

// The options of `health_ensemble` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_ensemble {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_ENSEMBLE_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_ENSEMBLE_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_ENSEMBLE_GREEN = 2,
} sdrkit_dab_health_fields_health_ensemble;

// The options of `health_msc` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_msc {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_MSC_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_MSC_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_MSC_GREEN = 2,
} sdrkit_dab_health_fields_health_msc;

// The options of `health_audio` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_audio {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_AUDIO_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_AUDIO_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_AUDIO_GREEN = 2,
} sdrkit_dab_health_fields_health_audio;

// The options of `health_concealed` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_health_concealed {
  // Off
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_CONCEALED_OFF = 0,
  // Amber
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_CONCEALED_AMBER = 1,
  // Green
  SDRKIT_DAB_HEALTH_FIELDS_HEALTH_CONCEALED_GREEN = 2,
} sdrkit_dab_health_fields_health_concealed;

// The options of `quality_text` of `sdrkit_dab_health_fields`.
typedef enum sdrkit_dab_health_fields_quality_text {
  // Poor
  SDRKIT_DAB_HEALTH_FIELDS_QUALITY_TEXT_POOR = 0,
  // Fair
  SDRKIT_DAB_HEALTH_FIELDS_QUALITY_TEXT_FAIR = 1,
  // Good
  SDRKIT_DAB_HEALTH_FIELDS_QUALITY_TEXT_GOOD = 2,
  // Excellent
  SDRKIT_DAB_HEALTH_FIELDS_QUALITY_TEXT_EXCELLENT = 3,
} sdrkit_dab_health_fields_quality_text;

// The health indicators and the signal quality.
typedef struct sdrkit_dab_health_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The receiver is locked. A value of `sdrkit_dab_health_fields_health_sync`.
  int32_t health_sync;
  // The synchronisation symbol peak. A value of `sdrkit_dab_health_fields_health_prs`.
  int32_t health_prs;
  // The FIBs of the last frame. A value of `sdrkit_dab_health_fields_health_fic`.
  int32_t health_fic;
  // The ensemble. A value of `sdrkit_dab_health_fields_health_ensemble`.
  int32_t health_ensemble;
  // The sub-channel playing. A value of `sdrkit_dab_health_fields_health_msc`.
  int32_t health_msc;
  // The audio. A value of `sdrkit_dab_health_fields_health_audio`.
  int32_t health_audio;
  // The concealment. A value of `sdrkit_dab_health_fields_health_concealed`.
  int32_t health_concealed;
  // True if `quality` holds a value.
  bool has_quality;
  // The signal quality, 0 to 1: (SNR − 4 dB) / 18 dB. No value before the lock.
  double quality;
  // True if `quality_text` holds a value.
  bool has_quality_text;
  // The signal quality in words. No value before the lock. A value of `sdrkit_dab_health_fields_quality_text`.
  int32_t quality_text;
} sdrkit_dab_health_fields;

// The options of `kind` of `sdrkit_dab_service_row`.
typedef enum sdrkit_dab_service_row_kind {
  // DAB+
  SDRKIT_DAB_SERVICE_ROW_KIND_DAB_PLUS = 0,
  // DAB
  SDRKIT_DAB_SERVICE_ROW_KIND_DAB = 1,
  // DMB
  SDRKIT_DAB_SERVICE_ROW_KIND_DMB = 2,
  // Data
  SDRKIT_DAB_SERVICE_ROW_KIND_DATA = 3,
} sdrkit_dab_service_row_kind;

// A service of the ensemble. The row key is the SId in hexadecimal: 4 digits, 8 for a data service.
typedef struct sdrkit_dab_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The service identifier (SId).
  int64_t sid;
  // True if `label` holds a value.
  bool has_label;
  // The label.
  sdrkit_string label;
  // True if `short_label` holds a value.
  bool has_short_label;
  // The short label.
  sdrkit_string short_label;
  // What the service carries. A value of `sdrkit_dab_service_row_kind`.
  int32_t kind;
  // True if `subchannel` holds a value.
  bool has_subchannel;
  // The sub-channel of the primary component.
  int64_t subchannel;
  // True if `bitrate_kbps` holds a value.
  bool has_bitrate_kbps;
  // The bit rate of that sub-channel. Unit: kbit/s.
  int64_t bitrate_kbps;
  // True if `protection` holds a value.
  bool has_protection;
  // The protection of that sub-channel ("EEP 3-A", "UEP 12").
  sdrkit_string protection;
  // True if `component_type` holds a value.
  bool has_component_type;
  // The ASCTy of the audio component, or the DSCTy of a data component.
  int64_t component_type;
  // True if `pty` holds a value.
  bool has_pty;
  // The programme type code.
  int64_t pty;
  // True if `pty_name` holds a value.
  bool has_pty_name;
  // The name of the programme type.
  sdrkit_string pty_name;
  // The programme type is static.
  bool pty_static;
  // True if `language` holds a value.
  bool has_language;
  // The language code (TS 101 756 tables 9 and 10).
  int64_t language;
  // True if `language_name` holds a value.
  bool has_language_name;
  // The name of the language.
  sdrkit_string language_name;
  // Scrambled (conditional access).
  bool scrambled;
  // The announcement support flags (ASu).
  int64_t announcement_support;
  // True if `now_playing` holds a value.
  bool has_now_playing;
  // The dynamic label of the service, while it plays.
  sdrkit_string now_playing;
  // The service plays.
  bool playing;
} sdrkit_dab_service_row;

// A sub-channel. The row key is its id.
typedef struct sdrkit_dab_subchannel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sub-channel id.
  int64_t id;
  // The first capacity unit. Unit: CU.
  int64_t start_cu;
  // The size in capacity units. Unit: CU.
  int64_t size_cu;
  // True if `protection` holds a value.
  bool has_protection;
  // The protection ("EEP 3-A", "UEP 12").
  sdrkit_string protection;
  // The bit rate. Unit: kbit/s.
  int64_t bitrate_kbps;
} sdrkit_dab_subchannel_row;

// A service component. The row key is `<service>/<n>`; n counts from 0 in the order of FIG 0/2 and FIG 0/8.
typedef struct sdrkit_dab_service_component_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the service.
  sdrkit_string service;
  // The transport mechanism (TMId: 0 audio stream, 1 data stream, 3 packet data).
  int64_t tmid;
  // The ASCTy of an audio component, or the DSCTy of a data component.
  int64_t component_type;
  // True if `subchannel` holds a value.
  bool has_subchannel;
  // The sub-channel.
  int64_t subchannel;
  // True if `packet_address` holds a value.
  bool has_packet_address;
  // The packet address.
  int64_t packet_address;
  // True if `scids` holds a value.
  bool has_scids;
  // The service component identifier within the service (SCIdS).
  int64_t scids;
  // The primary component.
  bool primary;
  // Scrambled (conditional access).
  bool scrambled;
  // True if `label` holds a value.
  bool has_label;
  // The component label.
  sdrkit_string label;
} sdrkit_dab_service_component_row;

// A user application of a service (FIG 0/13). The row key is `<service>/<SCIdS>/<type>`.
typedef struct sdrkit_dab_user_application_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the service.
  sdrkit_string service;
  // The service component identifier within the service (SCIdS).
  int64_t scids;
  // The user application type (TS 101 756 table 16).
  int64_t app_type;
  // The name of the type.
  sdrkit_string name;
} sdrkit_dab_user_application_row;

// An announcement cluster of a service (FIG 0/18). The row key is `<service>/<cluster>`.
typedef struct sdrkit_dab_service_cluster_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the service.
  sdrkit_string service;
  // The announcement cluster.
  int64_t cluster;
} sdrkit_dab_service_cluster_row;

// A transmitter of the database of FIG 0/22. The row key is `<main>.<sub>`.
typedef struct sdrkit_dab_tii_entry_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The transmitter main identifier.
  int64_t main;
  // The transmitter sub-identifier.
  int64_t sub;
  // The position.
  sdrkit_position position;
  // The delay. Unit: µs.
  int64_t delay_us;
} sdrkit_dab_tii_entry_row;

// A frequency where the service playing can also be received. The row key is the frequency in Hz.
typedef struct sdrkit_dab_following_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency. A frequency in Hz.
  double freq;
  // The channel name.
  sdrkit_string channel;
  // The ensemble identifier.
  int64_t eid;
} sdrkit_dab_following_row;

// An announcement running. The row key is its cluster.
typedef struct sdrkit_dab_announcement_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The announcement cluster.
  int64_t cluster;
  // The announcement switching flags (ASw).
  int64_t asw;
  // The sub-channel of the announcement.
  int64_t subchannel;
} sdrkit_dab_announcement_row;

// A transmitter identified. The row key is `<main>.<sub>`.
typedef struct sdrkit_dab_transmitter_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The transmitter main identifier.
  int64_t main;
  // The transmitter sub-identifier.
  int64_t sub;
  // The level relative to the strongest transmitter. Unit: dB.
  double level_db;
  // True if `position` holds a value.
  bool has_position;
  // The position, from the transmitter database.
  sdrkit_position position;
  // True if `delay_us` holds a value.
  bool has_delay_us;
  // The delay, from the transmitter database. Unit: µs.
  int64_t delay_us;
} sdrkit_dab_transmitter_row;

// The options of `kind` of `sdrkit_dab_journaline_row`.
typedef enum sdrkit_dab_journaline_row_kind {
  // Menu
  SDRKIT_DAB_JOURNALINE_ROW_KIND_MENU = 0,
  // Plain text
  SDRKIT_DAB_JOURNALINE_ROW_KIND_TEXT = 1,
  // Title only
  SDRKIT_DAB_JOURNALINE_ROW_KIND_TITLE = 2,
  // List
  SDRKIT_DAB_JOURNALINE_ROW_KIND_LIST = 3,
} sdrkit_dab_journaline_row_kind;

// A Journaline object. The row key is its id in hexadecimal.
typedef struct sdrkit_dab_journaline_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The object id.
  int64_t id;
  // The kind of the object. A value of `sdrkit_dab_journaline_row_kind`.
  int32_t kind;
  // The title.
  sdrkit_string title;
  // True if `body` holds a value.
  bool has_body;
  // The body text.
  sdrkit_string body;
} sdrkit_dab_journaline_row;

// A menu entry of a Journaline object. The row key is `<object>/<n>`.
typedef struct sdrkit_dab_journaline_link_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the object.
  sdrkit_string object;
  // The object the entry leads to.
  int64_t target;
  // The text of the entry.
  sdrkit_string text;
} sdrkit_dab_journaline_link_row;

// A list item of a Journaline object. The row key is `<object>/<n>`. Columns are separated by a tab.
typedef struct sdrkit_dab_journaline_item_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the object.
  sdrkit_string object;
  // The text of the item.
  sdrkit_string text;
} sdrkit_dab_journaline_item_row;

// The statistics of a FIG type. The row key is the FIG type ("0/2").
typedef struct sdrkit_dab_fig_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The FIG type.
  sdrkit_string fig;
  // FIGs received.
  int64_t count;
  // Bytes received.
  int64_t bytes;
} sdrkit_dab_fig_type_row;

// The options of `family` of `sdrkit_dab_ensemble_event`.
typedef enum sdrkit_dab_ensemble_event_family {
  // Aircraft
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_ENSEMBLE_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_ensemble_event_family;

// The fields of the event of a complete ensemble.
typedef struct sdrkit_dab_ensemble_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The EId in hexadecimal.
  sdrkit_string source;
  // The ensemble label.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_dab_ensemble_event_family`.
  int32_t family;
} sdrkit_dab_ensemble_event;

// The options of `family` of `sdrkit_dab_services_event`.
typedef enum sdrkit_dab_services_event_family {
  // Aircraft
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_SERVICES_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_services_event_family;

// The fields of the event of a changed service list.
typedef struct sdrkit_dab_services_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The EId in hexadecimal.
  sdrkit_string source;
  // The number of services.
  int64_t count;
  // The family of the sender: broadcast. A value of `sdrkit_dab_services_event_family`.
  int32_t family;
} sdrkit_dab_services_event;

// The options of `urgency` of `sdrkit_dab_announcement_started_event`.
typedef enum sdrkit_dab_announcement_started_event_urgency {
  // None
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_dab_announcement_started_event_urgency;

// The options of `family` of `sdrkit_dab_announcement_started_event`.
typedef enum sdrkit_dab_announcement_started_event_family {
  // Aircraft
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_ANNOUNCEMENT_STARTED_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_announcement_started_event_family;

// The fields of the event of a started announcement.
typedef struct sdrkit_dab_announcement_started_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The EId in hexadecimal.
  sdrkit_string source;
  // The announcement cluster.
  int64_t cluster;
  // The announcement switching flags (ASw).
  int64_t asw;
  // The sub-channel of the announcement.
  int64_t subchannel;
  // The name of the first type of the flags.
  sdrkit_string text;
  // Urgent for an alarm announcement (type 0), else a notice. A value of `sdrkit_dab_announcement_started_event_urgency`.
  int32_t urgency;
  // The family of the sender: broadcast. A value of `sdrkit_dab_announcement_started_event_family`.
  int32_t family;
} sdrkit_dab_announcement_started_event;

// The options of `family` of `sdrkit_dab_announcement_ended_event`.
typedef enum sdrkit_dab_announcement_ended_event_family {
  // Aircraft
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_ANNOUNCEMENT_ENDED_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_announcement_ended_event_family;

// The fields of the event of an ended announcement.
typedef struct sdrkit_dab_announcement_ended_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The EId in hexadecimal.
  sdrkit_string source;
  // The announcement cluster.
  int64_t cluster;
  // The family of the sender: broadcast. A value of `sdrkit_dab_announcement_ended_event_family`.
  int32_t family;
} sdrkit_dab_announcement_ended_event;

// The options of `family` of `sdrkit_dab_sync_lost_event`.
typedef enum sdrkit_dab_sync_lost_event_family {
  // Aircraft
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_SYNC_LOST_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_sync_lost_event_family;

// The fields of the event of a lost lock.
typedef struct sdrkit_dab_sync_lost_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The family of the sender: the receiver. A value of `sdrkit_dab_sync_lost_event_family`.
  int32_t family;
} sdrkit_dab_sync_lost_event;

// The options of `family` of `sdrkit_dab_label_event`.
typedef enum sdrkit_dab_label_event_family {
  // Aircraft
  SDRKIT_DAB_LABEL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_LABEL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_LABEL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_LABEL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_LABEL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_LABEL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_LABEL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_LABEL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_LABEL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_LABEL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_LABEL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_LABEL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_LABEL_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_label_event_family;

// The fields of the event of a new dynamic label.
typedef struct sdrkit_dab_label_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The SId of the service playing, in hexadecimal.
  sdrkit_string source;
  // The label.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_dab_label_event_family`.
  int32_t family;
} sdrkit_dab_label_event;

// The options of `family` of `sdrkit_dab_slide_event`.
typedef enum sdrkit_dab_slide_event_family {
  // Aircraft
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DAB_SLIDE_EVENT_FAMILY_OTHER = 12,
} sdrkit_dab_slide_event_family;

// The fields of the event of a new slide.
typedef struct sdrkit_dab_slide_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The SId of the service playing, in hexadecimal.
  sdrkit_string source;
  // The slide number.
  int64_t slide_sequence;
  // True if `text` holds a value.
  bool has_text;
  // The name of the slide.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_dab_slide_event_family`.
  int32_t family;
} sdrkit_dab_slide_event;

// The arguments of `service.select`.
typedef struct sdrkit_dab_service_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the service in `services`. Empty: the first service that can play.
  sdrkit_string row;
} sdrkit_dab_service_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DAB_SERVICE_SELECT_ARGS_INIT {sizeof(sdrkit_dab_service_select_args), {0}}

// Fills `rows` with the changed rows of the table "services" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `label` names a row.
static inline int32_t sdrkit_dab_services_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dab_service_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.ServiceRow", "services", rows, sizeof(sdrkit_dab_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "subchannels" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_subchannels_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_dab_subchannel_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.SubchannelRow", "subchannels", rows, sizeof(sdrkit_dab_subchannel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "service_components" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `label` names a row.
static inline int32_t sdrkit_dab_service_components_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_dab_service_component_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.ServiceComponentRow", "service_components", rows, sizeof(sdrkit_dab_service_component_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "user_applications" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_dab_user_applications_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_dab_user_application_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.UserApplicationRow", "user_applications", rows, sizeof(sdrkit_dab_user_application_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "service_clusters" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_service_clusters_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_dab_service_cluster_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.ServiceClusterRow", "service_clusters", rows, sizeof(sdrkit_dab_service_cluster_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tii_database" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `position`.
static inline int32_t sdrkit_dab_tii_database_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dab_tii_entry_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.TiiEntryRow", "tii_database", rows, sizeof(sdrkit_dab_tii_entry_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "following" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_following_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dab_following_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.FollowingRow", "following", rows, sizeof(sdrkit_dab_following_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "announcements" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_announcements_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_dab_announcement_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.AnnouncementRow", "announcements", rows, sizeof(sdrkit_dab_announcement_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "transmitters" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `position`.
static inline int32_t sdrkit_dab_transmitters_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dab_transmitter_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.TransmitterRow", "transmitters", rows, sizeof(sdrkit_dab_transmitter_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "journaline" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `title` names a row.
static inline int32_t sdrkit_dab_journaline_rows(SDRKIT_HANDLE handle,
                                                sdrkit_dab_journaline_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.JournalineRow", "journaline", rows, sizeof(sdrkit_dab_journaline_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "journaline_links" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_journaline_links_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_dab_journaline_link_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.JournalineLinkRow", "journaline_links", rows, sizeof(sdrkit_dab_journaline_link_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "journaline_items" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_journaline_items_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_dab_journaline_item_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.JournalineItemRow", "journaline_items", rows, sizeof(sdrkit_dab_journaline_item_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fig_types" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_fig_types_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dab_fig_type_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "dab.FigTypeRow", "fig_types", rows, sizeof(sdrkit_dab_fig_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "label_tags" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_label_tags_rows(SDRKIT_HANDLE handle,
                                                sdrkit_bcdata_label_tag_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.LabelTagRow", "label_tags", rows, sizeof(sdrkit_bcdata_label_tag_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_services" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_guide_services_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_bcdata_guide_service_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideServiceRow", "guide_services", rows, sizeof(sdrkit_bcdata_guide_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_programmes" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_guide_programmes_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_bcdata_guide_programme_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideProgrammeRow", "guide_programmes", rows, sizeof(sdrkit_bcdata_guide_programme_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_groups" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_guide_groups_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_bcdata_guide_group_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideGroupRow", "guide_groups", rows, sizeof(sdrkit_bcdata_guide_group_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slide_categories" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_slide_categories_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_bcdata_slide_category_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.SlideCategoryRow", "slide_categories", rows, sizeof(sdrkit_bcdata_slide_category_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "logos" of the stack "dab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dab_logos_rows(SDRKIT_HANDLE handle,
                                           sdrkit_bcdata_logo_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.LogoRow", "logos", rows, sizeof(sdrkit_bcdata_logo_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.ensemble"
// (Ensemble complete). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_ensemble_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_ensemble_event *event) {
  return sdrkit_typed_event(handle, n, "dab.ensemble", "dab.EnsembleEvent", event, sizeof(sdrkit_dab_ensemble_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.services"
// (Service list changed). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_services_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_services_event *event) {
  return sdrkit_typed_event(handle, n, "dab.services", "dab.ServicesEvent", event, sizeof(sdrkit_dab_services_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.announcement_started"
// (Announcement started). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_announcement_started_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_announcement_started_event *event) {
  return sdrkit_typed_event(handle, n, "dab.announcement_started", "dab.AnnouncementStartedEvent", event, sizeof(sdrkit_dab_announcement_started_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.announcement_ended"
// (Announcement ended). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_announcement_ended_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_announcement_ended_event *event) {
  return sdrkit_typed_event(handle, n, "dab.announcement_ended", "dab.AnnouncementEndedEvent", event, sizeof(sdrkit_dab_announcement_ended_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.sync_lost"
// (Sync lost). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_sync_lost_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_sync_lost_event *event) {
  return sdrkit_typed_event(handle, n, "dab.sync_lost", "dab.SyncLostEvent", event, sizeof(sdrkit_dab_sync_lost_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.label"
// (Dynamic label). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_label_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_label_event *event) {
  return sdrkit_typed_event(handle, n, "dab.label", "dab.LabelEvent", event, sizeof(sdrkit_dab_label_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dab.slide"
// (Slide). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dab_slide_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dab_slide_event *event) {
  return sdrkit_typed_event(handle, n, "dab.slide", "dab.SlideEvent", event, sizeof(sdrkit_dab_slide_event));
}

// Runs the command "service.select" (Select a service) of the stack "dab" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_dab_service_select(SDRKIT_HANDLE handle, const sdrkit_dab_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "dab.ServiceSelectArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dab" are the sdrkit_dab_gen_*() functions
// after the settings of its receiver.

// The setting "detection" (Detection) of the stack "dab". A value of `sdrkit_dab_detection`. Automatic uses the channel estimate when it is reliable. Differential forces plain differential detection.
static inline int32_t sdrkit_dab_get_detection(SDRKIT_HANDLE handle, sdrkit_dab_detection *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "detection", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dab_detection)index;
  return code;
}
static inline int32_t sdrkit_dab_set_detection(SDRKIT_HANDLE handle, sdrkit_dab_detection value) {
  return sdrkit_typed_setting_set_enum(handle, "detection", (int32_t)value);
}

// The setting "centring" (Centre the ensemble) of the stack "dab". Moves an ensemble found off centre to the middle before the stack locks.
static inline int32_t sdrkit_dab_get_centring(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "centring", value);
}
static inline int32_t sdrkit_dab_set_centring(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "centring", value);
}

// The setting "impulse_blanker" (Impulse blanker) of the stack "dab". Sets samples far above the running mean power to zero.
static inline int32_t sdrkit_dab_get_impulse_blanker(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "impulse_blanker", value);
}
static inline int32_t sdrkit_dab_set_impulse_blanker(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "impulse_blanker", value);
}

// The setting "early_start" (Early start) of the stack "dab". DAB+ decodes its first frames from 10 CIFs instead of 16.
static inline int32_t sdrkit_dab_get_early_start(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "early_start", value);
}
static inline int32_t sdrkit_dab_set_early_start(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "early_start", value);
}

// The setting "aac_packaging" (AAC packaging) of the stack "dab". A value of `sdrkit_dab_aac_packaging`. How DAB+ audio is given to the host's decoder.
static inline int32_t sdrkit_dab_get_aac_packaging(SDRKIT_HANDLE handle, sdrkit_dab_aac_packaging *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "aac_packaging", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dab_aac_packaging)index;
  return code;
}
static inline int32_t sdrkit_dab_set_aac_packaging(SDRKIT_HANDLE handle, sdrkit_dab_aac_packaging value) {
  return sdrkit_typed_setting_set_enum(handle, "aac_packaging", (int32_t)value);
}

// The setting "channels" (Channels) of the stack "dab". The centre frequencies in Hz of the channels that service following may tune to, separated by ";". At most 128 entries. A frequency within 50 kHz of a block of EN 300 401 takes the name of the block. Empty: every block of Band III and L-band.
static inline int32_t sdrkit_dab_get_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channels", value);
}
static inline int32_t sdrkit_dab_set_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channels", value);
}

// The setting "data_subchannel" (Data sub-channel) of the stack "dab". A further sub-channel to decode for data (packet mode SPI or Journaline, DMB video). −1 decodes none.
static inline int32_t sdrkit_dab_get_data_subchannel(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "data_subchannel", value);
}
static inline int32_t sdrkit_dab_set_data_subchannel(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "data_subchannel", value);
}

// The setting "tii" (Transmitter identification) of the stack "dab". Analyses the null symbols for transmitter identifiers.
static inline int32_t sdrkit_dab_get_tii(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tii", value);
}
static inline int32_t sdrkit_dab_set_tii(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tii", value);
}

// The setting "announcement_types" (Announcement types) of the stack "dab". The announcement types (a bit mask, TS 101 756 table 14) that apply to the service playing.
static inline int32_t sdrkit_dab_get_announcement_types(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "announcement_types", value);
}
static inline int32_t sdrkit_dab_set_announcement_types(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "announcement_types", value);
}

// The setting "gen_tii" (Transmitter identification) of the test signal generator of the stack "dab". Three transmitters (main 1, sub 1, 3 and 7 at 0, −5 and −11 dB).
static inline int32_t sdrkit_dab_gen_get_tii(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_tii", value);
}
static inline int32_t sdrkit_dab_gen_set_tii(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_tii", value);
}

// The setting "gen_fig2" (Extended labels) of the test signal generator of the stack "dab". Labels as FIG 2 (UTF-8).
static inline int32_t sdrkit_dab_gen_get_fig2(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_fig2", value);
}
static inline int32_t sdrkit_dab_gen_set_fig2(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_fig2", value);
}

// The setting "gen_pty" (Programme type) of the test signal generator of the stack "dab". 0 for none.
static inline int32_t sdrkit_dab_gen_get_pty(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_pty", value);
}
static inline int32_t sdrkit_dab_gen_set_pty(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_pty", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_signal_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_signal_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.SignalFields", fields, sizeof(sdrkit_dab_signal_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_ensemble_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_ensemble_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.EnsembleFields", fields, sizeof(sdrkit_dab_ensemble_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_sync_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_sync_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.SyncFields", fields, sizeof(sdrkit_dab_sync_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_playing_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_playing_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.PlayingFields", fields, sizeof(sdrkit_dab_playing_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_audio_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_audio_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.AudioFields", fields, sizeof(sdrkit_dab_audio_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_pad_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_pad_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.PadFields", fields, sizeof(sdrkit_dab_pad_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_timing_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_timing_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.TimingFields", fields, sizeof(sdrkit_dab_timing_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_announcement_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_announcement_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.AnnouncementFields", fields, sizeof(sdrkit_dab_announcement_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_dmb_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_dmb_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.DmbFields", fields, sizeof(sdrkit_dab_dmb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_data_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_data_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.DataFields", fields, sizeof(sdrkit_dab_data_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dab_health_fields_read(SDRKIT_HANDLE handle, sdrkit_dab_health_fields *fields) {
  return sdrkit_typed_fields(handle, "dab.HealthFields", fields, sizeof(sdrkit_dab_health_fields));
}

// Fills `fields` with the result fields of the component "playout" of the stack "dab" in the snapshot of
// `handle`. A generic consumer reads them under the keys "playout.<key>".
static inline int32_t sdrkit_dab_playout_fields_read(SDRKIT_HANDLE handle, sdrkit_bcdata_playout_fields *fields) {
  return sdrkit_typed_fields(handle, "bcdata.PlayoutFields@playout", fields, sizeof(sdrkit_bcdata_playout_fields));
}

#endif
