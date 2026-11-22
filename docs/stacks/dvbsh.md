<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DVB-SH

DVB-SH mobile TV to handhelds (ETSI EN 302 583): the OFDM form (1K to 8K, QPSK and 16-QAM, 1.7 to 8 MHz) and the TDM form (QPSK, 8PSK, 16APSK), turbo coded with a long time interleaver.

Stack id: `dvbsh`. Group: TV.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.1775 GHz |
| Tuning range | 400 MHz to 3 GHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 1.828571 MHz |
| Signal bandwidth | 5 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dvbsh` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `low_priority` | Low-priority stream | yes or no | false |  |  |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `width` | Channel width | choice | `auto` | `auto` (Automatic), `5` (5 MHz), `8` (8 MHz), `7` (7 MHz), `6` (6 MHz), `1.7` (1.7 MHz) |  |
| `max_interleaver_mb` | Time de-interleaver memory limit | number (MB) | 768 | 1 to 65536 |  |
| `turbo_iterations` | Turbo iterations | whole number | 8 | 1 to 32 |  |
| `waveform` | Waveform | choice | `auto` | `auto` (Automatic), `ofdm` (OFDM), `tdm` (TDM) |  |
| `tdm_gold` | TDM scrambling code number | whole number | 0 | 0 to 262141 |  |
| `tdm_symbol_rate` | TDM symbol rate | number (sym/s) | 0 | 0 to 8000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_width` | Test signal width | choice | `5` | `auto` (Automatic), `5` (5 MHz), `8` (8 MHz), `7` (7 MHz), `6` (6 MHz), `1.7` (1.7 MHz) |  |
| `gen_mode` | Test signal FFT size | choice | `2k` | `1k` (1K), `2k` (2K), `4k` (4K), `8k` (8K) |  |
| `gen_constellation` | Test signal constellation | choice | `qpsk` | `qpsk` (QPSK), `16qam` (16-QAM) |  |
| `gen_hierarchy` | Test signal hierarchy | choice | `none` | `none` (None), `1` (α = 1), `2` (α = 2), `4` (α = 4) |  |
| `gen_guard` | Test signal guard interval | choice | `1/8` | `1/32`, `1/16`, `1/8`, `1/4` |  |
| `gen_rate` | Test signal code rate | choice | `1/3` | `1/5`, `2/9`, `1/4`, `2/7`, `1/3`, `1/3c` (1/3 complementary), `2/5`, `2/5c` (2/5 complementary), `1/2`, `1/2c` (1/2 complementary), `2/3`, `2/3c` (2/3 complementary) |  |
| `gen_rate_lp` | Test signal LP code rate | choice | `1/2c` | `1/5`, `2/9`, `1/4`, `2/7`, `1/3`, `1/3c` (1/3 complementary), `2/5`, `2/5c` (2/5 complementary), `1/2`, `1/2c` (1/2 complementary), `2/3`, `2/3c` (2/3 complementary) |  |
| `gen_ti_cm` | Test signal common_multiplier | whole number | 4 | 1 to 63 |  |
| `gen_ti_lt` | Test signal nof_late_taps | whole number | 48 | 0 to 48 |  |
| `gen_ti_ns` | Test signal nof_slices | whole number | 1 | 1 to 63 |  |
| `gen_ti_sd` | Test signal slice_distance | whole number | 0 | 0 to 255 |  |
| `gen_ti_nli` | Test signal non_late_increment | whole number | 0 | 0 to 63 |  |
| `gen_waveform` | Test signal waveform | choice | `ofdm` | `ofdm` (OFDM), `tdm_qpsk` (TDM QPSK), `tdm_8psk` (TDM 8PSK), `tdm_16apsk` (TDM 16APSK) |  |
| `gen_rolloff` | Test signal TDM roll-off | choice | `0.25` | `0.15`, `0.25`, `0.35` |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `waveform` | Waveform | choice |
| `width_mhz` | Channel width | number (MHz) |
| `lock` | Lock | whole number |
| `cfo_hz` | Carrier offset | frequency |
| `fft_size` | FFT size | whole number |
| `guard_samples` | Guard samples | whole number |
| `mode` | Mode | text |
| `guard` | Guard interval | text |
| `constellation` | Constellation | text |
| `hierarchy` | Hierarchy | text |
| `rate_hp` | HP code rate | text |
| `rate_lp` | LP code rate | text |
| `stream` | Stream decoded | text |
| `interleaver` | Time interleaver | text |
| `interleaver_s` | Time interleaver length | duration (s) |
| `interleaver_mb` | Time de-interleaver memory | number (MB) |
| `refused` | Data path refused | text |
| `cell_id` | Cell id | whole number |
| `superframe` | Super-frame in the SH frame | whole number |
| `codewords_ok` | Code words decoded | whole number |
| `codewords_bad` | Code words failed | whole number |
| `packets_ok` | Packets good | whole number |
| `packets_bad` | Packets flagged | whole number |
| `turbo_iterations` | Turbo iterations (mean) | number |
| `first_packet_s` | First packet | duration (s) |
| `note` | Note | text |
| `clock_ppm` | Clock error | number (ppm) |
| `level_dbfs` | Level | number (dBFS) |
| `tdm_modulation` | TDM modulation | text |
| `tdm_symbol_rate` | TDM symbol rate | number (sym/s) |
| `tdm_rolloff` | TDM roll-off | number |
| `tdm_frame_width_cu` | TDM frame width | whole number (CU) |
| `tdm_mer_db` | TDM MER | number (dB) |
| `tdm_slots_erased` | TDM slots erased | whole number |
| `tdm_signalling_failed` | TDM signalling fields failed | whole number |
| `snr_db` | SNR | number (dB) |
| `tps_frames_ok` | TPS frames decoded | whole number |
| `tps_frames_failed` | TPS frames failed | whole number |
| `sync.timing` | Timing error | number (samples) |
| `sync.rho` | Correlation | number |
| `sync.frac` | Fractional offset | number (carriers) |
| `fft.integer_offset` | Integer carrier offset | whole number (carriers) |
| `fft.spectrum_inverted` | Spectrum inverted | yes or no |
| `eq.mer_db` | MER | number (dB) |
| `eq.alpha` | Averaging gain | number |
| `eq.delay_window_centre` | Delay window centre | number (samples) |
| `eq.delay_window_half` | Delay window half width | number (samples) |
| `eq.delay_spread` | Delay spread | number (samples) |
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
| `services` | Services | `service_id` (whole number), `name` (text), `provider` (text), `service_type` (whole number), `kind` (choice), `lcn` (whole number), `visible` (yes or no), `scrambled` (yes or no), `ca_state` (choice), `pmt_pid` (whole number), `pcr_pid` (whole number), `running_status` (whole number), `in_pat` (yes or no), `eit_schedule` (yes or no), `eit_present_following` (yes or no), `onid` (whole number), `tsid` (whole number), `now_title` (text), `now_text` (text), `now_start` (time), `now_duration` (duration (s)), `next_title` (text), `next_text` (text), `next_start` (time), `next_duration` (duration (s)) |
| `tracks` | Tracks | `service` (text), `pid` (whole number), `stream_type` (whole number), `kind` (choice), `codec` (choice), `packaging` (choice), `fourcc` (text), `lang` (text), `audio_type` (whole number), `audio_description` (yes or no), `hard_of_hearing` (yes or no), `component_tag` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no), `profile` (text), `level` (text), `width` (whole number (px)), `height` (whole number (px)), `frame_rate` (number (Hz)), `aspect` (text), `interlaced` (yes or no), `sample_rate` (frequency), `channels` (whole number), `layout` (text), `bitrate_kbps` (whole number (kbit/s)), `object_type` (text), `entry_type` (whole number), `composition_page` (whole number), `ancillary_page` (whole number), `magazine` (whole number), `teletext_page` (whole number) |
| `guide` | Guide | `service` (text), `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `running_status` (whole number), `title` (text), `text` (text), `extended` (text), `lang` (text), `genre` (whole number), `content` (bytes), `min_age` (whole number), `crid_programme` (text), `crid_series` (text), `free_ca` (yes or no), `other_ts` (yes or no), `schedule` (yes or no) |
| `guide_ratings` | Guide ratings | `event` (text), `country` (text), `rating` (whole number) |
| `ts_demux.pids` | PIDs | `pid` (whole number), `label` (text), `packets` (whole number), `cc_errors` (whole number), `tei` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no) |
| `psi_si.sections` | Sections | `accepted` (whole number), `crc` (whole number), `length` (whole number), `other` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `pilots` | Pilots |  |  |
| `tps` | TPS |  |  |
| `channel_db` | Channel magnitude | carrier | dB |
| `channel_phase` | Channel phase | carrier | rad |
| `impulse_db` | Impulse response | sample | dB |

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

The stack is in the family `dvbt`, version 2.6. The C header `sdrkit/dvbt.h`, the registry record `registry/dvbt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Television and video (`television`). Media: sound (programme), video.

### Result fields

Struct `DvbshFields`. The result fields of `dvbsh`, which `data` writes: the OFDM parameters, the SH frame and the turbo code. A value of the TPS has no value while the TPS is not locked.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `waveform` | choice | always | 1.0 | OFDM or TDM. |
| `width_mhz` | number (MHz) | always | 1.0 | The channel width in use. |
| `lock` | whole number | always | 1.0 | 0 searching, 1 OFDM symbols found, 2 TPS, 3 parameters, 4 transport stream flowing. |
| `cfo_hz` | frequency | optional | 1.0 | The carrier offset. No value before the FFT mode is found. |
| `fft_size` | whole number | optional | 1.0 | The FFT size. |
| `guard_samples` | whole number | optional | 1.0 | The guard samples. |
| `mode` | text | optional | 1.0 | The OFDM mode from TPS. |
| `guard` | text | optional | 1.0 | The guard interval from TPS. |
| `constellation` | text | optional | 1.0 | The constellation from TPS. |
| `hierarchy` | text | optional | 1.0 | The hierarchy from TPS. |
| `rate_hp` | text | optional | 1.0 | The code rate of the high-priority stream ("1/3", "1/3c" for complementary). |
| `rate_lp` | text | optional | 1.0 | The code rate of the low-priority stream of a hierarchical multiplex. |
| `stream` | text | always | 1.0 | The stream decoded, HP or LP. |
| `interleaver` | text | optional | 1.0 | The interleaver profile, "cm / lt / ns / sd / nli". |
| `interleaver_s` | duration (s) | optional | 1.0 | The length of the time interleaver. |
| `interleaver_mb` | number (MB) | optional | 1.0 | The memory that the time de-interleaver holds. |
| `refused` | text | optional | 1.0 | Why the data path does not run (an interleaver too long for this receiver, a reserved pattern). No value while it runs. |
| `cell_id` | whole number | optional | 1.0 | The 16-bit cell id from TPS. |
| `superframe` | whole number | optional | 1.0 | The super-frame number in the SH frame. |
| `codewords_ok` | whole number | always | 1.0 | Code words decoded. |
| `codewords_bad` | whole number | always | 1.0 | Code words not decoded. |
| `packets_ok` | whole number | always | 1.0 | Packets with a good CRC. |
| `packets_bad` | whole number | always | 1.0 | Packets passed with the transport_error_indicator. |
| `turbo_iterations` | number | optional | 1.0 | The mean turbo iterations of the decoded code words. No value before a code word decodes. |
| `first_packet_s` | duration (s) | optional | 1.0 | The signal time of the first good packet. |
| `note` | text | always | 1.0 | What the stack decodes and which of its values no public document confirms. |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the sample clock. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input. No value before it is measured. |
| `tdm_modulation` | text | optional | 2.5 | The modulation of the TDM form, which the SOF shows. |
| `tdm_symbol_rate` | number (sym/s) | optional | 2.5 | The symbol rate of the TDM form that is tried or received. |
| `tdm_rolloff` | number | optional | 2.5 | The roll-off of the TDM form, from the row of the symbol rate. No value before slots are found. |
| `tdm_frame_width_cu` | whole number (CU) | optional | 2.5 | The width of the TDM SH frame from its signalling field. |
| `tdm_mer_db` | number (dB) | optional | 2.5 | The power of a pilot symbol of the TDM form over the noise in the symbol rate. |
| `tdm_slots_erased` | whole number | always | 2.5 | Slots of the TDM form erased for want of pilots. |
| `tdm_signalling_failed` | whole number | always | 2.5 | Signalling fields of the TDM form that did not decode. |

### Result fields

Struct `DvbshChainFields`. The fields of the OFDM chain that `dvbsh` shows under its own keys: the SNR of the equaliser and the counts of the TPS decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `snr_db` | number (dB) | optional | 1.0 | The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled. |
| `tps_frames_ok` | whole number | always | 1.0 | TPS frames decoded. |
| `tps_frames_failed` | whole number | always | 1.0 | TPS frames that failed. |

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
| `service.select` | `sdrkit_dvbsh_service_select` | `sdrkit_dvbt_service_select_args` | none |
| `track.select` | `sdrkit_dvbsh_track_select` | `sdrkit_dvbt_track_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `width` | `sdrkit_dvbsh_get_width` | `sdrkit_dvbsh_set_width` |
| `low_priority` | `sdrkit_dvbsh_get_low_priority` | `sdrkit_dvbsh_set_low_priority` |
| `max_interleaver_mb` | `sdrkit_dvbsh_get_max_interleaver_mb` | `sdrkit_dvbsh_set_max_interleaver_mb` |
| `turbo_iterations` | `sdrkit_dvbsh_get_turbo_iterations` | `sdrkit_dvbsh_set_turbo_iterations` |
| `decode_video` | `sdrkit_dvbsh_get_decode_video` | `sdrkit_dvbsh_set_decode_video` |
| `waveform` | `sdrkit_dvbsh_get_waveform` | `sdrkit_dvbsh_set_waveform` |
| `tdm_gold` | `sdrkit_dvbsh_get_tdm_gold` | `sdrkit_dvbsh_set_tdm_gold` |
| `tdm_symbol_rate` | `sdrkit_dvbsh_get_tdm_symbol_rate` | `sdrkit_dvbsh_set_tdm_symbol_rate` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_width` | `sdrkit_dvbsh_gen_get_width` | `sdrkit_dvbsh_gen_set_width` |
| `gen_mode` | `sdrkit_dvbsh_gen_get_mode` | `sdrkit_dvbsh_gen_set_mode` |
| `gen_constellation` | `sdrkit_dvbsh_gen_get_constellation` | `sdrkit_dvbsh_gen_set_constellation` |
| `gen_hierarchy` | `sdrkit_dvbsh_gen_get_hierarchy` | `sdrkit_dvbsh_gen_set_hierarchy` |
| `gen_guard` | `sdrkit_dvbsh_gen_get_guard` | `sdrkit_dvbsh_gen_set_guard` |
| `gen_rate` | `sdrkit_dvbsh_gen_get_rate` | `sdrkit_dvbsh_gen_set_rate` |
| `gen_rate_lp` | `sdrkit_dvbsh_gen_get_rate_lp` | `sdrkit_dvbsh_gen_set_rate_lp` |
| `gen_ti_cm` | `sdrkit_dvbsh_gen_get_ti_cm` | `sdrkit_dvbsh_gen_set_ti_cm` |
| `gen_ti_lt` | `sdrkit_dvbsh_gen_get_ti_lt` | `sdrkit_dvbsh_gen_set_ti_lt` |
| `gen_ti_ns` | `sdrkit_dvbsh_gen_get_ti_ns` | `sdrkit_dvbsh_gen_set_ti_ns` |
| `gen_ti_sd` | `sdrkit_dvbsh_gen_get_ti_sd` | `sdrkit_dvbsh_gen_set_ti_sd` |
| `gen_ti_nli` | `sdrkit_dvbsh_gen_get_ti_nli` | `sdrkit_dvbsh_gen_set_ti_nli` |
| `gen_waveform` | `sdrkit_dvbsh_gen_get_waveform` | `sdrkit_dvbsh_gen_set_waveform` |
| `gen_rolloff` | `sdrkit_dvbsh_gen_get_rolloff` | `sdrkit_dvbsh_gen_set_rolloff` |

## Try it

```sh
sdrkit run dvbsh --source synth:dvbsh
sdrkit run dvbsh --source iiod:192.168.2.1 --freq 2177.5
```
<!-- generated: end -->

## Description

DVB-SH mobile TV to handhelds (ETSI EN 302 583): the OFDM form (1K to 8K, QPSK and 16-QAM, 1.7 to 8 MHz) and the TDM form
(QPSK, 8PSK, 16APSK), turbo coded with a long time interleaver.

DVB-SH brings TV to handhelds from a satellite in S band and from terrestrial repeaters. Its OFDM form SH-A is DVB-T with two more
FFT sizes (1K and 4K), QPSK and 16-QAM only, and a TPS that carries the code rates and the time interleaver. The stack runs the
DVB-T synchroniser, FFT, TPS decoder and equaliser in their DVB-SH reading, then the common channel coding of DVB-SH:

1. The cells of the chosen stream are de-interleaved and demapped into soft bits, stored in 8 bits.
2. The time de-interleaver has 48 branches whose delays come from the five interleaver parameters of TPS. A profile can spread a
   code word over many seconds, so that it survives a blockage of the satellite path.
3. Each code word goes through the bit-wise de-interleaver and the 3GPP2 turbo decoder (one of twelve puncturing patterns,
   rates 1/5 to 2/3), which stops as soon as the CRCs of the header and of all eight packets hold.
4. The packets go to the transport stream layer. A packet whose CRC fails is passed with the transport_error_indicator.

The SH frame always starts at an OFDM frame. TPS tells where: the frame number, and for SH frames longer than a super-frame the
super-frame number. When the synchroniser loses the signal for a moment, the data path fills the missing symbols with erasures
and keeps the de-interleaver in step, up to 2 s.

The TDM form SH-B is a single carrier on the satellite path with the same channel coding. The stack receives it on a chain
of its own, up to the code words:

1. A root raised cosine matched filter and a Gardner timing loop give one sample per symbol, at one symbol rate of table 5.13
   after another or at the rate of the setting `tdm_symbol_rate`.
2. The symbols come in slots of 2176: a pilot field of 80 symbols, 1008 data symbols, a second pilot field and 1008 data
   symbols, all scrambled with the sequence of EN 302 307 from the start of each slot. The slot search correlates the pilot
   fields of 8 slots in blocks of 20 symbols, which finds a carrier up to 1/40 of the symbol rate away. A larger part of the
   offset is taken off before the matched filter.
3. The phase of the carrier runs in a line from the middle of one pilot field to the middle of the next. The phase slope
   inside the fields tells a carrier that is off by a multiple of one turn between two fields. The pilot fields also give
   the level and the noise (the result `tdm_mer_db`).
4. The SOF (288 bits at the start of a frame, in the modulation of the frame) gives the frame start and the modulation. The
   signalling field after it (1146 bits, turbo rate 1/5) gives the frame width, the puncturing pattern and the time
   interleaver.
5. The data symbols are demapped (max-log) into the same soft bits as SH-A. The DATA part of the frame goes through the time
   de-interleaver; the header and the padding do not.
6. The code words go to the same turbo decoder and transport stream output as SH-A.

A slot whose pilot fields hold too little coherent energy is erased, and the slot and frame grids run on for up to 2 s, so
that the de-interleaver stays in step through a blockage. After that the search starts again.

The setting `waveform` chooses the form. `auto` gives the OFDM form its turn on every usable width (0.3 s each without OFDM
symbols), then the TDM form 1.3 s per width, and so on. The two forms are told apart by what each search finds: the guard
interval correlation of OFDM, or the pilot fields of the TDM slots. One form runs at a time.

Not decoded: a TDM symbol rate of unequal TDM and OFDM bandwidths unless the rate is set, the low-latency extension (annex B;
the signalling field says if it is present), the SH-IP packet as such (annex A; it passes as a transport packet), and the
combining of TDM and OFDM copies of the same code word or of two receive paths. One stream of hierarchical 16-QAM is decoded
at a time.

Standard: ETSI EN 302 583 V1.2.1; ETSI TS 102 584 clause 7.2.3.3 for the branch delays of the time interleaver; ETSI EN 300 744
for the parts DVB-T shares.

## Tuning: notes

Tune to the centre of the channel. The S-band raster (2170 to 2200 MHz in 5 MHz channels) is the data file
`data/plans/dvbsh.json`; the stack holds no channel list. A width is tried only when its OFDM rate is at most 1.02 times the
input rate:

| Width | OFDM rate | FFT sizes |
|---|---|---|
| 8 MHz | 64/7 MHz | 1K, 2K, 4K, 8K |
| 7 MHz | 8 MHz | 1K, 2K, 4K, 8K |
| 6 MHz | 48/7 MHz | 1K, 2K, 4K, 8K |
| 5 MHz | 40/7 MHz | 1K, 2K, 4K, 8K |
| 1.7 MHz | 64/35 MHz | 1K |

The TDM symbol rates are those of table 5.13 (TDM and OFDM of the same bandwidth): 12 per width, one for each guard interval
of the OFDM form and each roll-off, from 34/5 Msym/s (8 MHz, guard 1/4, roll-off 0.15) to 64/55 Msym/s (1.7 MHz, guard 1/32,
roll-off 0.35). A rate is tried when the signal fits the input: symbol rate times (1 + roll-off) at most 1.02 times the input
rate. The roll-off is that of the row of the rate.

## Settings: notes

| Key | Label | Kind | Default | Range or options |
|---|---|---|---|---|
| `width` | Channel width | choice | `auto` | `auto` (5, 8, 7, 6, 1.7 MHz in turn), `5`, `8`, `7`, `6`, `1.7` |
| `low_priority` | Low-priority stream | bool | false | decode the LP stream of hierarchical 16-QAM |
| `max_interleaver_mb` | Time de-interleaver memory limit | number, MB | 768 | 1 to 65 536 |
| `turbo_iterations` | Turbo iterations | whole number | 8 | 1 to 32 |
| `waveform` | Waveform | choice | `auto` | `auto` (OFDM on every width, then TDM, in turn), `ofdm`, `tdm` |
| `tdm_symbol_rate` | TDM symbol rate | number, sym/s | 0 | 0 (the rates of table 5.13 in turn) to 8 000 000 |
| `tdm_gold` | TDM scrambling code number | whole number | 0 | 0 to 262 141 |

The test signal (`gen_*`):

| Key | Label | Default | Options |
|---|---|---|---|
| `gen_width` | Test signal width | `5` | `5`, `8`, `7`, `6`, `1.7` (1.7 MHz forces 1K) |
| `gen_mode` | Test signal FFT size | `2k` | `1k`, `2k`, `4k`, `8k` |
| `gen_constellation` | Test signal constellation | `qpsk` | `qpsk`, `16qam` |
| `gen_hierarchy` | Test signal hierarchy | `none` | `none`, `1`, `2`, `4` (16-QAM only) |
| `gen_guard` | Test signal guard interval | `1/8` | `1/32`, `1/16`, `1/8`, `1/4` |
| `gen_rate` | Test signal code rate | `1/3` | `1/5`, `2/9`, `1/4`, `2/7`, `1/3`, `1/3c`, `2/5`, `2/5c`, `1/2`, `1/2c`, `2/3`, `2/3c` |
| `gen_rate_lp` | Test signal LP code rate | `1/2c` | as `gen_rate` |
| `gen_ti_cm`, `gen_ti_lt`, `gen_ti_ns`, `gen_ti_sd`, `gen_ti_nli` | The interleaver profile | 4, 48, 1, 0, 0 | 1-63, 0-48, 1-63, 0-255, 0-63 |
| `gen_waveform` | Test signal waveform | `ofdm` | `ofdm`, `tdm_qpsk`, `tdm_8psk`, `tdm_16apsk` |
| `gen_rolloff` | Test signal TDM roll-off | `0.25` | `0.15`, `0.25`, `0.35` |

## Results

The TPS and equaliser results and plots of [DVB-T](dvbt.md), the tables and events of the transport stream layer, and:

| Key | Label | Kind |
|---|---|---|
| `waveform` | Waveform (`ofdm`, `tdm`) | choice |
| `width_mhz` | Channel width | number, MHz |
| `lock` | Lock | whole number: 0 searching, 1 OFDM symbols or TDM slots found, 2 TPS or SOF, 3 parameters or signalling field, 4 transport stream flowing |
| `cfo_hz` | Carrier offset | Hz |
| `fft_size`, `guard_samples` | FFT size, guard samples | whole number |
| `mode`, `guard`, `constellation`, `hierarchy` | from TPS | text |
| `rate_hp`, `rate_lp` | Code rates ("1/3", "1/3c" for complementary) | text |
| `stream` | Stream decoded | text |
| `interleaver` | Time interleaver, "cm / lt / ns / sd / nli" | text |
| `interleaver_s` | Time interleaver length (48 L_max units of 126 bits at the stream's bit rate) | duration |
| `interleaver_mb` | Time de-interleaver memory | number, MB |
| `refused` | Why the data path does not run (interleaver too long, reserved pattern) | text |
| `cell_id` | Cell id | whole number |
| `superframe` | Super-frame in the SH frame | whole number |
| `codewords_ok`, `codewords_bad` | Code words decoded, failed | whole number |
| `packets_ok`, `packets_bad` | Packets good, flagged | whole number |
| `turbo_iterations` | Turbo iterations (mean) | number |
| `first_packet_s` | First packet | duration |
| `note` | Note | text |
| `tdm_modulation` | TDM modulation, from the SOF | text |
| `tdm_symbol_rate` | TDM symbol rate tried or received | number, sym/s |
| `tdm_rolloff` | TDM roll-off, from the row of the symbol rate | number |
| `tdm_frame_width_cu` | TDM frame width, from the signalling field | whole number, CU |
| `tdm_mer_db` | Power of a pilot symbol over the noise in the symbol rate | number, dB |
| `tdm_slots_erased` | Slots erased for want of pilots | whole number |
| `tdm_signalling_failed` | Signalling fields that did not decode | whole number |

For the TDM form `rate_hp`, `interleaver`, `interleaver_s`, `interleaver_mb` and `refused` come from the signalling field,
`cfo_hz` and `clock_ppm` from the pilot fields and the timing loop, and the status shows the MER as its SNR. The fields of
TPS have no value.

## Values that no public document confirms

These values are the specification's choices; the `note` field names them:

- The order of the copies of a repeated tail symbol: sent one after another (the receiver adds their soft values, so the order
  does not change the result).
- The automatic width search: 5, 8, 7, 6, 1.7 MHz, 0.3 s each without OFDM symbols, 3 s without TPS.
- The soft values in the time de-interleaver: 8 bits in steps of 1/4.
- The most turbo iterations: a setting (`turbo_iterations`, default 8).
- The memory limit of the time de-interleaver: a setting (`max_interleaver_mb`, default 768 MB).
- The data path fills a gap of up to 2 s in the symbols with erasures and keeps the de-interleaver in step; after a longer gap it
  starts again at the next SH frame.
- The SH frame is counted on one clock at the input and the output of the time interleaver: a unit at place `q` of the received
  stream is place `q - 48 L_max` of the transmitted SH frame.

For the TDM form:

- The code words of a TDM frame: the most whole code words that fit after the 3 units of the header (table 5.11 gives the
  number only for OFDM). Only this DATA part passes the time interleaver.
- The unit of the slice offset of the time interleaver: the DATA part in interleaving units over 48 (TS 102 584 gives it in a
  figure).
- The scrambling code number: no field of the signalling carries it. It is a setting (`tdm_gold`, default 0).
- A signalling field whose byte after the version is not 0 is refused.
- The automatic waveform: the OFDM form on every usable width, then the TDM form for 1.3 s per width. The symbol rates in
  turn: 0.1 s without slots, 0.7 s with slots and no frame (longer in the ratio of 5 MHz to a narrower width).
- The thresholds of the receiver: the slot search takes a coherent share of the pilot energy of 0.15 in blocks of 20 symbols
  over 8 slots; a slot is erased below a share of 0.15 over its two fields; the SOF is taken at a correlation of 0.5 and the
  frame is dropped after 3 frames without it; the grids run on for 2 s without pilots.

The turbo decoder is max-log-MAP with an extrinsic scale of 3/4, the decoder that sdrkit shares among its turbo codes.

## Test signal

The OFDM form at the OFDM rate of its width, resampled to the requested rate: numbered transport packets on PID 0x0100 (and on
PID 0x0200 in the LP stream of hierarchical 16-QAM), a null packet in place of every 20th (the payload at 0.95 of the net
rate). The default is 5 MHz, 2K, QPSK, rate 1/3, guard 1/8 and the uniform interleaver 4 / 48 / 1 / 0 / 0. The signal starts
at the first symbol of an SH frame and sends cell id 0.

With `gen_waveform` at `tdm_qpsk`, `tdm_8psk` or `tdm_16apsk` the signal is the TDM form with the same packets: the symbol
rate of table 5.13 for `gen_width`, `gen_guard` and `gen_rolloff` (35/9 Msym/s at the defaults), frames of the width of table
5.12 for that guard, `gen_constellation` and the modulation (784, 1176 or 1568 units at the defaults), `gen_rate` and the
interleaver profile in the signalling field, scrambling code `tdm_gold`. The pulse is shaped at the requested rate, which must
be at least the symbol rate times (1 + roll-off).

## Try it: notes

```sh
sdrkit run dvbsh --source synth:dvbsh
sdrkit run dvbsh gen_mode=8k gen_constellation=16qam gen_width=8 --source synth:dvbsh
sdrkit run dvbsh gen_waveform=tdm_8psk gen_rate=1/2 waveform=tdm --source synth:dvbsh
sdrkit run dvbsh --source iiod:192.168.2.1 --freq 2177.5
```
