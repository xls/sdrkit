/*
 * sdrkit: the typed interface of the stack family "broadcast", version 2.0. MIT licence.
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

#ifndef SDRKIT_BROADCAST_H
#define SDRKIT_BROADCAST_H

#include "../sdrkit.h"
#include "mpegts.h"
#include "bcdata.h"
#include "broadcast_scan.h"

// The version of the family that this header describes.
#define SDRKIT_BROADCAST_VERSION_MAJOR 2
#define SDRKIT_BROADCAST_VERSION_MINOR 0

// The stack "drm": its id, and the keys of its event types.
#define SDRKIT_STACK_DRM "drm"
#define SDRKIT_EVENT_DRM_TEXT "drm.text"
#define SDRKIT_EVENT_DRM_OBJECT "drm.object"

// The stack "hamdrm": its id, and the keys of its event types.
#define SDRKIT_STACK_HAMDRM "hamdrm"

// The stack "hdradio": its id, and the keys of its event types.
#define SDRKIT_STACK_HDRADIO "hdradio"
#define SDRKIT_EVENT_HDR_PSD "hdr.psd"

// The stack "cdr": its id, and the keys of its event types.
#define SDRKIT_STACK_CDR "cdr"

// The stack "cquam": its id, and the keys of its event types.
#define SDRKIT_STACK_CQUAM "cquam"

// The options of the setting "gen_robustness" of the stack "drm".
typedef enum sdrkit_drm_gen_robustness {
  // A
  SDRKIT_DRM_GEN_ROBUSTNESS_A = 0,
  // B
  SDRKIT_DRM_GEN_ROBUSTNESS_B = 1,
  // C
  SDRKIT_DRM_GEN_ROBUSTNESS_C = 2,
  // D
  SDRKIT_DRM_GEN_ROBUSTNESS_D = 3,
  // E
  SDRKIT_DRM_GEN_ROBUSTNESS_E = 4,
} sdrkit_drm_gen_robustness;

// The options of the setting "gen_occupancy" of the stack "drm".
typedef enum sdrkit_drm_gen_occupancy {
  // Usual
  SDRKIT_DRM_GEN_OCCUPANCY_USUAL = 0,
  // 4.5 kHz
  SDRKIT_DRM_GEN_OCCUPANCY_4_5_KHZ = 1,
  // 5 kHz
  SDRKIT_DRM_GEN_OCCUPANCY_5_KHZ = 2,
  // 9 kHz
  SDRKIT_DRM_GEN_OCCUPANCY_9_KHZ = 3,
  // 10 kHz
  SDRKIT_DRM_GEN_OCCUPANCY_10_KHZ = 4,
  // 18 kHz
  SDRKIT_DRM_GEN_OCCUPANCY_18_KHZ = 5,
  // 20 kHz
  SDRKIT_DRM_GEN_OCCUPANCY_20_KHZ = 6,
} sdrkit_drm_gen_occupancy;

// The options of the setting "gen_msc_qam" of the stack "drm".
typedef enum sdrkit_drm_gen_msc_qam {
  // 64-QAM (E: 16-QAM)
  SDRKIT_DRM_GEN_MSC_QAM_HIGH = 0,
  // 16-QAM (E: 4-QAM)
  SDRKIT_DRM_GEN_MSC_QAM_LOW = 1,
} sdrkit_drm_gen_msc_qam;

// The options of the setting "gen_protection_b" of the stack "drm".
typedef enum sdrkit_drm_gen_protection_b {
  // Usual
  SDRKIT_DRM_GEN_PROTECTION_B_USUAL = 0,
  // 0
  SDRKIT_DRM_GEN_PROTECTION_B_0 = 1,
  // 1
  SDRKIT_DRM_GEN_PROTECTION_B_1 = 2,
  // 2
  SDRKIT_DRM_GEN_PROTECTION_B_2 = 3,
  // 3
  SDRKIT_DRM_GEN_PROTECTION_B_3 = 4,
} sdrkit_drm_gen_protection_b;

// The options of the setting "gen_interleaver" of the stack "drm".
typedef enum sdrkit_drm_gen_interleaver {
  // Long
  SDRKIT_DRM_GEN_INTERLEAVER_LONG = 0,
  // Short
  SDRKIT_DRM_GEN_INTERLEAVER_SHORT = 1,
} sdrkit_drm_gen_interleaver;

// The options of the setting "gen_audio" of the stack "drm".
typedef enum sdrkit_drm_gen_audio {
  // Melody
  SDRKIT_DRM_GEN_AUDIO_MELODY = 0,
  // 1 kHz tone
  SDRKIT_DRM_GEN_AUDIO_TONE = 1,
  // Silence
  SDRKIT_DRM_GEN_AUDIO_SILENCE = 2,
  // Noise-like
  SDRKIT_DRM_GEN_AUDIO_NOISE = 3,
  // Not AAC
  SDRKIT_DRM_GEN_AUDIO_NOT_AAC = 4,
} sdrkit_drm_gen_audio;

// The options of the setting "gen_data" of the stack "drm".
typedef enum sdrkit_drm_gen_data {
  // None
  SDRKIT_DRM_GEN_DATA_NONE = 0,
  // MOT slide show
  SDRKIT_DRM_GEN_DATA_MOT_SLIDE_SHOW = 1,
  // MOT website
  SDRKIT_DRM_GEN_DATA_MOT_WEBSITE = 2,
  // TPEG
  SDRKIT_DRM_GEN_DATA_TPEG = 3,
  // EPG
  SDRKIT_DRM_GEN_DATA_EPG = 4,
  // Journaline
  SDRKIT_DRM_GEN_DATA_JOURNALINE = 5,
} sdrkit_drm_gen_data;

// The options of the setting "gen_mode" of the stack "hamdrm".
typedef enum sdrkit_hamdrm_gen_mode {
  // A
  SDRKIT_HAMDRM_GEN_MODE_A = 0,
  // B
  SDRKIT_HAMDRM_GEN_MODE_B = 1,
  // E
  SDRKIT_HAMDRM_GEN_MODE_E = 2,
} sdrkit_hamdrm_gen_mode;

// The options of the setting "gen_occupancy" of the stack "hamdrm".
typedef enum sdrkit_hamdrm_gen_occupancy {
  // 2.3 kHz
  SDRKIT_HAMDRM_GEN_OCCUPANCY_2_3 = 0,
  // 2.5 kHz
  SDRKIT_HAMDRM_GEN_OCCUPANCY_2_5 = 1,
} sdrkit_hamdrm_gen_occupancy;

// The options of the setting "gen_qam" of the stack "hamdrm".
typedef enum sdrkit_hamdrm_gen_qam {
  // 4-QAM
  SDRKIT_HAMDRM_GEN_QAM_4_QAM = 0,
  // 16-QAM
  SDRKIT_HAMDRM_GEN_QAM_16_QAM = 1,
  // 64-QAM
  SDRKIT_HAMDRM_GEN_QAM_64_QAM = 2,
} sdrkit_hamdrm_gen_qam;

// The options of the setting "gen_protection" of the stack "hamdrm".
typedef enum sdrkit_hamdrm_gen_protection {
  // 0
  SDRKIT_HAMDRM_GEN_PROTECTION_0 = 0,
  // 1
  SDRKIT_HAMDRM_GEN_PROTECTION_1 = 1,
} sdrkit_hamdrm_gen_protection;

// The options of the setting "gen_interleave" of the stack "hamdrm".
typedef enum sdrkit_hamdrm_gen_interleave {
  // 400 ms
  SDRKIT_HAMDRM_GEN_INTERLEAVE_SHORT = 0,
  // 2 s
  SDRKIT_HAMDRM_GEN_INTERLEAVE_LONG = 1,
} sdrkit_hamdrm_gen_interleave;

// The options of the setting "gen_waveform" of the stack "hdradio".
typedef enum sdrkit_hdradio_gen_waveform {
  // FM hybrid MP1
  SDRKIT_HDRADIO_GEN_WAVEFORM_MP1 = 0,
  // AM hybrid MA1
  SDRKIT_HDRADIO_GEN_WAVEFORM_MA1 = 1,
  // FM extended hybrid MP3
  SDRKIT_HDRADIO_GEN_WAVEFORM_MP3 = 2,
  // AM all-digital MA3
  SDRKIT_HDRADIO_GEN_WAVEFORM_MA3 = 3,
} sdrkit_hdradio_gen_waveform;

// The options of the setting "gen_tx_mode" of the stack "cdr".
typedef enum sdrkit_cdr_gen_tx_mode {
  // 1
  SDRKIT_CDR_GEN_TX_MODE_1 = 0,
  // 2
  SDRKIT_CDR_GEN_TX_MODE_2 = 1,
  // 3
  SDRKIT_CDR_GEN_TX_MODE_3 = 2,
} sdrkit_cdr_gen_tx_mode;

// The options of the setting "gen_spectrum_mode" of the stack "cdr".
typedef enum sdrkit_cdr_gen_spectrum_mode {
  // 1
  SDRKIT_CDR_GEN_SPECTRUM_MODE_1 = 0,
  // 2
  SDRKIT_CDR_GEN_SPECTRUM_MODE_2 = 1,
  // 9
  SDRKIT_CDR_GEN_SPECTRUM_MODE_9 = 2,
  // 10
  SDRKIT_CDR_GEN_SPECTRUM_MODE_10 = 3,
  // 22
  SDRKIT_CDR_GEN_SPECTRUM_MODE_22 = 4,
  // 23
  SDRKIT_CDR_GEN_SPECTRUM_MODE_23 = 5,
} sdrkit_cdr_gen_spectrum_mode;

// The options of the setting "gen_data_qam" of the stack "cdr".
typedef enum sdrkit_cdr_gen_data_qam {
  // QPSK
  SDRKIT_CDR_GEN_DATA_QAM_QPSK = 0,
  // 16-QAM
  SDRKIT_CDR_GEN_DATA_QAM_16_QAM = 1,
  // 64-QAM
  SDRKIT_CDR_GEN_DATA_QAM_64_QAM = 2,
} sdrkit_cdr_gen_data_qam;

// The options of the setting "gen_ldpc_rate" of the stack "cdr".
typedef enum sdrkit_cdr_gen_ldpc_rate {
  // 1/4
  SDRKIT_CDR_GEN_LDPC_RATE_1_4 = 0,
  // 1/3
  SDRKIT_CDR_GEN_LDPC_RATE_1_3 = 1,
  // 1/2
  SDRKIT_CDR_GEN_LDPC_RATE_1_2 = 2,
  // 3/4
  SDRKIT_CDR_GEN_LDPC_RATE_3_4 = 3,
} sdrkit_cdr_gen_ldpc_rate;

// The options of the setting "gen_sdi_qam" of the stack "cdr".
typedef enum sdrkit_cdr_gen_sdi_qam {
  // QPSK
  SDRKIT_CDR_GEN_SDI_QAM_QPSK = 0,
  // 16-QAM
  SDRKIT_CDR_GEN_SDI_QAM_16_QAM = 1,
  // 64-QAM
  SDRKIT_CDR_GEN_SDI_QAM_64_QAM = 2,
} sdrkit_cdr_gen_sdi_qam;

// The options of the setting "gen_allocation" of the stack "cdr".
typedef enum sdrkit_cdr_gen_allocation {
  // 1
  SDRKIT_CDR_GEN_ALLOCATION_1 = 0,
  // 2
  SDRKIT_CDR_GEN_ALLOCATION_2 = 1,
  // 3
  SDRKIT_CDR_GEN_ALLOCATION_3 = 2,
} sdrkit_cdr_gen_allocation;

// The options of the setting "stereo" of the stack "cquam".
typedef enum sdrkit_cquam_stereo {
  // Auto
  SDRKIT_CQUAM_STEREO_AUTO = 0,
  // Mono
  SDRKIT_CQUAM_STEREO_MONO = 1,
  // Stereo
  SDRKIT_CQUAM_STEREO_STEREO = 2,
} sdrkit_cquam_stereo;

// The options of the setting "gen_kind" of the stack "cquam".
typedef enum sdrkit_cquam_gen_kind {
  // C-QUAM stereo
  SDRKIT_CQUAM_GEN_KIND_STEREO = 0,
  // Plain AM (mono, no pilot)
  SDRKIT_CQUAM_GEN_KIND_AM = 1,
  // Noise only
  SDRKIT_CQUAM_GEN_KIND_NOISE = 2,
} sdrkit_cquam_gen_kind;

// The options of `robustness_mode` of `sdrkit_drm_fields`.
typedef enum sdrkit_drm_fields_robustness_mode {
  // A
  SDRKIT_DRM_FIELDS_ROBUSTNESS_MODE_A = 0,
  // B
  SDRKIT_DRM_FIELDS_ROBUSTNESS_MODE_B = 1,
  // C
  SDRKIT_DRM_FIELDS_ROBUSTNESS_MODE_C = 2,
  // D
  SDRKIT_DRM_FIELDS_ROBUSTNESS_MODE_D = 3,
  // E
  SDRKIT_DRM_FIELDS_ROBUSTNESS_MODE_E = 4,
} sdrkit_drm_fields_robustness_mode;

// The options of `audio_state` of `sdrkit_drm_fields`.
typedef enum sdrkit_drm_fields_audio_state {
  // No audio stream
  SDRKIT_DRM_FIELDS_AUDIO_STATE_NO_AUDIO_STREAM = 0,
  // Decoded by the host
  SDRKIT_DRM_FIELDS_AUDIO_STATE_NO_DECODER = 1,
  // Decoding
  SDRKIT_DRM_FIELDS_AUDIO_STATE_DECODING = 2,
} sdrkit_drm_fields_audio_state;

// The results of the DRM stack. No FAC values before the first FAC, no protection before the multiplex description, no time before SDC type 8, no measurement before the first frame and no first audio before it came.
typedef struct sdrkit_drm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `robustness_mode` holds a value.
  bool has_robustness_mode;
  // The robustness mode, A to E. A value of `sdrkit_drm_fields_robustness_mode`.
  int32_t robustness_mode;
  // True if `occupancy` holds a value.
  bool has_occupancy;
  // The spectrum occupancy code.
  int64_t occupancy;
  // True if `bandwidth` holds a value.
  bool has_bandwidth;
  // The channel bandwidth. A frequency in Hz.
  double bandwidth;
  // True if `interleaver` holds a value.
  bool has_interleaver;
  // The interleaver as text.
  sdrkit_string interleaver;
  // True if `msc` holds a value.
  bool has_msc;
  // The MSC mapping, with the hierarchical flag.
  sdrkit_string msc;
  // True if `sdc` holds a value.
  bool has_sdc;
  // The SDC mapping.
  sdrkit_string sdc;
  // True if `protection` holds a value.
  bool has_protection;
  // The protection levels and their code rates.
  sdrkit_string protection;
  // True if `protection_b` holds a value.
  bool has_protection_b;
  // The protection level of part B.
  int64_t protection_b;
  // True if `time` holds a value.
  bool has_time;
  // The time of SDC type 8 as text.
  sdrkit_string time;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The sample clock error. Unit: ppm.
  double clock_ppm;
  // True if `delay_ms` holds a value.
  bool has_delay_ms;
  // The delay of the strongest path from the symbol start. Unit: ms.
  double delay_ms;
  // True if `guard_correlation` holds a value.
  bool has_guard_correlation;
  // The guard correlation at the symbol timing.
  double guard_correlation;
  // True if `quality` holds a value.
  bool has_quality;
  // The quality, 0 to 1.
  double quality;
  // FAC blocks good.
  int64_t fac_ok;
  // FAC blocks bad.
  int64_t fac_bad;
  // SDC blocks good.
  int64_t sdc_ok;
  // SDC blocks bad.
  int64_t sdc_bad;
  // Multiplex frames good.
  int64_t mux_ok;
  // Multiplex frames bad.
  int64_t mux_bad;
  // Audio frames good.
  int64_t audio_ok;
  // Audio frames bad.
  int64_t audio_bad;
  // Fades held.
  int64_t fades;
  // Re-synchronisations.
  int64_t resyncs;
  // The audio state. A value of `sdrkit_drm_fields_audio_state`.
  int32_t audio_state;
  // What plays, or why nothing can.
  sdrkit_string audio_info;
  // Text segments good.
  int64_t text_ok;
  // Text segments bad.
  int64_t text_bad;
  // Seconds of signal. A length of time in seconds.
  double signal_time;
  // True if `first_audio` holds a value.
  bool has_first_audio;
  // When the first audio frame came. A length of time in seconds.
  double first_audio;
  // Holding through a fade.
  bool holding;
  // The input is mirrored (I and Q swapped).
  bool mirrored;
} sdrkit_drm_fields;

// A service. The row key is its short id.
typedef struct sdrkit_drm_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The short id.
  int64_t short_id;
  // The 24-bit service id, six hex digits.
  sdrkit_string service_id;
  // "audio" or "data".
  sdrkit_string kind;
  // True if `label` holds a value.
  bool has_label;
  // The label, once the SDC gave it.
  sdrkit_string label;
  // The language.
  sdrkit_string language;
  // True if `country` holds a value.
  bool has_country;
  // The country, once the SDC gave it.
  sdrkit_string country;
  // The programme type or application id.
  sdrkit_string programme;
  // Conditional access is flagged.
  bool ca;
  // True if `codec` holds a value.
  bool has_codec;
  // The audio coding as text, once the SDC gave it.
  sdrkit_string codec;
  // True if `core_rate` holds a value.
  bool has_core_rate;
  // The core sampling rate, once the SDC gave it. A frequency in Hz.
  double core_rate;
  // The audio mode code.
  int64_t audio_mode;
  // SBR.
  bool sbr;
  // Text messages carried.
  bool text;
  // The stream id.
  int64_t stream;
  // The net bit rate of the stream. Unit: bit/s.
  double bit_rate_bps;
  // True if `application` holds a value.
  bool has_application;
  // The application name of a data service.
  sdrkit_string application;
  // True if `mode_text` holds a value.
  bool has_mode_text;
  // The mode text of a data service.
  sdrkit_string mode_text;
  // Whether the service is decoded.
  bool decoded;
} sdrkit_drm_service_row;

// The bytes per logical frame of a stream. The row key is the stream number.
typedef struct sdrkit_drm_stream_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The bytes of part A. Unit: B.
  int64_t part_a;
  // The bytes of part B. Unit: B.
  int64_t part_b;
} sdrkit_drm_stream_row;

// A MOT object of a data service. The row key is `<short id>:<transport id>`.
typedef struct sdrkit_drm_object_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The short id of the service.
  int64_t short_id;
  // The MOT transport id.
  int64_t transport_id;
  // The name, or the transport id for an object without one.
  sdrkit_string name;
  // The content type.
  sdrkit_string content_type;
  // The size in bytes.
  int64_t size_;
  // Times received.
  int64_t received;
  // The order of arrival.
  int64_t order;
  // True if `note` holds a value.
  bool has_note;
  // What the object is, where the stack can say.
  sdrkit_string note;
} sdrkit_drm_object_row;

// The latest slide of a service. The row key is the short id.
typedef struct sdrkit_drm_slide_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The short id of the service.
  int64_t short_id;
  // The order number of the slide.
  int64_t order;
  // The name.
  sdrkit_string name;
  // True if `category` holds a value.
  bool has_category;
  // The category title.
  sdrkit_string category;
  // True if `category_id` holds a value.
  bool has_category_id;
  // The category id.
  int64_t category_id;
  // True if `slide_id` holds a value.
  bool has_slide_id;
  // The slide id.
  int64_t slide_id;
  // True if `click_url` holds a value.
  bool has_click_url;
  // The click-through URL.
  sdrkit_string click_url;
  // True if `alt_url` holds a value.
  bool has_alt_url;
  // The alternative location URL.
  sdrkit_string alt_url;
  // True if `trigger` holds a value.
  bool has_trigger;
  // The trigger time as text.
  sdrkit_string trigger;
  // The content type.
  sdrkit_string content_type;
  // The size in bytes.
  int64_t size_;
  // The width of the picture.
  int64_t width;
  // The height of the picture.
  int64_t height;
} sdrkit_drm_slide_row;

// A Journaline object. The row key is its id.
typedef struct sdrkit_drm_page_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The object id.
  int64_t id;
  // The object type.
  sdrkit_string kind;
  // The title.
  sdrkit_string title;
  // The content: links (id and text), body or list items, one per line.
  sdrkit_string content;
} sdrkit_drm_page_row;

// The packet counters of a packet-mode service. The row key is its short id.
typedef struct sdrkit_drm_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The short id of the service.
  int64_t short_id;
  // Packets.
  int64_t packets;
  // Packets with a bad CRC.
  int64_t bad_crc;
  // Packets lost.
  int64_t lost;
  // Continuation packets without their first.
  int64_t orphans;
  // Padding packets.
  int64_t empty;
  // Packets of another packet id.
  int64_t other_id;
  // Data units complete.
  int64_t units_ok;
  // Data units dropped.
  int64_t units_dropped;
  // Bytes of the data units.
  int64_t unit_bytes;
  // TPEG frames counted.
  int64_t tpeg_frames;
} sdrkit_drm_packet_row;

// A note on what the receiver cannot decode. The row key is its order.
typedef struct sdrkit_drm_note_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The note.
  sdrkit_string note;
} sdrkit_drm_note_row;

// The options of `mode` of `sdrkit_hamdrm_fields`.
typedef enum sdrkit_hamdrm_fields_mode {
  // A
  SDRKIT_HAMDRM_FIELDS_MODE_A = 0,
  // B
  SDRKIT_HAMDRM_FIELDS_MODE_B = 1,
  // E
  SDRKIT_HAMDRM_FIELDS_MODE_E = 2,
} sdrkit_hamdrm_fields_mode;

// The options of `qam` of `sdrkit_hamdrm_fields`.
typedef enum sdrkit_hamdrm_fields_qam {
  // 4-QAM
  SDRKIT_HAMDRM_FIELDS_QAM_4_QAM = 0,
  // 16-QAM
  SDRKIT_HAMDRM_FIELDS_QAM_16_QAM = 1,
  // 64-QAM
  SDRKIT_HAMDRM_FIELDS_QAM_64_QAM = 2,
} sdrkit_hamdrm_fields_qam;

// The options of `interleave` of `sdrkit_hamdrm_fields`.
typedef enum sdrkit_hamdrm_fields_interleave {
  // 400 ms
  SDRKIT_HAMDRM_FIELDS_INTERLEAVE_SHORT = 0,
  // 2 s
  SDRKIT_HAMDRM_FIELDS_INTERLEAVE_LONG = 1,
} sdrkit_hamdrm_fields_interleave;

// The results of the HamDRM stack. No mode, occupancy, constellation, protection, interleaver, label, SNR or offset before the first frame, and no file before its first segment.
typedef struct sdrkit_hamdrm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the format is loaded, and from what (or why not).
  sdrkit_string format;
  // True if `mode` holds a value.
  bool has_mode;
  // The robustness mode, A, B or E. A value of `sdrkit_hamdrm_fields_mode`.
  int32_t mode;
  // True if `occupancy` holds a value.
  bool has_occupancy;
  // The occupancy. A frequency in Hz.
  double occupancy;
  // True if `qam` holds a value.
  bool has_qam;
  // The constellation. A value of `sdrkit_hamdrm_fields_qam`.
  int32_t qam;
  // True if `protection` holds a value.
  bool has_protection;
  // The protection level, 0 or 1.
  int64_t protection;
  // True if `interleave` holds a value.
  bool has_interleave;
  // The interleaver. A value of `sdrkit_hamdrm_fields_interleave`.
  int32_t interleave;
  // True if `label` holds a value.
  bool has_label;
  // The label of the FAC (the station's call sign).
  sdrkit_string label;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last frame. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame. A frequency in Hz.
  double offset_hz;
  // Frames good.
  int64_t frames_ok;
  // Frames bad.
  int64_t frames_bad;
  // Files completed.
  int64_t files_completed;
  // True if `file_id` holds a value.
  bool has_file_id;
  // The transport id of the file being received.
  int64_t file_id;
  // True if `file_name` holds a value.
  bool has_file_name;
  // Its name.
  sdrkit_string file_name;
  // True if `file_call` holds a value.
  bool has_file_call;
  // Its call sign.
  sdrkit_string file_call;
  // True if `file_size` holds a value.
  bool has_file_size;
  // Its body size.
  int64_t file_size;
  // True if `segments_total` holds a value.
  bool has_segments_total;
  // Its segments in total, once the last segment number is known.
  int64_t segments_total;
  // True if `segments_received` holds a value.
  bool has_segments_received;
  // Its segments received.
  int64_t segments_received;
  // True if `transmissions` holds a value.
  bool has_transmissions;
  // The transmissions heard.
  int64_t transmissions;
  // True if `done` holds a value.
  bool has_done;
  // Whether it is complete.
  bool done;
} sdrkit_hamdrm_fields;

// A finished file. The row key is its transport id.
typedef struct sdrkit_hamdrm_file_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The transport id.
  int64_t id;
  // The name.
  sdrkit_string name;
  // The call sign of the sender.
  sdrkit_string call;
  // The content type.
  sdrkit_string mime;
  // The body size.
  int64_t size_;
  // The transmissions heard.
  int64_t transmissions;
  // When it was complete, in signal time. A length of time in seconds.
  double completed;
  // The CRC-32 of the body.
  int64_t crc32;
} sdrkit_hamdrm_file_row;

// A missing body segment of the file being received. The row key is the segment number.
typedef struct sdrkit_hamdrm_missing_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of the segment.
  int64_t segment;
} sdrkit_hamdrm_missing_row;

// The options of `band` of `sdrkit_hdradio_fields`.
typedef enum sdrkit_hdradio_fields_band {
  // FM hybrid
  SDRKIT_HDRADIO_FIELDS_BAND_FM_HYBRID = 0,
  // AM hybrid
  SDRKIT_HDRADIO_FIELDS_BAND_AM_HYBRID = 1,
  // AM all-digital
  SDRKIT_HDRADIO_FIELDS_BAND_AM_ALL_DIGITAL = 2,
} sdrkit_hdradio_fields_band;

// The options of `service_mode` of `sdrkit_hdradio_fields`.
typedef enum sdrkit_hdradio_fields_service_mode {
  // MP1
  SDRKIT_HDRADIO_FIELDS_SERVICE_MODE_MP1 = 0,
  // MP3
  SDRKIT_HDRADIO_FIELDS_SERVICE_MODE_MP3 = 1,
  // MA1
  SDRKIT_HDRADIO_FIELDS_SERVICE_MODE_MA1 = 2,
  // MA3
  SDRKIT_HDRADIO_FIELDS_SERVICE_MODE_MA3 = 3,
} sdrkit_hdradio_fields_service_mode;

// The results of the HD Radio stack. No band or mode before the first frame, no measurement while the receiver searches and no station data before the PIDS carry it.
typedef struct sdrkit_hdradio_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `band` holds a value.
  bool has_band;
  // The kind of signal. A value of `sdrkit_hdradio_fields_band`.
  int32_t band;
  // True if `service_mode` holds a value.
  bool has_service_mode;
  // The service mode. A value of `sdrkit_hdradio_fields_service_mode`.
  int32_t service_mode;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // True if `mer_lower_db` holds a value.
  bool has_mer_lower_db;
  // The MER of the lower sideband (AM: the primary). Unit: dB.
  double mer_lower_db;
  // True if `mer_upper_db` holds a value.
  bool has_mer_upper_db;
  // The MER of the upper sideband (AM: the secondary). Unit: dB.
  double mer_upper_db;
  // True if `ber` holds a value.
  bool has_ber;
  // The bit error rate of the P1 channel.
  double ber;
  // Blocks.
  int64_t blocks;
  // Block syncs.
  int64_t block_syncs;
  // P1 frames good.
  int64_t p1_ok;
  // P1 frames bad.
  int64_t p1_bad;
  // P3 frames good.
  int64_t p3_ok;
  // P3 frames bad.
  int64_t p3_bad;
  // PIDS frames good.
  int64_t pids_ok;
  // PIDS frames bad.
  int64_t pids_bad;
  // Audio PDUs.
  int64_t pdus;
  // AAS packets good.
  int64_t aas_ok;
  // AAS packets bad.
  int64_t aas_bad;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level. Unit: dBFS.
  double level_dbfs;
  // Signal time. A length of time in seconds.
  double signal_time;
  // True if `codec_note` holds a value.
  bool has_codec_note;
  // What the audio codec needs, or why it cannot play.
  sdrkit_string codec_note;
  // P1 frames arrive.
  bool p1_on;
  // PIDS frames arrive.
  bool pids_on;
  // Song data seen.
  bool psd_on;
  // Data seen.
  bool data_on;
  // The row key of the program to hear in [`PROGRAMS`].
  sdrkit_string program;
  // True if `call_sign` holds a value.
  bool has_call_sign;
  // The call sign.
  sdrkit_string call_sign;
  // True if `universal_name` holds a value.
  bool has_universal_name;
  // The universal name.
  sdrkit_string universal_name;
  // True if `long_name` holds a value.
  bool has_long_name;
  // The long name.
  sdrkit_string long_name;
  // True if `slogan` holds a value.
  bool has_slogan;
  // The slogan.
  sdrkit_string slogan;
  // True if `message` holds a value.
  bool has_message;
  // The station message.
  sdrkit_string message;
  // True if `country` holds a value.
  bool has_country;
  // The country.
  sdrkit_string country;
  // True if `facility_id` holds a value.
  bool has_facility_id;
  // The facility id.
  int64_t facility_id;
  // True if `location` holds a value.
  bool has_location;
  // The location of the station.
  sdrkit_position location;
  // True if `altitude_m` holds a value.
  bool has_altitude_m;
  // The altitude of the station. Unit: m.
  double altitude_m;
} sdrkit_hdradio_fields;

// An audio program. The row key is "HD1" to "HD8".
typedef struct sdrkit_hdradio_program_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The program number, 0 for HD1.
  int64_t number;
  // "HD1" to "HD8".
  sdrkit_string label;
  // True if `name` holds a value.
  bool has_name;
  // The name from the SIG.
  sdrkit_string name;
  // The program type code.
  int64_t program_type;
  // True if `type_name` holds a value.
  bool has_type_name;
  // The name of the program type.
  sdrkit_string type_name;
  // Restricted access.
  bool restricted;
  // True if `codec_mode` holds a value.
  bool has_codec_mode;
  // The codec mode.
  int64_t codec_mode;
  // True if `blend` holds a value.
  bool has_blend;
  // The blend control.
  int64_t blend;
  // The sound experience code.
  int64_t sound_experience;
  // The bit rate. Unit: bit/s.
  double bit_rate_bps;
  // Packets good.
  int64_t packets_ok;
  // Packets bad.
  int64_t packets_bad;
  // Listed in the SIS.
  bool in_sis;
  // Listed in the SIG.
  bool in_sig;
  // On air.
  bool broadcasting;
  // Song data messages.
  int64_t psd_count;
  // True if `title` holds a value.
  bool has_title;
  // The song title.
  sdrkit_string title;
  // True if `artist` holds a value.
  bool has_artist;
  // The artist.
  sdrkit_string artist;
  // True if `album` holds a value.
  bool has_album;
  // The album.
  sdrkit_string album;
  // True if `genre` holds a value.
  bool has_genre;
  // The genre.
  sdrkit_string genre;
  // True if `comment` holds a value.
  bool has_comment;
  // The comment.
  sdrkit_string comment;
  // True if `ufid` holds a value.
  bool has_ufid;
  // The unique file identifier.
  sdrkit_string ufid;
  // True if `commercial` holds a value.
  bool has_commercial;
  // The commercial frame, its parts separated by "; ".
  sdrkit_string commercial;
  // True if `art_lot` holds a value.
  bool has_art_lot;
  // The LOT number of the album art.
  int64_t art_lot;
  // True if `art_mime` holds a value.
  bool has_art_mime;
  // The type of the album art.
  sdrkit_string art_mime;
  // True if `logo_port` holds a value.
  bool has_logo_port;
  // The port of the station logo.
  int64_t logo_port;
  // True if `art_port` holds a value.
  bool has_art_port;
  // The port of the album art.
  int64_t art_port;
} sdrkit_hdradio_program_row;

// A data service component. The row key is its port.
typedef struct sdrkit_hdradio_data_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of the service.
  sdrkit_string name;
  // The service number.
  int64_t service;
  // The port.
  int64_t port;
  // The component type.
  int64_t kind;
  // The content type.
  sdrkit_string mime;
  // Packets.
  int64_t packets;
  // Bytes.
  int64_t bytes;
} sdrkit_hdradio_data_service_row;

// A file of the LOT transport. The row key is `<port>:<lot>`.
typedef struct sdrkit_hdradio_file_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The port.
  int64_t port;
  // The LOT number.
  int64_t lot;
  // The file name.
  sdrkit_string name;
  // The content type.
  sdrkit_string mime;
  // The size in bytes.
  int64_t size_;
  // Bytes received.
  int64_t received;
  // The file is complete.
  bool complete;
  // The version.
  int64_t version;
  // The expiry date. Unit: MJD.
  int64_t expiry_mjd;
  // When it last changed, in signal time. A length of time in seconds.
  double updated;
} sdrkit_hdradio_file_row;

// The results of the CDR stack. No transmission mode, spectrum or LDPC rate before the system information, no offset or SNR before the first sub-frame, no level before the first report of the decoder, no iterations before the first code word and no network before the network information table.
typedef struct sdrkit_cdr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the GY/T 268.1 tables are loaded ("loaded"), or what is missing.
  sdrkit_string tables;
  // True if `tx_mode` holds a value.
  bool has_tx_mode;
  // The transmission mode (1 to 3).
  int64_t tx_mode;
  // True if `spectrum_mode` holds a value.
  bool has_spectrum_mode;
  // The spectrum mode.
  int64_t spectrum_mode;
  // True if `spectrum` holds a value.
  bool has_spectrum;
  // The spectrum mode in words.
  sdrkit_string spectrum;
  // True if `ldpc_rate` holds a value.
  bool has_ldpc_rate;
  // The LDPC rate of the service data ("3/4").
  sdrkit_string ldpc_rate;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR (Es/N0). Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level. Unit: dBFS.
  double level_dbfs;
  // Sub-frames received.
  int64_t subframes;
  // System information good.
  int64_t si_ok;
  // System information bad.
  int64_t si_bad;
  // LDPC code words good.
  int64_t cw_ok;
  // LDPC code words bad.
  int64_t cw_bad;
  // True if `iterations` holds a value.
  bool has_iterations;
  // Mean LDPC iterations.
  double iterations;
  // Control multiplex frames good.
  int64_t control_ok;
  // Control multiplex frames bad.
  int64_t control_bad;
  // Service multiplex frames good.
  int64_t service_ok;
  // Service multiplex frames bad.
  int64_t service_bad;
  // True if `network` holds a value.
  bool has_network;
  // The network name.
  sdrkit_string network;
  // True if `country` holds a value.
  bool has_country;
  // The country of the network.
  sdrkit_string country;
  // True if `network_id` holds a value.
  bool has_network_id;
  // The network id.
  int64_t network_id;
  // True if `selected_service` holds a value.
  bool has_selected_service;
  // The row key of the service playing in [`SERVICES`].
  sdrkit_string selected_service;
  // DRA+ units of the service listened to.
  int64_t units_in;
} sdrkit_cdr_fields;

// A service of the multiplex. The row key is the service id in hexadecimal.
typedef struct sdrkit_cdr_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The service id.
  int64_t sid;
  // Audio units seen.
  bool audio;
  // Data units seen.
  bool data;
  // True if `sample_rate` holds a value.
  bool has_sample_rate;
  // The sample rate of the first audio stream. Unit: Hz.
  int64_t sample_rate;
  // True if `channels` holds a value.
  bool has_channels;
  // Channels of the first audio stream.
  int64_t channels;
  // True if `language` holds a value.
  bool has_language;
  // The language of the first audio stream.
  sdrkit_string language;
  // True if `algorithm` holds a value.
  bool has_algorithm;
  // The audio algorithm code of the first stream.
  int64_t algorithm;
  // The share of the multiplex. Unit: kbit/s.
  double kbps;
  // Audio units in the last sub-frame.
  int64_t units;
  // True if `text` holds a value.
  bool has_text;
  // The last data unit, when it is printable text.
  sdrkit_string text;
  // Sub-frame headers good.
  int64_t good;
  // Sub-frame headers or units bad.
  int64_t bad;
} sdrkit_cdr_service_row;

// A data unit type of a service. The row key is `<service>/<type>`.
typedef struct sdrkit_cdr_data_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the service.
  sdrkit_string service;
  // The data unit type.
  int64_t data_type;
} sdrkit_cdr_data_type_row;

// A frequency of the network information table. The row key is its place in the table.
typedef struct sdrkit_cdr_frequency_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency. A frequency in Hz.
  double freq_hz;
} sdrkit_cdr_frequency_row;

// The options of `stereo_mode` of `sdrkit_cquam_fields`.
typedef enum sdrkit_cquam_fields_stereo_mode {
  // Auto
  SDRKIT_CQUAM_FIELDS_STEREO_MODE_AUTO = 0,
  // Mono
  SDRKIT_CQUAM_FIELDS_STEREO_MODE_MONO = 1,
  // Stereo
  SDRKIT_CQUAM_FIELDS_STEREO_MODE_STEREO = 2,
} sdrkit_cquam_fields_stereo_mode;

// The results of the C-QUAM stack that the stereo matrix writes. No offset, carrier level, C/N or lock quality without a carrier, no pilot level before the first pilot check, no levels before the first audio, no width in mono and no noise floor before the first noise measure.
typedef struct sdrkit_cquam_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // True if `cn_db` holds a value.
  bool has_cn_db;
  // Carrier to noise in the channel. Unit: dB.
  double cn_db;
  // True if `carrier_dbfs` holds a value.
  bool has_carrier_dbfs;
  // The carrier level. Unit: dBFS.
  double carrier_dbfs;
  // Stereo now.
  bool stereo;
  // The 25 Hz pilot is seen (also with mono forced).
  bool pilot_seen;
  // True if `pilot_db` holds a value.
  bool has_pilot_db;
  // The pilot over its neighbours. Unit: dB.
  double pilot_db;
  // True if `pilot_pct` holds a value.
  bool has_pilot_pct;
  // The pilot as a percentage of the carrier. Unit: %.
  double pilot_pct;
  // True if `lock_quality` holds a value.
  bool has_lock_quality;
  // The mean cos φ of the stereo phase (1 for a mono station).
  double lock_quality;
  // True if `left_dbfs` holds a value.
  bool has_left_dbfs;
  // The left level, rms over 100 ms. Unit: dBFS.
  double left_dbfs;
  // True if `right_dbfs` holds a value.
  bool has_right_dbfs;
  // The right level, rms over 100 ms. Unit: dBFS.
  double right_dbfs;
  // True if `width_db` holds a value.
  bool has_width_db;
  // L - R over L + R. Unit: dB.
  double width_db;
  // The peak modulation of L + R in the last second. Unit: %.
  double mod_peak_pct;
  // The stereo mode setting. A value of `sdrkit_cquam_fields_stereo_mode`.
  int32_t stereo_mode;
  // The audio filter width either side of the carrier. A frequency in Hz.
  double filter_width;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // The noise floor in the audio width. Unit: dBFS.
  double noise_dbfs;
  // Pilot checks that found the pilot.
  int64_t pilot_checks_ok;
  // Pilot checks without it.
  int64_t pilot_checks_bad;
  // Signal time. A length of time in seconds.
  double signal_time;
} sdrkit_cquam_fields;

// The input rate, which the front end of the C-QUAM stack writes.
typedef struct sdrkit_cquam_front_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The input rate. A frequency in Hz.
  double input_rate;
} sdrkit_cquam_front_fields;

// The options of `family` of `sdrkit_drm_text_event`.
typedef enum sdrkit_drm_text_event_family {
  // Aircraft
  SDRKIT_DRM_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DRM_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DRM_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DRM_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DRM_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DRM_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DRM_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DRM_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DRM_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DRM_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DRM_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DRM_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DRM_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_drm_text_event_family;

// The fields of the event of a new text message.
typedef struct sdrkit_drm_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text message.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_drm_text_event_family`.
  int32_t family;
} sdrkit_drm_text_event;

// The options of `family` of `sdrkit_drm_object_event`.
typedef enum sdrkit_drm_object_event_family {
  // Aircraft
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DRM_OBJECT_EVENT_FAMILY_OTHER = 12,
} sdrkit_drm_object_event_family;

// The fields of the event of a received MOT object.
typedef struct sdrkit_drm_object_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The MOT transport id.
  int64_t transport_id;
  // The name of the object.
  sdrkit_string text;
  // The content type.
  sdrkit_string content_type;
  // The size in bytes.
  int64_t size_;
  // The family of the sender: broadcast. A value of `sdrkit_drm_object_event_family`.
  int32_t family;
} sdrkit_drm_object_event;

// The options of `family` of `sdrkit_hdradio_hdr_psd_event`.
typedef enum sdrkit_hdradio_hdr_psd_event_family {
  // Aircraft
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HDRADIO_HDR_PSD_EVENT_FAMILY_OTHER = 12,
} sdrkit_hdradio_hdr_psd_event_family;

// The fields of the song event.
typedef struct sdrkit_hdradio_hdr_psd_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The program ("HD1").
  sdrkit_string channel;
  // The song title.
  sdrkit_string title;
  // The artist.
  sdrkit_string artist;
  // "artist - title", or the one of them that is there.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_hdradio_hdr_psd_event_family`.
  int32_t family;
} sdrkit_hdradio_hdr_psd_event;

// The arguments of `service.select`.
typedef struct sdrkit_broadcast_service_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the service in `services`. Empty: the first service that can play.
  sdrkit_string row;
} sdrkit_broadcast_service_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_BROADCAST_SERVICE_SELECT_ARGS_INIT {sizeof(sdrkit_broadcast_service_select_args), {0}}

// Fills `rows` with the changed rows of the table "services" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `label` names a row.
static inline int32_t sdrkit_drm_services_rows(SDRKIT_HANDLE handle,
                                              sdrkit_drm_service_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmServiceRow", "services", rows, sizeof(sdrkit_drm_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_streams_rows(SDRKIT_HANDLE handle,
                                             sdrkit_drm_stream_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmStreamRow", "streams", rows, sizeof(sdrkit_drm_stream_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "objects" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_drm_objects_rows(SDRKIT_HANDLE handle,
                                             sdrkit_drm_object_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmObjectRow", "objects", rows, sizeof(sdrkit_drm_object_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slides" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_drm_slides_rows(SDRKIT_HANDLE handle,
                                            sdrkit_drm_slide_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmSlideRow", "slides", rows, sizeof(sdrkit_drm_slide_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "journaline" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `title` names a row.
static inline int32_t sdrkit_drm_journaline_rows(SDRKIT_HANDLE handle,
                                                sdrkit_drm_page_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmPageRow", "journaline", rows, sizeof(sdrkit_drm_page_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_packets_rows(SDRKIT_HANDLE handle,
                                             sdrkit_drm_packet_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmPacketRow", "packets", rows, sizeof(sdrkit_drm_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "notes" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_notes_rows(SDRKIT_HANDLE handle,
                                           sdrkit_drm_note_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.DrmNoteRow", "notes", rows, sizeof(sdrkit_drm_note_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "label_tags" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_label_tags_rows(SDRKIT_HANDLE handle,
                                                sdrkit_bcdata_label_tag_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.LabelTagRow", "label_tags", rows, sizeof(sdrkit_bcdata_label_tag_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_services" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_guide_services_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_bcdata_guide_service_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideServiceRow", "guide_services", rows, sizeof(sdrkit_bcdata_guide_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_programmes" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_guide_programmes_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_bcdata_guide_programme_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideProgrammeRow", "guide_programmes", rows, sizeof(sdrkit_bcdata_guide_programme_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_groups" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_guide_groups_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_bcdata_guide_group_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideGroupRow", "guide_groups", rows, sizeof(sdrkit_bcdata_guide_group_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slide_categories" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_slide_categories_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_bcdata_slide_category_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.SlideCategoryRow", "slide_categories", rows, sizeof(sdrkit_bcdata_slide_category_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "logos" of the stack "drm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_drm_logos_rows(SDRKIT_HANDLE handle,
                                           sdrkit_bcdata_logo_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.LogoRow", "logos", rows, sizeof(sdrkit_bcdata_logo_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "drm.text"
// (Text message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_drm_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_drm_text_event *event) {
  return sdrkit_typed_event(handle, n, "drm.text", "broadcast.DrmTextEvent", event, sizeof(sdrkit_drm_text_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "drm.object"
// (Object received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_drm_object_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_drm_object_event *event) {
  return sdrkit_typed_event(handle, n, "drm.object", "broadcast.DrmObjectEvent", event, sizeof(sdrkit_drm_object_event));
}

// Runs the command "service.select" (Select a service) of the stack "drm" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_drm_service_select(SDRKIT_HANDLE handle, const sdrkit_broadcast_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "broadcast.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "drm" on the session of `handle`. Forgets the signal and starts again.
static inline int32_t sdrkit_drm_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "drm" are the sdrkit_drm_gen_*() functions
// after the settings of its receiver.

// The setting "gen_robustness" (Robustness mode) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_robustness`.
static inline int32_t sdrkit_drm_gen_get_robustness(SDRKIT_HANDLE handle, sdrkit_drm_gen_robustness *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_robustness", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_robustness)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_robustness(SDRKIT_HANDLE handle, sdrkit_drm_gen_robustness value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_robustness", (int32_t)value);
}

// The setting "gen_occupancy" (Occupancy) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_occupancy`.
static inline int32_t sdrkit_drm_gen_get_occupancy(SDRKIT_HANDLE handle, sdrkit_drm_gen_occupancy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_occupancy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_occupancy)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_occupancy(SDRKIT_HANDLE handle, sdrkit_drm_gen_occupancy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_occupancy", (int32_t)value);
}

// The setting "gen_msc_qam" (MSC) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_msc_qam`.
static inline int32_t sdrkit_drm_gen_get_msc_qam(SDRKIT_HANDLE handle, sdrkit_drm_gen_msc_qam *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_msc_qam", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_msc_qam)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_msc_qam(SDRKIT_HANDLE handle, sdrkit_drm_gen_msc_qam value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_msc_qam", (int32_t)value);
}

// The setting "gen_protection_b" (Protection) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_protection_b`.
static inline int32_t sdrkit_drm_gen_get_protection_b(SDRKIT_HANDLE handle, sdrkit_drm_gen_protection_b *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_protection_b", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_protection_b)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_protection_b(SDRKIT_HANDLE handle, sdrkit_drm_gen_protection_b value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_protection_b", (int32_t)value);
}

// The setting "gen_interleaver" (Interleaver) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_interleaver`.
static inline int32_t sdrkit_drm_gen_get_interleaver(SDRKIT_HANDLE handle, sdrkit_drm_gen_interleaver *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_interleaver", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_interleaver)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_interleaver(SDRKIT_HANDLE handle, sdrkit_drm_gen_interleaver value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_interleaver", (int32_t)value);
}

// The setting "gen_audio" (Audio) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_audio`.
static inline int32_t sdrkit_drm_gen_get_audio(SDRKIT_HANDLE handle, sdrkit_drm_gen_audio *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_audio", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_audio)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_audio(SDRKIT_HANDLE handle, sdrkit_drm_gen_audio value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_audio", (int32_t)value);
}

// The setting "gen_text_message" (Text message) of the test signal generator of the stack "drm".
static inline int32_t sdrkit_drm_gen_get_text_message(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_text_message", value);
}
static inline int32_t sdrkit_drm_gen_set_text_message(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_text_message", value);
}

// The setting "gen_data" (Data service) of the test signal generator of the stack "drm". A value of `sdrkit_drm_gen_data`.
static inline int32_t sdrkit_drm_gen_get_data(SDRKIT_HANDLE handle, sdrkit_drm_gen_data *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_data", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_drm_gen_data)index;
  return code;
}
static inline int32_t sdrkit_drm_gen_set_data(SDRKIT_HANDLE handle, sdrkit_drm_gen_data value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_data", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "history" of the stack "hamdrm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_hamdrm_history_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hamdrm_file_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.HamdrmFileRow", "history", rows, sizeof(sdrkit_hamdrm_file_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "missing" of the stack "hamdrm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hamdrm_missing_rows(SDRKIT_HANDLE handle,
                                                sdrkit_hamdrm_missing_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.HamdrmMissingRow", "missing", rows, sizeof(sdrkit_hamdrm_missing_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "hamdrm" on the session of `handle`. Forgets the signal and starts again.
static inline int32_t sdrkit_hamdrm_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hamdrm" are the sdrkit_hamdrm_gen_*() functions
// after the settings of its receiver.

// The setting "hamdrm_format" (HamDRM format) of the stack "hamdrm". A JSON file with the parts of the HamDRM format that no document fixes. Without it nothing is decoded.
static inline int32_t sdrkit_hamdrm_get_hamdrm_format(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hamdrm_format", value);
}
static inline int32_t sdrkit_hamdrm_set_hamdrm_format(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hamdrm_format", value);
}

// The setting "gen_mode" (Mode) of the test signal generator of the stack "hamdrm". A value of `sdrkit_hamdrm_gen_mode`.
static inline int32_t sdrkit_hamdrm_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hamdrm_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_hamdrm_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_occupancy" (Occupancy) of the test signal generator of the stack "hamdrm". A value of `sdrkit_hamdrm_gen_occupancy`.
static inline int32_t sdrkit_hamdrm_gen_get_occupancy(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_occupancy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_occupancy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hamdrm_gen_occupancy)index;
  return code;
}
static inline int32_t sdrkit_hamdrm_gen_set_occupancy(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_occupancy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_occupancy", (int32_t)value);
}

// The setting "gen_qam" (QAM) of the test signal generator of the stack "hamdrm". A value of `sdrkit_hamdrm_gen_qam`.
static inline int32_t sdrkit_hamdrm_gen_get_qam(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_qam *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_qam", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hamdrm_gen_qam)index;
  return code;
}
static inline int32_t sdrkit_hamdrm_gen_set_qam(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_qam value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_qam", (int32_t)value);
}

// The setting "gen_protection" (Protection) of the test signal generator of the stack "hamdrm". A value of `sdrkit_hamdrm_gen_protection`. 4-QAM has protection 1 only.
static inline int32_t sdrkit_hamdrm_gen_get_protection(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_protection *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_protection", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hamdrm_gen_protection)index;
  return code;
}
static inline int32_t sdrkit_hamdrm_gen_set_protection(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_protection value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_protection", (int32_t)value);
}

// The setting "gen_interleave" (Interleaver) of the test signal generator of the stack "hamdrm". A value of `sdrkit_hamdrm_gen_interleave`.
static inline int32_t sdrkit_hamdrm_gen_get_interleave(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_interleave *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_interleave", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hamdrm_gen_interleave)index;
  return code;
}
static inline int32_t sdrkit_hamdrm_gen_set_interleave(SDRKIT_HANDLE handle, sdrkit_hamdrm_gen_interleave value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_interleave", (int32_t)value);
}

// The setting "gen_skip_every" (Leave out every n-th body segment) of the test signal generator of the stack "hamdrm".
static inline int32_t sdrkit_hamdrm_gen_get_skip_every(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_skip_every", value);
}
static inline int32_t sdrkit_hamdrm_gen_set_skip_every(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_skip_every", value);
}

// The setting "gen_transmissions" (Transmissions per round) of the test signal generator of the stack "hamdrm".
static inline int32_t sdrkit_hamdrm_gen_get_transmissions(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_transmissions", value);
}
static inline int32_t sdrkit_hamdrm_gen_set_transmissions(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_transmissions", value);
}

// Fills `rows` with the changed rows of the table "programs" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_hdradio_programs_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_hdradio_program_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.HdradioProgramRow", "programs", rows, sizeof(sdrkit_hdradio_program_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "data_services" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_hdradio_data_services_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_hdradio_data_service_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.HdradioDataServiceRow", "data_services", rows, sizeof(sdrkit_hdradio_data_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "files" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_hdradio_files_rows(SDRKIT_HANDLE handle,
                                               sdrkit_hdradio_file_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.HdradioFileRow", "files", rows, sizeof(sdrkit_hdradio_file_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "label_tags" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hdradio_label_tags_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_bcdata_label_tag_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.LabelTagRow", "label_tags", rows, sizeof(sdrkit_bcdata_label_tag_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_services" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hdradio_guide_services_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_bcdata_guide_service_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideServiceRow", "guide_services", rows, sizeof(sdrkit_bcdata_guide_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_programmes" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hdradio_guide_programmes_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_bcdata_guide_programme_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideProgrammeRow", "guide_programmes", rows, sizeof(sdrkit_bcdata_guide_programme_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "guide_groups" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hdradio_guide_groups_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_bcdata_guide_group_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.GuideGroupRow", "guide_groups", rows, sizeof(sdrkit_bcdata_guide_group_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "slide_categories" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hdradio_slide_categories_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_bcdata_slide_category_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.SlideCategoryRow", "slide_categories", rows, sizeof(sdrkit_bcdata_slide_category_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "logos" of the stack "hdradio" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_hdradio_logos_rows(SDRKIT_HANDLE handle,
                                               sdrkit_bcdata_logo_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "bcdata.LogoRow", "logos", rows, sizeof(sdrkit_bcdata_logo_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hdr.psd"
// (Song). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hdradio_hdr_psd_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hdradio_hdr_psd_event *event) {
  return sdrkit_typed_event(handle, n, "hdr.psd", "broadcast.HdradioSongEvent", event, sizeof(sdrkit_hdradio_hdr_psd_event));
}

// Runs the command "service.select" (Select a service) of the stack "hdradio" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_hdradio_service_select(SDRKIT_HANDLE handle, const sdrkit_broadcast_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "broadcast.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "hdradio" on the session of `handle`. Forgets the signal and starts again.
static inline int32_t sdrkit_hdradio_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hdradio" are the sdrkit_hdradio_gen_*() functions
// after the settings of its receiver.

// The setting "gen_waveform" (Waveform) of the test signal generator of the stack "hdradio". A value of `sdrkit_hdradio_gen_waveform`.
static inline int32_t sdrkit_hdradio_gen_get_waveform(SDRKIT_HANDLE handle, sdrkit_hdradio_gen_waveform *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_waveform", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hdradio_gen_waveform)index;
  return code;
}
static inline int32_t sdrkit_hdradio_gen_set_waveform(SDRKIT_HANDLE handle, sdrkit_hdradio_gen_waveform value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_waveform", (int32_t)value);
}

// The setting "gen_analog_db" (Analogue level) of the test signal generator of the stack "hdradio". Unit: dB.
static inline int32_t sdrkit_hdradio_gen_get_analog_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_analog_db", value);
}
static inline int32_t sdrkit_hdradio_gen_set_analog_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_analog_db", value);
}

// Fills `rows` with the changed rows of the table "services" of the stack "cdr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cdr_services_rows(SDRKIT_HANDLE handle,
                                              sdrkit_cdr_service_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.CdrServiceRow", "services", rows, sizeof(sdrkit_cdr_service_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "service_data_types" of the stack "cdr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cdr_service_data_types_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_cdr_data_type_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.CdrDataTypeRow", "service_data_types", rows, sizeof(sdrkit_cdr_data_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "network_frequencies" of the stack "cdr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cdr_network_frequencies_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_cdr_frequency_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "broadcast.CdrFrequencyRow", "network_frequencies", rows, sizeof(sdrkit_cdr_frequency_row), capacity, info);
}

// Runs the command "service.select" (Select a service) of the stack "cdr" on the session of `handle`. Plays a service of the multiplex. An empty row plays the first service that can play.
static inline int32_t sdrkit_cdr_service_select(SDRKIT_HANDLE handle, const sdrkit_broadcast_service_select_args *args) {
  return sdrkit_typed_command(handle, "service.select", "broadcast.ServiceSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "cdr" on the session of `handle`. Forgets the signal and starts again.
static inline int32_t sdrkit_cdr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "resync" (Clear) of the stack "cdr" on the session of `handle`. Forgets the signal and starts again.
static inline int32_t sdrkit_cdr_resync(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "resync", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cdr" are the sdrkit_cdr_gen_*() functions
// after the settings of its receiver.

// The setting "cdr_tables" (GY/T 268.1 tables) of the stack "cdr". A JSON file with the LDPC matrices and the bit interleavers of GY/T 268.1. Without it nothing is decoded.
static inline int32_t sdrkit_cdr_get_cdr_tables(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "cdr_tables", value);
}
static inline int32_t sdrkit_cdr_set_cdr_tables(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "cdr_tables", value);
}

// The setting "gen_tx_mode" (Transmission mode) of the test signal generator of the stack "cdr". A value of `sdrkit_cdr_gen_tx_mode`.
static inline int32_t sdrkit_cdr_gen_get_tx_mode(SDRKIT_HANDLE handle, sdrkit_cdr_gen_tx_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_tx_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdr_gen_tx_mode)index;
  return code;
}
static inline int32_t sdrkit_cdr_gen_set_tx_mode(SDRKIT_HANDLE handle, sdrkit_cdr_gen_tx_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_tx_mode", (int32_t)value);
}

// The setting "gen_spectrum_mode" (Spectrum mode) of the test signal generator of the stack "cdr". A value of `sdrkit_cdr_gen_spectrum_mode`.
static inline int32_t sdrkit_cdr_gen_get_spectrum_mode(SDRKIT_HANDLE handle, sdrkit_cdr_gen_spectrum_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_spectrum_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdr_gen_spectrum_mode)index;
  return code;
}
static inline int32_t sdrkit_cdr_gen_set_spectrum_mode(SDRKIT_HANDLE handle, sdrkit_cdr_gen_spectrum_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_spectrum_mode", (int32_t)value);
}

// The setting "gen_data_qam" (Data) of the test signal generator of the stack "cdr". A value of `sdrkit_cdr_gen_data_qam`.
static inline int32_t sdrkit_cdr_gen_get_data_qam(SDRKIT_HANDLE handle, sdrkit_cdr_gen_data_qam *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_data_qam", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdr_gen_data_qam)index;
  return code;
}
static inline int32_t sdrkit_cdr_gen_set_data_qam(SDRKIT_HANDLE handle, sdrkit_cdr_gen_data_qam value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_data_qam", (int32_t)value);
}

// The setting "gen_ldpc_rate" (LDPC rate) of the test signal generator of the stack "cdr". A value of `sdrkit_cdr_gen_ldpc_rate`.
static inline int32_t sdrkit_cdr_gen_get_ldpc_rate(SDRKIT_HANDLE handle, sdrkit_cdr_gen_ldpc_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_ldpc_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdr_gen_ldpc_rate)index;
  return code;
}
static inline int32_t sdrkit_cdr_gen_set_ldpc_rate(SDRKIT_HANDLE handle, sdrkit_cdr_gen_ldpc_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_ldpc_rate", (int32_t)value);
}

// The setting "gen_sdi_qam" (SDI) of the test signal generator of the stack "cdr". A value of `sdrkit_cdr_gen_sdi_qam`.
static inline int32_t sdrkit_cdr_gen_get_sdi_qam(SDRKIT_HANDLE handle, sdrkit_cdr_gen_sdi_qam *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_sdi_qam", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdr_gen_sdi_qam)index;
  return code;
}
static inline int32_t sdrkit_cdr_gen_set_sdi_qam(SDRKIT_HANDLE handle, sdrkit_cdr_gen_sdi_qam value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_sdi_qam", (int32_t)value);
}

// The setting "gen_allocation" (Allocation) of the test signal generator of the stack "cdr". A value of `sdrkit_cdr_gen_allocation`.
static inline int32_t sdrkit_cdr_gen_get_allocation(SDRKIT_HANDLE handle, sdrkit_cdr_gen_allocation *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_allocation", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdr_gen_allocation)index;
  return code;
}
static inline int32_t sdrkit_cdr_gen_set_allocation(SDRKIT_HANDLE handle, sdrkit_cdr_gen_allocation value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_allocation", (int32_t)value);
}

// The setting "gen_fm_programme" (FM programme) of the test signal generator of the stack "cdr".
static inline int32_t sdrkit_cdr_gen_get_fm_programme(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_fm_programme", value);
}
static inline int32_t sdrkit_cdr_gen_set_fm_programme(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_fm_programme", value);
}

// The setting "gen_text_service" (Text service) of the test signal generator of the stack "cdr".
static inline int32_t sdrkit_cdr_gen_get_text_service(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_text_service", value);
}
static inline int32_t sdrkit_cdr_gen_set_text_service(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_text_service", value);
}

// Runs the command "clear" (Clear) of the stack "cquam" on the session of `handle`. Forgets the signal and starts again.
static inline int32_t sdrkit_cquam_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cquam" are the sdrkit_cquam_gen_*() functions
// after the settings of its receiver.

// The setting "stereo" (Stereo) of the stack "cquam". A value of `sdrkit_cquam_stereo`. Auto follows the 25 Hz pilot. Mono plays L + R only. Stereo uses L - R whatever the pilot says.
static inline int32_t sdrkit_cquam_get_stereo(SDRKIT_HANDLE handle, sdrkit_cquam_stereo *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "stereo", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cquam_stereo)index;
  return code;
}
static inline int32_t sdrkit_cquam_set_stereo(SDRKIT_HANDLE handle, sdrkit_cquam_stereo value) {
  return sdrkit_typed_setting_set_enum(handle, "stereo", (int32_t)value);
}

// The setting "width" (Width) of the stack "cquam". A frequency in Hz. Audio width either side of the carrier.
static inline int32_t sdrkit_cquam_get_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "width", value);
}
static inline int32_t sdrkit_cquam_set_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "width", value);
}

// The setting "squelch" (Squelch (4 dB)) of the stack "cquam". Mutes below 2.5 dB C/N, opens above 4 dB.
static inline int32_t sdrkit_cquam_get_squelch(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "squelch", value);
}
static inline int32_t sdrkit_cquam_set_squelch(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "squelch", value);
}

// The setting "channel_offset" (Channel offset) of the stack "cquam". A frequency in Hz. The station's distance from the dial.
static inline int32_t sdrkit_cquam_get_channel_offset(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_offset", value);
}
static inline int32_t sdrkit_cquam_set_channel_offset(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_offset", value);
}

// The setting "gen_kind" (Test signal) of the test signal generator of the stack "cquam". A value of `sdrkit_cquam_gen_kind`.
static inline int32_t sdrkit_cquam_gen_get_kind(SDRKIT_HANDLE handle, sdrkit_cquam_gen_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cquam_gen_kind)index;
  return code;
}
static inline int32_t sdrkit_cquam_gen_set_kind(SDRKIT_HANDLE handle, sdrkit_cquam_gen_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_kind", (int32_t)value);
}

// The setting "gen_left_silent" (Left silent) of the test signal generator of the stack "cquam".
static inline int32_t sdrkit_cquam_gen_get_left_silent(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_left_silent", value);
}
static inline int32_t sdrkit_cquam_gen_set_left_silent(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_left_silent", value);
}

// The setting "gen_left_hz" (Left tone) of the test signal generator of the stack "cquam". A frequency in Hz.
static inline int32_t sdrkit_cquam_gen_get_left_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_left_hz", value);
}
static inline int32_t sdrkit_cquam_gen_set_left_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_left_hz", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_drm_fields_read(SDRKIT_HANDLE handle, sdrkit_drm_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast.DrmFields", fields, sizeof(sdrkit_drm_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hamdrm_fields_read(SDRKIT_HANDLE handle, sdrkit_hamdrm_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast.HamdrmFields", fields, sizeof(sdrkit_hamdrm_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hdradio_fields_read(SDRKIT_HANDLE handle, sdrkit_hdradio_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast.HdradioFields", fields, sizeof(sdrkit_hdradio_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cdr_fields_read(SDRKIT_HANDLE handle, sdrkit_cdr_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast.CdrFields", fields, sizeof(sdrkit_cdr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cquam_fields_read(SDRKIT_HANDLE handle, sdrkit_cquam_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast.CquamFields", fields, sizeof(sdrkit_cquam_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cquam_front_fields_read(SDRKIT_HANDLE handle, sdrkit_cquam_front_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast.CquamFrontFields", fields, sizeof(sdrkit_cquam_front_fields));
}

#endif
