<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DVB-C

Cable television (DVB-C, J.83 annex A and C), 16- to 256-QAM: services, guide, teletext and subtitles.

Stack id: `dvbc`. Group: TV.

Signal types: DVB-C.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 346 MHz |
| Tuning range | 47 MHz to 1.002 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 7 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dvbc` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 9.5 MHz | 0: measured from the signal. A manual rate that does not lock gives way to the measured one. |
| `qam` | QAM | choice | `auto` | `auto` (Automatic), `16` (16-QAM), `32` (32-QAM), `64` (64-QAM), `128` (128-QAM), `256` (256-QAM) |  |
| `annex_c` | Annex C | yes or no | false |  | ITU-T J.83 annex C (Japan, 6 MHz channels): roll-off 0.13 instead of 0.15. |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_qam` | QAM | choice | `64` | `16` (16-QAM), `32` (32-QAM), `64` (64-QAM), `128` (128-QAM), `256` (256-QAM) |  |
| `gen_annex_c` | Annex C | yes or no | false |  |  |
| `gen_symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 9.5 MHz | 0: 6.952 Msym/s (annex A) or 5.274 Msym/s (annex C). |
| `gen_payload` | Payload | choice | `programme` | `programme` (Test multiplex), `numbered` (Numbered packets) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `stage` | Stage | choice |
| `note` | Note | choice |
| `qam` | QAM | choice |
| `symbol_rate_hz` | Symbol rate | frequency |
| `measured_rate` | Measured symbol rate | frequency |
| `manual_rate` | Manual symbol rate | yes or no |
| `width` | Width (-3 dB) | frequency |
| `annex` | Annex | choice |
| `roll_off` | Roll-off | number |
| `inverted` | Spectrum inverted | yes or no |
| `clock_ppm` | Clock error | number (ppm) |
| `offset_hz` | Offset | frequency |
| `mer_db` | MER | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `pre_rs` | Byte errors before RS | number |
| `rs_clean` | RS words clean | whole number |
| `rs_corrected` | RS words corrected | whole number |
| `rs_failed` | RS words failed | whole number |
| `realigned` | Byte re-alignments | whole number |
| `net_bitrate_bps` | Net bit rate | number (bit/s) |
| `spectrum_lock` | Spectrum lock | yes or no |
| `timing_lock` | Timing lock | yes or no |
| `equaliser_lock` | Equaliser lock | yes or no |
| `carrier_lock` | Carrier lock | yes or no |
| `sync_lock` | Sync lock | yes or no |
| `ts_lock` | TS lock | yes or no |
| `searches` | Searches | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `packets` | Packets | whole number |
| `flagged` | Uncorrectable packets | whole number |
| `scrambled` | Scrambled packets | whole number |
| `numbered_good` | Test packets good | whole number |
| `numbered_wrong` | Test packets wrong | whole number |
| `numbered_gaps` | Test packet gaps | whole number |
| `bitrate_bps` | Bit rate | number (bit/s) |
| `pat_seen` | PAT seen | yes or no |
| `services` | Services | text |
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
| `teletext.data_units` | Data units | whole number |
| `teletext.pages` | Pages | whole number |
| `teletext.page` | Page | whole number |
| `teletext.page_text` | Page text | text |
| `dvb_subtitle.display_sets` | Display sets | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `services` | Services | `service_id` (whole number), `name` (text), `provider` (text), `service_type` (whole number), `kind` (choice), `lcn` (whole number), `visible` (yes or no), `scrambled` (yes or no), `ca_state` (choice), `pmt_pid` (whole number), `pcr_pid` (whole number), `running_status` (whole number), `in_pat` (yes or no), `eit_schedule` (yes or no), `eit_present_following` (yes or no), `onid` (whole number), `tsid` (whole number), `now_title` (text), `now_text` (text), `now_start` (time), `now_duration` (duration (s)), `next_title` (text), `next_text` (text), `next_start` (time), `next_duration` (duration (s)) |
| `tracks` | Tracks | `service` (text), `pid` (whole number), `stream_type` (whole number), `kind` (choice), `codec` (choice), `packaging` (choice), `fourcc` (text), `lang` (text), `audio_type` (whole number), `audio_description` (yes or no), `hard_of_hearing` (yes or no), `component_tag` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no), `profile` (text), `level` (text), `width` (whole number (px)), `height` (whole number (px)), `frame_rate` (number (Hz)), `aspect` (text), `interlaced` (yes or no), `sample_rate` (frequency), `channels` (whole number), `layout` (text), `bitrate_kbps` (whole number (kbit/s)), `object_type` (text), `entry_type` (whole number), `composition_page` (whole number), `ancillary_page` (whole number), `magazine` (whole number), `teletext_page` (whole number) |
| `guide` | Guide | `service` (text), `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `running_status` (whole number), `title` (text), `text` (text), `extended` (text), `lang` (text), `genre` (whole number), `content` (bytes), `min_age` (whole number), `crid_programme` (text), `crid_series` (text), `free_ca` (yes or no), `other_ts` (yes or no), `schedule` (yes or no) |
| `guide_ratings` | Guide ratings | `event` (text), `country` (text), `rating` (whole number) |
| `teletext_pages` | Teletext pages | `page` (whole number), `subpages` (whole number) |
| `teletext_subpages` | Teletext subpages | `page_row` (text), `subcode` (whole number), `version` (whole number) |
| `ts_demux.pids` | PIDs | `pid` (whole number), `label` (text), `packets` (whole number), `cc_errors` (whole number), `tei` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no) |
| `psi_si.sections` | Sections | `accepted` (whole number), `crc` (whole number), `length` (whole number), `other` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `spectrum` | Spectrum | bin | dB |
| `equaliser` | Equaliser taps | us | dB |
| `mer_history` | MER history | report | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `service.list` | Service list | `tsid` (whole number), `onid` (whole number), `network_id` (whole number), `network_name` (text), `service_count` (whole number) |
| `epg.event` | Guide event | `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `title` (text) |
| `ts.time` | Stream time | `utc` (time), `local_offset_min` (whole number (min)) |
| `ts.network` | Network | `network_id` (whole number), `network_name` (text), `onid` (whole number), `tsid` (whole number) |
| `teletext.page` | Teletext page | `page` (whole number), `subcode` (whole number), `version` (whole number), `text` (text) |
| `dvb.subtitle` | Subtitle | `pts` (whole number), `timeout_s` (duration (s)), `width` (whole number (px)), `height` (whole number (px)), `seq` (whole number), `region_count` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets every count. |

## Typed interface

The stack is in the family `dvbcs`, version 2.0. The C header `sdrkit/dvbcs.h`, the registry record `registry/dvbcs.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Television and video (`television`). Media: sound (programme), video.

### Result fields

Struct `DvbcFields`. The result fields of `dvbc`: the carrier, the demodulator, the outer decoder and the locks. A measurement of the carrier has no value before a carrier is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `stage` | choice | always | 1.0 | What the demodulator is doing. |
| `note` | choice | optional | 1.0 | The reason the demodulator does not lock, when known. |
| `qam` | choice | optional | 1.0 | The QAM order in use. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate in use. |
| `measured_rate` | frequency | optional | 1.0 | The symbol rate measured from the spectrum. |
| `manual_rate` | yes or no | always | 1.0 | The manual symbol rate is in use. |
| `width` | frequency | optional | 1.0 | The -3 dB width of the carrier. |
| `annex` | choice | always | 1.0 | The annex of J.83. |
| `roll_off` | number | always | 1.0 | The roll-off of the annex. |
| `inverted` | yes or no | always | 1.0 | The spectrum is inverted. |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the symbol clock. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset from the centre. |
| `mer_db` | number (dB) | optional | 1.0 | The modulation error ratio. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `pre_rs` | number | always | 1.0 | The fraction of bytes Reed-Solomon changed. |
| `rs_clean` | whole number | always | 1.0 | Reed-Solomon words with no error. |
| `rs_corrected` | whole number | always | 1.0 | Reed-Solomon words corrected. |
| `rs_failed` | whole number | always | 1.0 | Reed-Solomon words that failed. |
| `realigned` | whole number | always | 1.0 | Byte re-alignments without a loss of lock. |
| `net_bitrate_bps` | number (bit/s) | optional | 1.0 | The net bit rate. No value while the receiver is not locked. |
| `spectrum_lock` | yes or no | always | 1.0 | The carrier is found in the spectrum. |
| `timing_lock` | yes or no | always | 1.0 | The symbol timing holds. |
| `equaliser_lock` | yes or no | always | 1.0 | The equaliser has converged. |
| `carrier_lock` | yes or no | always | 1.0 | The carrier loop holds. |
| `sync_lock` | yes or no | always | 1.0 | The sync bytes are found. |
| `ts_lock` | yes or no | always | 1.0 | The transport stream is valid. |
| `searches` | whole number | always | 1.0 | Acquisitions started again. |
| `frames_ok` | whole number | always | 1.0 | Frames good. |
| `frames_bad` | whole number | always | 1.0 | Frames bad. |

### Result fields

Struct `TsMonitorFields`. The result fields of `mpegts.ts_monitor`: the counts of the transport stream, which a stack shows under its own keys.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `packets` | whole number | always | 1.0 | Packets seen. |
| `flagged` | whole number | always | 1.0 | Packets with the transport_error_indicator. |
| `scrambled` | whole number | always | 1.0 | Scrambled packets. |
| `numbered_good` | whole number | always | 1.0 | Numbered packets that match. |
| `numbered_wrong` | whole number | always | 1.0 | Numbered packets that do not match. |
| `numbered_gaps` | whole number | always | 1.0 | Breaks in the numbering. |
| `bitrate_bps` | number (bit/s) | always | 1.0 | Bit rate over the last second. |
| `pat_seen` | yes or no | always | 1.0 | A PAT was seen. |
| `services` | text | always | 1.0 | The service names of the SDT, one per line. |

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

### Table `teletext_pages`

Struct `TeletextPageRow`. A teletext page held. The row key is the page number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `page` | whole number | always | 1.0 | The page number. |
| `subpages` | whole number | always | 1.0 | The number of subpages held. |

### Table `teletext_subpages`

Struct `TeletextSubpageRow`. A subpage of a teletext page. The row key is `<page>/<subcode>`, the subcode as four hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `page_row` | text | always | 1.0 | The row key of the page in `teletext_pages`. |
| `subcode` | whole number | always | 1.0 | The subcode, in nibble form. |
| `version` | whole number | always | 1.0 | The version, raised on each change. |

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

### Event `teletext.page`

Struct `TeletextPageEvent`. A teletext page that arrived.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `page` | whole number | always | 1.0 | The page number. |
| `subcode` | whole number | always | 1.0 | The subcode, in nibble form. |
| `version` | whole number | always | 1.0 | The version, raised on each change. |
| `text` | text | always | 1.0 | The text of the page, its 25 rows joined with line feeds. |

### Event `dvb.subtitle`

Struct `DvbSubtitleEvent`. The fields of a subtitle display.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pts` | whole number | optional | 1.0 | The presentation time stamp, 90 kHz. No value for a display without one. |
| `timeout_s` | duration (s) | always | 1.0 | How long the display shows at most. |
| `width` | whole number (px) | always | 1.0 | The width of the display. |
| `height` | whole number (px) | always | 1.0 | The height of the display. |
| `seq` | whole number | always | 1.0 | The display sequence number. |
| `region_count` | whole number | always | 1.0 | The number of regions (0 clears the screen). |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dvbc_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_dvbc_get_symbol_rate` | `sdrkit_dvbc_set_symbol_rate` |
| `qam` | `sdrkit_dvbc_get_qam` | `sdrkit_dvbc_set_qam` |
| `annex_c` | `sdrkit_dvbc_get_annex_c` | `sdrkit_dvbc_set_annex_c` |
| `decode_video` | `sdrkit_dvbc_get_decode_video` | `sdrkit_dvbc_set_decode_video` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_qam` | `sdrkit_dvbc_gen_get_qam` | `sdrkit_dvbc_gen_set_qam` |
| `gen_annex_c` | `sdrkit_dvbc_gen_get_annex_c` | `sdrkit_dvbc_gen_set_annex_c` |
| `gen_symbol_rate` | `sdrkit_dvbc_gen_get_symbol_rate` | `sdrkit_dvbc_gen_set_symbol_rate` |
| `gen_payload` | `sdrkit_dvbc_gen_get_payload` | `sdrkit_dvbc_gen_set_payload` |

## Try it

```sh
sdrkit run dvbc --source synth:dvbc
sdrkit run dvbc --source iiod:192.168.2.1 --freq 346
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
