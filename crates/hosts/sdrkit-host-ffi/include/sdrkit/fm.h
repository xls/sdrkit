/*
 * sdrkit: the typed interface of the stack family "fm", version 1.3. MIT licence.
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

#ifndef SDRKIT_FM_H
#define SDRKIT_FM_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_FM_VERSION_MAJOR 1
#define SDRKIT_FM_VERSION_MINOR 3

// The stack "fm": its id, and the keys of its event types.
#define SDRKIT_STACK_FM "fm"
#define SDRKIT_EVENT_FM_STATION "fm.station"
#define SDRKIT_EVENT_FM_TEXT "fm.text"

// The stack "stl": its id, and the keys of its event types.
#define SDRKIT_STACK_STL "stl"
#define SDRKIT_EVENT_STL_LINK "stl.link"

// The options of the setting "stereo" of the stack "fm".
typedef enum sdrkit_fm_stereo {
  // Auto
  SDRKIT_FM_STEREO_AUTO = 0,
  // Mono
  SDRKIT_FM_STEREO_MONO = 1,
  // Stereo
  SDRKIT_FM_STEREO_STEREO = 2,
} sdrkit_fm_stereo;

// The options of the setting "deemphasis" of the stack "fm".
typedef enum sdrkit_fm_deemphasis {
  // Off
  SDRKIT_FM_DEEMPHASIS_OFF = 0,
  // 50 µs
  SDRKIT_FM_DEEMPHASIS_50_US = 1,
  // 75 µs
  SDRKIT_FM_DEEMPHASIS_75_US = 2,
} sdrkit_fm_deemphasis;

// The options of the setting "audio_bandwidth" of the stack "fm".
typedef enum sdrkit_fm_audio_bandwidth {
  // Full
  SDRKIT_FM_AUDIO_BANDWIDTH_FULL = 0,
  // 12 kHz
  SDRKIT_FM_AUDIO_BANDWIDTH_12_KHZ = 1,
  // 8 kHz
  SDRKIT_FM_AUDIO_BANDWIDTH_8_KHZ = 2,
  // 5 kHz
  SDRKIT_FM_AUDIO_BANDWIDTH_5_KHZ = 3,
} sdrkit_fm_audio_bandwidth;

// The options of the setting "channel_filter" of the stack "fm".
typedef enum sdrkit_fm_channel_filter {
  // Standard
  SDRKIT_FM_CHANNEL_FILTER_STANDARD = 0,
  // Wide
  SDRKIT_FM_CHANNEL_FILTER_WIDE = 1,
  // Narrow
  SDRKIT_FM_CHANNEL_FILTER_NARROW = 2,
  // Custom
  SDRKIT_FM_CHANNEL_FILTER_CUSTOM = 3,
} sdrkit_fm_channel_filter;

// The options of the setting "rds_variant" of the stack "fm".
typedef enum sdrkit_fm_rds_variant {
  // Auto
  SDRKIT_FM_RDS_VARIANT_AUTO = 0,
  // RDS
  SDRKIT_FM_RDS_VARIANT_RDS = 1,
  // RBDS
  SDRKIT_FM_RDS_VARIANT_RBDS = 2,
} sdrkit_fm_rds_variant;

// The options of the setting "rds_correction" of the stack "fm".
typedef enum sdrkit_fm_rds_correction {
  // Off
  SDRKIT_FM_RDS_CORRECTION_OFF = 0,
  // Short bursts
  SDRKIT_FM_RDS_CORRECTION_SHORT_BURSTS = 1,
  // Full
  SDRKIT_FM_RDS_CORRECTION_FULL = 2,
} sdrkit_fm_rds_correction;

// The options of the setting "rds_charset" of the stack "fm".
typedef enum sdrkit_fm_rds_charset {
  // EBU Latin
  SDRKIT_FM_RDS_CHARSET_EBU_LATIN = 0,
  // Auto GB2312
  SDRKIT_FM_RDS_CHARSET_AUTO_GB2312 = 1,
  // GB2312
  SDRKIT_FM_RDS_CHARSET_GB2312 = 2,
} sdrkit_fm_rds_charset;

// The options of the setting "gen_preemphasis" of the stack "fm".
typedef enum sdrkit_fm_gen_preemphasis {
  // None
  SDRKIT_FM_GEN_PREEMPHASIS_NONE = 0,
  // 50 µs
  SDRKIT_FM_GEN_PREEMPHASIS_50_US = 1,
  // 75 µs
  SDRKIT_FM_GEN_PREEMPHASIS_75_US = 2,
} sdrkit_fm_gen_preemphasis;

// The options of the setting "tx_preemphasis" of the stack "fm".
typedef enum sdrkit_fm_tx_preemphasis {
  // None
  SDRKIT_FM_TX_PREEMPHASIS_NONE = 0,
  // 50 µs
  SDRKIT_FM_TX_PREEMPHASIS_50_US = 1,
  // 75 µs
  SDRKIT_FM_TX_PREEMPHASIS_75_US = 2,
} sdrkit_fm_tx_preemphasis;

// The options of the setting "stereo" of the stack "stl".
typedef enum sdrkit_stl_stereo {
  // Auto
  SDRKIT_STL_STEREO_AUTO = 0,
  // Mono
  SDRKIT_STL_STEREO_MONO = 1,
  // Stereo
  SDRKIT_STL_STEREO_STEREO = 2,
} sdrkit_stl_stereo;

// The options of the setting "deemphasis" of the stack "stl".
typedef enum sdrkit_stl_deemphasis {
  // Off
  SDRKIT_STL_DEEMPHASIS_OFF = 0,
  // 50 µs
  SDRKIT_STL_DEEMPHASIS_50_US = 1,
  // 75 µs
  SDRKIT_STL_DEEMPHASIS_75_US = 2,
} sdrkit_stl_deemphasis;

// The options of the setting "rds_variant" of the stack "stl".
typedef enum sdrkit_stl_rds_variant {
  // Auto
  SDRKIT_STL_RDS_VARIANT_AUTO = 0,
  // RDS
  SDRKIT_STL_RDS_VARIANT_RDS = 1,
  // RBDS
  SDRKIT_STL_RDS_VARIANT_RBDS = 2,
} sdrkit_stl_rds_variant;

// The options of the setting "rds_correction" of the stack "stl".
typedef enum sdrkit_stl_rds_correction {
  // Off
  SDRKIT_STL_RDS_CORRECTION_OFF = 0,
  // Short bursts
  SDRKIT_STL_RDS_CORRECTION_SHORT_BURSTS = 1,
  // Full
  SDRKIT_STL_RDS_CORRECTION_FULL = 2,
} sdrkit_stl_rds_correction;

// The options of the setting "rds_charset" of the stack "stl".
typedef enum sdrkit_stl_rds_charset {
  // EBU Latin
  SDRKIT_STL_RDS_CHARSET_EBU_LATIN = 0,
  // Auto GB2312
  SDRKIT_STL_RDS_CHARSET_AUTO_GB2312 = 1,
  // GB2312
  SDRKIT_STL_RDS_CHARSET_GB2312 = 2,
} sdrkit_stl_rds_charset;

// The options of the setting "gen_link" of the stack "stl".
typedef enum sdrkit_stl_gen_link {
  // Composite stereo
  SDRKIT_STL_GEN_LINK_COMPOSITE = 0,
  // Mono
  SDRKIT_STL_GEN_LINK_MONO = 1,
  // Digital-like
  SDRKIT_STL_GEN_LINK_DIGITAL = 2,
} sdrkit_stl_gen_link;

// The options of the setting "gen_preemphasis" of the stack "stl".
typedef enum sdrkit_stl_gen_preemphasis {
  // None
  SDRKIT_STL_GEN_PREEMPHASIS_NONE = 0,
  // 50 µs
  SDRKIT_STL_GEN_PREEMPHASIS_50_US = 1,
  // 75 µs
  SDRKIT_STL_GEN_PREEMPHASIS_75_US = 2,
} sdrkit_stl_gen_preemphasis;

// The result fields of the FM receiver, in the order a screen lists them. The display writes the band plan ([`BandPart`]) and the measurements and RDS data ([`RdsPart`]); the broadcast data publisher writes the RadioText and whether its item is running.
typedef struct sdrkit_fm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The station name (PS): all 8 characters with trailing spaces, empty before the first name.
  sdrkit_string station;
  // The name being collected (`_` for a segment not yet received).
  sdrkit_string station_partial;
  // The RadioText.
  sdrkit_string text;
  // Every RadioText segment has arrived.
  bool text_complete;
  // True if `programme` holds a value.
  bool has_programme;
  // The programme type's name in the table in force.
  sdrkit_string programme;
  // True if `pty` holds a value.
  bool has_pty;
  // The programme type code.
  int64_t pty;
  // True if `pi` holds a value.
  bool has_pi;
  // The programme identification, four hexadecimal digits.
  sdrkit_string pi;
  // True if `call_letters` holds a value.
  bool has_call_letters;
  // RBDS call letters.
  sdrkit_string call_letters;
  // RBDS meanings apply.
  bool rbds;
  // Traffic programme.
  bool tp;
  // Traffic announcement.
  bool ta;
  // Music (true) or speech.
  bool music;
  // Programme type name (PTYN).
  sdrkit_string ptyn;
  // True if `ecc` holds a value.
  bool has_ecc;
  // Extended country code.
  int64_t ecc;
  // True if `language` holds a value.
  bool has_language;
  // Language code.
  int64_t language;
  // True if `pin` holds a value.
  bool has_pin;
  // Programme item number, `DD HH:MM`.
  sdrkit_string pin;
  // True if `clock` holds a value.
  bool has_clock;
  // The station's clock time. Seconds since 1970 (UTC).
  double clock;
  // True if `clock_offset` holds a value.
  bool has_clock_offset;
  // The station's local time offset. A length of time in seconds.
  double clock_offset;
  // Enhanced RadioText.
  sdrkit_string ert;
  // The RadioText+ item is running.
  bool item_running;
  // RDS block sync.
  bool rds_sync;
  // RDS usable blocks. Unit: %.
  double rds_ok_pct;
  // Accepted RDS groups.
  int64_t groups;
  // Good RDS blocks.
  int64_t blocks_good;
  // Corrected RDS blocks (voted ones too).
  int64_t blocks_corrected;
  // Bad RDS blocks.
  int64_t blocks_bad;
  // RDS blocks recovered by voting.
  int64_t blocks_voted;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The level of the channel. Unit: dBFS.
  double level_dbfs;
  // The offset of the carrier. A frequency in Hz.
  double offset_hz;
  // A carrier is present.
  bool carrier;
  // Stereo is reported.
  bool stereo;
  // The stereo blend while stereo is reported. Unit: %.
  double stereo_pct;
  // The pilot injection, percent of 75 kHz. Unit: %.
  double pilot_pct;
  // The pilot PLL is locked.
  bool pilot_locked;
  // The peak deviation. A frequency in Hz.
  double deviation_hz;
  // The multipath ripple. Unit: %.
  double multipath_pct;
  // The equaliser is in use for the sound.
  bool equaliser;
  // The squelch is closed.
  bool squelched;
  // The channel filter width in use. A frequency in Hz.
  double channel_width_hz;
  // The upper frequency of the `mpx` series. A frequency in Hz.
  double mpx_top_hz;
  // Signal decoded since the last reset. A length of time in seconds.
  double signal_time;
  // The lowest frequency of the band. A frequency in Hz.
  double band_start_hz;
  // The highest frequency of the band. A frequency in Hz.
  double band_end_hz;
  // The channel step. A frequency in Hz.
  double band_step_hz;
  // The first channel of the raster. A frequency in Hz.
  double band_first_hz;
  // True if `tmc_ltn` holds a value.
  bool has_tmc_ltn;
  // TMC location table number.
  int64_t tmc_ltn;
  // True if `tmc_sid` holds a value.
  bool has_tmc_sid;
  // TMC service identifier.
  int64_t tmc_sid;
  // TMC alternative-frequency indicator.
  bool tmc_afi;
  // TMC enhanced mode.
  bool tmc_enhanced_mode;
  // TMC geographical scope bits (I, N, R, U).
  int64_t tmc_scope;
} sdrkit_fm_fields;

// The result fields of the FM sender. The measurements have no value while the stack does not transmit.
typedef struct sdrkit_fm_tx_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the stack transmits now.
  bool transmitting;
  // True if `tx_freq_hz` holds a value.
  bool has_tx_freq_hz;
  // The transmit frequency: the tuned frequency. A frequency in Hz.
  double tx_freq_hz;
  // True if `tx_peak_hz` holds a value.
  bool has_tx_peak_hz;
  // The largest deviation of the multiplex over the last report. A frequency in Hz.
  double tx_peak_hz;
  // True if `tx_mpx_power_db` holds a value.
  bool has_tx_mpx_power_db;
  // The power of the multiplex over the last 60 s against that of a sine of ±19 kHz deviation (ITU-R BS.412): 0 dB is the limit. Unit: dB.
  double tx_mpx_power_db;
  // The RDS groups sent in this transmission.
  int64_t tx_groups;
} sdrkit_fm_tx_fields;

// A row of the alternative frequencies. The row key is the place in the list (`00`).
typedef struct sdrkit_fm_af_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A frequency. A frequency in Hz.
  double freq_hz;
} sdrkit_fm_af_row;

// A row of the tags of the RadioText, which the broadcast data publisher writes. The row key is the class name of the content type (`ITEM.TITLE`), or its code.
typedef struct sdrkit_fm_label_tag_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The content type code (0 to 127).
  int64_t code;
  // True if `type` holds a value.
  bool has_type;
  // What people call the content type ("Title").
  sdrkit_string type;
  // The tagged text.
  sdrkit_string text;
} sdrkit_fm_label_tag_row;

// A row of the open data applications. The row key is the AID in hexadecimal.
typedef struct sdrkit_fm_oda_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // An open data application identifier.
  int64_t aid;
  // The group type that carries an application ("11A").
  sdrkit_string group;
} sdrkit_fm_oda_row;

// A row of the other networks. The row key is the PI in hexadecimal.
typedef struct sdrkit_fm_eon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The programme identification, four hexadecimal digits.
  sdrkit_string pi;
  // A name.
  sdrkit_string name;
  // Traffic programme.
  bool tp;
  // Traffic announcement.
  bool ta;
  // True if `pty` holds a value.
  bool has_pty;
  // The programme type code.
  int64_t pty;
  // True if `pin` holds a value.
  bool has_pin;
  // Programme item number, `DD HH:MM`.
  sdrkit_string pin;
} sdrkit_fm_eon_row;

// A row of the alternative frequencies of the other networks. The row key is `<PI>/<place>` (`4A21/00`).
typedef struct sdrkit_fm_eon_af_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PI of the other network a row belongs to.
  sdrkit_string network;
  // A frequency. A frequency in Hz.
  double freq_hz;
} sdrkit_fm_eon_af_row;

// A row of the traffic messages. The row key is the place, oldest first (`00`).
typedef struct sdrkit_fm_tmc_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A TMC event code.
  int64_t event;
  // A TMC location code.
  int64_t location;
  // The TMC direction bit.
  bool negative_direction;
  // The TMC extent.
  int64_t extent;
  // The TMC diversion bit.
  bool diversion;
  // The TMC duration code.
  int64_t duration;
  // How often a message came.
  int64_t repeats;
  // When a message came first. Seconds since 1970 (UTC).
  double first;
  // When a message came last. Seconds since 1970 (UTC).
  double last;
  // When a message expires. Seconds since 1970 (UTC).
  double expires;
} sdrkit_fm_tmc_row;

// The options of `link` of `sdrkit_stl_fields`.
typedef enum sdrkit_stl_fields_link {
  // Nothing
  SDRKIT_STL_FIELDS_LINK_NOTHING = 0,
  // Composite (stereo multiplex)
  SDRKIT_STL_FIELDS_LINK_COMPOSITE = 1,
  // Mono
  SDRKIT_STL_FIELDS_LINK_MONO = 2,
  // Digital or unknown (not decoded)
  SDRKIT_STL_FIELDS_LINK_DIGITAL_UNKNOWN = 3,
  // Unknown (not FM)
  SDRKIT_STL_FIELDS_LINK_UNKNOWN = 4,
} sdrkit_stl_fields_link;

// The result fields of the STL link, beside the reception fields of the FM components.
typedef struct sdrkit_stl_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What the link is. A change holds after two reports of 0.5 s in a row. A value of `sdrkit_stl_fields_link`.
  int32_t link;
  // True if `peak_deviation_hz` holds a value.
  bool has_peak_deviation_hz;
  // The peak deviation over the last 0.5 s, noise included. No value without a carrier. A frequency in Hz.
  double peak_deviation_hz;
  // True if `modulation_pct` holds a value.
  bool has_modulation_pct;
  // The peak deviation against the deviation of full modulation. No value without a carrier. Unit: %.
  double modulation_pct;
  // True if `pilot_hz` holds a value.
  bool has_pilot_hz;
  // The pilot level, as deviation. No value while the pilot is not locked. A frequency in Hz.
  double pilot_hz;
  // The channel width follows the link (the automatic width).
  bool width_auto;
  // The spectrum of the capture reaches far enough past the channel to measure the floor. Without it the digital check is off.
  bool floor_measured;
  // True if `occupied_width_hz` holds a value.
  bool has_occupied_width_hz;
  // The width of the occupied region. No value while none stands out. A frequency in Hz.
  double occupied_width_hz;
  // True if `over_floor_db` holds a value.
  bool has_over_floor_db;
  // The mean of the occupied region over the floor. Unit: dB.
  double over_floor_db;
  // True if `flatness_db` holds a value.
  bool has_flatness_db;
  // The flatness of the occupied region: its 10th percentile against its mean (0 dB is flat). Unit: dB.
  double flatness_db;
  // True if `sca67_snr_db` holds a value.
  bool has_sca67_snr_db;
  // The SNR of the SCA subcarrier at 67 kHz. No value while it is absent. Unit: dB.
  double sca67_snr_db;
  // True if `sca92_snr_db` holds a value.
  bool has_sca92_snr_db;
  // The SNR of the SCA subcarrier at 92 kHz. No value while it is absent. Unit: dB.
  double sca92_snr_db;
  // True if `sound_peak_dbfs` holds a value.
  bool has_sound_peak_dbfs;
  // The peak of the sound over the last 0.5 s. No value for silence. Unit: dBFS.
  double sound_peak_dbfs;
  // True if `sound_rms_dbfs` holds a value.
  bool has_sound_rms_dbfs;
  // The RMS of the sound over the last 0.5 s. No value for silence. Unit: dBFS.
  double sound_rms_dbfs;
} sdrkit_stl_fields;

// The measurements of the FM receiver and the RDS data: a part of [`FmFields`], and a result set of its own in a stack that runs the FM receiver without the band plan.
typedef struct sdrkit_stl_rds_part {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The station name (PS): all 8 characters with trailing spaces, empty before the first name.
  sdrkit_string station;
  // The name being collected (`_` for a segment not yet received).
  sdrkit_string station_partial;
  // Every RadioText segment has arrived.
  bool text_complete;
  // True if `programme` holds a value.
  bool has_programme;
  // The programme type's name in the table in force.
  sdrkit_string programme;
  // True if `pty` holds a value.
  bool has_pty;
  // The programme type code.
  int64_t pty;
  // True if `pi` holds a value.
  bool has_pi;
  // The programme identification, four hexadecimal digits.
  sdrkit_string pi;
  // True if `call_letters` holds a value.
  bool has_call_letters;
  // RBDS call letters.
  sdrkit_string call_letters;
  // RBDS meanings apply.
  bool rbds;
  // Traffic programme.
  bool tp;
  // Traffic announcement.
  bool ta;
  // Music (true) or speech.
  bool music;
  // Programme type name (PTYN).
  sdrkit_string ptyn;
  // True if `ecc` holds a value.
  bool has_ecc;
  // Extended country code.
  int64_t ecc;
  // True if `language` holds a value.
  bool has_language;
  // Language code.
  int64_t language;
  // True if `pin` holds a value.
  bool has_pin;
  // Programme item number, `DD HH:MM`.
  sdrkit_string pin;
  // True if `clock` holds a value.
  bool has_clock;
  // The station's clock time. Seconds since 1970 (UTC).
  double clock;
  // True if `clock_offset` holds a value.
  bool has_clock_offset;
  // The station's local time offset. A length of time in seconds.
  double clock_offset;
  // Enhanced RadioText.
  sdrkit_string ert;
  // RDS block sync.
  bool rds_sync;
  // RDS usable blocks. Unit: %.
  double rds_ok_pct;
  // Accepted RDS groups.
  int64_t groups;
  // Good RDS blocks.
  int64_t blocks_good;
  // Corrected RDS blocks (voted ones too).
  int64_t blocks_corrected;
  // Bad RDS blocks.
  int64_t blocks_bad;
  // RDS blocks recovered by voting.
  int64_t blocks_voted;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The level of the channel. Unit: dBFS.
  double level_dbfs;
  // The offset of the carrier. A frequency in Hz.
  double offset_hz;
  // A carrier is present.
  bool carrier;
  // Stereo is reported.
  bool stereo;
  // The stereo blend while stereo is reported. Unit: %.
  double stereo_pct;
  // The pilot injection, percent of 75 kHz. Unit: %.
  double pilot_pct;
  // The pilot PLL is locked.
  bool pilot_locked;
  // The peak deviation. A frequency in Hz.
  double deviation_hz;
  // The multipath ripple. Unit: %.
  double multipath_pct;
  // The equaliser is in use for the sound.
  bool equaliser;
  // The squelch is closed.
  bool squelched;
  // The channel filter width in use. A frequency in Hz.
  double channel_width_hz;
  // The upper frequency of the `mpx` series. A frequency in Hz.
  double mpx_top_hz;
  // Signal decoded since the last reset. A length of time in seconds.
  double signal_time;
  // True if `tmc_ltn` holds a value.
  bool has_tmc_ltn;
  // TMC location table number.
  int64_t tmc_ltn;
  // True if `tmc_sid` holds a value.
  bool has_tmc_sid;
  // TMC service identifier.
  int64_t tmc_sid;
  // TMC alternative-frequency indicator.
  bool tmc_afi;
  // TMC enhanced mode.
  bool tmc_enhanced_mode;
  // TMC geographical scope bits (I, N, R, U).
  int64_t tmc_scope;
} sdrkit_stl_rds_part;

// The radio text and whether its item is running.
typedef struct sdrkit_stl_label_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `text` holds a value.
  bool has_text;
  // The radio text.
  sdrkit_string text;
  // True if `item_running` holds a value.
  bool has_item_running;
  // The tagged item (a song) is playing.
  bool item_running;
} sdrkit_stl_label_fields;

// The options of `link` of `sdrkit_stl_link_row`.
typedef enum sdrkit_stl_link_row_link {
  // Nothing
  SDRKIT_STL_LINK_ROW_LINK_NOTHING = 0,
  // Composite (stereo multiplex)
  SDRKIT_STL_LINK_ROW_LINK_COMPOSITE = 1,
  // Mono
  SDRKIT_STL_LINK_ROW_LINK_MONO = 2,
  // Digital or unknown (not decoded)
  SDRKIT_STL_LINK_ROW_LINK_DIGITAL_UNKNOWN = 3,
  // Unknown (not FM)
  SDRKIT_STL_LINK_ROW_LINK_UNKNOWN = 4,
} sdrkit_stl_link_row_link;

// A held change of the link. The row key is the count of changes (`0001`).
typedef struct sdrkit_stl_link_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the change. A length of time in seconds.
  double time_s;
  // What the link became. A value of `sdrkit_stl_link_row_link`.
  int32_t link;
  // The details: pilot, peak deviation and offset of an FM link, or level and width of a signal that is not FM.
  sdrkit_string text;
  // True if `over_floor_db` holds a value.
  bool has_over_floor_db;
  // The occupied region over the floor. No value without a region. Unit: dB.
  double over_floor_db;
} sdrkit_stl_link_row;

// The options of `family` of `sdrkit_fm_station_event`.
typedef enum sdrkit_fm_station_event_family {
  // Aircraft
  SDRKIT_FM_STATION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FM_STATION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FM_STATION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FM_STATION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FM_STATION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FM_STATION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FM_STATION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FM_STATION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FM_STATION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FM_STATION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FM_STATION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FM_STATION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FM_STATION_EVENT_FAMILY_OTHER = 12,
} sdrkit_fm_station_event_family;

// The fields of the two events of the FM receiver.
typedef struct sdrkit_fm_station_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The PI, four hexadecimal digits.
  sdrkit_string source;
  // The station name, or the RadioText.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_fm_station_event_family`.
  int32_t family;
} sdrkit_fm_station_event;

// The options of `family` of `sdrkit_fm_text_event`.
typedef enum sdrkit_fm_text_event_family {
  // Aircraft
  SDRKIT_FM_TEXT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FM_TEXT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FM_TEXT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FM_TEXT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FM_TEXT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FM_TEXT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FM_TEXT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FM_TEXT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FM_TEXT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FM_TEXT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FM_TEXT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FM_TEXT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FM_TEXT_EVENT_FAMILY_OTHER = 12,
} sdrkit_fm_text_event_family;

// The fields of the two events of the FM receiver.
typedef struct sdrkit_fm_text_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The PI, four hexadecimal digits.
  sdrkit_string source;
  // The station name, or the RadioText.
  sdrkit_string text;
  // The family of the sender: broadcast. A value of `sdrkit_fm_text_event_family`.
  int32_t family;
} sdrkit_fm_text_event;

// The options of `link` of `sdrkit_stl_link_event`.
typedef enum sdrkit_stl_link_event_link {
  // Nothing
  SDRKIT_STL_LINK_EVENT_LINK_NOTHING = 0,
  // Composite (stereo multiplex)
  SDRKIT_STL_LINK_EVENT_LINK_COMPOSITE = 1,
  // Mono
  SDRKIT_STL_LINK_EVENT_LINK_MONO = 2,
  // Digital or unknown (not decoded)
  SDRKIT_STL_LINK_EVENT_LINK_DIGITAL_UNKNOWN = 3,
  // Unknown (not FM)
  SDRKIT_STL_LINK_EVENT_LINK_UNKNOWN = 4,
} sdrkit_stl_link_event_link;

// The fields of a change of the link.
typedef struct sdrkit_stl_link_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the link became. A value of `sdrkit_stl_link_event_link`.
  int32_t link;
  // The details, as in the table of changes.
  sdrkit_string text;
  // True if `over_floor_db` holds a value.
  bool has_over_floor_db;
  // The occupied region over the floor. No value without a region. Unit: dB.
  double over_floor_db;
} sdrkit_stl_link_event;

// Fills `rows` with the changed rows of the table "af" of the stack "fm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fm_af_rows(SDRKIT_HANDLE handle,
                                       sdrkit_fm_af_row *rows,
                                       uint32_t capacity,
                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.AfRow", "af", rows, sizeof(sdrkit_fm_af_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "label_tags" of the stack "fm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fm_label_tags_rows(SDRKIT_HANDLE handle,
                                               sdrkit_fm_label_tag_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.LabelTagRow", "label_tags", rows, sizeof(sdrkit_fm_label_tag_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "oda" of the stack "fm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fm_oda_rows(SDRKIT_HANDLE handle,
                                        sdrkit_fm_oda_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.OdaRow", "oda", rows, sizeof(sdrkit_fm_oda_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "eon" of the stack "fm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_fm_eon_rows(SDRKIT_HANDLE handle,
                                        sdrkit_fm_eon_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.EonRow", "eon", rows, sizeof(sdrkit_fm_eon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "eon_af" of the stack "fm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fm_eon_af_rows(SDRKIT_HANDLE handle,
                                           sdrkit_fm_eon_af_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.EonAfRow", "eon_af", rows, sizeof(sdrkit_fm_eon_af_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tmc" of the stack "fm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_fm_tmc_rows(SDRKIT_HANDLE handle,
                                        sdrkit_fm_tmc_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.TmcRow", "tmc", rows, sizeof(sdrkit_fm_tmc_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fm.station"
// (FM station named). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fm_station_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fm_station_event *event) {
  return sdrkit_typed_event(handle, n, "fm.station", "fm.StationEvent", event, sizeof(sdrkit_fm_station_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fm.text"
// (FM RadioText complete). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fm_text_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fm_text_event *event) {
  return sdrkit_typed_event(handle, n, "fm.text", "fm.StationEvent", event, sizeof(sdrkit_fm_text_event));
}

// Runs the command "clear" (Clear) of the stack "fm" on the session of `handle`. Forgets the station's name, text and the other RDS data.
static inline int32_t sdrkit_fm_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "start" (Start sending) of the stack "fm" on the session of `handle`. Sends the FM broadcast signal without end: the audio input in stereo, with the pilot and RDS from the transmit settings, until stop. The setting tx_pi must be set. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_fm_start(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "start", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop sending) of the stack "fm" on the session of `handle`. Ends the transmission at once, with the fall of the carrier.
static inline int32_t sdrkit_fm_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fm" are the sdrkit_fm_gen_*() functions
// after the settings of its receiver.

// The setting "stereo" (Stereo) of the stack "fm". A value of `sdrkit_fm_stereo`. Auto plays stereo while the pilot tone is locked and moves towards mono as the signal gets weak. Mono always plays mono. Stereo plays full stereo whenever any pilot is there.
static inline int32_t sdrkit_fm_get_stereo(SDRKIT_HANDLE handle, sdrkit_fm_stereo *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "stereo", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_stereo)index;
  return code;
}
static inline int32_t sdrkit_fm_set_stereo(SDRKIT_HANDLE handle, sdrkit_fm_stereo value) {
  return sdrkit_typed_setting_set_enum(handle, "stereo", (int32_t)value);
}

// The setting "force_mono" (Force mono) of the stack "fm". Plays mono whatever the stereo choice.
static inline int32_t sdrkit_fm_get_force_mono(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "force_mono", value);
}
static inline int32_t sdrkit_fm_set_force_mono(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "force_mono", value);
}

// The setting "deemphasis" (De-emphasis) of the stack "fm". A value of `sdrkit_fm_deemphasis`. Undoes the treble boost stations add. 50 µs in Europe, Asia, Africa and Australia. 75 µs in the Americas and South Korea.
static inline int32_t sdrkit_fm_get_deemphasis(SDRKIT_HANDLE handle, sdrkit_fm_deemphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deemphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_deemphasis)index;
  return code;
}
static inline int32_t sdrkit_fm_set_deemphasis(SDRKIT_HANDLE handle, sdrkit_fm_deemphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "deemphasis", (int32_t)value);
}

// The setting "squelch" (Squelch) of the stack "fm". Unit: dB. Mutes the sound while the signal-to-noise measure is below this.
static inline int32_t sdrkit_fm_get_squelch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch", value);
}
static inline int32_t sdrkit_fm_set_squelch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch", value);
}

// The setting "audio_bandwidth" (Audio bandwidth) of the stack "fm". A value of `sdrkit_fm_audio_bandwidth`. Cuts the treble above this frequency. A lower value hides hiss on a weak station.
static inline int32_t sdrkit_fm_get_audio_bandwidth(SDRKIT_HANDLE handle, sdrkit_fm_audio_bandwidth *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "audio_bandwidth", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_audio_bandwidth)index;
  return code;
}
static inline int32_t sdrkit_fm_set_audio_bandwidth(SDRKIT_HANDLE handle, sdrkit_fm_audio_bandwidth value) {
  return sdrkit_typed_setting_set_enum(handle, "audio_bandwidth", (int32_t)value);
}

// The setting "bass" (Bass) of the stack "fm". Unit: dB. Boost or cut below 200 Hz.
static inline int32_t sdrkit_fm_get_bass(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "bass", value);
}
static inline int32_t sdrkit_fm_set_bass(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "bass", value);
}

// The setting "treble" (Treble) of the stack "fm". Unit: dB. Boost or cut above 6 kHz.
static inline int32_t sdrkit_fm_get_treble(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "treble", value);
}
static inline int32_t sdrkit_fm_set_treble(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "treble", value);
}

// The setting "warm" (Warm) of the stack "fm". Takes 3 dB more off the treble, for a softer sound.
static inline int32_t sdrkit_fm_get_warm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "warm", value);
}
static inline int32_t sdrkit_fm_set_warm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "warm", value);
}

// The setting "channel_width" (Channel width) of the stack "fm". A frequency in Hz. The width of the channel filter. A narrower width keeps out a strong neighbour, at the cost of distortion on loud passages and a weaker RDS signal. At least 10 kHz.
static inline int32_t sdrkit_fm_get_channel_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_width", value);
}
static inline int32_t sdrkit_fm_set_channel_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_width", value);
}

// The setting "channel_filter" (Channel filter) of the stack "fm". A value of `sdrkit_fm_channel_filter`. Standard is 220 kHz (or the channel width below when it is not 0). Wide is 300 kHz, for strong stations. Narrow is 150 kHz, to keep out a strong neighbour. Custom uses the channel width below.
static inline int32_t sdrkit_fm_get_channel_filter(SDRKIT_HANDLE handle, sdrkit_fm_channel_filter *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel_filter", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_channel_filter)index;
  return code;
}
static inline int32_t sdrkit_fm_set_channel_filter(SDRKIT_HANDLE handle, sdrkit_fm_channel_filter value) {
  return sdrkit_typed_setting_set_enum(handle, "channel_filter", (int32_t)value);
}

// The setting "band_low_hz" (Band start) of the stack "fm". A frequency in Hz. The lowest frequency of the FM band.
static inline int32_t sdrkit_fm_get_band_low_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "band_low_hz", value);
}
static inline int32_t sdrkit_fm_set_band_low_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "band_low_hz", value);
}

// The setting "band_high_hz" (Band end) of the stack "fm". A frequency in Hz. The highest frequency of the FM band.
static inline int32_t sdrkit_fm_get_band_high_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "band_high_hz", value);
}
static inline int32_t sdrkit_fm_set_band_high_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "band_high_hz", value);
}

// The setting "first_hz" (First channel) of the stack "fm". A frequency in Hz. The first channel of the band. The channels are this frequency plus a whole number of steps.
static inline int32_t sdrkit_fm_get_first_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "first_hz", value);
}
static inline int32_t sdrkit_fm_set_first_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "first_hz", value);
}

// The setting "raster_hz" (Channel step) of the stack "fm". A frequency in Hz. The distance between two channels of the band.
static inline int32_t sdrkit_fm_get_raster_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_hz", value);
}
static inline int32_t sdrkit_fm_set_raster_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_hz", value);
}

// The setting "scan_low" (Scan from) of the stack "fm". Unit: MHz. Where the band scan starts. The scan covers at least 0.5 MHz, within 76 to 108 MHz.
static inline int32_t sdrkit_fm_get_scan_low(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_low", value);
}
static inline int32_t sdrkit_fm_set_scan_low(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_low", value);
}

// The setting "scan_high" (Scan to) of the stack "fm". Unit: MHz. Where the band scan ends.
static inline int32_t sdrkit_fm_get_scan_high(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_high", value);
}
static inline int32_t sdrkit_fm_set_scan_high(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_high", value);
}

// The setting "scan_full_rds" (Scan for full RDS) of the stack "fm". The band scan stays on each station until its whole name, its radio text and its code have come in. Slower, but complete.
static inline int32_t sdrkit_fm_get_scan_full_rds(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan_full_rds", value);
}
static inline int32_t sdrkit_fm_set_scan_full_rds(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan_full_rds", value);
}

// The setting "station_finder" (Station finder) of the stack "fm". For recordings and the test signal only: finds a station that is off the centre and tunes to it.
static inline int32_t sdrkit_fm_get_station_finder(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "station_finder", value);
}
static inline int32_t sdrkit_fm_set_station_finder(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "station_finder", value);
}

// The setting "rds_variant" (RDS meanings) of the stack "fm". A value of `sdrkit_fm_rds_variant`. How programme types and station codes are read. Auto reads RDS if the station sends a country code outside the Americas, else RBDS on the channel raster of RBDS (odd tenths of a MHz from 87.9 to 107.9 MHz) and RDS elsewhere.
static inline int32_t sdrkit_fm_get_rds_variant(SDRKIT_HANDLE handle, sdrkit_fm_rds_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rds_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_rds_variant)index;
  return code;
}
static inline int32_t sdrkit_fm_set_rds_variant(SDRKIT_HANDLE handle, sdrkit_fm_rds_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "rds_variant", (int32_t)value);
}

// The setting "rds_correction" (RDS correction) of the stack "fm". A value of `sdrkit_fm_rds_correction`. How much the stack repairs damaged RDS data. Full gets the most from a weak station. A wrong character is still never shown.
static inline int32_t sdrkit_fm_get_rds_correction(SDRKIT_HANDLE handle, sdrkit_fm_rds_correction *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rds_correction", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_rds_correction)index;
  return code;
}
static inline int32_t sdrkit_fm_set_rds_correction(SDRKIT_HANDLE handle, sdrkit_fm_rds_correction value) {
  return sdrkit_typed_setting_set_enum(handle, "rds_correction", (int32_t)value);
}

// The setting "rds_voting" (RDS voting) of the stack "fm". With full correction, also repairs data by comparing repeated copies.
static inline int32_t sdrkit_fm_get_rds_voting(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "rds_voting", value);
}
static inline int32_t sdrkit_fm_set_rds_voting(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "rds_voting", value);
}

// The setting "rds_charset" (RDS characters) of the stack "fm". A value of `sdrkit_fm_rds_charset`. How station names and texts are read. Auto reads Chinese (GB2312) text when it sees it.
static inline int32_t sdrkit_fm_get_rds_charset(SDRKIT_HANDLE handle, sdrkit_fm_rds_charset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rds_charset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_rds_charset)index;
  return code;
}
static inline int32_t sdrkit_fm_set_rds_charset(SDRKIT_HANDLE handle, sdrkit_fm_rds_charset value) {
  return sdrkit_typed_setting_set_enum(handle, "rds_charset", (int32_t)value);
}

// The setting "tx_pi" (PI code) of the stack "fm". Four hexadecimal digits, 0001 to FFFF, from the licence of the station. No default: the transmission does not start while it is empty.
static inline int32_t sdrkit_fm_get_tx_pi(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "tx_pi", value);
}
static inline int32_t sdrkit_fm_set_tx_pi(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "tx_pi", value);
}

// The setting "tx_ps" (Station name (PS)) of the stack "fm". Up to 8 characters, padded with spaces. A change goes out from the next name cycle.
static inline int32_t sdrkit_fm_get_tx_ps(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "tx_ps", value);
}
static inline int32_t sdrkit_fm_set_tx_ps(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "tx_ps", value);
}

// The setting "tx_rt" (Radio text) of the stack "fm". Up to 64 characters. Empty sends no radio text. A change goes out from the next text cycle with the A/B flag turned over, so that receivers clear the old text.
static inline int32_t sdrkit_fm_get_tx_rt(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "tx_rt", value);
}
static inline int32_t sdrkit_fm_set_tx_rt(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "tx_rt", value);
}

// The setting "tx_pty" (Programme type) of the stack "fm". The code of the programme type. The names of the codes differ between RDS and RBDS.
static inline int32_t sdrkit_fm_get_tx_pty(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_pty", value);
}
static inline int32_t sdrkit_fm_set_tx_pty(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_pty", value);
}

// The setting "tx_tp" (Traffic programme) of the stack "fm". The station carries traffic announcements.
static inline int32_t sdrkit_fm_get_tx_tp(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_tp", value);
}
static inline int32_t sdrkit_fm_set_tx_tp(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_tp", value);
}

// The setting "tx_ta" (Traffic announcement now) of the stack "fm". A traffic announcement is on air now.
static inline int32_t sdrkit_fm_get_tx_ta(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_ta", value);
}
static inline int32_t sdrkit_fm_set_tx_ta(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_ta", value);
}

// The setting "tx_music" (Music) of the stack "fm". On for music, off for speech.
static inline int32_t sdrkit_fm_get_tx_music(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_music", value);
}
static inline int32_t sdrkit_fm_set_tx_music(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_music", value);
}

// The setting "tx_af" (Alternative frequencies) of the stack "fm". Up to 25 frequencies of the same programme, 87.6 to 107.9 MHz in steps of 0.1 MHz, with spaces between them. Empty sends no list.
static inline int32_t sdrkit_fm_get_tx_af(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "tx_af", value);
}
static inline int32_t sdrkit_fm_set_tx_af(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "tx_af", value);
}

// The setting "tx_ct" (Send the clock time) of the stack "fm". Sends the time of the clock (UTC) in group 4A as each minute starts.
static inline int32_t sdrkit_fm_get_tx_ct(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_ct", value);
}
static inline int32_t sdrkit_fm_set_tx_ct(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_ct", value);
}

// The setting "tx_ct_offset" (Local time offset) of the stack "fm". Unit: half hours. The local time less UTC, in half hours, sent with the clock time.
static inline int32_t sdrkit_fm_get_tx_ct_offset(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "tx_ct_offset", value);
}
static inline int32_t sdrkit_fm_set_tx_ct_offset(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "tx_ct_offset", value);
}

// The setting "tx_stereo" (Stereo) of the stack "fm". Sends the pilot and the difference signal; off sends mono. Applies at the next start.
static inline int32_t sdrkit_fm_get_tx_stereo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_stereo", value);
}
static inline int32_t sdrkit_fm_set_tx_stereo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_stereo", value);
}

// The setting "tx_preemphasis" (Pre-emphasis) of the stack "fm". A value of `sdrkit_fm_tx_preemphasis`. 50 µs in Europe, Asia, Africa and Oceania; 75 µs in the Americas and South Korea. Applies at the next start.
static inline int32_t sdrkit_fm_get_tx_preemphasis(SDRKIT_HANDLE handle, sdrkit_fm_tx_preemphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tx_preemphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_tx_preemphasis)index;
  return code;
}
static inline int32_t sdrkit_fm_set_tx_preemphasis(SDRKIT_HANDLE handle, sdrkit_fm_tx_preemphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "tx_preemphasis", (int32_t)value);
}

// The setting "tx_deviation" (Peak deviation) of the stack "fm". A frequency in Hz. The deviation that the whole multiplex never passes: audio, pilot and RDS together. 75 kHz is the broadcast value. Applies at the next start.
static inline int32_t sdrkit_fm_get_tx_deviation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_deviation", value);
}
static inline int32_t sdrkit_fm_set_tx_deviation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_deviation", value);
}

// The setting "tx_pilot" (Pilot level) of the stack "fm". Unit: %. The pilot deviation as a share of the peak deviation. 8 to 10 % is the broadcast range. Applies at the next start.
static inline int32_t sdrkit_fm_get_tx_pilot(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_pilot", value);
}
static inline int32_t sdrkit_fm_set_tx_pilot(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_pilot", value);
}

// The setting "tx_rds_level" (RDS level) of the stack "fm". Unit: kHz. The peak deviation by the RDS subcarrier; 2 to 3 kHz is usual. Applies at the next start.
static inline int32_t sdrkit_fm_get_tx_rds_level(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tx_rds_level", value);
}
static inline int32_t sdrkit_fm_set_tx_rds_level(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tx_rds_level", value);
}

// The setting "tx_limiter" (Audio limiter) of the stack "fm". Holds the audio peaks under the deviation with a gain that has no steps. Off clips each channel at full scale. Applies at the next start.
static inline int32_t sdrkit_fm_get_tx_limiter(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tx_limiter", value);
}
static inline int32_t sdrkit_fm_set_tx_limiter(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tx_limiter", value);
}

// The setting "gen_stereo" (Stereo) of the test signal generator of the stack "fm". Sends a stereo signal with its pilot tone.
static inline int32_t sdrkit_fm_gen_get_stereo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_stereo", value);
}
static inline int32_t sdrkit_fm_gen_set_stereo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_stereo", value);
}

// The setting "gen_left_hz" (Left tone) of the test signal generator of the stack "fm". A frequency in Hz.
static inline int32_t sdrkit_fm_gen_get_left_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_left_hz", value);
}
static inline int32_t sdrkit_fm_gen_set_left_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_left_hz", value);
}

// The setting "gen_left_amp" (Left level) of the test signal generator of the stack "fm". 0 is silent.
static inline int32_t sdrkit_fm_gen_get_left_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_left_amp", value);
}
static inline int32_t sdrkit_fm_gen_set_left_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_left_amp", value);
}

// The setting "gen_right_hz" (Right tone) of the test signal generator of the stack "fm". A frequency in Hz.
static inline int32_t sdrkit_fm_gen_get_right_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_right_hz", value);
}
static inline int32_t sdrkit_fm_gen_set_right_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_right_hz", value);
}

// The setting "gen_right_amp" (Right level) of the test signal generator of the stack "fm". 0 is silent.
static inline int32_t sdrkit_fm_gen_get_right_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_right_amp", value);
}
static inline int32_t sdrkit_fm_gen_set_right_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_right_amp", value);
}

// The setting "gen_pilot" (Pilot) of the test signal generator of the stack "fm". Unit: %. Pilot injection, as a percentage of 75 kHz.
static inline int32_t sdrkit_fm_gen_get_pilot(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pilot", value);
}
static inline int32_t sdrkit_fm_gen_set_pilot(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pilot", value);
}

// The setting "gen_rds" (RDS) of the test signal generator of the stack "fm".
static inline int32_t sdrkit_fm_gen_get_rds(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_rds", value);
}
static inline int32_t sdrkit_fm_gen_set_rds(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_rds", value);
}

// The setting "gen_rds_khz" (RDS injection) of the test signal generator of the stack "fm". Unit: kHz.
static inline int32_t sdrkit_fm_gen_get_rds_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rds_khz", value);
}
static inline int32_t sdrkit_fm_gen_set_rds_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rds_khz", value);
}

// The setting "gen_ps" (Station name) of the test signal generator of the stack "fm". Up to 8 characters.
static inline int32_t sdrkit_fm_gen_get_ps(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ps", value);
}
static inline int32_t sdrkit_fm_gen_set_ps(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ps", value);
}

// The setting "gen_rt" (Radio text) of the test signal generator of the stack "fm". Up to 64 characters.
static inline int32_t sdrkit_fm_gen_get_rt(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_rt", value);
}
static inline int32_t sdrkit_fm_gen_set_rt(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_rt", value);
}

// The setting "gen_pi" (PI code) of the test signal generator of the stack "fm". Four hexadecimal digits.
static inline int32_t sdrkit_fm_gen_get_pi(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_pi", value);
}
static inline int32_t sdrkit_fm_gen_set_pi(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_pi", value);
}

// The setting "gen_pty" (Programme type) of the test signal generator of the stack "fm".
static inline int32_t sdrkit_fm_gen_get_pty(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_pty", value);
}
static inline int32_t sdrkit_fm_gen_set_pty(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_pty", value);
}

// The setting "gen_tp" (Traffic programme) of the test signal generator of the stack "fm".
static inline int32_t sdrkit_fm_gen_get_tp(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_tp", value);
}
static inline int32_t sdrkit_fm_gen_set_tp(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_tp", value);
}

// The setting "gen_ta" (Traffic announcement) of the test signal generator of the stack "fm".
static inline int32_t sdrkit_fm_gen_get_ta(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_ta", value);
}
static inline int32_t sdrkit_fm_gen_set_ta(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_ta", value);
}

// The setting "gen_music" (Music) of the test signal generator of the stack "fm".
static inline int32_t sdrkit_fm_gen_get_music(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_music", value);
}
static inline int32_t sdrkit_fm_gen_set_music(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_music", value);
}

// The setting "gen_preemphasis" (Pre-emphasis) of the test signal generator of the stack "fm". A value of `sdrkit_fm_gen_preemphasis`.
static inline int32_t sdrkit_fm_gen_get_preemphasis(SDRKIT_HANDLE handle, sdrkit_fm_gen_preemphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_preemphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fm_gen_preemphasis)index;
  return code;
}
static inline int32_t sdrkit_fm_gen_set_preemphasis(SDRKIT_HANDLE handle, sdrkit_fm_gen_preemphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_preemphasis", (int32_t)value);
}

// The setting "gen_extra_groups" (Extra RDS groups) of the test signal generator of the stack "fm". Blocks B, C and D of groups to send as well, in hexadecimal, for example "1000 00E2 B945; A400 524F 434B".
static inline int32_t sdrkit_fm_gen_get_extra_groups(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_extra_groups", value);
}
static inline int32_t sdrkit_fm_gen_set_extra_groups(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_extra_groups", value);
}

// Fills `rows` with the changed rows of the table "links" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_stl_links_rows(SDRKIT_HANDLE handle,
                                           sdrkit_stl_link_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.LinkRow", "links", rows, sizeof(sdrkit_stl_link_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "af" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_stl_af_rows(SDRKIT_HANDLE handle,
                                        sdrkit_fm_af_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.AfRow", "af", rows, sizeof(sdrkit_fm_af_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "label_tags" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_stl_label_tags_rows(SDRKIT_HANDLE handle,
                                                sdrkit_fm_label_tag_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.LabelTagRow", "label_tags", rows, sizeof(sdrkit_fm_label_tag_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "oda" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_stl_oda_rows(SDRKIT_HANDLE handle,
                                         sdrkit_fm_oda_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.OdaRow", "oda", rows, sizeof(sdrkit_fm_oda_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "eon" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_stl_eon_rows(SDRKIT_HANDLE handle,
                                         sdrkit_fm_eon_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.EonRow", "eon", rows, sizeof(sdrkit_fm_eon_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "eon_af" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_stl_eon_af_rows(SDRKIT_HANDLE handle,
                                            sdrkit_fm_eon_af_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.EonAfRow", "eon_af", rows, sizeof(sdrkit_fm_eon_af_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tmc" of the stack "stl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_stl_tmc_rows(SDRKIT_HANDLE handle,
                                         sdrkit_fm_tmc_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "fm.TmcRow", "tmc", rows, sizeof(sdrkit_fm_tmc_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "stl.link"
// (STL link changed). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_stl_link_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_stl_link_event *event) {
  return sdrkit_typed_event(handle, n, "stl.link", "fm.LinkEvent", event, sizeof(sdrkit_stl_link_event));
}

// Runs the command "clear" (Clear) of the stack "stl" on the session of `handle`. Forgets the station's name, text and the other RDS data.
static inline int32_t sdrkit_stl_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "stl" are the sdrkit_stl_gen_*() functions
// after the settings of its receiver.

// The setting "stereo" (Stereo) of the stack "stl". A value of `sdrkit_stl_stereo`. Auto plays stereo while the pilot tone is locked and moves towards mono as the signal gets weak. Mono always plays mono. Stereo plays full stereo whenever any pilot is there.
static inline int32_t sdrkit_stl_get_stereo(SDRKIT_HANDLE handle, sdrkit_stl_stereo *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "stereo", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_stereo)index;
  return code;
}
static inline int32_t sdrkit_stl_set_stereo(SDRKIT_HANDLE handle, sdrkit_stl_stereo value) {
  return sdrkit_typed_setting_set_enum(handle, "stereo", (int32_t)value);
}

// The setting "deemphasis" (De-emphasis) of the stack "stl". A value of `sdrkit_stl_deemphasis`. Undoes the treble boost stations add. 50 µs in Europe, Asia, Africa and Australia. 75 µs in the Americas and South Korea.
static inline int32_t sdrkit_stl_get_deemphasis(SDRKIT_HANDLE handle, sdrkit_stl_deemphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "deemphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_deemphasis)index;
  return code;
}
static inline int32_t sdrkit_stl_set_deemphasis(SDRKIT_HANDLE handle, sdrkit_stl_deemphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "deemphasis", (int32_t)value);
}

// The setting "deviation" (Full deviation) of the stack "stl". A frequency in Hz. The deviation of full modulation of the link: 75 kHz, or 50 kHz where a region uses that (ITU-R BS.450). It sets the sound level only; the measurements stay in Hz.
static inline int32_t sdrkit_stl_get_deviation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "deviation", value);
}
static inline int32_t sdrkit_stl_set_deviation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "deviation", value);
}

// The setting "auto_width" (Automatic width) of the stack "stl". The channel width follows the link: twice the peak deviation plus the highest multiplex frequency (60 kHz, 99 kHz with an SCA subcarrier), or the occupied width if that is more, with 35 % and 10 kHz of margin. Each change restarts the channel filter.
static inline int32_t sdrkit_stl_get_auto_width(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "auto_width", value);
}
static inline int32_t sdrkit_stl_set_auto_width(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "auto_width", value);
}

// The setting "width" (Channel width) of the stack "stl". A frequency in Hz. The channel width when the automatic width is off. At most 400 kHz and 0.9 times the sample rate.
static inline int32_t sdrkit_stl_get_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "width", value);
}
static inline int32_t sdrkit_stl_set_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "width", value);
}

// The setting "rds_variant" (RDS meanings) of the stack "stl". A value of `sdrkit_stl_rds_variant`. How programme types and station codes are read. Auto reads RDS if the station sends a country code outside the Americas, else RBDS on the channel raster of RBDS (odd tenths of a MHz from 87.9 to 107.9 MHz) and RDS elsewhere.
static inline int32_t sdrkit_stl_get_rds_variant(SDRKIT_HANDLE handle, sdrkit_stl_rds_variant *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rds_variant", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_rds_variant)index;
  return code;
}
static inline int32_t sdrkit_stl_set_rds_variant(SDRKIT_HANDLE handle, sdrkit_stl_rds_variant value) {
  return sdrkit_typed_setting_set_enum(handle, "rds_variant", (int32_t)value);
}

// The setting "rds_correction" (RDS correction) of the stack "stl". A value of `sdrkit_stl_rds_correction`. How much the stack repairs damaged RDS data. Full gets the most from a weak station. A wrong character is still never shown.
static inline int32_t sdrkit_stl_get_rds_correction(SDRKIT_HANDLE handle, sdrkit_stl_rds_correction *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rds_correction", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_rds_correction)index;
  return code;
}
static inline int32_t sdrkit_stl_set_rds_correction(SDRKIT_HANDLE handle, sdrkit_stl_rds_correction value) {
  return sdrkit_typed_setting_set_enum(handle, "rds_correction", (int32_t)value);
}

// The setting "rds_voting" (RDS voting) of the stack "stl". With full correction, also repairs data by comparing repeated copies.
static inline int32_t sdrkit_stl_get_rds_voting(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "rds_voting", value);
}
static inline int32_t sdrkit_stl_set_rds_voting(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "rds_voting", value);
}

// The setting "rds_charset" (RDS characters) of the stack "stl". A value of `sdrkit_stl_rds_charset`. How station names and texts are read. Auto reads Chinese (GB2312) text when it sees it.
static inline int32_t sdrkit_stl_get_rds_charset(SDRKIT_HANDLE handle, sdrkit_stl_rds_charset *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "rds_charset", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_rds_charset)index;
  return code;
}
static inline int32_t sdrkit_stl_set_rds_charset(SDRKIT_HANDLE handle, sdrkit_stl_rds_charset value) {
  return sdrkit_typed_setting_set_enum(handle, "rds_charset", (int32_t)value);
}

// The setting "gen_link" (Link) of the test signal generator of the stack "stl". A value of `sdrkit_stl_gen_link`. Composite sends the stereo multiplex with pilot and RDS, mono sends programme audio without a pilot, digital-like sends band-limited noise.
static inline int32_t sdrkit_stl_gen_get_link(SDRKIT_HANDLE handle, sdrkit_stl_gen_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_gen_link)index;
  return code;
}
static inline int32_t sdrkit_stl_gen_set_link(SDRKIT_HANDLE handle, sdrkit_stl_gen_link value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_link", (int32_t)value);
}

// The setting "gen_deviation" (Full deviation) of the test signal generator of the stack "stl". A frequency in Hz. The deviation of a full-scale multiplex.
static inline int32_t sdrkit_stl_gen_get_deviation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation", value);
}
static inline int32_t sdrkit_stl_gen_set_deviation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation", value);
}

// The setting "gen_sca" (SCA subcarriers) of the test signal generator of the stack "stl". Adds subcarriers at 67 and 92 kHz, each at 10 % of full deviation and frequency modulated by a 1 kHz tone.
static inline int32_t sdrkit_stl_gen_get_sca(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_sca", value);
}
static inline int32_t sdrkit_stl_gen_set_sca(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_sca", value);
}

// The setting "gen_width" (Digital width) of the test signal generator of the stack "stl". A frequency in Hz. The occupied width of the digital-like link.
static inline int32_t sdrkit_stl_gen_get_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_width", value);
}
static inline int32_t sdrkit_stl_gen_set_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_width", value);
}

// The setting "gen_left_hz" (Left tone) of the test signal generator of the stack "stl". A frequency in Hz.
static inline int32_t sdrkit_stl_gen_get_left_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_left_hz", value);
}
static inline int32_t sdrkit_stl_gen_set_left_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_left_hz", value);
}

// The setting "gen_left_amp" (Left level) of the test signal generator of the stack "stl". 0 is silent.
static inline int32_t sdrkit_stl_gen_get_left_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_left_amp", value);
}
static inline int32_t sdrkit_stl_gen_set_left_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_left_amp", value);
}

// The setting "gen_right_hz" (Right tone) of the test signal generator of the stack "stl". A frequency in Hz.
static inline int32_t sdrkit_stl_gen_get_right_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_right_hz", value);
}
static inline int32_t sdrkit_stl_gen_set_right_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_right_hz", value);
}

// The setting "gen_right_amp" (Right level) of the test signal generator of the stack "stl". 0 is silent.
static inline int32_t sdrkit_stl_gen_get_right_amp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_right_amp", value);
}
static inline int32_t sdrkit_stl_gen_set_right_amp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_right_amp", value);
}

// The setting "gen_rds" (RDS) of the test signal generator of the stack "stl".
static inline int32_t sdrkit_stl_gen_get_rds(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_rds", value);
}
static inline int32_t sdrkit_stl_gen_set_rds(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_rds", value);
}

// The setting "gen_ps" (Station name) of the test signal generator of the stack "stl". Up to 8 characters.
static inline int32_t sdrkit_stl_gen_get_ps(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ps", value);
}
static inline int32_t sdrkit_stl_gen_set_ps(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ps", value);
}

// The setting "gen_rt" (Radio text) of the test signal generator of the stack "stl". Up to 64 characters.
static inline int32_t sdrkit_stl_gen_get_rt(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_rt", value);
}
static inline int32_t sdrkit_stl_gen_set_rt(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_rt", value);
}

// The setting "gen_pi" (PI code) of the test signal generator of the stack "stl". Four hexadecimal digits.
static inline int32_t sdrkit_stl_gen_get_pi(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_pi", value);
}
static inline int32_t sdrkit_stl_gen_set_pi(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_pi", value);
}

// The setting "gen_pty" (Programme type) of the test signal generator of the stack "stl".
static inline int32_t sdrkit_stl_gen_get_pty(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_pty", value);
}
static inline int32_t sdrkit_stl_gen_set_pty(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_pty", value);
}

// The setting "gen_preemphasis" (Pre-emphasis) of the test signal generator of the stack "stl". A value of `sdrkit_stl_gen_preemphasis`.
static inline int32_t sdrkit_stl_gen_get_preemphasis(SDRKIT_HANDLE handle, sdrkit_stl_gen_preemphasis *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_preemphasis", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_stl_gen_preemphasis)index;
  return code;
}
static inline int32_t sdrkit_stl_gen_set_preemphasis(SDRKIT_HANDLE handle, sdrkit_stl_gen_preemphasis value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_preemphasis", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fm_fields_read(SDRKIT_HANDLE handle, sdrkit_fm_fields *fields) {
  return sdrkit_typed_fields(handle, "fm.FmFields", fields, sizeof(sdrkit_fm_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fm_tx_fields_read(SDRKIT_HANDLE handle, sdrkit_fm_tx_fields *fields) {
  return sdrkit_typed_fields(handle, "fm.FmTxFields", fields, sizeof(sdrkit_fm_tx_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_stl_fields_read(SDRKIT_HANDLE handle, sdrkit_stl_fields *fields) {
  return sdrkit_typed_fields(handle, "fm.StlFields", fields, sizeof(sdrkit_stl_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_stl_rds_part_read(SDRKIT_HANDLE handle, sdrkit_stl_rds_part *fields) {
  return sdrkit_typed_fields(handle, "fm.RdsPart", fields, sizeof(sdrkit_stl_rds_part));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_stl_label_fields_read(SDRKIT_HANDLE handle, sdrkit_stl_label_fields *fields) {
  return sdrkit_typed_fields(handle, "fm.LabelFields", fields, sizeof(sdrkit_stl_label_fields));
}

#endif
