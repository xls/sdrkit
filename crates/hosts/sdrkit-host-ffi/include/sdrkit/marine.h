/*
 * sdrkit: the typed interface of the stack family "marine", version 2.0. MIT licence.
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

#ifndef SDRKIT_MARINE_H
#define SDRKIT_MARINE_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_MARINE_VERSION_MAJOR 2
#define SDRKIT_MARINE_VERSION_MINOR 0

// The stack "ais": its id, and the keys of its event types.
#define SDRKIT_STACK_AIS "ais"
#define SDRKIT_EVENT_AIS_VESSEL "ais.vessel"
#define SDRKIT_EVENT_AIS_NMEA "ais.nmea"
#define SDRKIT_EVENT_AIS_CPA "ais.cpa"
#define SDRKIT_EVENT_AIS_GEOFENCE "ais.geofence"

// The stack "marine": its id, and the keys of its event types.
#define SDRKIT_STACK_MARINE "marine"
#define SDRKIT_EVENT_NAVTEX_MESSAGE "navtex.message"
#define SDRKIT_EVENT_DSC_CALL "dsc.call"

// The stack "vdes": its id, and the keys of its event types.
#define SDRKIT_STACK_VDES "vdes"
#define SDRKIT_EVENT_VDES_ASM "vdes.asm"
#define SDRKIT_EVENT_VDES_NMEA "vdes.nmea"
#define SDRKIT_EVENT_VDES_BURST "vdes.burst"

// The stack "marinevhf": its id, and the keys of its event types.
#define SDRKIT_STACK_MARINEVHF "marinevhf"
#define SDRKIT_EVENT_MARINEVHF_CALL "marinevhf.call"

// The stack "atis": its id, and the keys of its event types.
#define SDRKIT_STACK_ATIS "atis"
#define SDRKIT_EVENT_ATIS_MESSAGE "atis.message"

// The options of the setting "channels" of the stack "ais".
typedef enum sdrkit_ais_channels {
  // All four
  SDRKIT_AIS_CHANNELS_ALL = 0,
  // AIS 1 + 2 only
  SDRKIT_AIS_CHANNELS_AIS12 = 1,
  // 75 + 76 only
  SDRKIT_AIS_CHANNELS_LONGRANGE = 2,
} sdrkit_ais_channels;

// The options of the setting "service" of the stack "marine".
typedef enum sdrkit_marine_service {
  // Automatic
  SDRKIT_MARINE_SERVICE_AUTO = 0,
  // NAVTEX
  SDRKIT_MARINE_SERVICE_NAVTEX = 1,
  // DSC
  SDRKIT_MARINE_SERVICE_DSC = 2,
  // Weather fax
  SDRKIT_MARINE_SERVICE_WEFAX = 3,
} sdrkit_marine_service;

// The options of the setting "fax_lpm" of the stack "marine".
typedef enum sdrkit_marine_fax_lpm {
  // Detect
  SDRKIT_MARINE_FAX_LPM_DETECT = 0,
  // 60
  SDRKIT_MARINE_FAX_LPM_60 = 1,
  // 90
  SDRKIT_MARINE_FAX_LPM_90 = 2,
  // 120
  SDRKIT_MARINE_FAX_LPM_120 = 3,
  // 240
  SDRKIT_MARINE_FAX_LPM_240 = 4,
} sdrkit_marine_fax_lpm;

// The options of the setting "fax_ioc" of the stack "marine".
typedef enum sdrkit_marine_fax_ioc {
  // From the start tone
  SDRKIT_MARINE_FAX_IOC_TONE = 0,
  // 576
  SDRKIT_MARINE_FAX_IOC_576 = 1,
  // 288
  SDRKIT_MARINE_FAX_IOC_288 = 2,
} sdrkit_marine_fax_ioc;

// The options of the setting "fax_rotate" of the stack "marine".
typedef enum sdrkit_marine_fax_rotate {
  // None
  SDRKIT_MARINE_FAX_ROTATE_NONE = 0,
  // 1 turn
  SDRKIT_MARINE_FAX_ROTATE_1_TURN = 1,
  // 2 turns
  SDRKIT_MARINE_FAX_ROTATE_2_TURNS = 2,
  // 3 turns
  SDRKIT_MARINE_FAX_ROTATE_3_TURNS = 3,
  // 1 turn back
  SDRKIT_MARINE_FAX_ROTATE_1_TURN_BACK = 4,
} sdrkit_marine_fax_rotate;

// The options of the setting "gen_service" of the stack "marine".
typedef enum sdrkit_marine_gen_service {
  // NAVTEX
  SDRKIT_MARINE_GEN_SERVICE_NAVTEX = 0,
  // DSC
  SDRKIT_MARINE_GEN_SERVICE_DSC = 1,
  // Weather fax
  SDRKIT_MARINE_GEN_SERVICE_WEFAX = 2,
  // DSC on VHF
  SDRKIT_MARINE_GEN_SERVICE_DSC_VHF = 3,
} sdrkit_marine_gen_service;

// The options of `state` of `sdrkit_ais_fields`.
typedef enum sdrkit_ais_fields_state {
  // Searching
  SDRKIT_AIS_FIELDS_STATE_SEARCHING = 0,
  // Signal
  SDRKIT_AIS_FIELDS_STATE_SIGNAL = 1,
  // Decoding
  SDRKIT_AIS_FIELDS_STATE_DECODING = 2,
} sdrkit_ais_fields_state;

// The result fields of the AIS stack.
typedef struct sdrkit_ais_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The reception state of a display. A value of `sdrkit_ais_fields_state`.
  int32_t state;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Bursts detected.
  int64_t bursts;
  // Messages found on another frequency candidate or part of a burst.
  int64_t recovered;
  // Messages found after cancelling a stronger burst.
  int64_t cancelled;
  // Recovered and cancelled messages together.
  int64_t collision_recoveries;
  // Stations in the table.
  int64_t stations;
  // Stations with a position.
  int64_t with_position;
  // Stations that sent message 27.
  int64_t long_range_stations;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // Reports made.
  int64_t reports;
  // Signal time since the start. A length of time in seconds.
  double signal_time;
  // The view filter hides some stations.
  bool filtered;
  // Names in the name table.
  int64_t names;
  // True if `next_pass` holds a value.
  bool has_next_pass;
  // The start of the next pass of the satellite over the station. Seconds since 1970 (UTC).
  double next_pass;
  // True if `next_pass_elevation_deg` holds a value.
  bool has_next_pass_elevation_deg;
  // The highest elevation of the next pass. Unit: °.
  double next_pass_elevation_deg;
} sdrkit_ais_fields;

// A row of the AIS station table. The row key is the MMSI.
typedef struct sdrkit_ais_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station name, else its MMSI.
  sdrkit_string name;
  // The MMSI, nine digits.
  sdrkit_string mmsi;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The call sign of a station heard.
  sdrkit_string callsign;
  // The ship type in words, else the class.
  sdrkit_string type;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The position came from message 27.
  bool long_range_position;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // True if `status` holds a value.
  bool has_status;
  // The navigational status in words.
  sdrkit_string status;
  // True if `destination` holds a value.
  bool has_destination;
  // The destination.
  sdrkit_string destination;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
} sdrkit_ais_station_row;

// A row of the track points of the stations whose track is shown. The row key is the MMSI, `/` and the time of the point.
typedef struct sdrkit_ais_track_point_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station a track point belongs to.
  sdrkit_string station;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `time` holds a value.
  bool has_time;
  // When (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
} sdrkit_ais_track_point_row;

// A row of the count of messages by type. The row key is the message type in two digits.
typedef struct sdrkit_ais_counter_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The message type name.
  sdrkit_string type;
  // Messages on AIS 1.
  int64_t count_a;
  // Messages on AIS 2.
  int64_t count_b;
  // Messages on channel 75.
  int64_t count_c;
  // Messages on channel 76.
  int64_t count_d;
  // Messages on every channel.
  int64_t total;
} sdrkit_ais_counter_row;

// A row of the channel table of the AIS stack. The row key is the letter of the channel.
typedef struct sdrkit_ais_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel's name.
  sdrkit_string name;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Good messages in the last minute. Unit: 1/min.
  double messages_per_min;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // Noise floor, dBFS. Unit: dBFS.
  double noise_dbfs;
  // True if `doppler_hz` holds a value.
  bool has_doppler_hz;
  // The Doppler of the channel's bursts now (satellite mode). A frequency in Hz.
  double doppler_hz;
  // True if `doppler_rate_hz_s` holds a value.
  bool has_doppler_rate_hz_s;
  // The rate of the Doppler (satellite mode). Unit: Hz/s.
  double doppler_rate_hz_s;
} sdrkit_ais_channel_row;

// A row of the NMEA sentence table. The row key is the place of the sentence in the log.
typedef struct sdrkit_ais_sentence_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // An `!AIVDM` sentence.
  sdrkit_string sentence;
  // The signal time of the reception of the sentence. Unit: s.
  double time_s;
} sdrkit_ais_sentence_row;

// The options of `layer` of `sdrkit_ais_reception_row`.
typedef enum sdrkit_ais_reception_row_layer {
  // Direct
  SDRKIT_AIS_RECEPTION_ROW_LAYER_DIRECT = 0,
  // Recovered
  SDRKIT_AIS_RECEPTION_ROW_LAYER_RECOVERED = 1,
  // Cancelled
  SDRKIT_AIS_RECEPTION_ROW_LAYER_CANCELLED = 2,
} sdrkit_ais_reception_row_layer;

// A row of the reception table. The row key is the place of the reception in the log.
typedef struct sdrkit_ais_reception_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // The message type.
  int64_t msg_type;
  // The MMSI, nine digits.
  sdrkit_string mmsi;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // How a message was found. A value of `sdrkit_ais_reception_row_layer`.
  int32_t layer;
} sdrkit_ais_reception_row;

// The options of `kind` of `sdrkit_ais_alert_row`.
typedef enum sdrkit_ais_alert_row_kind {
  // Close approach
  SDRKIT_AIS_ALERT_ROW_KIND_CPA = 0,
  // Entered a fence
  SDRKIT_AIS_ALERT_ROW_KIND_ENTER = 1,
  // Left a fence
  SDRKIT_AIS_ALERT_ROW_KIND_EXIT = 2,
} sdrkit_ais_alert_row_kind;

// A row of the alert table. The row key is the place of the alert in the log.
typedef struct sdrkit_ais_alert_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
  // What an alert is about. A value of `sdrkit_ais_alert_row_kind`.
  int32_t kind;
  // True if `fence` holds a value.
  bool has_fence;
  // The fence of an enter or exit.
  sdrkit_string fence;
  // The MMSI, nine digits.
  sdrkit_string mmsi;
  // The station name, else its MMSI.
  sdrkit_string name;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The distance from the station (CPA) or the fence. Unit: m.
  double distance_m;
  // True if `cpa_m` holds a value.
  bool has_cpa_m;
  // The distance at the closest point of approach. Unit: m.
  double cpa_m;
  // True if `tcpa` holds a value.
  bool has_tcpa;
  // The time to the closest point of approach. A length of time in seconds.
  double tcpa;
} sdrkit_ais_alert_row;

// A row of the hop plan of a receiver that is narrower than the four channels. The row key is the number of the step.
typedef struct sdrkit_ais_hop_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The centre of a hopping step. A frequency in Hz.
  double centre_hz;
  // True if `dwell` holds a value.
  bool has_dwell;
  // The dwell of a hopping step (none for one step that covers all). A length of time in seconds.
  double dwell;
} sdrkit_ais_hop_row;

// The row of the selected station, with every value that the station sent. The row key is the MMSI.
typedef struct sdrkit_ais_selected_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station name, else its MMSI.
  sdrkit_string name;
  // The MMSI, nine digits.
  sdrkit_string mmsi;
  // The station class.
  sdrkit_string class_;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The call sign of a station heard.
  sdrkit_string callsign;
  // True if `imo` holds a value.
  bool has_imo;
  // The IMO number.
  int64_t imo;
  // True if `type` holds a value.
  bool has_type;
  // The ship type in words, else the class.
  sdrkit_string type;
  // True if `destination` holds a value.
  bool has_destination;
  // The destination.
  sdrkit_string destination;
  // True if `eta` holds a value.
  bool has_eta;
  // The estimated arrival, "MM-DD HH:MM UTC".
  sdrkit_string eta;
  // True if `draught_m` holds a value.
  bool has_draught_m;
  // The draught. Unit: m.
  double draught_m;
  // True if `length_m` holds a value.
  bool has_length_m;
  // The length. Unit: m.
  double length_m;
  // True if `beam_m` holds a value.
  bool has_beam_m;
  // The beam. Unit: m.
  double beam_m;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `heading_deg` holds a value.
  bool has_heading_deg;
  // The true heading. Unit: °.
  double heading_deg;
  // True if `rate_of_turn_deg_min` holds a value.
  bool has_rate_of_turn_deg_min;
  // The rate of turn. Unit: °/min.
  double rate_of_turn_deg_min;
  // True if `altitude_m` holds a value.
  bool has_altitude_m;
  // The altitude of a SAR aircraft. Unit: m.
  double altitude_m;
  // True if `status` holds a value.
  bool has_status;
  // The navigational status in words.
  sdrkit_string status;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // The type of the last message.
  int64_t last_type;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // It sent message 27.
  bool long_range;
  // True if `latency` holds a value.
  bool has_latency;
  // The position latency flag of message 27.
  bool latency;
  // The last sentences, one per line.
  sdrkit_string sentences;
} sdrkit_ais_selected_row;

// The options of `state` of `sdrkit_marine_hf_fields`.
typedef enum sdrkit_marine_hf_fields_state {
  // Searching
  SDRKIT_MARINE_HF_FIELDS_STATE_SEARCHING = 0,
  // Signal
  SDRKIT_MARINE_HF_FIELDS_STATE_SIGNAL = 1,
  // Decoding
  SDRKIT_MARINE_HF_FIELDS_STATE_DECODING = 2,
} sdrkit_marine_hf_fields_state;

// The options of `active_service` of `sdrkit_marine_hf_fields`.
typedef enum sdrkit_marine_hf_fields_active_service {
  // None
  SDRKIT_MARINE_HF_FIELDS_ACTIVE_SERVICE_NONE = 0,
  // NAVTEX
  SDRKIT_MARINE_HF_FIELDS_ACTIVE_SERVICE_NAVTEX = 1,
  // DSC
  SDRKIT_MARINE_HF_FIELDS_ACTIVE_SERVICE_DSC = 2,
  // DSC VHF
  SDRKIT_MARINE_HF_FIELDS_ACTIVE_SERVICE_DSC_VHF = 3,
  // Weather fax
  SDRKIT_MARINE_HF_FIELDS_ACTIVE_SERVICE_FAX = 4,
} sdrkit_marine_hf_fields_active_service;

// The options of `fax_state` of `sdrkit_marine_hf_fields`.
typedef enum sdrkit_marine_hf_fields_fax_state {
  // Waiting
  SDRKIT_MARINE_HF_FIELDS_FAX_STATE_WAITING = 0,
  // Phasing
  SDRKIT_MARINE_HF_FIELDS_FAX_STATE_PHASING = 1,
  // Receiving
  SDRKIT_MARINE_HF_FIELDS_FAX_STATE_RECEIVING = 2,
  // Stopped
  SDRKIT_MARINE_HF_FIELDS_FAX_STATE_STOPPED = 3,
} sdrkit_marine_hf_fields_fax_state;

// The result fields of the marine HF stack.
typedef struct sdrkit_marine_hf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The reception state of a display. A value of `sdrkit_marine_hf_fields_state`.
  int32_t state;
  // The service heard last. A value of `sdrkit_marine_hf_fields_active_service`.
  int32_t active_service;
  // An FSK pair stands above the noise.
  bool fsk_found;
  // True if `fsk_low_hz` holds a value.
  bool has_fsk_low_hz;
  // The lower FSK tone, from the dial. A frequency in Hz.
  double fsk_low_hz;
  // True if `fsk_high_hz` holds a value.
  bool has_fsk_high_hz;
  // The higher FSK tone, from the dial. A frequency in Hz.
  double fsk_high_hz;
  // True if `shift_hz` holds a value.
  bool has_shift_hz;
  // The measured shift. A frequency in Hz.
  double shift_hz;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // Symbol rate found, Hz. A frequency in Hz.
  double symbol_rate_hz;
  // True if `tone_level_db` holds a value.
  bool has_tone_level_db;
  // The level of the tones over the noise, per bin. Unit: dB.
  double tone_level_db;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // NAVTEX messages complete with no unreadable character.
  int64_t navtex_good;
  // Other NAVTEX messages.
  int64_t navtex_bad;
  // DSC calls with a good check.
  int64_t dsc_good;
  // DSC calls with a failed check.
  int64_t dsc_bad;
  // The DSC list holds a distress call.
  bool distress;
  // True if `fax_state` holds a value.
  bool has_fax_state;
  // The fax decoder's state. A value of `sdrkit_marine_hf_fields_fax_state`.
  int32_t fax_state;
  // True if `ioc` holds a value.
  bool has_ioc;
  // The index of cooperation.
  int64_t ioc;
  // True if `lpm` holds a value.
  bool has_lpm;
  // Lines per minute.
  int64_t lpm;
  // True if `lines` holds a value.
  bool has_lines;
  // Picture lines received.
  int64_t lines;
  // True if `width` holds a value.
  bool has_width;
  // Picture width, pixels.
  int64_t width;
  // True if `slant_ppm` holds a value.
  bool has_slant_ppm;
  // The slant in use. Unit: ppm.
  double slant_ppm;
  // True if `black_level_hz` holds a value.
  bool has_black_level_hz;
  // The black level. A frequency in Hz.
  double black_level_hz;
  // True if `last_tone_hz` holds a value.
  bool has_last_tone_hz;
  // The last start or stop tone measured. A frequency in Hz.
  double last_tone_hz;
  // True if `phasing_lines` holds a value.
  bool has_phasing_lines;
  // Phasing lines seen.
  int64_t phasing_lines;
  // True if `fax_snr_db` holds a value.
  bool has_fax_snr_db;
  // The fax SNR: tone over audio power, or the black-white swing over the noise of the black level. Unit: dB.
  double fax_snr_db;
  // True if `pictures_started` holds a value.
  bool has_pictures_started;
  // Pictures started.
  int64_t pictures_started;
  // True if `pictures_done` holds a value.
  bool has_pictures_done;
  // Pictures finished.
  int64_t pictures_done;
  // True if `end_reason` holds a value.
  bool has_end_reason;
  // Why the last picture ended.
  sdrkit_string end_reason;
  // True if `scan_channel_hz` holds a value.
  bool has_scan_channel_hz;
  // The channel the scanner holds. A frequency in Hz.
  double scan_channel_hz;
  // Reports made.
  int64_t reports;
  // Signal time since the start. A length of time in seconds.
  double signal_time;
} sdrkit_marine_hf_fields;

// A row of the NAVTEX message table. The row key is the place of the message in the log.
typedef struct sdrkit_marine_navtex_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When a message or call was heard (wall time). Seconds since 1970 (UTC).
  double time;
  // B1 B2 B3 B4.
  sdrkit_string header;
  // The name of the subject.
  sdrkit_string subject_name;
  // Characters of the text.
  int64_t chars;
  // Unreadable characters.
  int64_t errors;
  // NNNN was seen.
  bool complete;
  // Times the message came again.
  int64_t repeats;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_marine_navtex_row;

// A row of the DSC call table. The row key is the place of the call in the log.
typedef struct sdrkit_marine_dsc_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When a message or call was heard (wall time). Seconds since 1970 (UTC).
  double time;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // The kind of call in words.
  sdrkit_string kind;
  // True if `from_mmsi` holds a value.
  bool has_from_mmsi;
  // The sender's MMSI.
  sdrkit_string from_mmsi;
  // True if `from_name` holds a value.
  bool has_from_name;
  // The sender's name.
  sdrkit_string from_name;
  // True if `from_country` holds a value.
  bool has_from_country;
  // The sender's country.
  sdrkit_string from_country;
  // True if `to_mmsi` holds a value.
  bool has_to_mmsi;
  // The called station.
  sdrkit_string to_mmsi;
  // True if `category_name` holds a value.
  bool has_category_name;
  // The category in words.
  sdrkit_string category_name;
  // True if `nature_name` holds a value.
  bool has_nature_name;
  // The nature of distress in words.
  sdrkit_string nature_name;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `telecommands` holds a value.
  bool has_telecommands;
  // The telecommands in words.
  sdrkit_string telecommands;
  // True if `freq_rx` holds a value.
  bool has_freq_rx;
  // The receive frequency or channel the call proposes.
  sdrkit_string freq_rx;
  // True if `freq_tx` holds a value.
  bool has_freq_tx;
  // The transmit frequency or channel the call proposes.
  sdrkit_string freq_tx;
  // The error-check character matched.
  bool ecc_ok;
  // Symbols taken from the second copy or rebuilt.
  int64_t repaired;
  // Symbols unreadable in both copies.
  int64_t erasures;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_marine_dsc_row;

// The result fields of the VDES stack.
typedef struct sdrkit_vdes_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Bursts found on the VDE-TER channels.
  int64_t bursts;
  // Reports made.
  int64_t reports;
} sdrkit_vdes_fields;

// A row of the ASM frame table. The row key is the place of the frame in the log.
typedef struct sdrkit_vdes_asm_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // The payload length.
  int64_t bits;
  // True if `msg_type` holds a value.
  bool has_msg_type;
  // The AIS message type of the payload.
  int64_t msg_type;
  // True if `mmsi` holds a value.
  bool has_mmsi;
  // The MMSI, nine digits.
  sdrkit_string mmsi;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // The payload in hexadecimal.
  sdrkit_string hex;
} sdrkit_vdes_asm_row;

// The options of `modulation` of `sdrkit_vdes_burst_row`.
typedef enum sdrkit_vdes_burst_row_modulation {
  // pi/4-QPSK
  SDRKIT_VDES_BURST_ROW_MODULATION_PI_4_Q_P_S_K = 0,
  // 8PSK
  SDRKIT_VDES_BURST_ROW_MODULATION_8_P_S_K = 1,
  // 16QAM
  SDRKIT_VDES_BURST_ROW_MODULATION_16_Q_A_M = 2,
  // Unknown
  SDRKIT_VDES_BURST_ROW_MODULATION_UNKNOWN = 3,
} sdrkit_vdes_burst_row_modulation;

// A row of the VDE-TER burst table. The row key is the place of the burst in the log.
typedef struct sdrkit_vdes_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
  // The centre of a burst's occupied sub-channels. A frequency in Hz.
  double centre_hz;
  // The bandwidth of a burst. A frequency in Hz.
  double bandwidth_hz;
  // The modulation of a burst. A value of `sdrkit_vdes_burst_row_modulation`.
  int32_t modulation;
  // The peak power of a burst. Unit: dBFS.
  double rssi_dbfs;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // How long a burst lasted. A length of time in seconds.
  double duration;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // The spread of the symbol amplitudes.
  double cv;
  // The mean of cos(4 x phase step).
  double psi;
  // The symbols used for the classification.
  int64_t symbols;
  // The occupied sub-channels.
  sdrkit_string subchannels;
} sdrkit_vdes_burst_row;

// A row of the channel table of the VDES stack. The row key is the letter of an ASM channel, or `TER` and the number of a VDE-TER band.
typedef struct sdrkit_vdes_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of a channel.
  sdrkit_string name;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // The capture holds the channel.
  bool in_view;
  // True if `frames_ok` holds a value.
  bool has_frames_ok;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // True if `frames_bad` holds a value.
  bool has_frames_bad;
  // Frames that failed their check.
  int64_t frames_bad;
  // True if `bursts` holds a value.
  bool has_bursts;
  // Bursts found on the VDE-TER channels.
  int64_t bursts;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // Noise floor, dBFS. Unit: dBFS.
  double noise_dbfs;
} sdrkit_vdes_channel_row;

// The options of `modulation` of `sdrkit_vdes_modulation_row`.
typedef enum sdrkit_vdes_modulation_row_modulation {
  // pi/4-QPSK
  SDRKIT_VDES_MODULATION_ROW_MODULATION_PI_4_Q_P_S_K = 0,
  // 8PSK
  SDRKIT_VDES_MODULATION_ROW_MODULATION_8_P_S_K = 1,
  // 16QAM
  SDRKIT_VDES_MODULATION_ROW_MODULATION_16_Q_A_M = 2,
  // Unknown
  SDRKIT_VDES_MODULATION_ROW_MODULATION_UNKNOWN = 3,
} sdrkit_vdes_modulation_row_modulation;

// A row of the count of bursts by modulation. The row key is the name of the modulation.
typedef struct sdrkit_vdes_modulation_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The modulation of a burst. A value of `sdrkit_vdes_modulation_row_modulation`.
  int32_t modulation;
  // A number of bursts.
  int64_t count;
} sdrkit_vdes_modulation_row;

// A row of the count of bursts by bandwidth. The row key is the bandwidth in Hz.
typedef struct sdrkit_vdes_bandwidth_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The bandwidth of a burst. A frequency in Hz.
  double bandwidth_hz;
  // A number of bursts.
  int64_t count;
} sdrkit_vdes_bandwidth_row;

// The options of `scan_state` of `sdrkit_marinevhf_vhf_fields`.
typedef enum sdrkit_marinevhf_vhf_fields_scan_state {
  // Idle
  SDRKIT_MARINEVHF_VHF_FIELDS_SCAN_STATE_IDLE = 0,
  // Priority channel
  SDRKIT_MARINEVHF_VHF_FIELDS_SCAN_STATE_PRIORITY = 1,
  // Looking at the priority channel
  SDRKIT_MARINEVHF_VHF_FIELDS_SCAN_STATE_PRIORITY_PROBE = 2,
  // Scanning
  SDRKIT_MARINEVHF_VHF_FIELDS_SCAN_STATE_SCANNING = 3,
  // Holding
  SDRKIT_MARINEVHF_VHF_FIELDS_SCAN_STATE_HOLDING = 4,
} sdrkit_marinevhf_vhf_fields_scan_state;

// The result fields of the marine VHF stack.
typedef struct sdrkit_marinevhf_vhf_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Carriers the finder tracks.
  int64_t carriers;
  // Carriers active now.
  int64_t active_channels;
  // True if `playing` holds a value.
  bool has_playing;
  // The channel that plays.
  sdrkit_string playing;
  // True if `playing_hz` holds a value.
  bool has_playing_hz;
  // The frequency that plays. A frequency in Hz.
  double playing_hz;
  // What the player does. A value of `sdrkit_marinevhf_vhf_fields_scan_state`.
  int32_t scan_state;
  // True if `tune_hz` holds a value.
  bool has_tune_hz;
  // The frequency the single-tuner scan requested from the radio. A frequency in Hz.
  double tune_hz;
  // Calls since the start.
  int64_t calls;
  // The capture holds more than one channel.
  bool multi_channel;
  // Reports made.
  int64_t reports;
} sdrkit_marinevhf_vhf_fields;

// A row of the call table of the marine VHF stack. The row key is the place of the call in the log.
typedef struct sdrkit_marinevhf_vhf_call_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of a call.
  int64_t serial;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // True if `time` holds a value.
  bool has_time;
  // When a call started (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
  // The signal time a call started. A length of time in seconds.
  double start;
  // How long a call lasted. A length of time in seconds.
  double duration;
  // The highest level of a call or channel. Unit: dBFS.
  double peak_dbfs;
  // The mean level of a call. Unit: dBFS.
  double mean_dbfs;
  // The call or channel is open.
  bool open;
} sdrkit_marinevhf_vhf_call_row;

// A row of the channel table of the marine VHF stack. The row key is the channel frequency in units of the raster.
typedef struct sdrkit_marinevhf_vhf_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // The channel carries a signal now.
  bool active;
  // The call or channel is open.
  bool open;
  // The channel plays.
  bool now_playing;
  // The channel is in the scan list.
  bool in_scan;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Calls on the channel.
  int64_t calls;
  // The time the channel was open in all. A length of time in seconds.
  double total;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // True if `peak_dbfs` holds a value.
  bool has_peak_dbfs;
  // The highest level of a call or channel. Unit: dBFS.
  double peak_dbfs;
} sdrkit_marinevhf_vhf_channel_row;

// The result fields of the ATIS stack.
typedef struct sdrkit_atis_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Checked DSC calls of another format.
  int64_t other_calls;
  // Stations in the table.
  int64_t stations;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // Reports made.
  int64_t reports;
} sdrkit_atis_fields;

// A row of the ATIS message table. The row key is the place of the message in the log.
typedef struct sdrkit_atis_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time` holds a value.
  bool has_time;
  // When (wall-clock time, when known). Seconds since 1970 (UTC).
  double time;
  // The ten digits.
  sdrkit_string digits;
  // The call sign of a station heard.
  sdrkit_string callsign;
  // The nine digits after the 9.
  sdrkit_string mmsi;
  // The country of the MID.
  sdrkit_string country;
  // The name from the name table.
  sdrkit_string name;
  // The check symbol agrees.
  bool check_ok;
  // The identity starts with 9.
  bool standard_z;
  // The tones were swapped.
  bool inverted;
  // Symbols taken from the second copy or rebuilt.
  int64_t repaired;
  // Symbols unreadable in both copies.
  int64_t erasures;
  // The level over the floor. Unit: dB.
  double level_db;
  // The offset of the branch that found the message. A frequency in Hz.
  double branch_hz;
} sdrkit_atis_message_row;

// A row of the ATIS station table. The row key is the digits of the identity.
typedef struct sdrkit_atis_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The ten digits.
  sdrkit_string digits;
  // The call sign of a station heard.
  sdrkit_string callsign;
  // The nine digits after the 9.
  sdrkit_string mmsi;
  // The country of the MID.
  sdrkit_string country;
  // The name from the name table.
  sdrkit_string name;
  // The best level over the floor. Unit: dB.
  double best_level_db;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_atis_station_row;

// The options of `family` of `sdrkit_ais_vessel_event`.
typedef enum sdrkit_ais_vessel_event_family {
  // Aircraft
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AIS_VESSEL_EVENT_FAMILY_OTHER = 12,
} sdrkit_ais_vessel_event_family;

// The options of `urgency` of `sdrkit_ais_vessel_event`.
typedef enum sdrkit_ais_vessel_event_urgency {
  // None
  SDRKIT_AIS_VESSEL_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_AIS_VESSEL_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_AIS_VESSEL_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_AIS_VESSEL_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_ais_vessel_event_urgency;

// The fields of the event of a station heard.
typedef struct sdrkit_ais_vessel_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ais_vessel_event_family`.
  int32_t family;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_ais_vessel_event_urgency`.
  int32_t urgency;
  // True if `text` holds a value.
  bool has_text;
  // The text of a decoded message.
  sdrkit_string text;
  // The station name, else its MMSI.
  sdrkit_string name;
  // The call sign of a station heard.
  sdrkit_string callsign;
  // True if `ship_type` holds a value.
  bool has_ship_type;
  // The ship and cargo type code.
  int64_t ship_type;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `nav_status` holds a value.
  bool has_nav_status;
  // The navigational status code.
  int64_t nav_status;
  // The station class.
  sdrkit_string class_;
  // The destination.
  sdrkit_string destination;
} sdrkit_ais_vessel_event;

// The options of `family` of `sdrkit_ais_nmea_event`.
typedef enum sdrkit_ais_nmea_event_family {
  // Aircraft
  SDRKIT_AIS_NMEA_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AIS_NMEA_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AIS_NMEA_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AIS_NMEA_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AIS_NMEA_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AIS_NMEA_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AIS_NMEA_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AIS_NMEA_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AIS_NMEA_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AIS_NMEA_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AIS_NMEA_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AIS_NMEA_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AIS_NMEA_EVENT_FAMILY_OTHER = 12,
} sdrkit_ais_nmea_event_family;

// The fields of the event of an NMEA sentence.
typedef struct sdrkit_ais_nmea_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ais_nmea_event_family`.
  int32_t family;
  // An `!AIVDM` sentence.
  sdrkit_string sentence;
} sdrkit_ais_nmea_event;

// The options of `family` of `sdrkit_ais_cpa_event`.
typedef enum sdrkit_ais_cpa_event_family {
  // Aircraft
  SDRKIT_AIS_CPA_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AIS_CPA_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AIS_CPA_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AIS_CPA_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AIS_CPA_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AIS_CPA_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AIS_CPA_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AIS_CPA_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AIS_CPA_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AIS_CPA_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AIS_CPA_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AIS_CPA_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AIS_CPA_EVENT_FAMILY_OTHER = 12,
} sdrkit_ais_cpa_event_family;

// The options of `urgency` of `sdrkit_ais_cpa_event`.
typedef enum sdrkit_ais_cpa_event_urgency {
  // None
  SDRKIT_AIS_CPA_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_AIS_CPA_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_AIS_CPA_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_AIS_CPA_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_ais_cpa_event_urgency;

// The fields of the event of a closest point of approach alert.
typedef struct sdrkit_ais_cpa_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ais_cpa_event_family`.
  int32_t family;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_ais_cpa_event_urgency`.
  int32_t urgency;
  // The station name, else its MMSI.
  sdrkit_string name;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The distance from the station (CPA) or the fence. Unit: m.
  double distance_m;
  // True if `cpa_m` holds a value.
  bool has_cpa_m;
  // The distance at the closest point of approach. Unit: m.
  double cpa_m;
  // True if `tcpa` holds a value.
  bool has_tcpa;
  // The time to the closest point of approach. A length of time in seconds.
  double tcpa;
} sdrkit_ais_cpa_event;

// The options of `family` of `sdrkit_ais_geofence_event`.
typedef enum sdrkit_ais_geofence_event_family {
  // Aircraft
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AIS_GEOFENCE_EVENT_FAMILY_OTHER = 12,
} sdrkit_ais_geofence_event_family;

// The options of `urgency` of `sdrkit_ais_geofence_event`.
typedef enum sdrkit_ais_geofence_event_urgency {
  // None
  SDRKIT_AIS_GEOFENCE_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_AIS_GEOFENCE_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_AIS_GEOFENCE_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_AIS_GEOFENCE_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_ais_geofence_event_urgency;

// The options of `transition` of `sdrkit_ais_geofence_event`.
typedef enum sdrkit_ais_geofence_event_transition {
  // Entered
  SDRKIT_AIS_GEOFENCE_EVENT_TRANSITION_ENTER = 0,
  // Left
  SDRKIT_AIS_GEOFENCE_EVENT_TRANSITION_EXIT = 1,
} sdrkit_ais_geofence_event_transition;

// The fields of the event of a geofence alert.
typedef struct sdrkit_ais_geofence_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ais_geofence_event_family`.
  int32_t family;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_ais_geofence_event_urgency`.
  int32_t urgency;
  // The station name, else its MMSI.
  sdrkit_string name;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The distance from the station (CPA) or the fence. Unit: m.
  double distance_m;
  // The fence of an enter or exit.
  sdrkit_string fence;
  // A fence crossed: enter or exit. A value of `sdrkit_ais_geofence_event_transition`.
  int32_t transition;
} sdrkit_ais_geofence_event;

// The options of `family` of `sdrkit_marine_navtex_message_event`.
typedef enum sdrkit_marine_navtex_message_event_family {
  // Aircraft
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_marine_navtex_message_event_family;

// The options of `urgency` of `sdrkit_marine_navtex_message_event`.
typedef enum sdrkit_marine_navtex_message_event_urgency {
  // None
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_MARINE_NAVTEX_MESSAGE_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_marine_navtex_message_event_urgency;

// The fields of the event of a NAVTEX message.
typedef struct sdrkit_marine_navtex_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_marine_navtex_message_event_family`.
  int32_t family;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_marine_navtex_message_event_urgency`.
  int32_t urgency;
  // The text of a decoded message.
  sdrkit_string text;
  // B1 B2 B3 B4.
  sdrkit_string header;
  // B2, the subject indicator.
  sdrkit_string subject;
  // The name of the subject.
  sdrkit_string subject_name;
  // B3 B4, the serial number.
  int64_t number;
  // NNNN was seen.
  bool complete;
  // Unreadable characters.
  int64_t errors;
  // Characters of the text.
  int64_t chars;
} sdrkit_marine_navtex_message_event;

// The options of `family` of `sdrkit_marine_dsc_call_event`.
typedef enum sdrkit_marine_dsc_call_event_family {
  // Aircraft
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MARINE_DSC_CALL_EVENT_FAMILY_OTHER = 12,
} sdrkit_marine_dsc_call_event_family;

// The options of `urgency` of `sdrkit_marine_dsc_call_event`.
typedef enum sdrkit_marine_dsc_call_event_urgency {
  // None
  SDRKIT_MARINE_DSC_CALL_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_MARINE_DSC_CALL_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_MARINE_DSC_CALL_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_MARINE_DSC_CALL_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_marine_dsc_call_event_urgency;

// The fields of the event of a DSC call.
typedef struct sdrkit_marine_dsc_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_marine_dsc_call_event_family`.
  int32_t family;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_marine_dsc_call_event_urgency`.
  int32_t urgency;
  // The text of a decoded message.
  sdrkit_string text;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The format specifier.
  int64_t format;
  // The kind of call in words.
  sdrkit_string kind;
  // True if `category` holds a value.
  bool has_category;
  // The category code.
  int64_t category;
  // True if `category_name` holds a value.
  bool has_category_name;
  // The category in words.
  sdrkit_string category_name;
  // True if `from_name` holds a value.
  bool has_from_name;
  // The sender's name.
  sdrkit_string from_name;
  // True if `from_country` holds a value.
  bool has_from_country;
  // The sender's country.
  sdrkit_string from_country;
  // True if `to_type` holds a value.
  bool has_to_type;
  // What the called station is.
  sdrkit_string to_type;
  // True if `distress_mmsi` holds a value.
  bool has_distress_mmsi;
  // The station in distress.
  sdrkit_string distress_mmsi;
  // True if `nature` holds a value.
  bool has_nature;
  // The nature of distress code.
  int64_t nature;
  // True if `nature_name` holds a value.
  bool has_nature_name;
  // The nature of distress in words.
  sdrkit_string nature_name;
  // True if `position_text` holds a value.
  bool has_position_text;
  // The position in the call, as sent.
  sdrkit_string position_text;
  // True if `utc` holds a value.
  bool has_utc;
  // The time in the call (HH:MM UTC).
  sdrkit_string utc;
  // True if `telecommand1` holds a value.
  bool has_telecommand1;
  // The first telecommand.
  int64_t telecommand1;
  // True if `telecommand2` holds a value.
  bool has_telecommand2;
  // The second telecommand.
  int64_t telecommand2;
  // True if `telecommands` holds a value.
  bool has_telecommands;
  // The telecommands in words.
  sdrkit_string telecommands;
  // The receive frequency or channel the call proposes.
  sdrkit_string freq_rx;
  // The transmit frequency or channel the call proposes.
  sdrkit_string freq_tx;
  // True if `eos` holds a value.
  bool has_eos;
  // The end of sequence character.
  int64_t eos;
  // The error-check character matched.
  bool ecc_ok;
  // Symbols taken from the second copy or rebuilt.
  int64_t repaired;
  // Symbols unreadable in both copies.
  int64_t erasures;
  // Heard on VHF channel 70.
  bool vhf;
} sdrkit_marine_dsc_call_event;

// The options of `family` of `sdrkit_vdes_asm_event`.
typedef enum sdrkit_vdes_asm_event_family {
  // Aircraft
  SDRKIT_VDES_ASM_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_VDES_ASM_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_VDES_ASM_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_VDES_ASM_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_VDES_ASM_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_VDES_ASM_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_VDES_ASM_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_VDES_ASM_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_VDES_ASM_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_VDES_ASM_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_VDES_ASM_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_VDES_ASM_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_VDES_ASM_EVENT_FAMILY_OTHER = 12,
} sdrkit_vdes_asm_event_family;

// The fields of the event of an ASM frame.
typedef struct sdrkit_vdes_asm_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_vdes_asm_event_family`.
  int32_t family;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // The payload length.
  int64_t bits;
  // The payload in hexadecimal.
  sdrkit_string hex;
  // True if `msg_type` holds a value.
  bool has_msg_type;
  // The AIS message type of the payload.
  int64_t msg_type;
} sdrkit_vdes_asm_event;

// The options of `family` of `sdrkit_vdes_nmea_event`.
typedef enum sdrkit_vdes_nmea_event_family {
  // Aircraft
  SDRKIT_VDES_NMEA_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_VDES_NMEA_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_VDES_NMEA_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_VDES_NMEA_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_VDES_NMEA_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_VDES_NMEA_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_VDES_NMEA_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_VDES_NMEA_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_VDES_NMEA_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_VDES_NMEA_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_VDES_NMEA_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_VDES_NMEA_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_VDES_NMEA_EVENT_FAMILY_OTHER = 12,
} sdrkit_vdes_nmea_event_family;

// The fields of the event of an NMEA sentence of an ASM frame.
typedef struct sdrkit_vdes_nmea_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_vdes_nmea_event_family`.
  int32_t family;
  // An `!AIVDM` sentence.
  sdrkit_string sentence;
} sdrkit_vdes_nmea_event;

// The options of `family` of `sdrkit_vdes_burst_event`.
typedef enum sdrkit_vdes_burst_event_family {
  // Aircraft
  SDRKIT_VDES_BURST_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_VDES_BURST_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_VDES_BURST_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_VDES_BURST_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_VDES_BURST_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_VDES_BURST_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_VDES_BURST_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_VDES_BURST_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_VDES_BURST_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_VDES_BURST_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_VDES_BURST_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_VDES_BURST_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_VDES_BURST_EVENT_FAMILY_OTHER = 12,
} sdrkit_vdes_burst_event_family;

// The options of `modulation` of `sdrkit_vdes_burst_event`.
typedef enum sdrkit_vdes_burst_event_modulation {
  // pi/4-QPSK
  SDRKIT_VDES_BURST_EVENT_MODULATION_PI_4_Q_P_S_K = 0,
  // 8PSK
  SDRKIT_VDES_BURST_EVENT_MODULATION_8_P_S_K = 1,
  // 16QAM
  SDRKIT_VDES_BURST_EVENT_MODULATION_16_Q_A_M = 2,
  // Unknown
  SDRKIT_VDES_BURST_EVENT_MODULATION_UNKNOWN = 3,
} sdrkit_vdes_burst_event_modulation;

// The fields of the event of a VDE-TER burst.
typedef struct sdrkit_vdes_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_vdes_burst_event_family`.
  int32_t family;
  // The modulation of a burst. A value of `sdrkit_vdes_burst_event_modulation`.
  int32_t modulation;
  // The bandwidth of a burst. A frequency in Hz.
  double bandwidth_hz;
  // How long a burst lasted. A length of time in seconds.
  double duration;
} sdrkit_vdes_burst_event;

// The options of `family` of `sdrkit_marinevhf_call_event`.
typedef enum sdrkit_marinevhf_call_event_family {
  // Aircraft
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MARINEVHF_CALL_EVENT_FAMILY_OTHER = 12,
} sdrkit_marinevhf_call_event_family;

// The fields of the event of a call on a marine VHF channel.
typedef struct sdrkit_marinevhf_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_marinevhf_call_event_family`.
  int32_t family;
  // The signal time a call started. A length of time in seconds.
  double start;
  // How long a call lasted. A length of time in seconds.
  double duration;
  // The highest level of a call or channel. Unit: dBFS.
  double peak_dbfs;
} sdrkit_marinevhf_call_event;

// The options of `family` of `sdrkit_atis_message_event`.
typedef enum sdrkit_atis_message_event_family {
  // Aircraft
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ATIS_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_atis_message_event_family;

// The fields of the event of an ATIS message.
typedef struct sdrkit_atis_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `target` holds a value.
  bool has_target;
  // The receiver of a message or call: an address, a call sign or a talk group member.
  sdrkit_string target;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel or talk group of a call.
  sdrkit_string channel;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_atis_message_event_family`.
  int32_t family;
  // The call sign of a station heard.
  sdrkit_string callsign;
  // The nine digits after the 9.
  sdrkit_string mmsi;
  // The country of the MID.
  sdrkit_string country;
  // The check symbol agrees.
  bool check_ok;
} sdrkit_atis_message_event;

// The arguments of the commands `select` and `remove` of the AIS stack.
typedef struct sdrkit_ais_station_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_ais_station_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_AIS_STATION_ARGS_INIT {sizeof(sdrkit_ais_station_args), {0}}

// The arguments of the command `remove` of the ATIS stack.
typedef struct sdrkit_atis_remove_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the station.
  sdrkit_string row;
} sdrkit_atis_remove_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ATIS_REMOVE_ARGS_INIT {sizeof(sdrkit_atis_remove_args), {0}}

// Fills `rows` with the changed rows of the table "stations" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Place of a row: the column `position`.
// Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_ais_stations_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ais_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisStationRow", "stations", rows, sizeof(sdrkit_ais_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "track_points" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `station` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_ais_track_points_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_ais_track_point_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisTrackPointRow", "track_points", rows, sizeof(sdrkit_ais_track_point_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "counters" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `type` names a row.
static inline int32_t sdrkit_ais_counters_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ais_counter_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisCounterRow", "counters", rows, sizeof(sdrkit_ais_counter_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Default order: the column `freq_hz`, lowest first.
static inline int32_t sdrkit_ais_channels_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ais_channel_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisChannelRow", "channels", rows, sizeof(sdrkit_ais_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sentences" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_ais_sentences_rows(SDRKIT_HANDLE handle,
                                               sdrkit_ais_sentence_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisSentenceRow", "sentences", rows, sizeof(sdrkit_ais_sentence_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "receptions" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `mmsi` names a row.
// Time: `time`.
static inline int32_t sdrkit_ais_receptions_rows(SDRKIT_HANDLE handle,
                                                sdrkit_ais_reception_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisReceptionRow", "receptions", rows, sizeof(sdrkit_ais_reception_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "alerts" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `name` names a row.
// Place of a row: the column `position`.
// Time: `time`.
static inline int32_t sdrkit_ais_alerts_rows(SDRKIT_HANDLE handle,
                                            sdrkit_ais_alert_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisAlertRow", "alerts", rows, sizeof(sdrkit_ais_alert_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "hop_plan" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `centre_hz`, lowest first.
static inline int32_t sdrkit_ais_hop_plan_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ais_hop_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisHopRow", "hop_plan", rows, sizeof(sdrkit_ais_hop_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "selected" of the stack "ais" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Place of a row: the column `position`.
// Height: `altitude_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_ais_selected_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ais_selected_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AisSelectedRow", "selected", rows, sizeof(sdrkit_ais_selected_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ais.vessel"
// (Vessel). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ais_vessel_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ais_vessel_event *event) {
  return sdrkit_typed_event(handle, n, "ais.vessel", "marine.AisVesselEvent", event, sizeof(sdrkit_ais_vessel_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ais.nmea"
// (AIS sentence). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ais_nmea_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ais_nmea_event *event) {
  return sdrkit_typed_event(handle, n, "ais.nmea", "marine.AisNmeaEvent", event, sizeof(sdrkit_ais_nmea_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ais.cpa"
// (CPA alert). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ais_cpa_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ais_cpa_event *event) {
  return sdrkit_typed_event(handle, n, "ais.cpa", "marine.AisCpaEvent", event, sizeof(sdrkit_ais_cpa_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ais.geofence"
// (Geofence alert). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ais_geofence_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ais_geofence_event *event) {
  return sdrkit_typed_event(handle, n, "ais.geofence", "marine.AisGeofenceEvent", event, sizeof(sdrkit_ais_geofence_event));
}

// Runs the command "select" (Select) of the stack "ais" on the session of `handle`. Shows one station in full.
static inline int32_t sdrkit_ais_select(SDRKIT_HANDLE handle, const sdrkit_ais_station_args *args) {
  return sdrkit_typed_command(handle, "select", "marine.AisStationArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "ais" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ais_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ais" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ais_remove(SDRKIT_HANDLE handle, const sdrkit_ais_station_args *args) {
  return sdrkit_typed_command(handle, "remove", "marine.AisStationArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ais" are the sdrkit_ais_gen_*() functions
// after the settings of its receiver.

// The setting "channels" (Channels) of the stack "ais". A value of `sdrkit_ais_channels`. Which AIS channels are decoded.
static inline int32_t sdrkit_ais_get_channels(SDRKIT_HANDLE handle, sdrkit_ais_channels *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channels", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ais_channels)index;
  return code;
}
static inline int32_t sdrkit_ais_set_channels(SDRKIT_HANDLE handle, sdrkit_ais_channels value) {
  return sdrkit_typed_setting_set_enum(handle, "channels", (int32_t)value);
}

// The setting "collision_cancel" (Collision handling) of the stack "ais". Tries again on other frequencies and after cancelling a stronger burst.
static inline int32_t sdrkit_ais_get_collision_cancel(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "collision_cancel", value);
}
static inline int32_t sdrkit_ais_set_collision_cancel(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "collision_cancel", value);
}

// The setting "satellite" (Satellite mode) of the stack "ais". Wider filters, Doppler candidates and successive cancellation for reception from orbit.
static inline int32_t sdrkit_ais_get_satellite(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "satellite", value);
}
static inline int32_t sdrkit_ais_set_satellite(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "satellite", value);
}

// The setting "hop" (Hopping) of the stack "ais". Alternates between AIS 1 + 2 and 75 + 76 when the radio cannot hold all four.
static inline int32_t sdrkit_ais_get_hop(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hop", value);
}
static inline int32_t sdrkit_ais_set_hop(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hop", value);
}

// The setting "time_base" (Time base) of the stack "ais". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_ais_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_ais_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// The setting "satellite_tle" (Satellite elements) of the stack "ais". Two-line elements of the satellite (with checksums), for predicted Doppler and passes.
static inline int32_t sdrkit_ais_get_satellite_tle(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "satellite_tle", value);
}
static inline int32_t sdrkit_ais_set_satellite_tle(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "satellite_tle", value);
}

// The setting "min_elevation" (Minimum elevation) of the stack "ais". Unit: °.
static inline int32_t sdrkit_ais_get_min_elevation(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_elevation", value);
}
static inline int32_t sdrkit_ais_set_min_elevation(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_elevation", value);
}

// The setting "arm_passes" (Arm the passes) of the stack "ais". Predicts the next pass over the station. The station position is needed.
static inline int32_t sdrkit_ais_get_arm_passes(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "arm_passes", value);
}
static inline int32_t sdrkit_ais_set_arm_passes(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "arm_passes", value);
}

// The setting "cpa" (CPA alerts) of the stack "ais". Alerts for vessels that will pass close to the station. The station position is needed.
static inline int32_t sdrkit_ais_get_cpa(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "cpa", value);
}
static inline int32_t sdrkit_ais_set_cpa(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "cpa", value);
}

// The setting "cpa_nm" (CPA limit) of the stack "ais". Unit: nm.
static inline int32_t sdrkit_ais_get_cpa_nm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "cpa_nm", value);
}
static inline int32_t sdrkit_ais_set_cpa_nm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "cpa_nm", value);
}

// The setting "tcpa_min" (TCPA limit) of the stack "ais". Unit: min.
static inline int32_t sdrkit_ais_get_tcpa_min(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tcpa_min", value);
}
static inline int32_t sdrkit_ais_set_tcpa_min(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tcpa_min", value);
}

// The setting "cpa_cooldown_min" (CPA cool-down) of the stack "ais". Unit: min.
static inline int32_t sdrkit_ais_get_cpa_cooldown_min(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "cpa_cooldown_min", value);
}
static inline int32_t sdrkit_ais_set_cpa_cooldown_min(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "cpa_cooldown_min", value);
}

// The setting "own_moving" (Own ship moving) of the stack "ais".
static inline int32_t sdrkit_ais_get_own_moving(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "own_moving", value);
}
static inline int32_t sdrkit_ais_set_own_moving(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "own_moving", value);
}

// The setting "own_sog_kn" (Own speed) of the stack "ais". Unit: kt.
static inline int32_t sdrkit_ais_get_own_sog_kn(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "own_sog_kn", value);
}
static inline int32_t sdrkit_ais_set_own_sog_kn(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "own_sog_kn", value);
}

// The setting "own_cog_deg" (Own course) of the stack "ais". Unit: °.
static inline int32_t sdrkit_ais_get_own_cog_deg(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "own_cog_deg", value);
}
static inline int32_t sdrkit_ais_set_own_cog_deg(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "own_cog_deg", value);
}

// The setting "geofences" (Geofences) of the stack "ais". One per line: name;lat,lon,radius_nm or name;poly;lat,lon lat,lon ...
static inline int32_t sdrkit_ais_get_geofences(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "geofences", value);
}
static inline int32_t sdrkit_ais_set_geofences(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "geofences", value);
}

// The setting "filter" (Filter) of the stack "ais". classes=63;groups=1023;moving=0;sog=0.5
static inline int32_t sdrkit_ais_get_filter(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "filter", value);
}
static inline int32_t sdrkit_ais_set_filter(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "filter", value);
}

// The setting "nmea_out" (Send AIS as NMEA) of the stack "ais".
static inline int32_t sdrkit_ais_get_nmea_out(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "nmea_out", value);
}
static inline int32_t sdrkit_ais_set_nmea_out(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "nmea_out", value);
}

// The setting "names" (Name table) of the stack "ais". Ship and station names by MMSI to import, one record per line.
static inline int32_t sdrkit_ais_get_names(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "names", value);
}
static inline int32_t sdrkit_ais_set_names(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "names", value);
}

// The setting "gen_vessels" (Vessels) of the test signal generator of the stack "ais". Vessels in the port demo, beside the base station, two aids and a SAR aircraft.
static inline int32_t sdrkit_ais_gen_get_vessels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_vessels", value);
}
static inline int32_t sdrkit_ais_gen_set_vessels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_vessels", value);
}

// The setting "gen_speed" (Speed of time) of the test signal generator of the stack "ais". Vessels move this many times faster than real time.
static inline int32_t sdrkit_ais_gen_get_speed(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_speed", value);
}
static inline int32_t sdrkit_ais_gen_set_speed(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_speed", value);
}

// Fills `rows` with the changed rows of the table "navtex" of the stack "marine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `header` names a row.
// Text: `text`. Time: `time`.
static inline int32_t sdrkit_marine_navtex_rows(SDRKIT_HANDLE handle,
                                               sdrkit_marine_navtex_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.NavtexRow", "navtex", rows, sizeof(sdrkit_marine_navtex_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "dsc" of the stack "marine" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the calls of the stack.
// The column `from_mmsi` names a row.
// Place of a row: the column `position`.
// Sender: `from_mmsi`. Addressee: `to_mmsi`. Text: `text`. Time: `time`.
static inline int32_t sdrkit_marine_dsc_rows(SDRKIT_HANDLE handle,
                                            sdrkit_marine_dsc_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.DscRow", "dsc", rows, sizeof(sdrkit_marine_dsc_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "navtex.message"
// (NAVTEX message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_marine_navtex_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_marine_navtex_message_event *event) {
  return sdrkit_typed_event(handle, n, "navtex.message", "marine.NavtexMessageEvent", event, sizeof(sdrkit_marine_navtex_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dsc.call"
// (DSC call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_marine_dsc_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_marine_dsc_call_event *event) {
  return sdrkit_typed_event(handle, n, "dsc.call", "marine.DscCallEvent", event, sizeof(sdrkit_marine_dsc_call_event));
}

// Runs the command "clear" (Clear) of the stack "marine" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_marine_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "clear_picture" (Clear the picture) of the stack "marine" on the session of `handle`. Starts the fax picture again.
static inline int32_t sdrkit_marine_clear_picture(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear_picture", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "marine" are the sdrkit_marine_gen_*() functions
// after the settings of its receiver.

// The setting "service" (Service) of the stack "marine". A value of `sdrkit_marine_service`. Which decoders run. Automatic chooses from the dial.
static inline int32_t sdrkit_marine_get_service(SDRKIT_HANDLE handle, sdrkit_marine_service *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "service", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_marine_service)index;
  return code;
}
static inline int32_t sdrkit_marine_set_service(SDRKIT_HANDLE handle, sdrkit_marine_service value) {
  return sdrkit_typed_setting_set_enum(handle, "service", (int32_t)value);
}

// The setting "fax_lpm" (Lines per minute) of the stack "marine". A value of `sdrkit_marine_fax_lpm`.
static inline int32_t sdrkit_marine_get_fax_lpm(SDRKIT_HANDLE handle, sdrkit_marine_fax_lpm *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fax_lpm", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_marine_fax_lpm)index;
  return code;
}
static inline int32_t sdrkit_marine_set_fax_lpm(SDRKIT_HANDLE handle, sdrkit_marine_fax_lpm value) {
  return sdrkit_typed_setting_set_enum(handle, "fax_lpm", (int32_t)value);
}

// The setting "fax_ioc" (Index of cooperation) of the stack "marine". A value of `sdrkit_marine_fax_ioc`.
static inline int32_t sdrkit_marine_get_fax_ioc(SDRKIT_HANDLE handle, sdrkit_marine_fax_ioc *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fax_ioc", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_marine_fax_ioc)index;
  return code;
}
static inline int32_t sdrkit_marine_set_fax_ioc(SDRKIT_HANDLE handle, sdrkit_marine_fax_ioc value) {
  return sdrkit_typed_setting_set_enum(handle, "fax_ioc", (int32_t)value);
}

// The setting "fax_auto_slant" (Automatic slant) of the stack "marine".
static inline int32_t sdrkit_marine_get_fax_auto_slant(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "fax_auto_slant", value);
}
static inline int32_t sdrkit_marine_set_fax_auto_slant(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "fax_auto_slant", value);
}

// The setting "fax_slant_ppm" (Slant) of the stack "marine". Unit: ppm.
static inline int32_t sdrkit_marine_get_fax_slant_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "fax_slant_ppm", value);
}
static inline int32_t sdrkit_marine_set_fax_slant_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "fax_slant_ppm", value);
}

// The setting "fax_max_lines" (Most lines) of the stack "marine".
static inline int32_t sdrkit_marine_get_fax_max_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "fax_max_lines", value);
}
static inline int32_t sdrkit_marine_set_fax_max_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "fax_max_lines", value);
}

// The setting "fax_rotate" (Rotation) of the stack "marine". A value of `sdrkit_marine_fax_rotate`.
static inline int32_t sdrkit_marine_get_fax_rotate(SDRKIT_HANDLE handle, sdrkit_marine_fax_rotate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fax_rotate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_marine_fax_rotate)index;
  return code;
}
static inline int32_t sdrkit_marine_set_fax_rotate(SDRKIT_HANDLE handle, sdrkit_marine_fax_rotate value) {
  return sdrkit_typed_setting_set_enum(handle, "fax_rotate", (int32_t)value);
}

// The setting "row_filter" (Row filter) of the stack "marine". Smooths the noise along each row of a weak fax picture, with the noise measured on the row.
static inline int32_t sdrkit_marine_get_row_filter(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "row_filter", value);
}
static inline int32_t sdrkit_marine_set_row_filter(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "row_filter", value);
}

// The setting "listen" (Listen) of the stack "marine".
static inline int32_t sdrkit_marine_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_marine_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "dsc_scan" (DSC scan) of the stack "marine".
static inline int32_t sdrkit_marine_get_dsc_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "dsc_scan", value);
}
static inline int32_t sdrkit_marine_set_dsc_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "dsc_scan", value);
}

// The setting "dsc_scan_sets" (DSC channel sets) of the stack "marine". distress, worldwide, series_a, series_b, vhf
static inline int32_t sdrkit_marine_get_dsc_scan_sets(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "dsc_scan_sets", value);
}
static inline int32_t sdrkit_marine_set_dsc_scan_sets(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "dsc_scan_sets", value);
}

// The setting "dsc_scan_dwell_s" (DSC dwell) of the stack "marine". A length of time in seconds.
static inline int32_t sdrkit_marine_get_dsc_scan_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dsc_scan_dwell_s", value);
}
static inline int32_t sdrkit_marine_set_dsc_scan_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dsc_scan_dwell_s", value);
}

// The setting "dsc_parallel" (Parallel DSC) of the stack "marine".
static inline int32_t sdrkit_marine_get_dsc_parallel(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "dsc_parallel", value);
}
static inline int32_t sdrkit_marine_set_dsc_parallel(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "dsc_parallel", value);
}

// The setting "names" (Name table) of the stack "marine". Ship and coast station names by MMSI to import, one record per line.
static inline int32_t sdrkit_marine_get_names(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "names", value);
}
static inline int32_t sdrkit_marine_set_names(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "names", value);
}

// The setting "time_base" (Time base) of the stack "marine". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_marine_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_marine_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// The setting "gen_service" (Test signal) of the test signal generator of the stack "marine". A value of `sdrkit_marine_gen_service`. What the test signal sends when no submode names it.
static inline int32_t sdrkit_marine_gen_get_service(SDRKIT_HANDLE handle, sdrkit_marine_gen_service *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_service", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_marine_gen_service)index;
  return code;
}
static inline int32_t sdrkit_marine_gen_set_service(SDRKIT_HANDLE handle, sdrkit_marine_gen_service value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_service", (int32_t)value);
}

// The setting "gen_idle_s" (NAVTEX phasing) of the test signal generator of the stack "marine". A length of time in seconds.
static inline int32_t sdrkit_marine_gen_get_idle_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_idle_s", value);
}
static inline int32_t sdrkit_marine_gen_set_idle_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_idle_s", value);
}

// The setting "gen_ioc" (Fax IOC) of the test signal generator of the stack "marine".
static inline int32_t sdrkit_marine_gen_get_ioc(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_ioc", value);
}
static inline int32_t sdrkit_marine_gen_set_ioc(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_ioc", value);
}

// The setting "gen_lines" (Fax lines) of the test signal generator of the stack "marine".
static inline int32_t sdrkit_marine_gen_get_lines(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_lines", value);
}
static inline int32_t sdrkit_marine_gen_set_lines(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_lines", value);
}

// The setting "gen_lpm" (Fax lines per minute) of the test signal generator of the stack "marine".
static inline int32_t sdrkit_marine_gen_get_lpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lpm", value);
}
static inline int32_t sdrkit_marine_gen_set_lpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lpm", value);
}

// The setting "gen_phasing_s" (Fax phasing) of the test signal generator of the stack "marine". A length of time in seconds.
static inline int32_t sdrkit_marine_gen_get_phasing_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_phasing_s", value);
}
static inline int32_t sdrkit_marine_gen_set_phasing_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_phasing_s", value);
}

// Fills `rows` with the changed rows of the table "asm" of the stack "vdes" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `mmsi` names a row.
// Place of a row: the column `position`.
// Course: `course_deg`. Speed: `speed_kt`.
// Time: `time`.
static inline int32_t sdrkit_vdes_asm_rows(SDRKIT_HANDLE handle,
                                          sdrkit_vdes_asm_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VdesAsmRow", "asm", rows, sizeof(sdrkit_vdes_asm_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "vdes" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time`.
static inline int32_t sdrkit_vdes_bursts_rows(SDRKIT_HANDLE handle,
                                             sdrkit_vdes_burst_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VdesBurstRow", "bursts", rows, sizeof(sdrkit_vdes_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "vdes" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Default order: the column `freq_hz`, lowest first.
static inline int32_t sdrkit_vdes_channels_rows(SDRKIT_HANDLE handle,
                                               sdrkit_vdes_channel_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VdesChannelRow", "channels", rows, sizeof(sdrkit_vdes_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_modulation" of the stack "vdes" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `modulation` names a row.
static inline int32_t sdrkit_vdes_by_modulation_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_vdes_modulation_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VdesModulationRow", "by_modulation", rows, sizeof(sdrkit_vdes_modulation_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_bandwidth" of the stack "vdes" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `bandwidth_hz` names a row.
// Default order: the column `bandwidth_hz`, lowest first.
static inline int32_t sdrkit_vdes_by_bandwidth_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_vdes_bandwidth_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VdesBandwidthRow", "by_bandwidth", rows, sizeof(sdrkit_vdes_bandwidth_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "vdes.asm"
// (ASM frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_vdes_asm_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_vdes_asm_event *event) {
  return sdrkit_typed_event(handle, n, "vdes.asm", "marine.VdesAsmEvent", event, sizeof(sdrkit_vdes_asm_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "vdes.nmea"
// (ASM sentence). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_vdes_nmea_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_vdes_nmea_event *event) {
  return sdrkit_typed_event(handle, n, "vdes.nmea", "marine.VdesSentenceEvent", event, sizeof(sdrkit_vdes_nmea_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "vdes.burst"
// (VDE-TER burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_vdes_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_vdes_burst_event *event) {
  return sdrkit_typed_event(handle, n, "vdes.burst", "marine.VdesBurstEvent", event, sizeof(sdrkit_vdes_burst_event));
}

// Runs the command "clear" (Clear) of the stack "vdes" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_vdes_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "nmea_out" (Send ASM as NMEA) of the stack "vdes". AIS-like ASM frames go out as !AIVDM sentences.
static inline int32_t sdrkit_vdes_get_nmea_out(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "nmea_out", value);
}
static inline int32_t sdrkit_vdes_set_nmea_out(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "nmea_out", value);
}

// The setting "time_base" (Time base) of the stack "vdes". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_vdes_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_vdes_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "marinevhf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the calls of the stack.
// The column `channel` names a row.
// Time: `time`.
static inline int32_t sdrkit_marinevhf_calls_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_marinevhf_vhf_call_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VhfCallRow", "calls", rows, sizeof(sdrkit_marinevhf_vhf_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "marinevhf" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `channel` names a row.
// Default order: the column `freq_hz`, lowest first.
// Time: `last_heard`.
static inline int32_t sdrkit_marinevhf_channels_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_marinevhf_vhf_channel_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.VhfChannelRow", "channels", rows, sizeof(sdrkit_marinevhf_vhf_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "marinevhf.call"
// (Call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_marinevhf_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_marinevhf_call_event *event) {
  return sdrkit_typed_event(handle, n, "marinevhf.call", "marine.VhfCallEvent", event, sizeof(sdrkit_marinevhf_call_event));
}

// Runs the command "clear" (Clear) of the stack "marinevhf" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_marinevhf_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "close_all" (Close all) of the stack "marinevhf" on the session of `handle`. Ends every open call.
static inline int32_t sdrkit_marinevhf_close_all(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "close_all", NULL, NULL, NULL, NULL, 0);
}

// The setting "plan" (Channel plan) of the stack "marinevhf". The channels: entries separated by ";", each a label, "=" and the frequency in Hz, or the ship and the coast frequency of a duplex pair separated by "/", for example "16=156800000" or "1=156050000/160650000". An entry that ends in ":data" is a channel of data, left out of an empty scan list. At most 200 entries. The host fills it from a channel plan file. Empty: no channel has a label.
static inline int32_t sdrkit_marinevhf_get_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "plan", value);
}
static inline int32_t sdrkit_marinevhf_set_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "plan", value);
}

// The setting "channel_hz" (Channel) of the stack "marinevhf". A frequency in Hz. The frequency of the channel to play; either half of a duplex pair of the plan plays both. 0 plays the channel that is active.
static inline int32_t sdrkit_marinevhf_get_channel_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_hz", value);
}
static inline int32_t sdrkit_marinevhf_set_channel_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_hz", value);
}

// The setting "squelch" (Squelch) of the stack "marinevhf". Unit: %.
static inline int32_t sdrkit_marinevhf_get_squelch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch", value);
}
static inline int32_t sdrkit_marinevhf_set_squelch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch", value);
}

// The setting "deemphasis" (De-emphasis) of the stack "marinevhf".
static inline int32_t sdrkit_marinevhf_get_deemphasis(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deemphasis", value);
}
static inline int32_t sdrkit_marinevhf_set_deemphasis(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deemphasis", value);
}

// The setting "priority_hz" (Priority channel) of the stack "marinevhf". A frequency in Hz. A channel that takes over while it carries a call. The default is 156.8 MHz, the distress, safety and calling frequency of the ITU Radio Regulations (Appendix 15).
static inline int32_t sdrkit_marinevhf_get_priority_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "priority_hz", value);
}
static inline int32_t sdrkit_marinevhf_set_priority_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "priority_hz", value);
}

// The setting "scan" (Scan) of the stack "marinevhf".
static inline int32_t sdrkit_marinevhf_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_marinevhf_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "scan_hz" (Scan list) of the stack "marinevhf". Frequencies in Hz separated by spaces, commas or semicolons; a frequency of a duplex pair of the plan scans both halves. Empty scans every voice channel of the plan.
static inline int32_t sdrkit_marinevhf_get_scan_hz(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "scan_hz", value);
}
static inline int32_t sdrkit_marinevhf_set_scan_hz(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "scan_hz", value);
}

// The setting "scan_dwell_s" (Dwell) of the stack "marinevhf". A length of time in seconds.
static inline int32_t sdrkit_marinevhf_get_scan_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_dwell_s", value);
}
static inline int32_t sdrkit_marinevhf_set_scan_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_dwell_s", value);
}

// The setting "scan_hang_s" (Hang) of the stack "marinevhf". A length of time in seconds.
static inline int32_t sdrkit_marinevhf_get_scan_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_hang_s", value);
}
static inline int32_t sdrkit_marinevhf_set_scan_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_hang_s", value);
}

// The setting "carriers" (Carriers) of the stack "marinevhf".
static inline int32_t sdrkit_marinevhf_get_carriers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "carriers", value);
}
static inline int32_t sdrkit_marinevhf_set_carriers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "carriers", value);
}

// The setting "show_heard" (Only channels heard) of the stack "marinevhf". Lists only the channels that are active now or had a call.
static inline int32_t sdrkit_marinevhf_get_show_heard(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "show_heard", value);
}
static inline int32_t sdrkit_marinevhf_set_show_heard(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "show_heard", value);
}

// The setting "time_base" (Time base) of the stack "marinevhf". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_marinevhf_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_marinevhf_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "atis" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `name` names a row.
// Sender: `callsign`. Time: `time`.
static inline int32_t sdrkit_atis_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_atis_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AtisMessageRow", "messages", rows, sizeof(sdrkit_atis_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "atis" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// The column `name` names a row.
// Time: `last_heard`.
static inline int32_t sdrkit_atis_stations_rows(SDRKIT_HANDLE handle,
                                               sdrkit_atis_station_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "marine.AtisStationRow", "stations", rows, sizeof(sdrkit_atis_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "atis.message"
// (ATIS message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_atis_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_atis_message_event *event) {
  return sdrkit_typed_event(handle, n, "atis.message", "marine.AtisMessageEvent", event, sizeof(sdrkit_atis_message_event));
}

// Runs the command "clear" (Clear) of the stack "atis" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_atis_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "atis" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_atis_remove(SDRKIT_HANDLE handle, const sdrkit_atis_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "marine.AtisRemoveArgs", args, NULL, NULL, 0);
}

// The setting "listen" (Listen) of the stack "atis". The voice of the channel on the audio output.
static inline int32_t sdrkit_atis_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_atis_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "names" (Name table) of the stack "atis". Ship and station names by MMSI to import, one record per line.
static inline int32_t sdrkit_atis_get_names(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "names", value);
}
static inline int32_t sdrkit_atis_set_names(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "names", value);
}

// The setting "plan" (Channel plan) of the stack "atis". The channels: entries separated by ";", each a label, "=" and the frequency in Hz, or the ship and the coast frequency of a duplex pair separated by "/", for example "16=156800000" or "1=156050000/160650000". An entry that ends in ":data" is a channel of data, left out of an empty scan list. At most 200 entries. The host fills it from a channel plan file. Empty: no channel has a label.
static inline int32_t sdrkit_atis_get_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "plan", value);
}
static inline int32_t sdrkit_atis_set_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "plan", value);
}

// The setting "time_base" (Time base) of the stack "atis". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_atis_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_atis_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ais_fields_read(SDRKIT_HANDLE handle, sdrkit_ais_fields *fields) {
  return sdrkit_typed_fields(handle, "marine.AisFields", fields, sizeof(sdrkit_ais_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_marine_hf_fields_read(SDRKIT_HANDLE handle, sdrkit_marine_hf_fields *fields) {
  return sdrkit_typed_fields(handle, "marine.HfFields", fields, sizeof(sdrkit_marine_hf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_vdes_fields_read(SDRKIT_HANDLE handle, sdrkit_vdes_fields *fields) {
  return sdrkit_typed_fields(handle, "marine.VdesFields", fields, sizeof(sdrkit_vdes_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_marinevhf_vhf_fields_read(SDRKIT_HANDLE handle, sdrkit_marinevhf_vhf_fields *fields) {
  return sdrkit_typed_fields(handle, "marine.VhfFields", fields, sizeof(sdrkit_marinevhf_vhf_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_atis_fields_read(SDRKIT_HANDLE handle, sdrkit_atis_fields *fields) {
  return sdrkit_typed_fields(handle, "marine.AtisFields", fields, sizeof(sdrkit_atis_fields));
}

#endif
