<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ISDB-TSB

ISDB-TSB narrow-band digital sound and data broadcasting (ITU-R BS.1114 System F), 1 or 3 segments; also the one-segment service of ISDB-T

Stack id: `isdbtsb`. Group: TV.

Signal types: ISDB-TSB.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 100 MHz |
| Tuning range | 90 MHz to 770 MHz |
| Sample rate | 2.048 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 1.3 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:isdbtsb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `width` | Channel width | choice | `6` | `6` (6 MHz), `7` (7 MHz), `8` (8 MHz (Maldives)), `auto` (Automatic) |  |
| `segments` | Segments | choice | `auto` | `auto` (Automatic), `1`, `3` |  |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `edge_pilot` | Upper-edge pilot | choice | `sequence` | `sequence` (Pilot sequence), `plus` (+4/3), `minus` (-4/3) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mode` | Test signal mode | choice | `3` | `1` (Mode 1), `2` (Mode 2), `3` (Mode 3) |  |
| `gen_guard` | Test signal guard | choice | `1/8` | `1/4`, `1/8`, `1/16`, `1/32` |  |
| `gen_modulation` | Test signal layer B modulation | choice | `64qam` | `qpsk` (QPSK), `16qam` (16QAM), `64qam` (64QAM) |  |
| `gen_code_rate` | Test signal layer B code rate | choice | `3/4` | `1/2`, `2/3`, `3/4`, `5/6`, `7/8` |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `detection` | Detection level | whole number |
| `width` | Channel width | frequency |
| `width_auto` | Width found automatically | yes or no |
| `ifft_rate` | IFFT rate | frequency |
| `mode` | Mode | whole number |
| `guard` | Guard interval | choice |
| `fft_size` | FFT size | whole number |
| `carriers` | Carriers | whole number |
| `carrier_spacing` | Carrier spacing | frequency |
| `frame_length` | Frame length | duration (s) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `snr_db` | SNR | number (dB) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `carrier_shift` | Integer carrier shift | whole number |
| `frame_symbol` | Symbol in the frame | whole number |
| `since_tmcc` | Since the last TMCC | duration (s) |
| `mirrored` | I and Q swapped | yes or no |
| `system_id` | System identification | whole number |
| `switch_countdown` | Parameter switching countdown | whole number |
| `emergency_alarm` | Emergency alarm flag | yes or no |
| `partial_reception` | Partial reception (one-seg) | yes or no |
| `packets_ok_pct` | Packets ok | number (%) |
| `net_rate_bit_s` | Net bit rate | number (bit/s) |
| `quality_pct` | Signal quality | number (%) |
| `quality` | Signal quality | choice |
| `step_mode` | Mode step | choice |
| `step_tmcc` | TMCC step | choice |
| `step_layers` | Layers step | choice |
| `step_ts` | TS step | choice |
| `band_segments` | Segments in use | whole number |
| `note` | Note | text |
| `dc.dc_dbfs` | DC level | number (dBFS) |
| `ts_demux.packets` | Packets | whole number |
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
| `layers` | Layers | `layer` (choice), `partial` (yes or no), `segments` (whole number), `modulation` (choice), `code_rate` (choice), `interleave_length` (whole number), `rate_bit_s` (number (bit/s)), `packets` (whole number), `rs_clean` (whole number), `rs_corrected` (whole number), `rs_failed` (whole number), `rs_locked` (yes or no), `viterbi_margin` (number) |
| `services` | Services | `service_id` (whole number), `name` (text), `provider` (text), `service_type` (whole number), `kind` (choice), `lcn` (whole number), `visible` (yes or no), `scrambled` (yes or no), `ca_state` (choice), `pmt_pid` (whole number), `pcr_pid` (whole number), `running_status` (whole number), `in_pat` (yes or no), `eit_schedule` (yes or no), `eit_present_following` (yes or no), `onid` (whole number), `tsid` (whole number), `now_title` (text), `now_text` (text), `now_start` (time), `now_duration` (duration (s)), `next_title` (text), `next_text` (text), `next_start` (time), `next_duration` (duration (s)) |
| `tracks` | Tracks | `service` (text), `pid` (whole number), `stream_type` (whole number), `kind` (choice), `codec` (choice), `packaging` (choice), `fourcc` (text), `lang` (text), `audio_type` (whole number), `audio_description` (yes or no), `hard_of_hearing` (yes or no), `component_tag` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no), `profile` (text), `level` (text), `width` (whole number (px)), `height` (whole number (px)), `frame_rate` (number (Hz)), `aspect` (text), `interlaced` (yes or no), `sample_rate` (frequency), `channels` (whole number), `layout` (text), `bitrate_kbps` (whole number (kbit/s)), `object_type` (text), `entry_type` (whole number), `composition_page` (whole number), `ancillary_page` (whole number), `magazine` (whole number), `teletext_page` (whole number) |
| `guide` | Guide | `service` (text), `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `running_status` (whole number), `title` (text), `text` (text), `extended` (text), `lang` (text), `genre` (whole number), `content` (bytes), `min_age` (whole number), `crid_programme` (text), `crid_series` (text), `free_ca` (yes or no), `other_ts` (yes or no), `schedule` (yes or no) |
| `guide_ratings` | Guide ratings | `event` (text), `country` (text), `rating` (whole number) |
| `ts_demux.pids` | PIDs | `pid` (whole number), `label` (text), `packets` (whole number), `cc_errors` (whole number), `tei` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no) |
| `psi_si.sections` | Sections | `accepted` (whole number), `crc` (whole number), `length` (whole number), `other` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Equalised data cells | I | Q |
| `pilots` | Scattered pilots | I | Q |
| `channel_db` | Channel magnitude | carrier | dB |
| `channel_phase` | Channel phase | carrier | rad |
| `impulse` | Impulse response | samples | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `isdbt.tmcc` | TMCC | `system_id` (whole number), `switch_countdown` (whole number), `emergency_alarm` (yes or no), `partial_reception` (yes or no), `text` (text), `urgency` (choice), `family` (choice) |
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

The stack is in the family `tvterr`, version 1.5. The C header `sdrkit/tvterr.h`, the registry record `registry/tvterr.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `IsdbtFields`. The result fields of `isdbt` and `isdbtsb`: the detection, the transmission parameters, the signal and the TMCC. A parameter of the mode has no value before the mode is found. A value of the TMCC has no value while no TMCC is decoded.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `detection` | whole number | always | 1.0 | Detection level: 0 nothing, 1 a periodic cyclic prefix, 2 TMCC decoded, 3 transport stream flowing. |
| `width` | frequency | always | 1.0 | The channel width in use. |
| `width_auto` | yes or no | always | 1.0 | The width setting is automatic. |
| `ifft_rate` | frequency | always | 1.0 | The IFFT sampling rate of the width. |
| `mode` | whole number | optional | 1.0 | Mode 1, 2 or 3. |
| `guard` | choice | optional | 1.0 | The guard interval. |
| `fft_size` | whole number | optional | 1.0 | The FFT size. |
| `carriers` | whole number | optional | 1.0 | The carriers in the band. |
| `carrier_spacing` | frequency | optional | 1.0 | The carrier spacing. |
| `frame_length` | duration (s) | optional | 1.0 | The frame length (204 symbols). |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The error of the sample clock. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the equalised cells. |
| `frames_ok` | whole number | always | 1.0 | Reed-Solomon words clean or corrected. |
| `frames_bad` | whole number | always | 1.0 | Reed-Solomon words that failed. |
| `carrier_shift` | whole number | optional | 1.0 | The integer carrier shift that the TMCC search found. |
| `frame_symbol` | whole number | optional | 1.0 | The index of the symbol in the frame. No value while no TMCC is decoded. |
| `since_tmcc` | duration (s) | optional | 1.0 | The time since the last good TMCC. |
| `mirrored` | yes or no | always | 1.0 | I and Q are swapped. |
| `system_id` | whole number | optional | 1.0 | The system identification of the TMCC (0 ISDB-T, 1 ISDB-TSB). |
| `switch_countdown` | whole number | optional | 1.0 | The parameter switching countdown of the TMCC (15: no switch announced). |
| `emergency_alarm` | yes or no | optional | 1.0 | The emergency alarm start flag of the TMCC. |
| `partial_reception` | yes or no | optional | 1.0 | The partial reception flag of the TMCC. |
| `packets_ok_pct` | number (%) | always | 1.0 | The share of the Reed-Solomon words that are clean or corrected. |
| `net_rate_bit_s` | number (bit/s) | always | 1.0 | The net bit rate of every layer together. |
| `quality_pct` | number (%) | always | 1.0 | The signal quality score. |
| `quality` | choice | always | 1.0 | The signal quality class. |
| `step_mode` | choice | always | 1.0 | The mode step: the mode and the guard are found. |
| `step_tmcc` | choice | always | 1.0 | The TMCC step. |
| `step_layers` | choice | always | 1.0 | The layers step: a layer is synchronised. |
| `step_ts` | choice | always | 1.0 | The transport stream step. |

### Result fields

Struct `IsdbtNarrowFields`. The result fields that a narrow-band receiver (`isdbtsb`) adds.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band_segments` | whole number | always | 1.0 | The segment count that the receiver uses (1 or 3). |
| `note` | text | always | 1.0 | What the receiver decodes and which of its values no public document confirms. |

### Table `layers`

Struct `IsdbtLayerRow`. A row of the layer table of `isdbt` and `isdbtsb`, one for each layer in use. The row key is `A`, `B` or `C`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `layer` | choice | always | 1.0 | The layer. |
| `partial` | yes or no | always | 1.0 | The layer is the partial-reception segment (one-seg). |
| `segments` | whole number | always | 1.0 | The segments of the layer. |
| `modulation` | choice | optional | 1.0 | The carrier modulation. No value for a code that the standard reserves. |
| `code_rate` | choice | optional | 1.0 | The inner code rate. No value for a code that the standard reserves. |
| `interleave_length` | whole number | always | 1.0 | The time interleaving length I. |
| `rate_bit_s` | number (bit/s) | always | 1.0 | The net bit rate of the layer. |
| `packets` | whole number | always | 1.0 | The packets delivered. |
| `rs_clean` | whole number | always | 1.0 | Reed-Solomon words clean. |
| `rs_corrected` | whole number | always | 1.0 | Reed-Solomon words corrected. |
| `rs_failed` | whole number | always | 1.0 | Reed-Solomon words that failed. |
| `rs_locked` | yes or no | always | 1.0 | Reed-Solomon has synchronised the layer. |
| `viterbi_margin` | number | always | 1.0 | The Viterbi margin (1: no noise). |

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

### Event `isdbt.tmcc`

Struct `IsdbtTmccEvent`. The fields of the event `isdbt.tmcc`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system_id` | whole number | always | 1.0 | The system identification (0 ISDB-T, 1 ISDB-TSB). |
| `switch_countdown` | whole number | always | 1.0 | The parameter switching countdown (15: no switch announced). |
| `emergency_alarm` | yes or no | always | 1.0 | The emergency alarm start flag. |
| `partial_reception` | yes or no | always | 1.0 | The partial reception flag. |
| `text` | text | always | 1.0 | The layers in use ("A 1 QPSK 2/3, B 12 64QAM 3/4"). |
| `urgency` | choice | always | 1.0 | An emergency when the emergency alarm start flag is set. |
| `family` | choice | always | 1.0 | The family of the source: a broadcast. |

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
| `service.select` | `sdrkit_isdbtsb_service_select` | `sdrkit_tvterr_service_select_args` | none |
| `track.select` | `sdrkit_isdbtsb_track_select` | `sdrkit_tvterr_track_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `width` | `sdrkit_isdbtsb_get_width` | `sdrkit_isdbtsb_set_width` |
| `segments` | `sdrkit_isdbtsb_get_segments` | `sdrkit_isdbtsb_set_segments` |
| `edge_pilot` | `sdrkit_isdbtsb_get_edge_pilot` | `sdrkit_isdbtsb_set_edge_pilot` |
| `decode_video` | `sdrkit_isdbtsb_get_decode_video` | `sdrkit_isdbtsb_set_decode_video` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mode` | `sdrkit_isdbtsb_gen_get_mode` | `sdrkit_isdbtsb_gen_set_mode` |
| `gen_guard` | `sdrkit_isdbtsb_gen_get_guard` | `sdrkit_isdbtsb_gen_set_guard` |
| `gen_modulation` | `sdrkit_isdbtsb_gen_get_modulation` | `sdrkit_isdbtsb_gen_set_modulation` |
| `gen_code_rate` | `sdrkit_isdbtsb_gen_get_code_rate` | `sdrkit_isdbtsb_gen_set_code_rate` |

## Try it

```sh
sdrkit run isdbtsb --source synth:isdbtsb
sdrkit run isdbtsb --source iiod:192.168.2.1 --freq 100
```
<!-- generated: end -->

## Description

ISDB-TSB narrow-band digital sound and data broadcasting (ITU-R BS.1114 System F), 1 or 3 segments; also the one-segment service
of ISDB-T.

ISDB-TSB is ISDB-T cut down to the centre segment or the three centre segments of a 13-segment band. The stack is the ISDB-T
receiver with a narrow band: the FFT is an eighth (one segment) or a fourth (three segments) of the ISDB-T size, and only the
carriers of the narrow band are taken. The TMCC, the layers A and B, the de-interleavers, Viterbi and Reed-Solomon are those of
`isdbt`. The transport stream of all decoded layers goes to the transport stream layer.

With one segment the stack also takes the one-segment partial-reception service of a wider signal: the TMCC must set the
partial-reception flag with a layer A of one segment, and only layer A is decoded. This works for a 3-segment ISDB-TSB signal
and for a 13-segment ISDB-T signal.

With `segments=auto` the stack starts with one segment. When the TMCC of an ISDB-TSB signal (system identification 01) has
three segments and the input rate carries them, it changes over to three segments once. When three segments are not found at
any place of the channel, it goes back to one.

Not decoded: the AC bits and the phase-shift field of connected segment transmission. A channel of a connected transmission is
taken alone.

Standard: ITU-R BS.1114 Annex 3 (System F); ARIB STD-B31 for the parts ISDB-T shares.

## Tuning: notes

Tune to the centre of the narrow band. An input rate carries a band when it is at least 1.05 times its occupied width:

| Raster | Segment width | 1 segment | 3 segments |
|---|---|---|---|
| 6 MHz | 428.571 kHz | 432.540 kHz | 1289.683 kHz |
| 7 MHz | 500.000 kHz | 504.630 kHz | 1504.630 kHz |
| 8 MHz | 571.429 kHz | 576.720 kHz | 1719.577 kHz |

## Settings: notes

| Key | Label | Kind | Default | Range or options |
|---|---|---|---|---|
| `width` | Channel width | choice | `6` | `6`, `7`, `8`, `auto` (the segment width is a fourteenth of the raster) |
| `segments` | Segments | choice | `auto` | `auto`, `1`, `3` |

Advanced settings. The stack works without any of them set.

| Key | Label | Kind | Default | Range or options |
|---|---|---|---|---|
| `edge_pilot` | Upper-edge pilot | choice | `sequence` | `sequence` (the pilot sequence's value at that carrier), `plus` (+4/3), `minus` (-4/3) |
| `gen_mode` | Test signal mode | choice | `3` | `1`, `2`, `3` |
| `gen_guard` | Test signal guard | choice | `1/8` | `1/4`, `1/8`, `1/16`, `1/32` |
| `gen_modulation` | Test signal layer B modulation | choice | `64qam` | `qpsk`, `16qam`, `64qam` |
| `gen_code_rate` | Test signal layer B code rate | choice | `3/4` | `1/2`, `2/3`, `3/4`, `5/6`, `7/8` |

## Results

The result fields, the layer table, the plots and the TMCC event of [ISDB-T](isdbt.md), and:

| Key | Label | Kind |
|---|---|---|
| `band_segments` | Segments in use | whole number |
| `note` | Note | text |

`system_id` is 0 for ISDB-T and 1 for ISDB-TSB. `partial_reception` is the partial-reception flag of the TMCC.

## Values that no public document confirms

ARIB STD-B29, which defines ISDB-TSB in full, was not available. These values follow the ISDB-T segments at the same places, and
the `note` field names them:

- The segment numbers of the 3-segment signal: 1, 0, 2 from low to high.
- The value of the continual pilot above the top segment: a setting (`edge_pilot`), by default the value the pilot sequence has
  at that carrier.
- The use of the partial-reception flag for three segments.
- The TMCC code of the fifth time-interleave length (I = 32, 16, 8 in modes 1, 2, 3). The stack refuses a TMCC that uses it.

## Test signal

Three segments, mode 3, guard 1/8, partial reception: layer A one segment of QPSK 2/3 (interleave code 2) with null packets,
layer B two segments of 64-QAM 3/4 (interleave code 1) with a test programme at 90 % of its rate. The TMCC sends system
identification 01 and no next configuration.

## Try it: notes

```sh
sdrkit run isdbtsb --source synth:isdbtsb
sdrkit run isdbtsb --source iiod:192.168.2.1 --freq 100
sdrkit run isdbtsb segments=1 --source iiod:192.168.2.1 --freq 473.142857
```
