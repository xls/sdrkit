<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DTMB

DTMB digital terrestrial TV (China, Hong Kong, Macau, Cuba, Pakistan), 8 or 6 MHz channel

Stack id: `dtmb`. Group: TV.

Signal types: DTMB.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 530 MHz |
| Tuning range | 170 MHz to 870 MHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 8 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dtmb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `width` | Channel | choice | `8` | `8` (8 MHz), `6` (6 MHz (Cuba)) | A stack set to the wrong width finds nothing. |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `detection` | Detection level | whole number |
| `tracking` | Frames tracked | yes or no |
| `header` | Frame header | choice |
| `pn_phase_rotates` | PN phase rotates | yes or no |
| `width` | Channel width | frequency |
| `frame_length` | Signal frame | duration (s) |
| `cn_db` | C/N (PN header) | number (dB) |
| `echo_span` | Echo span | duration (s) |
| `frames` | Frames | whole number |
| `frames_erased` | Frames erased | whole number |
| `no_header_pct` | Recent frames without a usable header | number (%) |
| `rate_ok` | Input rate ok | yes or no |
| `step_pn_sync` | PN sync step | choice |
| `si_index` | System information index | whole number |
| `si_corr` | System information correlation | number |
| `mapping` | Modulation | choice |
| `code_rate` | LDPC code rate | choice |
| `interleaver` | Time interleaver | choice |
| `carriers` | Carriers | choice |
| `mer_db` | MER (data symbols) | number (dB) |
| `ldpc_ok` | LDPC code words good | whole number |
| `ldpc_bad` | LDPC code words bad | whole number |
| `ldpc_iterations` | LDPC iterations | number |
| `bch_corrected` | BCH corrections | whole number |
| `codewords_dropped` | Code words dropped | whole number |
| `codewords_skipped` | Code words skipped | whole number |
| `packets` | Transport packets | whole number |
| `ts_lock` | Transport stream lock | yes or no |
| `payload_bps` | Payload rate | number (bit/s) |
| `note` | Note | text |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `ts_demux.null_packets` | Null packets | whole number |
| `ts_demux.tei_packets` | Packets with errors | whole number |
| `ts_demux.cc_errors` | Continuity errors | whole number |
| `ts_demux.mux_kbps` | Multiplex bit rate | number (kbit/s) |
| `ts_demux.null_kbps` | Null bit rate | number (kbit/s) |
| `ts_demux.selected_service` | Service playing | whole number |
| `ts_demux.selected_audio` | Audio PID | whole number |
| `ts_demux.selected_subtitle` | Subtitle PID | whole number |
| `ts_demux.player_status` | Player | choice |
| `ts_demux.audio_decoded` | Sound decoded | yes or no |
| `psi_si.network_name` | Network | text |
| `psi_si.network_id` | Network id | whole number |
| `psi_si.onid` | Original network id | whole number |
| `psi_si.tsid` | Transport stream id | whole number |
| `psi_si.service_count` | Services | whole number |
| `psi_si.utc` | UTC | time |
| `psi_si.local_offset_min` | Local offset | whole number (min) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `services` | Services | `service_id` (whole number), `name` (text), `provider` (text), `service_type` (whole number), `kind` (choice), `lcn` (whole number), `visible` (yes or no), `scrambled` (yes or no), `ca_state` (choice), `pmt_pid` (whole number), `pcr_pid` (whole number), `running_status` (whole number), `in_pat` (yes or no), `eit_schedule` (yes or no), `eit_present_following` (yes or no), `onid` (whole number), `tsid` (whole number), `now_title` (text), `now_text` (text), `now_start` (time), `now_duration` (duration (s)), `next_title` (text), `next_text` (text), `next_start` (time), `next_duration` (duration (s)) |
| `tracks` | Tracks | `service` (text), `pid` (whole number), `stream_type` (whole number), `kind` (choice), `codec` (choice), `packaging` (choice), `fourcc` (text), `lang` (text), `audio_type` (whole number), `audio_description` (yes or no), `hard_of_hearing` (yes or no), `component_tag` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no), `profile` (text), `level` (text), `width` (whole number (px)), `height` (whole number (px)), `frame_rate` (number (Hz)), `aspect` (text), `interlaced` (yes or no), `sample_rate` (frequency), `channels` (whole number), `layout` (text), `bitrate_kbps` (whole number (kbit/s)), `object_type` (text), `entry_type` (whole number), `composition_page` (whole number), `ancillary_page` (whole number), `magazine` (whole number), `teletext_page` (whole number) |
| `guide` | Guide | `service` (text), `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `running_status` (whole number), `title` (text), `text` (text), `extended` (text), `lang` (text), `genre` (whole number), `content` (bytes), `min_age` (whole number), `crid_programme` (text), `crid_series` (text), `free_ca` (yes or no), `other_ts` (yes or no), `schedule` (yes or no) |
| `guide_ratings` | Guide ratings | `event` (text), `country` (text), `rating` (whole number) |
| `ts_demux.pids` | PIDs | `pid` (whole number), `label` (text), `packets` (whole number), `cc_errors` (whole number), `tei` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no) |
| `psi_si.sections` | Sections | `accepted` (whole number), `crc` (whole number), `length` (whole number), `other` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `cir` | Channel impulse response | symbol | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `service.list` | Service list | `tsid` (whole number), `onid` (whole number), `network_id` (whole number), `network_name` (text), `service_count` (whole number) |
| `epg.event` | Guide event | `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `title` (text) |
| `ts.time` | Stream time | `utc` (time), `local_offset_min` (whole number (min)) |
| `ts.network` | Network | `network_id` (whole number), `network_name` (text), `onid` (whole number), `tsid` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |
| `track.select` | Select a track | `row` (text, required) |  | Plays an audio track or shows a subtitle track. |

## Typed interface

The stack is in the family `tvterr`, version 2.0. The C header `sdrkit/tvterr.h`, the registry record `registry/tvterr.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Television and video (`television`). Media: sound (programme), video.

Key readings: `mer_db`, `snr_db`.

### Result fields

Struct `DtmbFields`. The result fields of `dtmb`: the frame headers, the system information, the decoder and the signal. A value of the header has no value while no header is tracked, and a value of the profile none before the system information is decoded.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `detection` | whole number | always | 1.0 | Detection level: 0 nothing, 1 frame headers tracked, 2 system information decoded, 3 transport stream locked. |
| `tracking` | yes or no | always | 1.0 | The header grid holds. |
| `header` | choice | optional | 1.0 | The frame header. |
| `pn_phase_rotates` | yes or no | optional | 1.0 | The phase of the header changes from frame to frame. |
| `width` | frequency | always | 1.0 | The channel width. |
| `frame_length` | duration (s) | optional | 1.0 | The length of a signal frame (header and body). |
| `cn_db` | number (dB) | optional | 1.0 | C/N from the fit of the header. |
| `echo_span` | duration (s) | optional | 1.0 | The time between the first and the last significant path. |
| `frames` | whole number | always | 1.0 | Frames with a usable header. |
| `frames_erased` | whole number | always | 1.0 | Frames erased: their header window was exact silence (a zero-filled gap of the source). |
| `no_header_pct` | number (%) | always | 1.0 | The share of recent frames without a usable header. |
| `rate_ok` | yes or no | always | 1.0 | The input rate is enough for the width. |
| `step_pn_sync` | choice | always | 1.0 | The header sync step. |
| `si_index` | whole number | optional | 1.0 | The system information index, 3 to 24. |
| `si_corr` | number | optional | 1.0 | The normalised score of the leading system information word (1 is perfect). No value while no header is tracked. |
| `mapping` | choice | optional | 1.0 | The mapping. |
| `code_rate` | choice | optional | 1.0 | The LDPC rate. |
| `interleaver` | choice | optional | 1.0 | The time interleaver. |
| `carriers` | choice | optional | 1.0 | The carrier form. |
| `mer_db` | number (dB) | optional | 1.0 | The MER of the equalised data symbols. |
| `ldpc_ok` | whole number | always | 1.0 | LDPC code words that decoded. |
| `ldpc_bad` | whole number | always | 1.0 | LDPC code words that did not. |
| `ldpc_iterations` | number | always | 1.0 | The mean LDPC iterations of recent code words. |
| `bch_corrected` | whole number | always | 1.0 | BCH blocks with one bit corrected. |
| `codewords_dropped` | whole number | always | 1.0 | Code words dropped for lack of time. |
| `codewords_skipped` | whole number | always | 1.0 | Code words not tried during a long run of failures. |
| `packets` | whole number | always | 1.0 | Transport packets delivered. |
| `ts_lock` | yes or no | always | 1.0 | A good packet came out within the last 0.7 s. |
| `payload_bps` | number (bit/s) | optional | 1.0 | The net bit rate of the signalled profile. No value before the profile and the header are known. |
| `note` | text | always | 1.0 | What the stack decodes and which of its tables no public document confirms. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. No value while no header is tracked. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The error of the symbol clock. No value while no header is tracked. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR: the MER once the profile is known and it lies between 8 dB and the C/N, else the C/N. |

### Table `services`

Struct `TsServiceRow`. A service of the multiplex. The row key is the service_id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service_id` | whole number | always | 1.0 | service_id. |
| `name` | text | always | 1.0 | The name. |
| `provider` | text | always | 1.0 | The provider. |
| `service_type` | whole number | always | 1.0 | service_type. |
| `kind` | choice | always | 1.0 | What the service is. |
| `lcn` | whole number | always | 1.0 | The logical channel number (0 if none). |
| `visible` | yes or no | always | 1.0 | Shown in a channel list. |
| `scrambled` | yes or no | always | 1.0 | Scrambled. |
| `ca_state` | choice | always | 1.0 | Whether it is scrambled. |
| `pmt_pid` | whole number | always | 1.0 | The PMT PID. |
| `pcr_pid` | whole number | always | 1.0 | The PCR PID. |
| `running_status` | whole number | always | 1.0 | running_status. |
| `in_pat` | yes or no | always | 1.0 | Listed in the PAT. |
| `eit_schedule` | yes or no | always | 1.0 | EIT_schedule_flag. |
| `eit_present_following` | yes or no | always | 1.0 | EIT_present_following_flag. |
| `onid` | whole number | always | 1.0 | original_network_id. |
| `tsid` | whole number | always | 1.0 | transport_stream_id. |
| `now_title` | text | optional | 1.0 | The title of the present event. |
| `now_text` | text | optional | 1.0 | The text of the present event. |
| `now_start` | time | optional | 1.0 | The start of the present event. |
| `now_duration` | duration (s) | optional | 1.0 | The length of the present event. |
| `next_title` | text | optional | 1.0 | The title of the following event. |
| `next_text` | text | optional | 1.0 | The text of the following event. |
| `next_start` | time | optional | 1.0 | The start of the following event. |
| `next_duration` | duration (s) | optional | 1.0 | The length of the following event. |

### Table `tracks`

Struct `TsTrackRow`. A track of a service: an elementary stream, or one entry of a subtitle or teletext stream. The row key is `<service>/<pid>` or `<service>/<pid>/<n>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service` | text | always | 1.0 | The row key of the service in `services`. |
| `pid` | whole number | always | 1.0 | The PID. |
| `stream_type` | whole number | always | 1.0 | stream_type. |
| `kind` | choice | optional | 1.0 | What the track is. |
| `codec` | choice | optional | 1.0 | The codec. |
| `packaging` | choice | optional | 1.0 | The packaging. |
| `fourcc` | text | optional | 1.0 | The four-character code. |
| `lang` | text | always | 1.0 | The ISO 639-2 language code. |
| `audio_type` | whole number | always | 1.0 | audio_type of the language entry. |
| `audio_description` | yes or no | always | 1.0 | An audio description for people who cannot see. |
| `hard_of_hearing` | yes or no | always | 1.0 | Subtitles for the hard of hearing. |
| `component_tag` | whole number | optional | 1.0 | component_tag. |
| `kbps` | number (kbit/s) | always | 1.0 | The bit rate of the PID. |
| `scrambled` | yes or no | always | 1.0 | Scrambled. |
| `profile` | text | optional | 1.0 | The profile. |
| `level` | text | optional | 1.0 | The level. |
| `width` | whole number (px) | optional | 1.0 | The picture width. No value for a track that is not video. |
| `height` | whole number (px) | optional | 1.0 | The picture height. No value for a track that is not video. |
| `frame_rate` | number (Hz) | optional | 1.0 | Frames per second. |
| `aspect` | text | optional | 1.0 | The display aspect. |
| `interlaced` | yes or no | optional | 1.0 | Interlaced. |
| `sample_rate` | frequency | optional | 1.0 | The audio sample rate. No value for a track that is not audio. |
| `channels` | whole number | optional | 1.0 | The audio channels. No value for a track that is not audio. |
| `layout` | text | optional | 1.0 | The channel layout. |
| `bitrate_kbps` | whole number (kbit/s) | optional | 1.0 | The bit rate that the frame header provides. |
| `object_type` | text | optional | 1.0 | The AAC object type or the MPEG audio layer. |
| `entry_type` | whole number | optional | 1.0 | subtitling_type, or teletext_type. |
| `composition_page` | whole number | optional | 1.0 | composition_page_id. |
| `ancillary_page` | whole number | optional | 1.0 | ancillary_page_id. |
| `magazine` | whole number | optional | 1.0 | teletext_magazine_number. |
| `teletext_page` | whole number | optional | 1.0 | teletext_page_number. |

### Table `guide`

Struct `TsGuideRow`. An event of the programme guide. The row key is `<onid>/<tsid>/<service_id>/<event_id>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service` | text | always | 1.0 | The row key of the service in `services`. |
| `onid` | whole number | always | 1.0 | original_network_id. |
| `tsid` | whole number | always | 1.0 | transport_stream_id. |
| `service_id` | whole number | always | 1.0 | service_id. |
| `event_id` | whole number | always | 1.0 | event_id. |
| `start` | time | always | 1.0 | The start. |
| `duration` | duration (s) | always | 1.0 | The length. |
| `running_status` | whole number | always | 1.0 | running_status. |
| `title` | text | always | 1.0 | The title. |
| `text` | text | always | 1.0 | The short text. |
| `extended` | text | always | 1.0 | The extended text. |
| `lang` | text | always | 1.0 | The ISO 639-2 language code. |
| `genre` | whole number | optional | 1.0 | The first level-1 content nibble. No value for an event without one. |
| `content` | bytes | always | 1.0 | The content bytes (up to 8). |
| `min_age` | whole number | always | 1.0 | The highest minimum age over all countries (0 if none). |
| `crid_programme` | text | always | 1.0 | The programme CRID. |
| `crid_series` | text | always | 1.0 | The series CRID. |
| `free_ca` | yes or no | always | 1.0 | free_CA_mode. |
| `other_ts` | yes or no | always | 1.0 | From an EIT of another transport stream. |
| `schedule` | yes or no | always | 1.0 | From a schedule table. |

### Table `guide_ratings`

Struct `TsGuideRatingRow`. The parental rating of a guide event in one country. The row key is `<guide row>/<country>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `event` | text | always | 1.0 | The row key of the guide event in `guide`. |
| `country` | text | always | 1.0 | The ISO 3166 country code. |
| `rating` | whole number | always | 1.0 | The rating (minimum age minus 3). |

### Table `ts_demux.pids`

Struct `TsPidRow`. A PID of the multiplex. The row key is the PID as four hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pid` | whole number | always | 1.0 | The PID. |
| `label` | text | always | 1.0 | What the PID carries. |
| `packets` | whole number | always | 1.0 | Packets of the PID. |
| `cc_errors` | whole number | always | 1.0 | Continuity errors. |
| `tei` | whole number | always | 1.0 | Packets of the PID with the transport_error_indicator. |
| `kbps` | number (kbit/s) | always | 1.0 | The bit rate of the PID. |
| `scrambled` | yes or no | always | 1.0 | Scrambled. |

### Table `psi_si.sections`

Struct `TsSectionRow`. The section counts of one table. The row key is the table name (PAT, PMT, SDT, ...).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `accepted` | whole number | always | 1.0 | Sections accepted. |
| `crc` | whole number | always | 1.0 | Sections with a bad CRC. |
| `length` | whole number | always | 1.0 | Sections with a bad length. |
| `other` | whole number | always | 1.0 | Sections refused for another reason. |

### Event `service.list`

Struct `TsServiceListEvent`. The fields of a new service list.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tsid` | whole number | always | 1.0 | transport_stream_id. |
| `onid` | whole number | always | 1.0 | original_network_id. |
| `network_id` | whole number | always | 1.0 | network_id. |
| `network_name` | text | always | 1.0 | The network name. |
| `service_count` | whole number | always | 1.0 | The number of services. |

### Event `epg.event`

Struct `TsGuideEvent`. The fields of a new or changed guide event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `onid` | whole number | always | 1.0 | original_network_id. |
| `tsid` | whole number | always | 1.0 | transport_stream_id. |
| `service_id` | whole number | always | 1.0 | service_id. |
| `event_id` | whole number | always | 1.0 | event_id. |
| `start` | time | always | 1.0 | The start. |
| `duration` | duration (s) | always | 1.0 | The length. |
| `title` | text | always | 1.0 | The title. |

### Event `ts.time`

Struct `TsTimeEvent`. The fields of a new stream time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the stream. |
| `local_offset_min` | whole number (min) | optional | 1.0 | The local time offset in force. No value before the stream gave one. |

### Event `ts.network`

Struct `TsNetworkEvent`. The fields of a new network.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `network_id` | whole number | always | 1.0 | network_id. |
| `network_name` | text | always | 1.0 | The network name. |
| `onid` | whole number | always | 1.0 | original_network_id. |
| `tsid` | whole number | always | 1.0 | transport_stream_id. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `service.select` | `sdrkit_dtmb_service_select` | `sdrkit_tvterr_service_select_args` | none |
| `track.select` | `sdrkit_dtmb_track_select` | `sdrkit_tvterr_track_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `width` | `sdrkit_dtmb_get_width` | `sdrkit_dtmb_set_width` |
| `decode_video` | `sdrkit_dtmb_get_decode_video` | `sdrkit_dtmb_set_decode_video` |

## Try it

```sh
sdrkit run dtmb --source synth:dtmb
sdrkit run dtmb --source iiod:192.168.2.1 --freq 530
```
<!-- generated: end -->
