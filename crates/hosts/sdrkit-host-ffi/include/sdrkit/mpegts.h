/*
 * sdrkit: the typed interface of the shared interface "mpegts", whose items the stacks of several families show, version 1.1. MIT licence.
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

#ifndef SDRKIT_MPEGTS_H
#define SDRKIT_MPEGTS_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_MPEGTS_VERSION_MAJOR 1
#define SDRKIT_MPEGTS_VERSION_MINOR 1

// The keys of the event types.
#define SDRKIT_EVENT_TELETEXT_PAGE "teletext.page"
#define SDRKIT_EVENT_SERVICE_LIST "service.list"
#define SDRKIT_EVENT_EPG_EVENT "epg.event"
#define SDRKIT_EVENT_TS_TIME "ts.time"
#define SDRKIT_EVENT_TS_NETWORK "ts.network"
#define SDRKIT_EVENT_DVB_SUBTITLE "dvb.subtitle"

// The result fields of `mpegts.ts_monitor`: the counts of the transport stream, which a stack shows under its own keys.
typedef struct sdrkit_mpegts_ts_monitor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Packets seen.
  int64_t packets;
  // Packets with the transport_error_indicator.
  int64_t flagged;
  // Scrambled packets.
  int64_t scrambled;
  // Numbered packets that match.
  int64_t numbered_good;
  // Numbered packets that do not match.
  int64_t numbered_wrong;
  // Breaks in the numbering.
  int64_t numbered_gaps;
  // Bit rate over the last second. Unit: bit/s.
  double bitrate_bps;
  // A PAT was seen.
  bool pat_seen;
  // The service names of the SDT, one per line.
  sdrkit_string services;
} sdrkit_mpegts_ts_monitor_fields;

// The options of `player_status` of `sdrkit_mpegts_ts_demux_fields`.
typedef enum sdrkit_mpegts_ts_demux_fields_player_status {
  // Starting
  SDRKIT_MPEGTS_TS_DEMUX_FIELDS_PLAYER_STATUS_STARTING = 0,
  // Playing
  SDRKIT_MPEGTS_TS_DEMUX_FIELDS_PLAYER_STATUS_PLAYING = 1,
  // Scrambled
  SDRKIT_MPEGTS_TS_DEMUX_FIELDS_PLAYER_STATUS_SCRAMBLED = 2,
  // No service
  SDRKIT_MPEGTS_TS_DEMUX_FIELDS_PLAYER_STATUS_NO_SERVICE = 3,
} sdrkit_mpegts_ts_demux_fields_player_status;

// The result fields of `mpegts.ts_demux`: the counts of the packet layer and the selection.
typedef struct sdrkit_mpegts_ts_demux_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Packets seen.
  int64_t packets;
  // Null packets seen.
  int64_t null_packets;
  // Packets with the transport_error_indicator.
  int64_t tei_packets;
  // Continuity errors.
  int64_t cc_errors;
  // The bit rate of the multiplex. Unit: kbit/s.
  double mux_kbps;
  // The bit rate of the null packets. Unit: kbit/s.
  double null_kbps;
  // True if `selected_service` holds a value.
  bool has_selected_service;
  // The service that plays. No value while none plays.
  int64_t selected_service;
  // True if `selected_audio` holds a value.
  bool has_selected_audio;
  // The audio PID that plays. No value while none plays.
  int64_t selected_audio;
  // True if `selected_subtitle` holds a value.
  bool has_selected_subtitle;
  // The subtitle PID shown. No value while none is shown.
  int64_t selected_subtitle;
  // What the player does. A value of `sdrkit_mpegts_ts_demux_fields_player_status`.
  int32_t player_status;
  // True if `audio_decoded` holds a value.
  bool has_audio_decoded;
  // Whether the decoders of the library decode the audio that plays: absent while no audio plays, and when the host gave no decoder of MPEG audio or AAC (it decodes the coded audio itself).
  bool audio_decoded;
} sdrkit_mpegts_ts_demux_fields;

// The result fields of `mpegts.psi_si`: the network and the time of the stream.
typedef struct sdrkit_mpegts_psi_si_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The network name.
  sdrkit_string network_name;
  // network_id.
  int64_t network_id;
  // original_network_id.
  int64_t onid;
  // transport_stream_id.
  int64_t tsid;
  // The number of services.
  int64_t service_count;
  // True if `utc` holds a value.
  bool has_utc;
  // The UTC of the stream. No value before the stream gave the time. Seconds since 1970 (UTC).
  double utc;
  // True if `local_offset_min` holds a value.
  bool has_local_offset_min;
  // The local time offset in force. No value before the stream gave one. Unit: min.
  int64_t local_offset_min;
} sdrkit_mpegts_psi_si_fields;

// The result fields of `mpegts.dvb_subtitle`.
typedef struct sdrkit_mpegts_dvb_subtitle_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Display sets decoded.
  int64_t display_sets;
} sdrkit_mpegts_dvb_subtitle_fields;

// The result fields of `mpegts.teletext`: the counts of the stream and the page shown.
typedef struct sdrkit_mpegts_teletext_stream_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Teletext data units decoded.
  int64_t data_units;
  // Teletext pages held.
  int64_t pages;
  // The page shown.
  int64_t page;
  // True if `page_text` holds a value.
  bool has_page_text;
  // The text of the page shown, its rows joined with line feeds. No value while the page is not held.
  sdrkit_string page_text;
} sdrkit_mpegts_teletext_stream_fields;

// The options of `codec` of `sdrkit_mpegts_pes_video_fields`.
typedef enum sdrkit_mpegts_pes_video_fields_codec {
  // MPEG-1 video
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_MPEG1VIDEO = 0,
  // MPEG-2 video
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_MPEG2VIDEO = 1,
  // H.264
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_H264 = 2,
  // HEVC
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_HEVC = 3,
  // AVS
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_AVS = 4,
  // AVS2
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_AVS2 = 5,
  // AVS3
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_AVS3 = 6,
  // MPEG audio
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_MP2 = 7,
  // AAC
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_AAC = 8,
  // AC-3
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_AC3 = 9,
  // E-AC-3
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_EAC3 = 10,
  // AC-4
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_AC4 = 11,
  // DTS
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_DTS = 12,
  // DRA
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_DRA = 13,
  // Linear PCM
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_LPCM = 14,
  // Opus
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_OPUS = 15,
  // Teletext
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_TELETEXT = 16,
  // DVB subtitles
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_DVBSUB = 17,
  // Data
  SDRKIT_MPEGTS_PES_VIDEO_FIELDS_CODEC_DATA = 18,
} sdrkit_mpegts_pes_video_fields_codec;

// The result fields of `mpegts.pes_video`: the stream and its picture.
typedef struct sdrkit_mpegts_pes_video_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The codec of the stream. A value of `sdrkit_mpegts_pes_video_fields_codec`.
  int32_t codec;
  // The PID.
  int64_t pid;
  // The picture width. Unit: px.
  int64_t width;
  // The picture height. Unit: px.
  int64_t height;
  // True if `frame_rate` holds a value.
  bool has_frame_rate;
  // Frames per second. No value before the stream gave the rate. Unit: Hz.
  double frame_rate;
} sdrkit_mpegts_pes_video_fields;

// The options of `codec` of `sdrkit_mpegts_pes_audio_fields`.
typedef enum sdrkit_mpegts_pes_audio_fields_codec {
  // MPEG-1 video
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_MPEG1VIDEO = 0,
  // MPEG-2 video
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_MPEG2VIDEO = 1,
  // H.264
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_H264 = 2,
  // HEVC
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_HEVC = 3,
  // AVS
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_AVS = 4,
  // AVS2
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_AVS2 = 5,
  // AVS3
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_AVS3 = 6,
  // MPEG audio
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_MP2 = 7,
  // AAC
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_AAC = 8,
  // AC-3
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_AC3 = 9,
  // E-AC-3
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_EAC3 = 10,
  // AC-4
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_AC4 = 11,
  // DTS
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_DTS = 12,
  // DRA
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_DRA = 13,
  // Linear PCM
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_LPCM = 14,
  // Opus
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_OPUS = 15,
  // Teletext
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_TELETEXT = 16,
  // DVB subtitles
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_DVBSUB = 17,
  // Data
  SDRKIT_MPEGTS_PES_AUDIO_FIELDS_CODEC_DATA = 18,
} sdrkit_mpegts_pes_audio_fields_codec;

// The result fields of `mpegts.pes_audio`: the stream and its sound.
typedef struct sdrkit_mpegts_pes_audio_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The codec of the stream. A value of `sdrkit_mpegts_pes_audio_fields_codec`.
  int32_t codec;
  // The PID.
  int64_t pid;
  // The audio sample rate. A frequency in Hz.
  double sample_rate;
  // The audio channels.
  int64_t channels;
} sdrkit_mpegts_pes_audio_fields;

// A teletext page held. The row key is the page number.
typedef struct sdrkit_mpegts_teletext_page_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The page number.
  int64_t page;
  // The number of subpages held.
  int64_t subpages;
} sdrkit_mpegts_teletext_page_row;

// A subpage of a teletext page. The row key is `<page>/<subcode>`, the subcode as four hex digits.
typedef struct sdrkit_mpegts_teletext_subpage_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the page in `teletext_pages`.
  sdrkit_string page_row;
  // The subcode, in nibble form.
  int64_t subcode;
  // The version, raised on each change.
  int64_t version;
} sdrkit_mpegts_teletext_subpage_row;

// The options of `kind` of `sdrkit_mpegts_ts_service_row`.
typedef enum sdrkit_mpegts_ts_service_row_kind {
  // TV
  SDRKIT_MPEGTS_TS_SERVICE_ROW_KIND_TV = 0,
  // Radio
  SDRKIT_MPEGTS_TS_SERVICE_ROW_KIND_RADIO = 1,
  // Data
  SDRKIT_MPEGTS_TS_SERVICE_ROW_KIND_DATA = 2,
  // Teletext
  SDRKIT_MPEGTS_TS_SERVICE_ROW_KIND_TELETEXT = 3,
  // Other
  SDRKIT_MPEGTS_TS_SERVICE_ROW_KIND_OTHER = 4,
  // Unknown
  SDRKIT_MPEGTS_TS_SERVICE_ROW_KIND_UNKNOWN = 5,
} sdrkit_mpegts_ts_service_row_kind;

// The options of `ca_state` of `sdrkit_mpegts_ts_service_row`.
typedef enum sdrkit_mpegts_ts_service_row_ca_state {
  // Unknown
  SDRKIT_MPEGTS_TS_SERVICE_ROW_CA_STATE_UNKNOWN = 0,
  // Clear
  SDRKIT_MPEGTS_TS_SERVICE_ROW_CA_STATE_CLEAR = 1,
  // CA flag set, but the stream is clear
  SDRKIT_MPEGTS_TS_SERVICE_ROW_CA_STATE_FLAGGED_CLEAR = 2,
  // Scrambled
  SDRKIT_MPEGTS_TS_SERVICE_ROW_CA_STATE_SCRAMBLED = 3,
} sdrkit_mpegts_ts_service_row_ca_state;

// A service of the multiplex. The row key is the service_id.
typedef struct sdrkit_mpegts_ts_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // service_id.
  int64_t service_id;
  // The name.
  sdrkit_string name;
  // The provider.
  sdrkit_string provider;
  // service_type.
  int64_t service_type;
  // What the service is. A value of `sdrkit_mpegts_ts_service_row_kind`.
  int32_t kind;
  // The logical channel number (0 if none).
  int64_t lcn;
  // Shown in a channel list.
  bool visible;
  // Scrambled.
  bool scrambled;
  // Whether it is scrambled. A value of `sdrkit_mpegts_ts_service_row_ca_state`.
  int32_t ca_state;
  // The PMT PID.
  int64_t pmt_pid;
  // The PCR PID.
  int64_t pcr_pid;
  // running_status.
  int64_t running_status;
  // Listed in the PAT.
  bool in_pat;
  // EIT_schedule_flag.
  bool eit_schedule;
  // EIT_present_following_flag.
  bool eit_present_following;
  // original_network_id.
  int64_t onid;
  // transport_stream_id.
  int64_t tsid;
  // True if `now_title` holds a value.
  bool has_now_title;
  // The title of the present event.
  sdrkit_string now_title;
  // True if `now_text` holds a value.
  bool has_now_text;
  // The text of the present event.
  sdrkit_string now_text;
  // True if `now_start` holds a value.
  bool has_now_start;
  // The start of the present event. Seconds since 1970 (UTC).
  double now_start;
  // True if `now_duration` holds a value.
  bool has_now_duration;
  // The length of the present event. A length of time in seconds.
  double now_duration;
  // True if `next_title` holds a value.
  bool has_next_title;
  // The title of the following event.
  sdrkit_string next_title;
  // True if `next_text` holds a value.
  bool has_next_text;
  // The text of the following event.
  sdrkit_string next_text;
  // True if `next_start` holds a value.
  bool has_next_start;
  // The start of the following event. Seconds since 1970 (UTC).
  double next_start;
  // True if `next_duration` holds a value.
  bool has_next_duration;
  // The length of the following event. A length of time in seconds.
  double next_duration;
} sdrkit_mpegts_ts_service_row;

// The options of `kind` of `sdrkit_mpegts_ts_track_row`.
typedef enum sdrkit_mpegts_ts_track_row_kind {
  // Video
  SDRKIT_MPEGTS_TS_TRACK_ROW_KIND_VIDEO = 0,
  // Audio
  SDRKIT_MPEGTS_TS_TRACK_ROW_KIND_AUDIO = 1,
  // Subtitles
  SDRKIT_MPEGTS_TS_TRACK_ROW_KIND_SUBTITLES = 2,
  // Teletext
  SDRKIT_MPEGTS_TS_TRACK_ROW_KIND_TELETEXT = 3,
  // Data
  SDRKIT_MPEGTS_TS_TRACK_ROW_KIND_DATA = 4,
} sdrkit_mpegts_ts_track_row_kind;

// The options of `codec` of `sdrkit_mpegts_ts_track_row`.
typedef enum sdrkit_mpegts_ts_track_row_codec {
  // MPEG-1 video
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_MPEG1VIDEO = 0,
  // MPEG-2 video
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_MPEG2VIDEO = 1,
  // H.264
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_H264 = 2,
  // HEVC
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_HEVC = 3,
  // AVS
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_AVS = 4,
  // AVS2
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_AVS2 = 5,
  // AVS3
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_AVS3 = 6,
  // MPEG audio
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_MP2 = 7,
  // AAC
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_AAC = 8,
  // AC-3
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_AC3 = 9,
  // E-AC-3
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_EAC3 = 10,
  // AC-4
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_AC4 = 11,
  // DTS
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_DTS = 12,
  // DRA
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_DRA = 13,
  // Linear PCM
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_LPCM = 14,
  // Opus
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_OPUS = 15,
  // Teletext
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_TELETEXT = 16,
  // DVB subtitles
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_DVBSUB = 17,
  // Data
  SDRKIT_MPEGTS_TS_TRACK_ROW_CODEC_DATA = 18,
} sdrkit_mpegts_ts_track_row_codec;

// The options of `packaging` of `sdrkit_mpegts_ts_track_row`.
typedef enum sdrkit_mpegts_ts_track_row_packaging {
  // Start codes
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_ANNEXB = 0,
  // Elementary frames
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_ELEMENTARY = 1,
  // ADTS
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_ADTS = 2,
  // LATM
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_LATM = 3,
  // Sync frames
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_FRAMES = 4,
  // SMPTE 302M
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_SMPTE302M = 5,
  // PES
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_PES = 6,
  // None
  SDRKIT_MPEGTS_TS_TRACK_ROW_PACKAGING_NONE = 7,
} sdrkit_mpegts_ts_track_row_packaging;

// A track of a service: an elementary stream, or one entry of a subtitle or teletext stream. The row key is `<service>/<pid>` or `<service>/<pid>/<n>`.
typedef struct sdrkit_mpegts_ts_track_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the service in `services`.
  sdrkit_string service;
  // The PID.
  int64_t pid;
  // stream_type.
  int64_t stream_type;
  // True if `kind` holds a value.
  bool has_kind;
  // What the track is. A value of `sdrkit_mpegts_ts_track_row_kind`.
  int32_t kind;
  // True if `codec` holds a value.
  bool has_codec;
  // The codec. A value of `sdrkit_mpegts_ts_track_row_codec`.
  int32_t codec;
  // True if `packaging` holds a value.
  bool has_packaging;
  // The packaging. A value of `sdrkit_mpegts_ts_track_row_packaging`.
  int32_t packaging;
  // True if `fourcc` holds a value.
  bool has_fourcc;
  // The four-character code.
  sdrkit_string fourcc;
  // The ISO 639-2 language code.
  sdrkit_string lang;
  // audio_type of the language entry.
  int64_t audio_type;
  // An audio description for people who cannot see.
  bool audio_description;
  // Subtitles for the hard of hearing.
  bool hard_of_hearing;
  // True if `component_tag` holds a value.
  bool has_component_tag;
  // component_tag.
  int64_t component_tag;
  // The bit rate of the PID. Unit: kbit/s.
  double kbps;
  // Scrambled.
  bool scrambled;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile.
  sdrkit_string profile;
  // True if `level` holds a value.
  bool has_level;
  // The level.
  sdrkit_string level;
  // True if `width` holds a value.
  bool has_width;
  // The picture width. No value for a track that is not video. Unit: px.
  int64_t width;
  // True if `height` holds a value.
  bool has_height;
  // The picture height. No value for a track that is not video. Unit: px.
  int64_t height;
  // True if `frame_rate` holds a value.
  bool has_frame_rate;
  // Frames per second. Unit: Hz.
  double frame_rate;
  // True if `aspect` holds a value.
  bool has_aspect;
  // The display aspect.
  sdrkit_string aspect;
  // True if `interlaced` holds a value.
  bool has_interlaced;
  // Interlaced.
  bool interlaced;
  // True if `sample_rate` holds a value.
  bool has_sample_rate;
  // The audio sample rate. No value for a track that is not audio. A frequency in Hz.
  double sample_rate;
  // True if `channels` holds a value.
  bool has_channels;
  // The audio channels. No value for a track that is not audio.
  int64_t channels;
  // True if `layout` holds a value.
  bool has_layout;
  // The channel layout.
  sdrkit_string layout;
  // True if `bitrate_kbps` holds a value.
  bool has_bitrate_kbps;
  // The bit rate that the frame header provides. Unit: kbit/s.
  int64_t bitrate_kbps;
  // True if `object_type` holds a value.
  bool has_object_type;
  // The AAC object type or the MPEG audio layer.
  sdrkit_string object_type;
  // True if `entry_type` holds a value.
  bool has_entry_type;
  // subtitling_type, or teletext_type.
  int64_t entry_type;
  // True if `composition_page` holds a value.
  bool has_composition_page;
  // composition_page_id.
  int64_t composition_page;
  // True if `ancillary_page` holds a value.
  bool has_ancillary_page;
  // ancillary_page_id.
  int64_t ancillary_page;
  // True if `magazine` holds a value.
  bool has_magazine;
  // teletext_magazine_number.
  int64_t magazine;
  // True if `teletext_page` holds a value.
  bool has_teletext_page;
  // teletext_page_number.
  int64_t teletext_page;
} sdrkit_mpegts_ts_track_row;

// An event of the programme guide. The row key is `<onid>/<tsid>/<service_id>/<event_id>`.
typedef struct sdrkit_mpegts_ts_guide_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the service in `services`.
  sdrkit_string service;
  // original_network_id.
  int64_t onid;
  // transport_stream_id.
  int64_t tsid;
  // service_id.
  int64_t service_id;
  // event_id.
  int64_t event_id;
  // The start. Seconds since 1970 (UTC).
  double start;
  // The length. A length of time in seconds.
  double duration;
  // running_status.
  int64_t running_status;
  // The title.
  sdrkit_string title;
  // The short text.
  sdrkit_string text;
  // The extended text.
  sdrkit_string extended;
  // The ISO 639-2 language code.
  sdrkit_string lang;
  // True if `genre` holds a value.
  bool has_genre;
  // The first level-1 content nibble. No value for an event without one.
  int64_t genre;
  // The content bytes (up to 8).
  sdrkit_bytes content;
  // The highest minimum age over all countries (0 if none).
  int64_t min_age;
  // The programme CRID.
  sdrkit_string crid_programme;
  // The series CRID.
  sdrkit_string crid_series;
  // free_CA_mode.
  bool free_ca;
  // From an EIT of another transport stream.
  bool other_ts;
  // From a schedule table.
  bool schedule;
} sdrkit_mpegts_ts_guide_row;

// The parental rating of a guide event in one country. The row key is `<guide row>/<country>`.
typedef struct sdrkit_mpegts_ts_guide_rating_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the guide event in `guide`.
  sdrkit_string event;
  // The ISO 3166 country code.
  sdrkit_string country;
  // The rating (minimum age minus 3).
  int64_t rating;
} sdrkit_mpegts_ts_guide_rating_row;

// A PID of the multiplex. The row key is the PID as four hex digits.
typedef struct sdrkit_mpegts_ts_pid_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PID.
  int64_t pid;
  // What the PID carries.
  sdrkit_string label;
  // Packets of the PID.
  int64_t packets;
  // Continuity errors.
  int64_t cc_errors;
  // Packets of the PID with the transport_error_indicator.
  int64_t tei;
  // The bit rate of the PID. Unit: kbit/s.
  double kbps;
  // Scrambled.
  bool scrambled;
} sdrkit_mpegts_ts_pid_row;

// The section counts of one table. The row key is the table name (PAT, PMT, SDT, ...).
typedef struct sdrkit_mpegts_ts_section_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Sections accepted.
  int64_t accepted;
  // Sections with a bad CRC.
  int64_t crc;
  // Sections with a bad length.
  int64_t length;
  // Sections refused for another reason.
  int64_t other;
} sdrkit_mpegts_ts_section_row;

// A teletext page that arrived.
typedef struct sdrkit_mpegts_teletext_page_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The page number.
  int64_t page;
  // The subcode, in nibble form.
  int64_t subcode;
  // The version, raised on each change.
  int64_t version;
  // The text of the page, its 25 rows joined with line feeds.
  sdrkit_string text;
} sdrkit_mpegts_teletext_page_event;

// The fields of a new service list.
typedef struct sdrkit_mpegts_service_list_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // transport_stream_id.
  int64_t tsid;
  // original_network_id.
  int64_t onid;
  // network_id.
  int64_t network_id;
  // The network name.
  sdrkit_string network_name;
  // The number of services.
  int64_t service_count;
} sdrkit_mpegts_service_list_event;

// The fields of a new or changed guide event.
typedef struct sdrkit_mpegts_epg_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // original_network_id.
  int64_t onid;
  // transport_stream_id.
  int64_t tsid;
  // service_id.
  int64_t service_id;
  // event_id.
  int64_t event_id;
  // The start. Seconds since 1970 (UTC).
  double start;
  // The length. A length of time in seconds.
  double duration;
  // The title.
  sdrkit_string title;
} sdrkit_mpegts_epg_event;

// The fields of a new stream time.
typedef struct sdrkit_mpegts_ts_time_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The UTC of the stream. Seconds since 1970 (UTC).
  double utc;
  // True if `local_offset_min` holds a value.
  bool has_local_offset_min;
  // The local time offset in force. No value before the stream gave one. Unit: min.
  int64_t local_offset_min;
} sdrkit_mpegts_ts_time_event;

// The fields of a new network.
typedef struct sdrkit_mpegts_ts_network_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // network_id.
  int64_t network_id;
  // The network name.
  sdrkit_string network_name;
  // original_network_id.
  int64_t onid;
  // transport_stream_id.
  int64_t tsid;
} sdrkit_mpegts_ts_network_event;

// The fields of a subtitle display.
typedef struct sdrkit_mpegts_dvb_subtitle_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `pts` holds a value.
  bool has_pts;
  // The presentation time stamp, 90 kHz. No value for a display without one.
  int64_t pts;
  // How long the display shows at most. A length of time in seconds.
  double timeout_s;
  // The width of the display. Unit: px.
  int64_t width;
  // The height of the display. Unit: px.
  int64_t height;
  // The display sequence number.
  int64_t seq;
  // The number of regions (0 clears the screen).
  int64_t region_count;
} sdrkit_mpegts_dvb_subtitle_event;

// The arguments of `service.select`.
typedef struct sdrkit_mpegts_service_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the service in `services`. Empty: the first service that can play.
  sdrkit_string row;
} sdrkit_mpegts_service_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_MPEGTS_SERVICE_SELECT_ARGS_INIT {sizeof(sdrkit_mpegts_service_select_args), {0}}

// The arguments of `track.select`.
typedef struct sdrkit_mpegts_track_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the track in `tracks`.
  sdrkit_string row;
} sdrkit_mpegts_track_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_MPEGTS_TRACK_SELECT_ARGS_INIT {sizeof(sdrkit_mpegts_track_select_args), {0}}

// The arguments of `teletext.select`.
typedef struct sdrkit_mpegts_teletext_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the page in `teletext_pages`.
  sdrkit_string row;
} sdrkit_mpegts_teletext_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_MPEGTS_TELETEXT_SELECT_ARGS_INIT {sizeof(sdrkit_mpegts_teletext_select_args), {0}}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "teletext.page"
// (Teletext page). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpegts_teletext_page_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpegts_teletext_page_event *event) {
  return sdrkit_typed_event(handle, n, "teletext.page", "mpegts.TeletextPageEvent", event, sizeof(sdrkit_mpegts_teletext_page_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "service.list"
// (Service list). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpegts_service_list_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpegts_service_list_event *event) {
  return sdrkit_typed_event(handle, n, "service.list", "mpegts.TsServiceListEvent", event, sizeof(sdrkit_mpegts_service_list_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "epg.event"
// (Guide event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpegts_epg_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpegts_epg_event *event) {
  return sdrkit_typed_event(handle, n, "epg.event", "mpegts.TsGuideEvent", event, sizeof(sdrkit_mpegts_epg_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ts.time"
// (Stream time). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpegts_ts_time_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpegts_ts_time_event *event) {
  return sdrkit_typed_event(handle, n, "ts.time", "mpegts.TsTimeEvent", event, sizeof(sdrkit_mpegts_ts_time_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ts.network"
// (Network). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpegts_ts_network_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpegts_ts_network_event *event) {
  return sdrkit_typed_event(handle, n, "ts.network", "mpegts.TsNetworkEvent", event, sizeof(sdrkit_mpegts_ts_network_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dvb.subtitle"
// (Subtitle). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mpegts_dvb_subtitle_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mpegts_dvb_subtitle_event *event) {
  return sdrkit_typed_event(handle, n, "dvb.subtitle", "mpegts.DvbSubtitleEvent", event, sizeof(sdrkit_mpegts_dvb_subtitle_event));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mpegts_ts_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsMonitorFields", fields, sizeof(sdrkit_mpegts_ts_monitor_fields));
}

// Fills `fields` with the result fields of the component "monitor" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "monitor.<key>".
static inline int32_t sdrkit_mpegts_monitor_ts_monitor_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_monitor_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsMonitorFields@monitor", fields, sizeof(sdrkit_mpegts_ts_monitor_fields));
}

// Fills `fields` with the result fields of the component "ts_demux" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ts_demux.<key>".
static inline int32_t sdrkit_mpegts_ts_demux_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_ts_demux_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TsDemuxFields@ts_demux", fields, sizeof(sdrkit_mpegts_ts_demux_fields));
}

// Fills `fields` with the result fields of the component "psi_si" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "psi_si.<key>".
static inline int32_t sdrkit_mpegts_psi_si_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_psi_si_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PsiSiFields@psi_si", fields, sizeof(sdrkit_mpegts_psi_si_fields));
}

// Fills `fields` with the result fields of the component "dvb_subtitle" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dvb_subtitle.<key>".
static inline int32_t sdrkit_mpegts_dvb_subtitle_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_dvb_subtitle_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.DvbSubtitleFields@dvb_subtitle", fields, sizeof(sdrkit_mpegts_dvb_subtitle_fields));
}

// Fills `fields` with the result fields of the component "teletext" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "teletext.<key>".
static inline int32_t sdrkit_mpegts_teletext_stream_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_teletext_stream_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.TeletextStreamFields@teletext", fields, sizeof(sdrkit_mpegts_teletext_stream_fields));
}

// Fills `fields` with the result fields of the component "pes_video" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "pes_video.<key>".
static inline int32_t sdrkit_mpegts_pes_video_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_pes_video_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PesVideoFields@pes_video", fields, sizeof(sdrkit_mpegts_pes_video_fields));
}

// Fills `fields` with the result fields of the component "pes_audio" of the stack "mpegts" in the snapshot of
// `handle`. A generic consumer reads them under the keys "pes_audio.<key>".
static inline int32_t sdrkit_mpegts_pes_audio_fields_read(SDRKIT_HANDLE handle, sdrkit_mpegts_pes_audio_fields *fields) {
  return sdrkit_typed_fields(handle, "mpegts.PesAudioFields@pes_audio", fields, sizeof(sdrkit_mpegts_pes_audio_fields));
}

#endif
