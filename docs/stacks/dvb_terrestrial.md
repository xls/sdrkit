<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DVB-T2 / DVB-T

Digital terrestrial TV, DVB-T2 or DVB-T, whichever the multiplex is.

Stack id: `dvb_terrestrial`. Group: TV.

Signal types: DVB-T2, DVB-T.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 47 MHz to 1 GHz |
| Sample rate | 9.142857 MHz |
| Smallest sample rate | 4.9375 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dvb_terrestrial` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bandwidth` | Channel bandwidth | choice | `auto` | `auto` (Auto), `8` (8 MHz), `7` (7 MHz), `6` (6 MHz), `5` (5 MHz) | The width of the channel. Auto measures the width from the spectrum, then tries the other widths when nothing locks. |
| `stream` | Hierarchical stream | choice | `hp` | `hp` (High priority), `lp` (Low priority) | Which stream of a hierarchical multiplex is decoded. A multiplex that is not hierarchical has only one. |
| `viterbi` | Viterbi decoder | choice | `auto` | `auto` (Auto), `exact` (Exact), `fast` (Fast (8-bit)) |  |
| `time_interp` | Time interpolation of pilots | yes or no | true |  | Off uses the newest pilot: lower latency, worse with Doppler. |
| `interferer_erase` | Narrowband interferer weighting | yes or no | true |  |  |
| `standard` | Standard | choice | `auto` | `auto` (Auto (DVB-T2 / DVB-T)), `dvbt` (DVB-T only) |  |
| `channels` | Channels | text | empty |  | The centre frequencies in Hz of the channels in use, separated by ";". At most 256 entries. A radio tuned to none of them asks the host to tune to the lowest. Empty: no list. |
| `audio_lang` | Audio language | text | empty |  |  |
| `audio_description` | Audio description | yes or no | false |  |  |
| `downmix` | Stereo downmix | yes or no | true |  | Off: 5.1 sound leaves with all its channels, in the order that the audio information states. |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |
| `subtitle_lang` | Subtitle language | text | empty |  | Empty: subtitles off. |
| `hard_of_hearing` | Subtitles for the hard of hearing | yes or no | false |  |  |
| `country` | Country | text | empty |  | ISO 3166 alpha-3 code (NOR, AUS), for channel numbers and the local time. |
| `guide_lang` | Guide language | text | empty |  |  |
| `test_flat_csi` | Flat noise weighting | yes or no | false |  |  |
| `test_full_window` | Full delay window | yes or no | false |  |  |
| `test_peak_only` | Window from the correlation peak | yes or no | false |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mode` | Mode | choice | `2K` | `2K`, `8K` |  |
| `gen_guard` | Guard interval | choice | `1/8` | `1/32`, `1/16`, `1/8`, `1/4` |  |
| `gen_constellation` | Constellation | choice | `16-QAM` | `QPSK`, `16-QAM`, `64-QAM` |  |
| `gen_hierarchy` | Hierarchy | choice | `none` | `none` (None), `1` (α = 1), `2` (α = 2), `4` (α = 4) |  |
| `gen_rate_hp` | HP code rate | choice | `2/3` | `1/2`, `2/3`, `3/4`, `5/6`, `7/8` |  |
| `gen_rate_lp` | LP code rate | choice | `2/3` | `1/2`, `2/3`, `3/4`, `5/6`, `7/8` |  |
| `gen_bandwidth` | Bandwidth | choice | `auto` | `auto` (Auto), `8` (8 MHz), `7` (7 MHz), `6` (6 MHz), `5` (5 MHz) |  |
| `gen_cell_id` | Cell id | whole number | 0 | 0 to 65535 |  |
| `gen_length` | TPS length indicator | choice | `23` | `23`, `31` |  |
| `gen_phase` | Puncturing start phase | whole number | 0 | 0 to 7 |  |
| `gen_service` | Service id | whole number | 1 | 1 to 65535 |  |
| `gen_name` | Service name | text | `Test service` |  |  |
| `gen_source` | Packets | choice | `builtin` | `builtin` (Built-in multiplex), `pattern` (Counted test pattern), `null` (Null packets) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `detect_level` | Detection level | whole number |
| `channel_width` | Channel width | frequency |
| `bandwidth_auto` | Bandwidth found automatically | yes or no |
| `bandwidth_from` | Bandwidth from | choice |
| `measured_width` | Measured width | frequency |
| `occupancy_db` | Channel above floor | number (dB) |
| `input_rate` | Input rate | frequency |
| `natural_rate` | Natural rate | frequency |
| `resampling` | Resampling | yes or no |
| `rate_ok` | Input rate enough | yes or no |
| `fft` | FFT mode | choice |
| `fft_size` | FFT size | whole number |
| `guard` | Guard interval | choice |
| `guard_samples` | Guard samples | whole number |
| `carriers` | Carriers | whole number |
| `constellation` | Constellation | choice |
| `hierarchy` | Hierarchy | choice |
| `code_rate_hp` | Code rate (HP) | choice |
| `code_rate_lp` | Code rate (LP) | choice |
| `stream` | Stream | choice |
| `cell_id` | Cell id | whole number |
| `tps_length` | TPS length | whole number |
| `dvbh_flags` | DVB-H flags | whole number |
| `frame` | Frame | whole number |
| `symbol` | Symbol | whole number |
| `frame_duration` | Frame duration | duration (s) |
| `net_bitrate_bps` | Net bit rate | number (bit/s) |
| `offset_hz` | Offset | frequency |
| `integer_offset` | Integer carrier offset | whole number (carriers) |
| `locator_hz` | Channel locator shift | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `spectrum_inverted` | Spectrum inverted | yes or no |
| `snr_db` | SNR | number (dB) |
| `mer_db` | MER | number (dB) |
| `ber_pre_viterbi` | BER before Viterbi | number |
| `ber_post_viterbi` | BER after Viterbi | number |
| `viterbi_margin` | Viterbi margin | number |
| `puncture_phase` | Puncturing phase | whole number |
| `rs_blocks` | RS blocks | whole number |
| `rs_clean` | RS blocks clean | whole number |
| `rs_corrected` | RS blocks corrected | whole number |
| `rs_failed` | RS blocks failed | whole number |
| `rs_bytes_corrected` | RS bytes corrected | whole number |
| `packets` | Packets | whole number |
| `fec_sync` | Packet alignment | yes or no |
| `resyncs` | Resynchronisations | whole number |
| `since_tps` | Since the last good TPS | duration (s) |
| `quality_pct` | Quality | number (%) |
| `quality_label` | Quality | choice |
| `required_cn_db` | Required C/N | number (dB) |
| `margin_db` | SNR margin | number (dB) |
| `unsupported` | Unsupported features | text |
| `data_valid` | Data valid | yes or no |
| `first_packet` | First packet | duration (s) |
| `level_dbfs` | Level | number (dBFS) |
| `network_name` | Network | text |
| `network_id` | Network id | whole number |
| `service_count` | Services | whole number |
| `utc` | UTC | time |
| `selected_service` | Service playing | whole number |
| `player_status` | Player | choice |
| `mux_kbps` | Multiplex bit rate | number (kbit/s) |
| `sync.timing` | Timing error | number (samples) |
| `sync.rho` | Correlation | number |
| `sync.frac` | Fractional offset | number (carriers) |
| `tps.tps_frames_ok` | TPS frames decoded | whole number |
| `tps.tps_frames_failed` | TPS frames failed | whole number |
| `eq.alpha` | Averaging gain | number |
| `eq.delay_window_centre` | Delay window centre | number (samples) |
| `eq.delay_window_half` | Delay window half width | number (samples) |
| `eq.delay_spread` | Delay spread | number (samples) |
| `fec.frames_ok` | Frames good | whole number |
| `fec.frames_bad` | Frames bad | whole number |
| `ts_demux.null_packets` | Null packets | whole number |
| `ts_demux.tei_packets` | Packets with errors | whole number |
| `ts_demux.cc_errors` | Continuity errors | whole number |
| `ts_demux.null_kbps` | Null bit rate | number (kbit/s) |
| `ts_demux.selected_audio` | Audio PID | whole number |
| `ts_demux.selected_subtitle` | Subtitle PID | whole number |
| `ts_demux.audio_decoded` | Sound decoded | yes or no |
| `psi_si.onid` | Original network id | whole number |
| `psi_si.tsid` | Transport stream id | whole number |
| `psi_si.local_offset_min` | Local offset | whole number (min) |
| `teletext.data_units` | Data units | whole number |
| `teletext.pages` | Pages | whole number |
| `teletext.page` | Page | whole number |
| `teletext.page_text` | Page text | text |
| `dvb_subtitle.display_sets` | Display sets | whole number |
| `standard` | Standard | choice |

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
| `teletext.page` | Teletext page | `page` (whole number), `subcode` (whole number), `version` (whole number), `text` (text) |
| `dvb.subtitle` | Subtitle | `pts` (whole number), `timeout_s` (duration (s)), `width` (whole number (px)), `height` (whole number (px)), `seq` (whole number), `region_count` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |
| `teletext.select` | Select a teletext page | `row` (text, required) |  | Shows a teletext page. |
| `track.select` | Select a track | `row` (text, required) |  | Plays an audio track or shows a subtitle track. |

## Typed interface

The stack is in the family `dvbt`, version 2.4. The C header `sdrkit/dvbt.h`, the registry record `registry/dvbt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `DvbtFields`. The result fields of `dvbt`, which `report` writes: the input, the OFDM parameters, the TPS, the error correction and the quality. A value of the TPS has no value while the TPS is not locked; a value of the FFT mode has no value before the mode is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `detect_level` | whole number | always | 1.0 | The detection level, 0 to 3. |
| `channel_width` | frequency | always | 1.0 | The channel width in use. |
| `bandwidth_auto` | yes or no | always | 1.0 | The `bandwidth` setting is Auto. |
| `bandwidth_from` | choice | optional | 1.0 | Where the width in use came from. |
| `measured_width` | frequency | optional | 1.0 | The measured occupied width. |
| `occupancy_db` | number (dB) | optional | 1.0 | The level of the channel above the floor outside it. |
| `input_rate` | frequency | always | 1.0 | The input rate. |
| `natural_rate` | frequency | always | 1.0 | The natural rate of the width in use. |
| `resampling` | yes or no | always | 1.0 | The input is resampled. |
| `rate_ok` | yes or no | always | 1.0 | The input rate is enough for the width. |
| `fft` | choice | optional | 1.0 | The FFT mode. |
| `fft_size` | whole number | optional | 1.0 | The FFT size. |
| `guard` | choice | optional | 1.0 | The guard interval. |
| `guard_samples` | whole number | optional | 1.0 | The guard samples. |
| `carriers` | whole number | optional | 1.0 | The carriers. |
| `constellation` | choice | optional | 1.0 | The constellation. |
| `hierarchy` | choice | optional | 1.0 | The hierarchy. |
| `code_rate_hp` | choice | optional | 1.0 | The HP code rate. |
| `code_rate_lp` | choice | optional | 1.0 | The LP code rate. |
| `stream` | choice | always | 1.0 | The stream decoded. |
| `cell_id` | whole number | optional | 1.0 | The cell id. No value until two frames agree. |
| `tps_length` | whole number | optional | 1.0 | The TPS length indicator. |
| `dvbh_flags` | whole number | optional | 1.0 | The DVB-H signalling bits. |
| `frame` | whole number | optional | 1.0 | The frame number, 0 to 3. |
| `symbol` | whole number | optional | 1.0 | The symbol in the frame, 0 to 67. |
| `frame_duration` | duration (s) | optional | 1.0 | The duration of a frame. |
| `net_bitrate_bps` | number (bit/s) | optional | 1.0 | The net bit rate of the stream decoded. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset from the centre. |
| `integer_offset` | whole number (carriers) | optional | 1.0 | The integer carrier offset. |
| `locator_hz` | frequency | always | 1.0 | The shift of the channel locator. |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the sample clock. |
| `spectrum_inverted` | yes or no | always | 1.0 | The spectrum is mirrored. |
| `snr_db` | number (dB) | optional | 1.0 | The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled. |
| `mer_db` | number (dB) | optional | 1.0 | The modulation error ratio. No value before the channel estimate is filled. |
| `ber_pre_viterbi` | number | always | 1.0 | The BER before Viterbi over the last second: the received hard bits that differ from the decoded bits, coded again. |
| `ber_post_viterbi` | number | always | 1.0 | The BER after Viterbi over the last second: the bits that Reed-Solomon corrected over the bits of the blocks that it decoded. A block that failed is not in it and is counted in `rs_failed`. 0 while no block decoded. |
| `viterbi_margin` | number | always | 1.0 | The Viterbi margin. |
| `puncture_phase` | whole number | optional | 1.0 | The puncturing phase. |
| `rs_blocks` | whole number | always | 1.0 | RS blocks handled: those without error, those corrected and those that failed. |
| `rs_clean` | whole number | always | 1.0 | RS blocks without error. |
| `rs_corrected` | whole number | always | 1.0 | RS blocks with 1 to 8 bytes wrong, which were corrected. Their corrected bits are the BER after Viterbi. |
| `rs_failed` | whole number | always | 1.0 | RS blocks that could not be corrected (more than 8 bytes wrong). Each gives a packet with the transport_error_indicator set. They do not enter the BER after Viterbi. |
| `rs_bytes_corrected` | whole number | always | 1.0 | Bytes that were corrected in the RS blocks corrected. |
| `packets` | whole number | always | 1.0 | Transport packets delivered, with those of the blocks that failed. |
| `fec_sync` | yes or no | always | 1.0 | The packet alignment holds. |
| `resyncs` | whole number | always | 1.0 | Resynchronisations. |
| `since_tps` | duration (s) | optional | 1.0 | The signal time since the last good TPS frame. No value before the first good frame. |
| `quality_pct` | number (%) | always | 1.0 | The quality score, 0 to 100. |
| `quality_label` | choice | always | 1.0 | The quality class. |
| `required_cn_db` | number (dB) | optional | 1.0 | The C/N that the constellation and code rate need. |
| `margin_db` | number (dB) | optional | 1.0 | The SNR above the required C/N. |
| `unsupported` | text | optional | 1.0 | The features that TPS signals and the receiver does not decode. |
| `data_valid` | yes or no | always | 1.0 | The channel estimate is filled. |
| `first_packet` | duration (s) | optional | 1.0 | The signal time of the first good packet. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input. No value before it is measured. |

### Result fields

Struct `DvbtMultiplexFields`. The fields of the transport stream layer that `dvbt` shows under its own keys: the network and the time of the stream (`psi_si`), and the service that plays (`ts_demux`).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `network_name` | text | always | 1.0 | The network name. |
| `network_id` | whole number | always | 1.0 | network_id. |
| `service_count` | whole number | always | 1.0 | The number of services. |
| `utc` | time | optional | 1.0 | The UTC of the stream. No value before the stream gave the time. |
| `selected_service` | whole number | optional | 1.0 | The service that plays. No value while none plays. |
| `player_status` | choice | always | 1.0 | What the player does. |
| `mux_kbps` | number (kbit/s) | always | 1.0 | The bit rate of the multiplex. |

### Result fields

Struct `DvbTerrestrialFields`. The result set that the receiver of `dvb_terrestrial` adds to the results of the DVB-T receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `standard` | choice | always | 1.0 | The standard that decodes now. |

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
| `service.select` | `sdrkit_dvb_terrestrial_service_select` | `sdrkit_dvbt_service_select_args` | none |
| `teletext.select` | `sdrkit_dvb_terrestrial_teletext_select` | `sdrkit_dvbt_teletext_select_args` | none |
| `track.select` | `sdrkit_dvb_terrestrial_track_select` | `sdrkit_dvbt_track_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bandwidth` | `sdrkit_dvb_terrestrial_get_bandwidth` | `sdrkit_dvb_terrestrial_set_bandwidth` |
| `stream` | `sdrkit_dvb_terrestrial_get_stream` | `sdrkit_dvb_terrestrial_set_stream` |
| `viterbi` | `sdrkit_dvb_terrestrial_get_viterbi` | `sdrkit_dvb_terrestrial_set_viterbi` |
| `time_interp` | `sdrkit_dvb_terrestrial_get_time_interp` | `sdrkit_dvb_terrestrial_set_time_interp` |
| `interferer_erase` | `sdrkit_dvb_terrestrial_get_interferer_erase` | `sdrkit_dvb_terrestrial_set_interferer_erase` |
| `standard` | `sdrkit_dvb_terrestrial_get_standard` | `sdrkit_dvb_terrestrial_set_standard` |
| `channels` | `sdrkit_dvb_terrestrial_get_channels` | `sdrkit_dvb_terrestrial_set_channels` |
| `audio_lang` | `sdrkit_dvb_terrestrial_get_audio_lang` | `sdrkit_dvb_terrestrial_set_audio_lang` |
| `audio_description` | `sdrkit_dvb_terrestrial_get_audio_description` | `sdrkit_dvb_terrestrial_set_audio_description` |
| `downmix` | `sdrkit_dvb_terrestrial_get_downmix` | `sdrkit_dvb_terrestrial_set_downmix` |
| `decode_video` | `sdrkit_dvb_terrestrial_get_decode_video` | `sdrkit_dvb_terrestrial_set_decode_video` |
| `subtitle_lang` | `sdrkit_dvb_terrestrial_get_subtitle_lang` | `sdrkit_dvb_terrestrial_set_subtitle_lang` |
| `hard_of_hearing` | `sdrkit_dvb_terrestrial_get_hard_of_hearing` | `sdrkit_dvb_terrestrial_set_hard_of_hearing` |
| `country` | `sdrkit_dvb_terrestrial_get_country` | `sdrkit_dvb_terrestrial_set_country` |
| `guide_lang` | `sdrkit_dvb_terrestrial_get_guide_lang` | `sdrkit_dvb_terrestrial_set_guide_lang` |
| `test_flat_csi` | `sdrkit_dvb_terrestrial_get_test_flat_csi` | `sdrkit_dvb_terrestrial_set_test_flat_csi` |
| `test_full_window` | `sdrkit_dvb_terrestrial_get_test_full_window` | `sdrkit_dvb_terrestrial_set_test_full_window` |
| `test_peak_only` | `sdrkit_dvb_terrestrial_get_test_peak_only` | `sdrkit_dvb_terrestrial_set_test_peak_only` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mode` | `sdrkit_dvb_terrestrial_gen_get_mode` | `sdrkit_dvb_terrestrial_gen_set_mode` |
| `gen_guard` | `sdrkit_dvb_terrestrial_gen_get_guard` | `sdrkit_dvb_terrestrial_gen_set_guard` |
| `gen_constellation` | `sdrkit_dvb_terrestrial_gen_get_constellation` | `sdrkit_dvb_terrestrial_gen_set_constellation` |
| `gen_hierarchy` | `sdrkit_dvb_terrestrial_gen_get_hierarchy` | `sdrkit_dvb_terrestrial_gen_set_hierarchy` |
| `gen_rate_hp` | `sdrkit_dvb_terrestrial_gen_get_rate_hp` | `sdrkit_dvb_terrestrial_gen_set_rate_hp` |
| `gen_rate_lp` | `sdrkit_dvb_terrestrial_gen_get_rate_lp` | `sdrkit_dvb_terrestrial_gen_set_rate_lp` |
| `gen_bandwidth` | `sdrkit_dvb_terrestrial_gen_get_bandwidth` | `sdrkit_dvb_terrestrial_gen_set_bandwidth` |
| `gen_cell_id` | `sdrkit_dvb_terrestrial_gen_get_cell_id` | `sdrkit_dvb_terrestrial_gen_set_cell_id` |
| `gen_length` | `sdrkit_dvb_terrestrial_gen_get_length` | `sdrkit_dvb_terrestrial_gen_set_length` |
| `gen_phase` | `sdrkit_dvb_terrestrial_gen_get_phase` | `sdrkit_dvb_terrestrial_gen_set_phase` |
| `gen_service` | `sdrkit_dvb_terrestrial_gen_get_service` | `sdrkit_dvb_terrestrial_gen_set_service` |
| `gen_name` | `sdrkit_dvb_terrestrial_gen_get_name` | `sdrkit_dvb_terrestrial_gen_set_name` |
| `gen_source` | `sdrkit_dvb_terrestrial_gen_get_source` | `sdrkit_dvb_terrestrial_gen_set_source` |

## Try it

```sh
sdrkit run dvb_terrestrial --source synth:dvb_terrestrial
sdrkit run dvb_terrestrial --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
