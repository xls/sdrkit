/*
 * sdrkit: the typed interface of the stack family "custom", version 1.4. MIT licence.
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

#ifndef SDRKIT_CUSTOM_H
#define SDRKIT_CUSTOM_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_CUSTOM_VERSION_MAJOR 1
#define SDRKIT_CUSTOM_VERSION_MINOR 4

// The stack "slotted_voice": its id, and the keys of its event types.
#define SDRKIT_STACK_SLOTTED_VOICE "slotted_voice"
#define SDRKIT_EVENT_VOICE_STATION "voice.station"
#define SDRKIT_EVENT_VOICE_TX "voice.tx"

// The stack "qam_video": its id, and the keys of its event types.
#define SDRKIT_STACK_QAM_VIDEO "qam_video"
#define SDRKIT_EVENT_VIDEO_TX "video.tx"

// The stack "picture_burst": its id, and the keys of its event types.
#define SDRKIT_STACK_PICTURE_BURST "picture_burst"
#define SDRKIT_EVENT_PICTURE_BURST "picture.burst"
#define SDRKIT_EVENT_PICTURE_TX "picture.tx"

// The stack "arq_packet": its id, and the keys of its event types.
#define SDRKIT_STACK_ARQ_PACKET "arq_packet"
#define SDRKIT_EVENT_PACKET_MESSAGE "packet.message"
#define SDRKIT_EVENT_PACKET_ACK_OUT "packet.ack_out"
#define SDRKIT_EVENT_PACKET_ACK_IN "packet.ack_in"
#define SDRKIT_EVENT_PACKET_TX "packet.tx"

// The stack "file_transfer": its id, and the keys of its event types.
#define SDRKIT_STACK_FILE_TRANSFER "file_transfer"
#define SDRKIT_EVENT_FILE_RECEIVED "file.received"
#define SDRKIT_EVENT_FILE_OPEN "file.open"
#define SDRKIT_EVENT_FILE_TX "file.tx"

// The stack "hf_chat": its id, and the keys of its event types.
#define SDRKIT_STACK_HF_CHAT "hf_chat"
#define SDRKIT_EVENT_CHAT_MESSAGE "chat.message"
#define SDRKIT_EVENT_CHAT_BEACON "chat.beacon"
#define SDRKIT_EVENT_CHAT_ACK "chat.ack"
#define SDRKIT_EVENT_CHAT_VOICE "chat.voice"
#define SDRKIT_EVENT_CHAT_TX "chat.tx"

// The options of the setting "listen_rate" of the stack "slotted_voice".
typedef enum sdrkit_slotted_voice_listen_rate {
  // 8000 and 16000 symbols a second
  SDRKIT_SLOTTED_VOICE_LISTEN_RATE_BOTH = 0,
  // 8000 symbols a second
  SDRKIT_SLOTTED_VOICE_LISTEN_RATE_8000_SYMBOLS_A_SECOND = 1,
  // 16000 symbols a second
  SDRKIT_SLOTTED_VOICE_LISTEN_RATE_16000_SYMBOLS_A_SECOND = 2,
} sdrkit_slotted_voice_listen_rate;

// The options of the setting "frame_version" of the stack "slotted_voice".
typedef enum sdrkit_slotted_voice_frame_version {
  // Version 3 (one voice stream)
  SDRKIT_SLOTTED_VOICE_FRAME_VERSION_VERSION_3 = 0,
  // Version 4 (four slots)
  SDRKIT_SLOTTED_VOICE_FRAME_VERSION_VERSION_4 = 1,
} sdrkit_slotted_voice_frame_version;

// The options of the setting "bit_rate" of the stack "slotted_voice".
typedef enum sdrkit_slotted_voice_bit_rate {
  // 12 kbit/s
  SDRKIT_SLOTTED_VOICE_BIT_RATE_12_KBIT_S = 0,
  // 16 kbit/s
  SDRKIT_SLOTTED_VOICE_BIT_RATE_16_KBIT_S = 1,
  // 24 kbit/s
  SDRKIT_SLOTTED_VOICE_BIT_RATE_24_KBIT_S = 2,
  // 32 kbit/s
  SDRKIT_SLOTTED_VOICE_BIT_RATE_32_KBIT_S = 3,
} sdrkit_slotted_voice_bit_rate;

// The options of the setting "voice_profile" of the stack "slotted_voice".
typedef enum sdrkit_slotted_voice_profile {
  // 8.0 kbit/s
  SDRKIT_SLOTTED_VOICE_PROFILE_V80 = 0,
  // 6.0 kbit/s
  SDRKIT_SLOTTED_VOICE_PROFILE_V60 = 1,
  // 4.8 kbit/s
  SDRKIT_SLOTTED_VOICE_PROFILE_V48 = 2,
  // 3.2 kbit/s robust
  SDRKIT_SLOTTED_VOICE_PROFILE_V32R = 3,
} sdrkit_slotted_voice_profile;

// The options of the setting "gen_bit_rate" of the stack "slotted_voice".
typedef enum sdrkit_slotted_voice_gen_bit_rate {
  // 12 kbit/s
  SDRKIT_SLOTTED_VOICE_GEN_BIT_RATE_12_KBIT_S = 0,
  // 16 kbit/s
  SDRKIT_SLOTTED_VOICE_GEN_BIT_RATE_16_KBIT_S = 1,
  // 24 kbit/s
  SDRKIT_SLOTTED_VOICE_GEN_BIT_RATE_24_KBIT_S = 2,
  // 32 kbit/s
  SDRKIT_SLOTTED_VOICE_GEN_BIT_RATE_32_KBIT_S = 3,
} sdrkit_slotted_voice_gen_bit_rate;

// The options of the setting "gen_frame_version" of the stack "slotted_voice".
typedef enum sdrkit_slotted_voice_gen_frame_version {
  // Version 3 (one voice stream)
  SDRKIT_SLOTTED_VOICE_GEN_FRAME_VERSION_VERSION_3 = 0,
  // Version 4 (four slots)
  SDRKIT_SLOTTED_VOICE_GEN_FRAME_VERSION_VERSION_4 = 1,
} sdrkit_slotted_voice_gen_frame_version;

// The options of the setting "listen_profile" of the stack "qam_video".
typedef enum sdrkit_qam_video_listen_profile {
  // A: 700 ksym/s
  SDRKIT_QAM_VIDEO_LISTEN_PROFILE_A = 0,
  // B: 1050 ksym/s
  SDRKIT_QAM_VIDEO_LISTEN_PROFILE_B = 1,
  // C and D: 1400 ksym/s
  SDRKIT_QAM_VIDEO_LISTEN_PROFILE_CD = 2,
} sdrkit_qam_video_listen_profile;

// The options of the setting "profile" of the stack "qam_video".
typedef enum sdrkit_qam_video_profile {
  // A: 700 ksym/s, 16-QAM
  SDRKIT_QAM_VIDEO_PROFILE_A = 0,
  // B: 1050 ksym/s, 16-QAM
  SDRKIT_QAM_VIDEO_PROFILE_B = 1,
  // C: 1400 ksym/s, 16-QAM
  SDRKIT_QAM_VIDEO_PROFILE_C = 2,
  // D: 1400 ksym/s, 64-QAM
  SDRKIT_QAM_VIDEO_PROFILE_D = 3,
} sdrkit_qam_video_profile;

// The options of the setting "profile" of the stack "picture_burst".
typedef enum sdrkit_picture_burst_profile {
  // 0: Balanced
  SDRKIT_PICTURE_BURST_PROFILE_0_BALANCED = 0,
  // 1: Robust
  SDRKIT_PICTURE_BURST_PROFILE_1_ROBUST = 1,
  // 2: Quality
  SDRKIT_PICTURE_BURST_PROFILE_2_QUALITY = 2,
  // 3: Direct
  SDRKIT_PICTURE_BURST_PROFILE_3_DIRECT = 3,
} sdrkit_picture_burst_profile;

// The options of the setting "profile" of the stack "arq_packet".
typedef enum sdrkit_arq_packet_profile {
  // 0: BPSK 1/4
  SDRKIT_ARQ_PACKET_PROFILE_0_BPSK_1_4 = 0,
  // 1: QPSK 1/4
  SDRKIT_ARQ_PACKET_PROFILE_1_QPSK_1_4 = 1,
  // 2: QPSK 1/2
  SDRKIT_ARQ_PACKET_PROFILE_2_QPSK_1_2 = 2,
  // 3: QPSK 3/4
  SDRKIT_ARQ_PACKET_PROFILE_3_QPSK_3_4 = 3,
  // 4: 16-QAM 1/2
  SDRKIT_ARQ_PACKET_PROFILE_4_16_QAM_1_2 = 4,
  // 5: 16-QAM 3/4
  SDRKIT_ARQ_PACKET_PROFILE_5_16_QAM_3_4 = 5,
  // 6: 64-QAM 2/3
  SDRKIT_ARQ_PACKET_PROFILE_6_64_QAM_2_3 = 6,
  // 7: 256-QAM 3/4
  SDRKIT_ARQ_PACKET_PROFILE_7_256_QAM_3_4 = 7,
  // 8: 1024-QAM 5/6
  SDRKIT_ARQ_PACKET_PROFILE_8_1024_QAM_5_6 = 8,
} sdrkit_arq_packet_profile;

// The options of the setting "link" of the stack "arq_packet".
typedef enum sdrkit_arq_packet_link {
  // One way, repeated passes
  SDRKIT_ARQ_PACKET_LINK_ONE_WAY = 0,
  // Two way, selective repeat
  SDRKIT_ARQ_PACKET_LINK_TWO_WAY = 1,
} sdrkit_arq_packet_link;

// The options of the setting "widths" of the stack "file_transfer".
typedef enum sdrkit_file_transfer_widths {
  // All widths
  SDRKIT_FILE_TRANSFER_WIDTHS_ALL = 0,
  // 1 MHz only
  SDRKIT_FILE_TRANSFER_WIDTHS_1M = 1,
  // 500 kHz only
  SDRKIT_FILE_TRANSFER_WIDTHS_500K = 2,
  // 200 kHz only
  SDRKIT_FILE_TRANSFER_WIDTHS_200K = 3,
} sdrkit_file_transfer_widths;

// The options of the setting "width" of the stack "file_transfer".
typedef enum sdrkit_file_transfer_width {
  // 1 MHz
  SDRKIT_FILE_TRANSFER_WIDTH_1M = 0,
  // 500 kHz
  SDRKIT_FILE_TRANSFER_WIDTH_500K = 1,
  // 200 kHz
  SDRKIT_FILE_TRANSFER_WIDTH_200K = 2,
} sdrkit_file_transfer_width;

// The options of the setting "profile" of the stack "file_transfer".
typedef enum sdrkit_file_transfer_profile {
  // 0: BPSK 1/4
  SDRKIT_FILE_TRANSFER_PROFILE_0_BPSK_1_4 = 0,
  // 1: QPSK 1/4
  SDRKIT_FILE_TRANSFER_PROFILE_1_QPSK_1_4 = 1,
  // 2: QPSK 1/2
  SDRKIT_FILE_TRANSFER_PROFILE_2_QPSK_1_2 = 2,
  // 3: QPSK 3/4
  SDRKIT_FILE_TRANSFER_PROFILE_3_QPSK_3_4 = 3,
  // 4: 16-QAM 3/4
  SDRKIT_FILE_TRANSFER_PROFILE_4_16_QAM_3_4 = 4,
  // 5: 64-QAM 3/4
  SDRKIT_FILE_TRANSFER_PROFILE_5_64_QAM_3_4 = 5,
  // 6: 256-QAM 3/4
  SDRKIT_FILE_TRANSFER_PROFILE_6_256_QAM_3_4 = 6,
  // 7: 256-QAM 5/6
  SDRKIT_FILE_TRANSFER_PROFILE_7_256_QAM_5_6 = 7,
  // 8: 1024-QAM 5/6
  SDRKIT_FILE_TRANSFER_PROFILE_8_1024_QAM_5_6 = 8,
  // 9: 1024-QAM 9/10
  SDRKIT_FILE_TRANSFER_PROFILE_9_1024_QAM_9_10 = 9,
} sdrkit_file_transfer_profile;

// The options of the setting "link" of the stack "file_transfer".
typedef enum sdrkit_file_transfer_link {
  // One way, repeated passes
  SDRKIT_FILE_TRANSFER_LINK_ONE_WAY = 0,
  // Two way, acknowledged
  SDRKIT_FILE_TRANSFER_LINK_TWO_WAY = 1,
} sdrkit_file_transfer_link;

// The options of the setting "listen" of the stack "hf_chat".
typedef enum sdrkit_hf_chat_listen {
  // All profiles
  SDRKIT_HF_CHAT_LISTEN_ALL = 0,
  // Profile 0
  SDRKIT_HF_CHAT_LISTEN_PROFILE_0 = 1,
  // Profile 1
  SDRKIT_HF_CHAT_LISTEN_PROFILE_1 = 2,
  // Profile 2
  SDRKIT_HF_CHAT_LISTEN_PROFILE_2 = 3,
  // Profile 3
  SDRKIT_HF_CHAT_LISTEN_PROFILE_3 = 4,
  // Profile 4
  SDRKIT_HF_CHAT_LISTEN_PROFILE_4 = 5,
  // Profile 5
  SDRKIT_HF_CHAT_LISTEN_PROFILE_5 = 6,
  // Profile 6
  SDRKIT_HF_CHAT_LISTEN_PROFILE_6 = 7,
  // Profile 7
  SDRKIT_HF_CHAT_LISTEN_PROFILE_7 = 8,
} sdrkit_hf_chat_listen;

// The options of the setting "profile" of the stack "hf_chat".
typedef enum sdrkit_hf_chat_profile {
  // 0: Abyss, chirp SF 10, rate 1/4 x 3
  SDRKIT_HF_CHAT_PROFILE_0_ABYSS_CHIRP_SF_10_RATE_1_4_X_3 = 0,
  // 1: Deep, chirp SF 10, rate 1/4
  SDRKIT_HF_CHAT_PROFILE_1_DEEP_CHIRP_SF_10_RATE_1_4 = 1,
  // 2: Weak, chirp SF 9, rate 1/3
  SDRKIT_HF_CHAT_PROFILE_2_WEAK_CHIRP_SF_9_RATE_1_3 = 2,
  // 3: Chat, chirp SF 8, rate 1/2
  SDRKIT_HF_CHAT_PROFILE_3_CHAT_CHIRP_SF_8_RATE_1_2 = 3,
  // 4: Mid, chirp SF 6, rate 1/2
  SDRKIT_HF_CHAT_PROFILE_4_MID_CHIRP_SF_6_RATE_1_2 = 4,
  // 5: Fast data, OFDM BPSK, rate 1/4
  SDRKIT_HF_CHAT_PROFILE_5_FAST_DATA_OFDM_BPSK_RATE_1_4 = 5,
  // 6: Voice-700, OFDM QPSK
  SDRKIT_HF_CHAT_PROFILE_6_VOICE_700_OFDM_QPSK = 6,
  // 7: Voice-1200, OFDM QPSK
  SDRKIT_HF_CHAT_PROFILE_7_VOICE_1200_OFDM_QPSK = 7,
} sdrkit_hf_chat_profile;

// The options of the setting "kind" of the stack "hf_chat".
typedef enum sdrkit_hf_chat_kind {
  // Chat
  SDRKIT_HF_CHAT_KIND_CHAT = 0,
  // Short beacon
  SDRKIT_HF_CHAT_KIND_SHORT_BEACON = 1,
  // Full beacon
  SDRKIT_HF_CHAT_KIND_FULL_BEACON = 2,
  // Data
  SDRKIT_HF_CHAT_KIND_DATA = 3,
} sdrkit_hf_chat_kind;

// The options of the setting "link" of the stack "hf_chat".
typedef enum sdrkit_hf_chat_link {
  // One way, repeated passes
  SDRKIT_HF_CHAT_LINK_ONE_WAY = 0,
  // Two way, acknowledged
  SDRKIT_HF_CHAT_LINK_TWO_WAY = 1,
} sdrkit_hf_chat_link;

// The result fields of `slotted_voice`. A value of the signal has no value while no receiver is locked.
typedef struct sdrkit_slotted_voice_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A receiver is locked.
  bool locked;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR. Unit: dB.
  double snr_db;
  // True if `frame_version` holds a value.
  bool has_frame_version;
  // The frame version received, 3 or 4.
  int64_t frame_version;
  // True if `voice_kbps` holds a value.
  bool has_voice_kbps;
  // The voice bit rate. Unit: kbit/s.
  double voice_kbps;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The symbol rate. A frequency in Hz.
  double symbol_rate_hz;
  // True if `frames_ok` holds a value.
  bool has_frames_ok;
  // Words or slots that decoded, of the last receiver that was locked.
  int64_t frames_ok;
  // True if `frames_bad` holds a value.
  bool has_frames_bad;
  // Words or slots that did not decode, of the last receiver that was locked.
  int64_t frames_bad;
  // True if `sweep_id` holds a value.
  bool has_sweep_id;
  // The station identity of the sweep, while a receiver reads it.
  sdrkit_string sweep_id;
  // True if `sweep_db` holds a value.
  bool has_sweep_db;
  // The sweep level relative to the voice. Unit: dB.
  double sweep_db;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The callsign of the newest beacon.
  sdrkit_string callsign;
  // The note on the source of the stack.
  sdrkit_string note;
} sdrkit_slotted_voice_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_custom_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_custom_decoder_fields;

// A row of the stations heard in beacons. The row key is the callsign.
typedef struct sdrkit_slotted_voice_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign.
  sdrkit_string callsign;
  // The grid locator.
  sdrkit_string locator;
  // The position.
  sdrkit_position position;
  // The beacon power value. Unit: dBm.
  double power_dbm;
  // The link quality value.
  int64_t link;
  // The beacon text, or the text of the last full beacon of the station.
  sdrkit_string text;
  // When the station was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_slotted_voice_station_row;

// The result fields of `qam_video`. A value of the last frame has no value before the first frame.
typedef struct sdrkit_qam_video_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A frame header passed in the last 2 s.
  bool locked;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile of the last frame.
  sdrkit_string profile;
  // True if `esn0_db` holds a value.
  bool has_esn0_db;
  // The Es/N0 of the last frame. Unit: dB.
  double esn0_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame. A frequency in Hz.
  double offset_hz;
  // Frames with all four blocks good.
  int64_t frames_ok;
  // Frames with a failed block.
  int64_t frames_bad;
  // Blocks that failed.
  int64_t blocks_bad;
  // Groups restored by the erasure code.
  int64_t groups_restored;
  // Groups lost.
  int64_t groups_lost;
  // Packets received.
  int64_t packets;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The callsign of the station identification.
  sdrkit_string callsign;
  // The symbol rate of the listen profile. A frequency in Hz.
  double symbol_rate_hz;
  // The note on the source of the stack.
  sdrkit_string note;
} sdrkit_qam_video_fields;

// A row of the streams heard. The row key is the packet type and the stream id.
typedef struct sdrkit_qam_video_stream_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The stream id.
  sdrkit_string stream;
  // The packet type: video, audio, text, picture, data or other.
  sdrkit_string type;
  // Packets of the stream.
  int64_t packets;
} sdrkit_qam_video_stream_row;

// The result fields of `picture_burst`. A value of the last burst has no value before the first decoded burst.
typedef struct sdrkit_picture_burst_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `header_ok` holds a value.
  bool has_header_ok;
  // The header of the last burst decoded.
  bool header_ok;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile of the last burst.
  int64_t profile;
  // True if `layers` holds a value.
  bool has_layers;
  // Layers of the last burst decoded.
  int64_t layers;
  // True if `layers_sent` holds a value.
  bool has_layers_sent;
  // Layers of the last burst sent.
  int64_t layers_sent;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last burst. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last burst. A frequency in Hz.
  double offset_hz;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The clock error of the last burst, if measured. Unit: ppm.
  double clock_ppm;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The callsign of the last burst.
  sdrkit_string callsign;
  // Bursts seen.
  int64_t bursts;
  // Bursts decoded.
  int64_t bursts_decoded;
  // A burst is being received, or one was decoded.
  bool locked;
  // The note on the source of the stack.
  sdrkit_string note;
} sdrkit_picture_burst_fields;

// A row of the pictures received. The row key is the number of the decoded burst.
typedef struct sdrkit_picture_burst_picture_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign.
  sdrkit_string callsign;
  // The picture size, "width x height".
  sdrkit_string size_;
  // Layers decoded.
  int64_t layers;
  // When the burst was decoded. Seconds since 1970 (UTC).
  double time;
} sdrkit_picture_burst_picture_row;

// The result fields of `arq_packet`. A value of the last burst has no value before the first burst.
typedef struct sdrkit_arq_packet_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Blocks whose CRC passed.
  int64_t blocks_good;
  // Blocks whose CRC failed.
  int64_t blocks_bad;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile of the last burst.
  int64_t profile;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the last burst. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last burst. A frequency in Hz.
  double offset_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The callsign of the last burst.
  sdrkit_string callsign;
  // Bursts whose header was read.
  int64_t bursts;
  // Messages listed.
  int64_t messages;
  // Messages heard again within 120 s.
  int64_t repeats;
  // Acknowledgement bursts heard.
  int64_t acks_heard;
  // A burst header was read in the last 5 s.
  bool locked;
  // The note on the source of the stack.
  sdrkit_string note;
  // True if `sequence` holds a value.
  bool has_sequence;
  // The sequence number of the last burst.
  int64_t sequence;
} sdrkit_arq_packet_fields;

// A row of the messages received. The row key is a running number.
typedef struct sdrkit_arq_packet_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sending station.
  sdrkit_string callsign;
  // The sequence number of the last burst of the message.
  int64_t sequence;
  // The message length. Unit: bytes.
  int64_t length;
  // The message as text.
  sdrkit_string text;
  // When the message was completed. Seconds since 1970 (UTC).
  double time;
} sdrkit_arq_packet_message_row;

// The result fields of `file_transfer`. A value of the last frame has no value before the first frame, and a value of the transfer none while no transfer is announced.
typedef struct sdrkit_file_transfer_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `session` holds a value.
  bool has_session;
  // The session id of the active transfer.
  int64_t session;
  // Blocks whose CRC passed.
  int64_t blocks_good;
  // Blocks whose CRC failed.
  int64_t blocks_bad;
  // Frames whose header was read.
  int64_t frames;
  // True if `progress` holds a value.
  bool has_progress;
  // Bytes of the active transfer received. Unit: %.
  double progress;
  // True if `hash_ok` holds a value.
  bool has_hash_ok;
  // The last file passed its hash.
  bool hash_ok;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile of the last frame.
  int64_t profile;
  // True if `width` holds a value.
  bool has_width;
  // The width of the last frame.
  sdrkit_string width;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The Es/N0 of the last frame. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last frame. A frequency in Hz.
  double offset_hz;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The callsign of the last frame.
  sdrkit_string callsign;
  // Files received.
  int64_t files;
  // Bytes held from an earlier session when the last transfer was announced. Unit: bytes.
  int64_t held_bytes;
  // Acknowledgement frames heard.
  int64_t acks_heard;
  // A frame header was read in the last 5 s.
  bool locked;
  // The note on the source of the stack.
  sdrkit_string note;
} sdrkit_file_transfer_fields;

// A row of the files heard. The row key is the session id and the start of the hash.
typedef struct sdrkit_file_transfer_file_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The file name.
  sdrkit_string name;
  // The size. Unit: bytes.
  int64_t size_;
  // The hash, hex.
  sdrkit_string hash;
  // The state: receiving, verified, or the hash failed.
  sdrkit_string state;
  // The sending station.
  sdrkit_string callsign;
  // When the state was reached. Seconds since 1970 (UTC).
  double time;
} sdrkit_file_transfer_file_row;

// The result fields of `hf_chat`. A value of the last burst has no value before the first burst.
typedef struct sdrkit_hf_chat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `profile` holds a value.
  bool has_profile;
  // The profile of the last burst.
  int64_t profile;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the last burst. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the last burst. A frequency in Hz.
  double offset_hz;
  // True if `clock_error_s` holds a value.
  bool has_clock_error_s;
  // The clock error of the last burst against the slot grid. Unit: s.
  double clock_error_s;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The sending station of the last burst; empty if the burst names none.
  sdrkit_string callsign;
  // Bursts decoded.
  int64_t bursts;
  // Blocks whose code word and CRC held.
  int64_t blocks_good;
  // Blocks that did not decode.
  int64_t blocks_bad;
  // Messages completed.
  int64_t messages;
  // Beacons heard.
  int64_t beacons;
  // Acknowledgements heard.
  int64_t acks_heard;
  // Voice blocks heard.
  int64_t voice_blocks;
  // A burst was decoded in the last 30 s.
  bool locked;
  // The note on the source of the stack.
  sdrkit_string note;
  // True if `message_id` holds a value.
  bool has_message_id;
  // The message id of the last burst; 0 if it carries no message part.
  int64_t message_id;
} sdrkit_hf_chat_fields;

// A row of the messages received. The row key is a running number.
typedef struct sdrkit_hf_chat_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sending station; empty for a data message.
  sdrkit_string callsign;
  // The recipient; empty for a data message.
  sdrkit_string to;
  // The text.
  sdrkit_string text;
  // When the message was completed, UTC. Seconds since 1970 (UTC).
  double time;
} sdrkit_hf_chat_message_row;

// A row of the beacons received. The row key is a running number.
typedef struct sdrkit_hf_chat_beacon_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station.
  sdrkit_string callsign;
  // The locator.
  sdrkit_string locator;
  // The position.
  sdrkit_position position;
  // The power. Unit: dBm.
  double power_dbm;
  // The text of a full beacon; empty for a short one.
  sdrkit_string text;
  // When the beacon was completed, UTC. Seconds since 1970 (UTC).
  double time;
} sdrkit_hf_chat_beacon_row;

// The fields of a station beacon.
typedef struct sdrkit_slotted_voice_station_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The callsign.
  sdrkit_string callsign;
  // The grid locator.
  sdrkit_string locator;
  // The position.
  sdrkit_position position;
  // The beacon power value. Unit: dBm.
  double power_dbm;
  // The beacon text.
  sdrkit_string text;
} sdrkit_slotted_voice_station_event;

// The fields of a transmission that started, finished or was refused.
typedef struct sdrkit_slotted_voice_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the transmission did: `started`, `finished` or `refused`.
  sdrkit_string state;
  // Why the transmission was refused or ended early; empty for none.
  sdrkit_string reason;
} sdrkit_slotted_voice_tx_event;

// The fields of a transmission that started, finished or was refused.
typedef struct sdrkit_qam_video_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the transmission did: `started`, `finished` or `refused`.
  sdrkit_string state;
  // Why the transmission was refused or ended early; empty for none.
  sdrkit_string reason;
} sdrkit_qam_video_tx_event;

// The fields of a decoded burst.
typedef struct sdrkit_picture_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The callsign.
  sdrkit_string callsign;
  // The picture size, "width x height".
  sdrkit_string size_;
  // Layers decoded.
  int64_t layers;
} sdrkit_picture_burst_event;

// The fields of a transmission that started, finished or was refused.
typedef struct sdrkit_picture_burst_picture_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the transmission did: `started`, `finished` or `refused`.
  sdrkit_string state;
  // Why the transmission was refused or ended early; empty for none.
  sdrkit_string reason;
} sdrkit_picture_burst_picture_tx_event;

// The fields of a completed message.
typedef struct sdrkit_arq_packet_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sending station.
  sdrkit_string callsign;
  // The sequence number of the last burst of the message.
  int64_t sequence;
  // The message length. Unit: bytes.
  int64_t length;
  // The message as text.
  sdrkit_string text;
} sdrkit_arq_packet_message_event;

// The fields of the acknowledgement that the receiver would send for a data burst.
typedef struct sdrkit_arq_packet_ack_out_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sending station of the burst.
  sdrkit_string callsign;
  // The sequence number of the burst.
  int64_t sequence;
  // Blocks of the burst.
  int64_t blocks;
  // Blocks acknowledged.
  int64_t blocks_acked;
  // The Es/N0 of the burst. Unit: dB.
  double snr_db;
} sdrkit_arq_packet_ack_out_event;

// The fields of an acknowledgement burst heard.
typedef struct sdrkit_arq_packet_ack_in_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The acknowledging station.
  sdrkit_string callsign;
  // The sequence number that it acknowledges.
  int64_t sequence;
  // Blocks acknowledged.
  int64_t blocks_acked;
  // The Es/N0 that it reports. Unit: dB.
  double snr_db;
} sdrkit_arq_packet_ack_in_event;

// The fields of a transmission that started, finished or was refused.
typedef struct sdrkit_arq_packet_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the transmission did: `started`, `finished` or `refused`.
  sdrkit_string state;
  // Why the transmission was refused or ended early; empty for none.
  sdrkit_string reason;
} sdrkit_arq_packet_tx_event;

// The fields of a file that passed every check and was saved.
typedef struct sdrkit_file_transfer_file_received_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The file name.
  sdrkit_string name;
  // The size. Unit: bytes.
  int64_t size_;
  // The hash, hex.
  sdrkit_string hash;
  // Where the file was saved; empty if it was not saved.
  sdrkit_string path;
  // The sending station.
  sdrkit_string callsign;
} sdrkit_file_transfer_file_received_event;

// The fields of an announced transfer.
typedef struct sdrkit_file_transfer_file_open_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The session id.
  int64_t session;
  // The file name.
  sdrkit_string name;
  // The size. Unit: bytes.
  int64_t size_;
  // Bytes held from an earlier session. Unit: bytes.
  int64_t held_bytes;
  // The sending station.
  sdrkit_string callsign;
} sdrkit_file_transfer_file_open_event;

// The fields of a transmission that started, finished or was refused.
typedef struct sdrkit_file_transfer_file_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the transmission did: `started`, `finished` or `refused`.
  sdrkit_string state;
  // Why the transmission was refused or ended early; empty for none.
  sdrkit_string reason;
} sdrkit_file_transfer_file_tx_event;

// The fields of a completed chat or data message.
typedef struct sdrkit_hf_chat_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sending station; empty for a data message.
  sdrkit_string callsign;
  // The recipient; empty for a data message.
  sdrkit_string to;
  // The text.
  sdrkit_string text;
} sdrkit_hf_chat_message_event;

// The fields of a completed beacon.
typedef struct sdrkit_hf_chat_beacon_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The station.
  sdrkit_string callsign;
  // The locator.
  sdrkit_string locator;
  // The position.
  sdrkit_position position;
  // The power. Unit: dBm.
  double power_dbm;
  // The text of a full beacon; empty for a short one.
  sdrkit_string text;
} sdrkit_hf_chat_beacon_event;

// The fields of an acknowledgement heard.
typedef struct sdrkit_hf_chat_ack_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The acknowledging station.
  sdrkit_string callsign;
  // The message id that it acknowledges.
  int64_t message_id;
  // Blocks acknowledged.
  int64_t blocks_acked;
  // The SNR that it reports. Unit: dB.
  double snr_db;
  // The profile that it asks for.
  int64_t profile;
} sdrkit_hf_chat_ack_event;

// The fields of a voice block heard.
typedef struct sdrkit_hf_chat_voice_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The calling station.
  sdrkit_string callsign;
  // The call number.
  int64_t call;
  // The profile of the burst.
  int64_t profile;
} sdrkit_hf_chat_voice_event;

// The fields of a transmission that started, finished or was refused.
typedef struct sdrkit_hf_chat_tx_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the transmission did: `started`, `finished` or `refused`.
  sdrkit_string state;
  // Why the transmission was refused or ended early; empty for none.
  sdrkit_string reason;
} sdrkit_hf_chat_tx_event;

// Fills `rows` with the changed rows of the table "stations" of the stack "slotted_voice" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_slotted_voice_stations_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_slotted_voice_station_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.SlottedVoiceStationRow", "stations", rows, sizeof(sdrkit_slotted_voice_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "voice.station"
// (Station beacon). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_slotted_voice_station_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_slotted_voice_station_event *event) {
  return sdrkit_typed_event(handle, n, "voice.station", "custom.SlottedVoiceStationEvent", event, sizeof(sdrkit_slotted_voice_station_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "voice.tx"
// (Voice transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_slotted_voice_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_slotted_voice_tx_event *event) {
  return sdrkit_typed_event(handle, n, "voice.tx", "custom.TxEvent", event, sizeof(sdrkit_slotted_voice_tx_event));
}

// Runs the command "clear" (Clear) of the stack "slotted_voice" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_slotted_voice_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "slotted_voice" on the session of `handle`. Speaks the speech file (or the test vowel) once with the station's settings. Transmit must be on. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_slotted_voice_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop) of the stack "slotted_voice" on the session of `handle`. Ends the transmission at the next frame.
static inline int32_t sdrkit_slotted_voice_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "slotted_voice" are the sdrkit_slotted_voice_gen_*() functions
// after the settings of its receiver.

// The setting "listen_rate" (Symbol rates) of the stack "slotted_voice". A value of `sdrkit_slotted_voice_listen_rate`.
static inline int32_t sdrkit_slotted_voice_get_listen_rate(SDRKIT_HANDLE handle, sdrkit_slotted_voice_listen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "listen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_slotted_voice_listen_rate)index;
  return code;
}
static inline int32_t sdrkit_slotted_voice_set_listen_rate(SDRKIT_HANDLE handle, sdrkit_slotted_voice_listen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "listen_rate", (int32_t)value);
}

// The setting "callsign" (Station callsign) of the stack "slotted_voice". Sent in the sweep (up to 6 characters and a suffix such as /P) and in the beacon (up to 9 characters).
static inline int32_t sdrkit_slotted_voice_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_slotted_voice_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "grid" (Grid locator) of the stack "slotted_voice". Six characters such as KO26ab. Empty sends AA00aa.
static inline int32_t sdrkit_slotted_voice_get_grid(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "grid", value);
}
static inline int32_t sdrkit_slotted_voice_set_grid(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "grid", value);
}

// The setting "lat" (Latitude) of the stack "slotted_voice". Unit: °.
static inline int32_t sdrkit_slotted_voice_get_lat(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "lat", value);
}
static inline int32_t sdrkit_slotted_voice_set_lat(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "lat", value);
}

// The setting "lon" (Longitude) of the stack "slotted_voice". Unit: °.
static inline int32_t sdrkit_slotted_voice_get_lon(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "lon", value);
}
static inline int32_t sdrkit_slotted_voice_set_lon(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "lon", value);
}

// The setting "power_dbm" (Beacon power value) of the stack "slotted_voice". Unit: dBm.
static inline int32_t sdrkit_slotted_voice_get_power_dbm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "power_dbm", value);
}
static inline int32_t sdrkit_slotted_voice_set_power_dbm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "power_dbm", value);
}

// The setting "link" (Link quality value) of the stack "slotted_voice".
static inline int32_t sdrkit_slotted_voice_get_link(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "link", value);
}
static inline int32_t sdrkit_slotted_voice_set_link(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "link", value);
}

// The setting "beacon" (Beacon) of the stack "slotted_voice".
static inline int32_t sdrkit_slotted_voice_get_beacon(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "beacon", value);
}
static inline int32_t sdrkit_slotted_voice_set_beacon(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "beacon", value);
}

// The setting "beacon_text" (Beacon text) of the stack "slotted_voice". Up to 100 bytes, sent every 10 s in version 3 and in the control channel in version 4.
static inline int32_t sdrkit_slotted_voice_get_beacon_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "beacon_text", value);
}
static inline int32_t sdrkit_slotted_voice_set_beacon_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "beacon_text", value);
}

// The setting "frame_version" (Frame version) of the stack "slotted_voice". A value of `sdrkit_slotted_voice_frame_version`.
static inline int32_t sdrkit_slotted_voice_get_frame_version(SDRKIT_HANDLE handle, sdrkit_slotted_voice_frame_version *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "frame_version", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_slotted_voice_frame_version)index;
  return code;
}
static inline int32_t sdrkit_slotted_voice_set_frame_version(SDRKIT_HANDLE handle, sdrkit_slotted_voice_frame_version value) {
  return sdrkit_typed_setting_set_enum(handle, "frame_version", (int32_t)value);
}

// The setting "bit_rate" (Bit rate) of the stack "slotted_voice". A value of `sdrkit_slotted_voice_bit_rate`.
static inline int32_t sdrkit_slotted_voice_get_bit_rate(SDRKIT_HANDLE handle, sdrkit_slotted_voice_bit_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bit_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_slotted_voice_bit_rate)index;
  return code;
}
static inline int32_t sdrkit_slotted_voice_set_bit_rate(SDRKIT_HANDLE handle, sdrkit_slotted_voice_bit_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "bit_rate", (int32_t)value);
}

// The setting "voice_slots" (Voice slots) of the stack "slotted_voice".
static inline int32_t sdrkit_slotted_voice_get_voice_slots(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "voice_slots", value);
}
static inline int32_t sdrkit_slotted_voice_set_voice_slots(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "voice_slots", value);
}

// The setting "voice_profile" (Voice profile) of the stack "slotted_voice". A value of `sdrkit_slotted_voice_profile`.
static inline int32_t sdrkit_slotted_voice_get_voice_profile(SDRKIT_HANDLE handle, sdrkit_slotted_voice_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "voice_profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_slotted_voice_profile)index;
  return code;
}
static inline int32_t sdrkit_slotted_voice_set_voice_profile(SDRKIT_HANDLE handle, sdrkit_slotted_voice_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "voice_profile", (int32_t)value);
}

// The setting "robust_data" (Robust data slots) of the stack "slotted_voice".
static inline int32_t sdrkit_slotted_voice_get_robust_data(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "robust_data", value);
}
static inline int32_t sdrkit_slotted_voice_set_robust_data(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "robust_data", value);
}

// The setting "speech_file" (Speech file) of the stack "slotted_voice". A 16-bit PCM WAV file that `send` speaks. Empty speaks a test vowel.
static inline int32_t sdrkit_slotted_voice_get_speech_file(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "speech_file", value);
}
static inline int32_t sdrkit_slotted_voice_set_speech_file(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "speech_file", value);
}

// The setting "gen_bit_rate" (Test signal bit rate) of the test signal generator of the stack "slotted_voice". A value of `sdrkit_slotted_voice_gen_bit_rate`.
static inline int32_t sdrkit_slotted_voice_gen_get_bit_rate(SDRKIT_HANDLE handle, sdrkit_slotted_voice_gen_bit_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bit_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_slotted_voice_gen_bit_rate)index;
  return code;
}
static inline int32_t sdrkit_slotted_voice_gen_set_bit_rate(SDRKIT_HANDLE handle, sdrkit_slotted_voice_gen_bit_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bit_rate", (int32_t)value);
}

// The setting "gen_frame_version" (Test signal frame version) of the test signal generator of the stack "slotted_voice". A value of `sdrkit_slotted_voice_gen_frame_version`.
static inline int32_t sdrkit_slotted_voice_gen_get_frame_version(SDRKIT_HANDLE handle, sdrkit_slotted_voice_gen_frame_version *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_frame_version", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_slotted_voice_gen_frame_version)index;
  return code;
}
static inline int32_t sdrkit_slotted_voice_gen_set_frame_version(SDRKIT_HANDLE handle, sdrkit_slotted_voice_gen_frame_version value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_frame_version", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "streams" of the stack "qam_video" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_qam_video_streams_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_qam_video_stream_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.QamVideoStreamRow", "streams", rows, sizeof(sdrkit_qam_video_stream_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "video.tx"
// (Video link transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_qam_video_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_qam_video_tx_event *event) {
  return sdrkit_typed_event(handle, n, "video.tx", "custom.TxEvent", event, sizeof(sdrkit_qam_video_tx_event));
}

// Runs the command "clear" (Clear) of the stack "qam_video" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_qam_video_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "qam_video" on the session of `handle`. Sends the file (or the synthetic test stream) once with the station's settings. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_qam_video_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop) of the stack "qam_video" on the session of `handle`. Ends the transmission at once.
static inline int32_t sdrkit_qam_video_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The setting "listen_profile" (Listen profile) of the stack "qam_video". A value of `sdrkit_qam_video_listen_profile`.
static inline int32_t sdrkit_qam_video_get_listen_profile(SDRKIT_HANDLE handle, sdrkit_qam_video_listen_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "listen_profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qam_video_listen_profile)index;
  return code;
}
static inline int32_t sdrkit_qam_video_set_listen_profile(SDRKIT_HANDLE handle, sdrkit_qam_video_listen_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "listen_profile", (int32_t)value);
}

// The setting "callsign" (Station callsign) of the stack "qam_video". 1 to 7 characters of A-Z, 0-9 and /. Empty sends the demonstration callsign TEST1.
static inline int32_t sdrkit_qam_video_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_qam_video_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "station_tag" (Station tag) of the stack "qam_video". Hex bytes that start the station identification text, before " de " and the callsign. The bytes that other stations of this signal send are a row of data/custom/custom_tags.csv.
static inline int32_t sdrkit_qam_video_get_station_tag(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "station_tag", value);
}
static inline int32_t sdrkit_qam_video_set_station_tag(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "station_tag", value);
}

// The setting "profile" (Profile) of the stack "qam_video". A value of `sdrkit_qam_video_profile`.
static inline int32_t sdrkit_qam_video_get_profile(SDRKIT_HANDLE handle, sdrkit_qam_video_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_qam_video_profile)index;
  return code;
}
static inline int32_t sdrkit_qam_video_set_profile(SDRKIT_HANDLE handle, sdrkit_qam_video_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "profile", (int32_t)value);
}

// The setting "fec" (FEC profile) of the stack "qam_video". 0: 4 data cells; 1: 14 + 2; 2: 12 + 4; 3: 28 + 4; 4: 24 + 8; 5: 6 + 2; 6: 56 + 8; 7: 48 + 16.
static inline int32_t sdrkit_qam_video_get_fec(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "fec", value);
}
static inline int32_t sdrkit_qam_video_set_fec(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "fec", value);
}

// The setting "file" (File) of the stack "qam_video". A file sent as Data packets of stream 1. Empty sends the synthetic test stream.
static inline int32_t sdrkit_qam_video_get_file(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "file", value);
}
static inline int32_t sdrkit_qam_video_set_file(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "file", value);
}

// The setting "media_s" (Synthetic stream) of the stack "qam_video". Unit: s.
static inline int32_t sdrkit_qam_video_get_media_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "media_s", value);
}
static inline int32_t sdrkit_qam_video_set_media_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "media_s", value);
}

// Fills `rows` with the changed rows of the table "pictures" of the stack "picture_burst" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
static inline int32_t sdrkit_picture_burst_pictures_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_picture_burst_picture_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.PictureBurstPictureRow", "pictures", rows, sizeof(sdrkit_picture_burst_picture_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "picture.burst"
// (Picture received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_picture_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_picture_burst_event *event) {
  return sdrkit_typed_event(handle, n, "picture.burst", "custom.PictureBurstEvent", event, sizeof(sdrkit_picture_burst_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "picture.tx"
// (Picture transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_picture_burst_picture_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_picture_burst_picture_tx_event *event) {
  return sdrkit_typed_event(handle, n, "picture.tx", "custom.TxEvent", event, sizeof(sdrkit_picture_burst_picture_tx_event));
}

// Runs the command "clear" (Clear) of the stack "picture_burst" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_picture_burst_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "picture_burst" on the session of `handle`. Sends the picture (or the test picture) once in a burst of 10 s. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_picture_burst_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop) of the stack "picture_burst" on the session of `handle`. Ends the transmission at once.
static inline int32_t sdrkit_picture_burst_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The setting "callsign" (Station callsign) of the stack "picture_burst". Up to 9 characters of A-Z, 0-9, /, - and . Empty sends the demonstration callsign TEST1.
static inline int32_t sdrkit_picture_burst_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_picture_burst_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "profile" (Profile) of the stack "picture_burst". A value of `sdrkit_picture_burst_profile`.
static inline int32_t sdrkit_picture_burst_get_profile(SDRKIT_HANDLE handle, sdrkit_picture_burst_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_picture_burst_profile)index;
  return code;
}
static inline int32_t sdrkit_picture_burst_set_profile(SDRKIT_HANDLE handle, sdrkit_picture_burst_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "profile", (int32_t)value);
}

// The setting "picture" (Picture) of the stack "picture_burst". A binary PPM (P6, maxval 255) picture. Empty sends a test picture. A picture above 1920 x 1080 is shrunk.
static inline int32_t sdrkit_picture_burst_get_picture(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "picture", value);
}
static inline int32_t sdrkit_picture_burst_set_picture(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "picture", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "arq_packet" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
static inline int32_t sdrkit_arq_packet_messages_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_arq_packet_message_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.ArqPacketMessageRow", "messages", rows, sizeof(sdrkit_arq_packet_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "packet.message"
// (Message received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_arq_packet_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_arq_packet_message_event *event) {
  return sdrkit_typed_event(handle, n, "packet.message", "custom.ArqPacketMessageEvent", event, sizeof(sdrkit_arq_packet_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "packet.ack_out"
// (Acknowledgement to send). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_arq_packet_ack_out_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_arq_packet_ack_out_event *event) {
  return sdrkit_typed_event(handle, n, "packet.ack_out", "custom.ArqPacketAckOutEvent", event, sizeof(sdrkit_arq_packet_ack_out_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "packet.ack_in"
// (Acknowledgement heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_arq_packet_ack_in_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_arq_packet_ack_in_event *event) {
  return sdrkit_typed_event(handle, n, "packet.ack_in", "custom.ArqPacketAckInEvent", event, sizeof(sdrkit_arq_packet_ack_in_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "packet.tx"
// (Packet transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_arq_packet_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_arq_packet_tx_event *event) {
  return sdrkit_typed_event(handle, n, "packet.tx", "custom.TxEvent", event, sizeof(sdrkit_arq_packet_tx_event));
}

// Runs the command "clear" (Clear) of the stack "arq_packet" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_arq_packet_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "arq_packet" on the session of `handle`. Sends the message (the text, the file or a test text) once. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_arq_packet_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop) of the stack "arq_packet" on the session of `handle`. Ends the transmission after the burst on the air.
static inline int32_t sdrkit_arq_packet_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The setting "callsign" (Station callsign) of the stack "arq_packet". 1 to 7 characters of A-Z, 0-9 and /. Empty sends the demonstration callsign TEST1.
static inline int32_t sdrkit_arq_packet_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_arq_packet_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "profile" (Profile) of the stack "arq_packet". A value of `sdrkit_arq_packet_profile`.
static inline int32_t sdrkit_arq_packet_get_profile(SDRKIT_HANDLE handle, sdrkit_arq_packet_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arq_packet_profile)index;
  return code;
}
static inline int32_t sdrkit_arq_packet_set_profile(SDRKIT_HANDLE handle, sdrkit_arq_packet_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "profile", (int32_t)value);
}

// The setting "passes" (Passes) of the stack "arq_packet".
static inline int32_t sdrkit_arq_packet_get_passes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "passes", value);
}
static inline int32_t sdrkit_arq_packet_set_passes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "passes", value);
}

// The setting "link" (Link) of the stack "arq_packet". A value of `sdrkit_arq_packet_link`.
static inline int32_t sdrkit_arq_packet_get_link(SDRKIT_HANDLE handle, sdrkit_arq_packet_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_arq_packet_link)index;
  return code;
}
static inline int32_t sdrkit_arq_packet_set_link(SDRKIT_HANDLE handle, sdrkit_arq_packet_link value) {
  return sdrkit_typed_setting_set_enum(handle, "link", (int32_t)value);
}

// The setting "message" (Message) of the stack "arq_packet". The text to send. Empty sends a test text, unless a file is set.
static inline int32_t sdrkit_arq_packet_get_message(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "message", value);
}
static inline int32_t sdrkit_arq_packet_set_message(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "message", value);
}

// The setting "file" (File) of the stack "arq_packet". A file of 1 byte to 8 MiB to send in place of the text.
static inline int32_t sdrkit_arq_packet_get_file(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "file", value);
}
static inline int32_t sdrkit_arq_packet_set_file(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "file", value);
}

// Fills `rows` with the changed rows of the table "files" of the stack "file_transfer" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_file_transfer_files_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_file_transfer_file_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.FileTransferFileRow", "files", rows, sizeof(sdrkit_file_transfer_file_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "file.received"
// (File received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_file_transfer_file_received_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_file_transfer_file_received_event *event) {
  return sdrkit_typed_event(handle, n, "file.received", "custom.FileTransferReceivedEvent", event, sizeof(sdrkit_file_transfer_file_received_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "file.open"
// (Transfer announced). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_file_transfer_file_open_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_file_transfer_file_open_event *event) {
  return sdrkit_typed_event(handle, n, "file.open", "custom.FileTransferOpenEvent", event, sizeof(sdrkit_file_transfer_file_open_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "file.tx"
// (File transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_file_transfer_file_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_file_transfer_file_tx_event *event) {
  return sdrkit_typed_event(handle, n, "file.tx", "custom.TxEvent", event, sizeof(sdrkit_file_transfer_file_tx_event));
}

// Runs the command "clear" (Clear) of the stack "file_transfer" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_file_transfer_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "file_transfer" on the session of `handle`. Sends the file (or a test text) one way or in a two-way session. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_file_transfer_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop) of the stack "file_transfer" on the session of `handle`. Ends the transmission after the frame or burst on the air.
static inline int32_t sdrkit_file_transfer_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The setting "callsign" (Station callsign) of the stack "file_transfer". 1 to 6 characters of A-Z, 0-9, /, - and . that the sender sends; empty sends TEST1. When set, the stack takes only a file sent to this callsign or to CQ.
static inline int32_t sdrkit_file_transfer_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_file_transfer_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "widths" (Widths searched) of the stack "file_transfer". A value of `sdrkit_file_transfer_widths`.
static inline int32_t sdrkit_file_transfer_get_widths(SDRKIT_HANDLE handle, sdrkit_file_transfer_widths *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "widths", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_file_transfer_widths)index;
  return code;
}
static inline int32_t sdrkit_file_transfer_set_widths(SDRKIT_HANDLE handle, sdrkit_file_transfer_widths value) {
  return sdrkit_typed_setting_set_enum(handle, "widths", (int32_t)value);
}

// The setting "folder" (Folder) of the stack "file_transfer". The folder in the host's storage where received files are saved. Empty saves at the top.
static inline int32_t sdrkit_file_transfer_get_folder(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "folder", value);
}
static inline int32_t sdrkit_file_transfer_set_folder(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "folder", value);
}

// The setting "width" (Width) of the stack "file_transfer". A value of `sdrkit_file_transfer_width`.
static inline int32_t sdrkit_file_transfer_get_width(SDRKIT_HANDLE handle, sdrkit_file_transfer_width *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "width", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_file_transfer_width)index;
  return code;
}
static inline int32_t sdrkit_file_transfer_set_width(SDRKIT_HANDLE handle, sdrkit_file_transfer_width value) {
  return sdrkit_typed_setting_set_enum(handle, "width", (int32_t)value);
}

// The setting "profile" (Profile) of the stack "file_transfer". A value of `sdrkit_file_transfer_profile`.
static inline int32_t sdrkit_file_transfer_get_profile(SDRKIT_HANDLE handle, sdrkit_file_transfer_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_file_transfer_profile)index;
  return code;
}
static inline int32_t sdrkit_file_transfer_set_profile(SDRKIT_HANDLE handle, sdrkit_file_transfer_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "profile", (int32_t)value);
}

// The setting "passes" (Passes) of the stack "file_transfer".
static inline int32_t sdrkit_file_transfer_get_passes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "passes", value);
}
static inline int32_t sdrkit_file_transfer_set_passes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "passes", value);
}

// The setting "link" (Link) of the stack "file_transfer". A value of `sdrkit_file_transfer_link`.
static inline int32_t sdrkit_file_transfer_get_link(SDRKIT_HANDLE handle, sdrkit_file_transfer_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_file_transfer_link)index;
  return code;
}
static inline int32_t sdrkit_file_transfer_set_link(SDRKIT_HANDLE handle, sdrkit_file_transfer_link value) {
  return sdrkit_typed_setting_set_enum(handle, "link", (int32_t)value);
}

// The setting "file" (File) of the stack "file_transfer". A file in the host's storage, 1 byte to 256 MiB. Empty sends a short test text.
static inline int32_t sdrkit_file_transfer_get_file(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "file", value);
}
static inline int32_t sdrkit_file_transfer_set_file(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "file", value);
}

// The setting "recipient" (Recipient) of the stack "file_transfer". The callsign of the receiving station of a two-way session; CQ for anyone.
static inline int32_t sdrkit_file_transfer_get_recipient(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "recipient", value);
}
static inline int32_t sdrkit_file_transfer_set_recipient(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "recipient", value);
}

// The setting "session" (Session id) of the stack "file_transfer". 1 to 65000; 0 takes a number from the clock.
static inline int32_t sdrkit_file_transfer_get_session(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "session", value);
}
static inline int32_t sdrkit_file_transfer_set_session(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "session", value);
}

// The setting "fixed_profile" (Fixed profile) of the stack "file_transfer". Off: a two-way sender takes the profile that the receiver asks for.
static inline int32_t sdrkit_file_transfer_get_fixed_profile(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "fixed_profile", value);
}
static inline int32_t sdrkit_file_transfer_set_fixed_profile(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "fixed_profile", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "hf_chat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
static inline int32_t sdrkit_hf_chat_messages_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_hf_chat_message_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.HfChatMessageRow", "messages", rows, sizeof(sdrkit_hf_chat_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "beacons" of the stack "hf_chat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_hf_chat_beacons_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_hf_chat_beacon_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "custom.HfChatBeaconRow", "beacons", rows, sizeof(sdrkit_hf_chat_beacon_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "chat.message"
// (Message received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hf_chat_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hf_chat_message_event *event) {
  return sdrkit_typed_event(handle, n, "chat.message", "custom.HfChatMessageEvent", event, sizeof(sdrkit_hf_chat_message_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "chat.beacon"
// (Beacon received). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hf_chat_beacon_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hf_chat_beacon_event *event) {
  return sdrkit_typed_event(handle, n, "chat.beacon", "custom.HfChatBeaconEvent", event, sizeof(sdrkit_hf_chat_beacon_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "chat.ack"
// (Acknowledgement heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hf_chat_ack_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hf_chat_ack_event *event) {
  return sdrkit_typed_event(handle, n, "chat.ack", "custom.HfChatAckEvent", event, sizeof(sdrkit_hf_chat_ack_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "chat.voice"
// (Voice heard). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hf_chat_voice_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hf_chat_voice_event *event) {
  return sdrkit_typed_event(handle, n, "chat.voice", "custom.HfChatVoiceEvent", event, sizeof(sdrkit_hf_chat_voice_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "chat.tx"
// (Chat transmission). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hf_chat_tx_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hf_chat_tx_event *event) {
  return sdrkit_typed_event(handle, n, "chat.tx", "custom.TxEvent", event, sizeof(sdrkit_hf_chat_tx_event));
}

// Runs the command "clear" (Clear) of the stack "hf_chat" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_hf_chat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "send" (Send) of the stack "hf_chat" on the session of `handle`. Sends the message in the next caller slots. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
static inline int32_t sdrkit_hf_chat_send(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "send", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "stop" (Stop) of the stack "hf_chat" on the session of `handle`. Ends the transmission after the burst on the air.
static inline int32_t sdrkit_hf_chat_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// The setting "listen" (Profiles searched) of the stack "hf_chat". A value of `sdrkit_hf_chat_listen`.
static inline int32_t sdrkit_hf_chat_get_listen(SDRKIT_HANDLE handle, sdrkit_hf_chat_listen *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "listen", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hf_chat_listen)index;
  return code;
}
static inline int32_t sdrkit_hf_chat_set_listen(SDRKIT_HANDLE handle, sdrkit_hf_chat_listen value) {
  return sdrkit_typed_setting_set_enum(handle, "listen", (int32_t)value);
}

// The setting "callsign" (Station callsign) of the stack "hf_chat". 1 to 9 characters of A-Z, 0-9, '-', '/' and '.'. Empty sends the demonstration callsign TEST1.
static inline int32_t sdrkit_hf_chat_get_callsign(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "callsign", value);
}
static inline int32_t sdrkit_hf_chat_set_callsign(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "callsign", value);
}

// The setting "profile" (Profile) of the stack "hf_chat". A value of `sdrkit_hf_chat_profile`.
static inline int32_t sdrkit_hf_chat_get_profile(SDRKIT_HANDLE handle, sdrkit_hf_chat_profile *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "profile", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hf_chat_profile)index;
  return code;
}
static inline int32_t sdrkit_hf_chat_set_profile(SDRKIT_HANDLE handle, sdrkit_hf_chat_profile value) {
  return sdrkit_typed_setting_set_enum(handle, "profile", (int32_t)value);
}

// The setting "kind" (Message) of the stack "hf_chat". A value of `sdrkit_hf_chat_kind`.
static inline int32_t sdrkit_hf_chat_get_kind(SDRKIT_HANDLE handle, sdrkit_hf_chat_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hf_chat_kind)index;
  return code;
}
static inline int32_t sdrkit_hf_chat_set_kind(SDRKIT_HANDLE handle, sdrkit_hf_chat_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "kind", (int32_t)value);
}

// The setting "to" (Recipient) of the stack "hf_chat". A callsign; empty sends to CQ.
static inline int32_t sdrkit_hf_chat_get_to(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "to", value);
}
static inline int32_t sdrkit_hf_chat_set_to(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "to", value);
}

// The setting "text" (Text) of the stack "hf_chat". Up to 100 characters for a chat, 10 for a full beacon. Empty sends a test text.
static inline int32_t sdrkit_hf_chat_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "text", value);
}
static inline int32_t sdrkit_hf_chat_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "text", value);
}

// The setting "locator" (Locator) of the stack "hf_chat". Six characters, AA00AA.
static inline int32_t sdrkit_hf_chat_get_locator(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "locator", value);
}
static inline int32_t sdrkit_hf_chat_set_locator(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "locator", value);
}

// The setting "lat" (Latitude) of the stack "hf_chat". Unit: °.
static inline int32_t sdrkit_hf_chat_get_lat(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "lat", value);
}
static inline int32_t sdrkit_hf_chat_set_lat(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "lat", value);
}

// The setting "lon" (Longitude) of the stack "hf_chat". Unit: °.
static inline int32_t sdrkit_hf_chat_get_lon(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "lon", value);
}
static inline int32_t sdrkit_hf_chat_set_lon(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "lon", value);
}

// The setting "power_step" (Power step) of the stack "hf_chat". EIRP dBm = -40 + 0.5 x step; a short beacon takes 0 to 63.
static inline int32_t sdrkit_hf_chat_get_power_step(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "power_step", value);
}
static inline int32_t sdrkit_hf_chat_set_power_step(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "power_step", value);
}

// The setting "file" (File) of the stack "hf_chat". The bytes of a data message.
static inline int32_t sdrkit_hf_chat_get_file(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "file", value);
}
static inline int32_t sdrkit_hf_chat_set_file(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "file", value);
}

// The setting "passes" (Passes) of the stack "hf_chat".
static inline int32_t sdrkit_hf_chat_get_passes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "passes", value);
}
static inline int32_t sdrkit_hf_chat_set_passes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "passes", value);
}

// The setting "link" (Link) of the stack "hf_chat". A value of `sdrkit_hf_chat_link`.
static inline int32_t sdrkit_hf_chat_get_link(SDRKIT_HANDLE handle, sdrkit_hf_chat_link *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "link", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hf_chat_link)index;
  return code;
}
static inline int32_t sdrkit_hf_chat_set_link(SDRKIT_HANDLE handle, sdrkit_hf_chat_link value) {
  return sdrkit_typed_setting_set_enum(handle, "link", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_slotted_voice_fields_read(SDRKIT_HANDLE handle, sdrkit_slotted_voice_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.SlottedVoiceFields", fields, sizeof(sdrkit_slotted_voice_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_custom_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_custom_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.DecoderFields", fields, sizeof(sdrkit_custom_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_qam_video_fields_read(SDRKIT_HANDLE handle, sdrkit_qam_video_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.QamVideoFields", fields, sizeof(sdrkit_qam_video_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_picture_burst_fields_read(SDRKIT_HANDLE handle, sdrkit_picture_burst_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.PictureBurstFields", fields, sizeof(sdrkit_picture_burst_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_arq_packet_fields_read(SDRKIT_HANDLE handle, sdrkit_arq_packet_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.ArqPacketFields", fields, sizeof(sdrkit_arq_packet_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_file_transfer_fields_read(SDRKIT_HANDLE handle, sdrkit_file_transfer_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.FileTransferFields", fields, sizeof(sdrkit_file_transfer_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hf_chat_fields_read(SDRKIT_HANDLE handle, sdrkit_hf_chat_fields *fields) {
  return sdrkit_typed_fields(handle, "custom.HfChatFields", fields, sizeof(sdrkit_hf_chat_fields));
}

#endif
