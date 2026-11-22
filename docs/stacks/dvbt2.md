<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DVB-T2

Digital terrestrial TV (DVB-T2 and T2-Lite): PLPs, channels, programme guide and subtitles. Transmits a transport stream that the host gives.

Stack id: `dvbt2`. Group: TV.

Signal types: DVB-T2.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 47 MHz to 1 GHz |
| Sample rate | 9.142857 MHz |
| Smallest sample rate | 7.84 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dvbt2` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bandwidth` | Channel bandwidth | choice | `8` | `1.7` (1.7 MHz), `5` (5 MHz), `6` (6 MHz), `7` (7 MHz), `8` (8 MHz), `10` (10 MHz) |  |
| `plp` | PLP | whole number | -1 | -1 to 255 | The data PLP to decode. -1 decodes the first data PLP. Its common PLP is decoded with it. |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |
| `tx_bandwidth` | Channel width | choice | `8` | `1.7` (1.7 MHz), `5` (5 MHz), `6` (6 MHz), `7` (7 MHz), `8` (8 MHz), `10` (10 MHz) | The width of the channel sent. It sets the elementary rate of the signal. |
| `tx_fft` | FFT size | choice | `8k` | `1k` (1K), `2k` (2K), `4k` (4K), `8k` (8K), `16k` (16K), `32k` (32K) |  |
| `tx_guard` | Guard interval | choice | `1/128` | `1/32`, `1/16`, `1/8`, `1/4`, `1/128`, `19/128`, `19/256` | A guard interval that the FFT size does not take is refused at start. |
| `tx_pilots` | Pilot pattern | choice | `pp7` | `pp1` (PP1), `pp2` (PP2), `pp3` (PP3), `pp4` (PP4), `pp5` (PP5), `pp6` (PP6), `pp7` (PP7), `pp8` (PP8) | A pattern that EN 302 755 tables 58 and 59 do not allow with the FFT size and the guard interval is refused at start. |
| `tx_extended` | Extended carriers | yes or no | false |  | 8K, 16K and 32K only. |
| `tx_modulation` | Constellation | choice | `64qam` | `qpsk` (QPSK), `16qam` (16-QAM), `64qam` (64-QAM), `256qam` (256-QAM) |  |
| `tx_rate` | Code rate | choice | `2/3` | `1/2`, `3/5`, `2/3`, `3/4`, `4/5`, `5/6`, `1/3` (1/3 (T2-Lite)), `2/5` (2/5 (T2-Lite)) |  |
| `tx_rotation` | Rotated constellation | yes or no | true |  |  |
| `tx_fec_frame` | FEC frame | choice | `64800` | `16200` (16 200 bits), `64800` (64 800 bits) |  |
| `tx_ti_blocks` | Time interleaver | whole number (TI-blocks) | 3 | 0 to 8 | The TI-blocks of a T2-frame. 0: no time interleaving. Fewer TI-blocks interleave deeper and hold fewer FEC blocks a frame. |
| `tx_frame_ms` | Longest frame | whole number (ms) | 100 | 10 to 250 | The T2-frame holds whole symbols and whole FEC blocks and lasts this long at most, and never less than the shortest frame of the FFT size. |
| `tx_l1_mod` | L1-post constellation | choice | `qpsk` | `bpsk` (BPSK), `qpsk` (QPSK), `16qam` (16-QAM), `64qam` (64-QAM) |  |
| `tx_cell_id` | Cell id | whole number | 0 | 0 to 65535 |  |
| `tx_network_id` | Network id | whole number | 0 | 0 to 65535 |  |
| `tx_system_id` | T2 system id | whole number | 0 | 0 to 65535 |  |
| `tx_profile` | Profile | choice | `base` | `base` (T2-base), `lite` (T2-Lite) | T2-Lite (EN 302 755 annex I) takes 2K to 16K, the code rates 1/2 to 3/4, 1/3 and 2/5, always the 16 200-bit FEC frame, 256-QAM only with 1/3, 2/5, 1/2 and 3/5 and never rotated, and at most 4 Mbit/s. A pattern that tables I.5 and I.6 do not allow is refused at start. |
| `tx_miso` | MISO | choice | `siso` | `siso` (SISO), `group1` (MISO group 1), `group2` (MISO group 2) | In MISO each transmitter of group 2 sends the modified Alamouti code of the cells that group 1 sends, and its own pilot signs (EN 302 755 clause 9.1). A receiver decodes the sum of both groups. A pattern that table 59 does not allow is refused at start. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_fft` | FFT size | choice | `8k` | `1k` (1K), `2k` (2K), `4k` (4K), `8k` (8K), `16k` (16K), `32k` (32K) |  |
| `gen_guard` | Guard interval | choice | `1/8` | `1/32`, `1/16`, `1/8`, `1/4`, `1/128`, `19/128`, `19/256` |  |
| `gen_pilots` | Pilot pattern | choice | `pp3` | `pp1` (PP1), `pp2` (PP2), `pp3` (PP3), `pp4` (PP4), `pp5` (PP5), `pp6` (PP6), `pp7` (PP7), `pp8` (PP8) |  |
| `gen_extended` | Extended carriers | yes or no | false |  |  |
| `gen_modulation` | Constellation | choice | `64qam` | `qpsk` (QPSK), `16qam` (16-QAM), `64qam` (64-QAM), `256qam` (256-QAM) |  |
| `gen_rate` | Code rate | choice | `2/3` | `1/2`, `3/5`, `2/3`, `3/4`, `4/5` (4/5 (T2-base)), `5/6` (5/6 (T2-base)), `1/3` (1/3 (T2-Lite)), `2/5` (2/5 (T2-Lite)) |  |
| `gen_fec_frame` | FEC frame | choice | `64800` | `16200` (16 200 bits), `64800` (64 800 bits) |  |
| `gen_rotation` | Rotated constellation | yes or no | true |  |  |
| `gen_profile` | Profile | choice | `base` | `base` (T2-base), `lite` (T2-Lite) |  |
| `gen_miso` | MISO | yes or no | false |  |  |
| `gen_blocks` | FEC blocks per frame | whole number | 4 | 1 to 100 |  |
| `gen_source` | Content | choice | `mux` | `mux` (Test multiplex), `pattern` (Numbered packets) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `detection` | Detection | choice |
| `fft` | FFT size | choice |
| `guard` | Guard interval | choice |
| `pilot_pattern` | Pilot pattern | choice |
| `extended_carriers` | Extended carriers | yes or no |
| `miso` | MISO | yes or no |
| `profile` | Profile | choice |
| `l1_modulation` | L1-post constellation | choice |
| `cell_id` | Cell id | whole number |
| `network_id` | Network id | whole number |
| `t2_system_id` | T2 system id | whole number |
| `frames_per_superframe` | T2-frames per super-frame | whole number |
| `data_symbols` | Data symbols per frame | whole number |
| `frame_length` | Frame length | duration (s) |
| `t2_frames` | T2-frames | whole number |
| `l1_errors` | L1 failures | whole number |
| `plp` | PLP | whole number |
| `common_plp` | Common PLP | whole number |
| `ldpc_iterations` | LDPC iterations | number |
| `net_rate_bit_s` | Net bit rate | number (bit/s) |
| `problem` | Problem | choice |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `bits_corrected` | Bits corrected | whole number |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_bitrate_bps` | Useful bit rate | number (bit/s) |
| `tx_packets` | Packets sent | whole number |
| `tx_nulls` | Null packets inserted | whole number |
| `tx_queue` | Packets waiting | whole number |
| `tx_queue_s` | Queue length | duration (s) |
| `tx_pcr_restamped` | PCRs rewritten | whole number |
| `tx_frame_s` | Frame length | duration (s) |
| `tx_fec_blocks` | FEC blocks per frame | whole number |
| `tx_problem` | Settings refused | text |
| `ts.ts_packets` | TS packets | whole number |
| `ts.crc_errors` | CRC errors | whole number |
| `ts.pdus` | PDUs | whole number |
| `ts.non_ip` | Non-IP PDUs | whole number |
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
| `plps` | PLPs | `plp_id` (whole number), `plp_type` (choice), `payload` (choice), `modulation` (choice), `code_rate` (choice), `fec_frame_bits` (whole number (bits)), `rotated` (yes or no), `fec_blocks` (whole number), `group_id` (whole number), `time_interleaving` (choice), `time_il_length` (whole number) |
| `services` | Services | `service_id` (whole number), `name` (text), `provider` (text), `service_type` (whole number), `kind` (choice), `lcn` (whole number), `visible` (yes or no), `scrambled` (yes or no), `ca_state` (choice), `pmt_pid` (whole number), `pcr_pid` (whole number), `running_status` (whole number), `in_pat` (yes or no), `eit_schedule` (yes or no), `eit_present_following` (yes or no), `onid` (whole number), `tsid` (whole number), `now_title` (text), `now_text` (text), `now_start` (time), `now_duration` (duration (s)), `next_title` (text), `next_text` (text), `next_start` (time), `next_duration` (duration (s)) |
| `tracks` | Tracks | `service` (text), `pid` (whole number), `stream_type` (whole number), `kind` (choice), `codec` (choice), `packaging` (choice), `fourcc` (text), `lang` (text), `audio_type` (whole number), `audio_description` (yes or no), `hard_of_hearing` (yes or no), `component_tag` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no), `profile` (text), `level` (text), `width` (whole number (px)), `height` (whole number (px)), `frame_rate` (number (Hz)), `aspect` (text), `interlaced` (yes or no), `sample_rate` (frequency), `channels` (whole number), `layout` (text), `bitrate_kbps` (whole number (kbit/s)), `object_type` (text), `entry_type` (whole number), `composition_page` (whole number), `ancillary_page` (whole number), `magazine` (whole number), `teletext_page` (whole number) |
| `guide` | Guide | `service` (text), `onid` (whole number), `tsid` (whole number), `service_id` (whole number), `event_id` (whole number), `start` (time), `duration` (duration (s)), `running_status` (whole number), `title` (text), `text` (text), `extended` (text), `lang` (text), `genre` (whole number), `content` (bytes), `min_age` (whole number), `crid_programme` (text), `crid_series` (text), `free_ca` (yes or no), `other_ts` (yes or no), `schedule` (yes or no) |
| `guide_ratings` | Guide ratings | `event` (text), `country` (text), `rating` (whole number) |
| `teletext_pages` | Teletext pages | `page` (whole number), `subpages` (whole number) |
| `teletext_subpages` | Teletext subpages | `page_row` (text), `subcode` (whole number), `version` (whole number) |
| `ts_demux.pids` | PIDs | `pid` (whole number), `label` (text), `packets` (whole number), `cc_errors` (whole number), `tei` (whole number), `kbps` (number (kbit/s)), `scrambled` (yes or no) |
| `psi_si.sections` | Sections | `accepted` (whole number), `crc` (whole number), `length` (whole number), `other` (whole number) |

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
| `start` | Start sending |  |  | Sends a DVB-T2 signal with the tx_* settings from the start of a T2-frame, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. |
| `stop` | Stop sending |  |  | Ends the transmission after the T2-frame on air with a fall of 5 ms. The packets that wait are dropped. |
| `append` | Add packets | `packets` (bytes, required) |  | Adds transport packets to the queue of the stream. All or none: refused when the queue has no room, so the host feeds at the useful bit rate. |

## Typed interface

The stack is in the family `dvbt2`, version 1.3. The C header `sdrkit/dvbt2.h`, the registry record `registry/dvbt2.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `Dvbt2Fields`. The result fields of `dvbt2`, which the receiver `rx` writes. A value of the frame shape has no value before the P2 symbols are found, and a value of the L1 signalling has no value before the L1-pre is decoded.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `detection` | choice | always | 1.0 | Detection: searching, P1 found, L1 decoded, or packets flowing. |
| `fft` | choice | optional | 1.0 | The FFT size. |
| `guard` | choice | optional | 1.0 | The guard interval. |
| `pilot_pattern` | choice | optional | 1.0 | The scattered pilot pattern. |
| `extended_carriers` | yes or no | optional | 1.0 | Extended carrier mode. |
| `miso` | yes or no | optional | 1.0 | MISO. |
| `profile` | choice | optional | 1.0 | The profile. |
| `l1_modulation` | choice | optional | 1.0 | The L1-post constellation. |
| `cell_id` | whole number | optional | 1.0 | CELL_ID. |
| `network_id` | whole number | optional | 1.0 | NETWORK_ID. |
| `t2_system_id` | whole number | optional | 1.0 | T2_SYSTEM_ID. |
| `frames_per_superframe` | whole number | optional | 1.0 | NUM_T2_FRAMES. |
| `data_symbols` | whole number | optional | 1.0 | NUM_DATA_SYMBOLS. |
| `frame_length` | duration (s) | optional | 1.0 | The T2-frame length. |
| `t2_frames` | whole number | always | 1.0 | T2-frames decoded. |
| `l1_errors` | whole number | always | 1.0 | L1 signalling failures. |
| `plp` | whole number | optional | 1.0 | The data PLP decoded. |
| `common_plp` | whole number | optional | 1.0 | Its common PLP. |
| `ldpc_iterations` | number | always | 1.0 | Mean LDPC iterations per FEC block. |
| `net_rate_bit_s` | number (bit/s) | optional | 1.0 | The net bit rate of the decoded data PLP. |
| `problem` | choice | optional | 1.0 | A configuration that the receiver cannot follow. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset from the centre. No value while the receiver searches. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR. |
| `frames_ok` | whole number | always | 1.0 | FEC blocks good. |
| `frames_bad` | whole number | always | 1.0 | FEC blocks that failed. |
| `bits_corrected` | whole number | always | 1.0 | Bits that the BCH decoder corrected. |

### Result fields

Struct `Dvbt2TxFields`. The result fields of the sender `transmit` of `dvbt2`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.2 | Whether a transmission is under way. |
| `tx_freq_hz` | frequency | optional | 1.2 | The transmit frequency: the centre of the channel sent. No value while the stack does not transmit. |
| `tx_bitrate_bps` | number (bit/s) | always | 1.2 | The bit rate of the transport stream that the settings carry: the rate that the host feeds at most. 0 while the settings cannot be sent. |
| `tx_packets` | whole number | always | 1.2 | The packets of the host sent in this transmission, or in the last one. |
| `tx_nulls` | whole number | always | 1.2 | The null packets put in slots that found no packet of the host. |
| `tx_queue` | whole number | always | 1.2 | The packets that wait to be sent. |
| `tx_queue_s` | duration (s) | always | 1.2 | The time that the waiting packets take at the useful bit rate. |
| `tx_pcr_restamped` | whole number | always | 1.2 | The PCRs rewritten to the time of their slot. |
| `tx_frame_s` | duration (s) | optional | 1.2 | The length of a T2-frame. No value while the settings cannot be sent. |
| `tx_fec_blocks` | whole number | optional | 1.2 | The FEC blocks of a T2-frame. No value while the settings cannot be sent. |
| `tx_problem` | text | optional | 1.2 | Why the settings cannot be sent. No value while they can. |

### Table `plps`

Struct `Dvbt2PlpRow`. A row of the PLPs of the multiplex. The row key is PLP_ID in decimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plp_id` | whole number | always | 1.0 | PLP_ID. |
| `plp_type` | choice | always | 1.0 | PLP_TYPE. |
| `payload` | choice | always | 1.0 | PLP_PAYLOAD_TYPE. |
| `modulation` | choice | optional | 1.0 | The constellation. No value for a reserved code. |
| `code_rate` | choice | optional | 1.0 | The code rate. No value for a code that the profile does not use. |
| `fec_frame_bits` | whole number (bits) | optional | 1.0 | The FEC frame length. No value for a reserved code. |
| `rotated` | yes or no | always | 1.0 | Rotated constellation. |
| `fec_blocks` | whole number | always | 1.0 | PLP_NUM_BLOCKS of this frame. |
| `group_id` | whole number | always | 1.0 | PLP_GROUP_ID. |
| `time_interleaving` | choice | always | 1.0 | How the PLP is time interleaved. |
| `time_il_length` | whole number | always | 1.0 | TIME_IL_LENGTH: TI-blocks per frame, or T2-frames per TI-block. |

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
| `service.select` | `sdrkit_dvbt2_service_select` | `sdrkit_dvbt2_service_select_args` | none |
| `teletext.select` | `sdrkit_dvbt2_teletext_select` | `sdrkit_dvbt2_teletext_select_args` | none |
| `track.select` | `sdrkit_dvbt2_track_select` | `sdrkit_dvbt2_track_select_args` | none |
| `start` | `sdrkit_dvbt2_start` | none | none |
| `stop` | `sdrkit_dvbt2_stop` | none | none |
| `append` | `sdrkit_dvbt2_append` | `sdrkit_dvbt2_tx_append_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bandwidth` | `sdrkit_dvbt2_get_bandwidth` | `sdrkit_dvbt2_set_bandwidth` |
| `plp` | `sdrkit_dvbt2_get_plp` | `sdrkit_dvbt2_set_plp` |
| `decode_video` | `sdrkit_dvbt2_get_decode_video` | `sdrkit_dvbt2_set_decode_video` |
| `tx_bandwidth` | `sdrkit_dvbt2_get_tx_bandwidth` | `sdrkit_dvbt2_set_tx_bandwidth` |
| `tx_fft` | `sdrkit_dvbt2_get_tx_fft` | `sdrkit_dvbt2_set_tx_fft` |
| `tx_guard` | `sdrkit_dvbt2_get_tx_guard` | `sdrkit_dvbt2_set_tx_guard` |
| `tx_pilots` | `sdrkit_dvbt2_get_tx_pilots` | `sdrkit_dvbt2_set_tx_pilots` |
| `tx_extended` | `sdrkit_dvbt2_get_tx_extended` | `sdrkit_dvbt2_set_tx_extended` |
| `tx_modulation` | `sdrkit_dvbt2_get_tx_modulation` | `sdrkit_dvbt2_set_tx_modulation` |
| `tx_rate` | `sdrkit_dvbt2_get_tx_rate` | `sdrkit_dvbt2_set_tx_rate` |
| `tx_rotation` | `sdrkit_dvbt2_get_tx_rotation` | `sdrkit_dvbt2_set_tx_rotation` |
| `tx_fec_frame` | `sdrkit_dvbt2_get_tx_fec_frame` | `sdrkit_dvbt2_set_tx_fec_frame` |
| `tx_ti_blocks` | `sdrkit_dvbt2_get_tx_ti_blocks` | `sdrkit_dvbt2_set_tx_ti_blocks` |
| `tx_frame_ms` | `sdrkit_dvbt2_get_tx_frame_ms` | `sdrkit_dvbt2_set_tx_frame_ms` |
| `tx_l1_mod` | `sdrkit_dvbt2_get_tx_l1_mod` | `sdrkit_dvbt2_set_tx_l1_mod` |
| `tx_cell_id` | `sdrkit_dvbt2_get_tx_cell_id` | `sdrkit_dvbt2_set_tx_cell_id` |
| `tx_network_id` | `sdrkit_dvbt2_get_tx_network_id` | `sdrkit_dvbt2_set_tx_network_id` |
| `tx_system_id` | `sdrkit_dvbt2_get_tx_system_id` | `sdrkit_dvbt2_set_tx_system_id` |
| `tx_profile` | `sdrkit_dvbt2_get_tx_profile` | `sdrkit_dvbt2_set_tx_profile` |
| `tx_miso` | `sdrkit_dvbt2_get_tx_miso` | `sdrkit_dvbt2_set_tx_miso` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_fft` | `sdrkit_dvbt2_gen_get_fft` | `sdrkit_dvbt2_gen_set_fft` |
| `gen_guard` | `sdrkit_dvbt2_gen_get_guard` | `sdrkit_dvbt2_gen_set_guard` |
| `gen_pilots` | `sdrkit_dvbt2_gen_get_pilots` | `sdrkit_dvbt2_gen_set_pilots` |
| `gen_extended` | `sdrkit_dvbt2_gen_get_extended` | `sdrkit_dvbt2_gen_set_extended` |
| `gen_modulation` | `sdrkit_dvbt2_gen_get_modulation` | `sdrkit_dvbt2_gen_set_modulation` |
| `gen_rate` | `sdrkit_dvbt2_gen_get_rate` | `sdrkit_dvbt2_gen_set_rate` |
| `gen_fec_frame` | `sdrkit_dvbt2_gen_get_fec_frame` | `sdrkit_dvbt2_gen_set_fec_frame` |
| `gen_rotation` | `sdrkit_dvbt2_gen_get_rotation` | `sdrkit_dvbt2_gen_set_rotation` |
| `gen_profile` | `sdrkit_dvbt2_gen_get_profile` | `sdrkit_dvbt2_gen_set_profile` |
| `gen_miso` | `sdrkit_dvbt2_gen_get_miso` | `sdrkit_dvbt2_gen_set_miso` |
| `gen_blocks` | `sdrkit_dvbt2_gen_get_blocks` | `sdrkit_dvbt2_gen_set_blocks` |
| `gen_source` | `sdrkit_dvbt2_gen_get_source` | `sdrkit_dvbt2_gen_set_source` |

## Try it

```sh
sdrkit run dvbt2 --source synth:dvbt2
sdrkit run dvbt2 --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack `dvbt2` sends a DVB-T2 signal (ETSI EN 302 755) of a transport stream that the host gives. The library has no
video encoder: the host, or the user, codes the pictures and the sound into an MPEG transport stream, and the stack
modulates its packets. The test signal and the sender run one transmitter: mode adaptation in normal mode (BBHEADER with
its CRC-8, the CRC-8 of each packet, BB scrambling), BCH and LDPC, the bit interleaver and the demultiplexer, the
mapping with rotation and cyclic Q delay, the cell and time interleavers, L1-pre and L1-post in the P2 symbols, the
frame builder with the frame closing symbol where the pilot pattern has one, the frequency interleaver, the pilots, the
inverse FFT, the guard interval and the P1 symbol. The sender sends T2-base with one data PLP (PLP_ID 0, type 1) from one
transmitter, two T2-frames a super-frame. The engine setting `transmit_allowed` is the only switch; the stack adds no
other gate.

Commands:

- `start` sends with the `tx_*` settings from the start of a T2-frame, until `stop`. A setting change applies at the
  next `start`. A combination that EN 302 755 does not allow is refused with the rule: a pilot pattern that tables 58 and
  59 do not allow with the FFT size and the guard interval, a guard interval that the FFT size does not take, extended
  carriers below 8K. The result field `tx_problem` states the same before `start`.
- `stop` ends the transmission after the T2-frame on air, with a fall of 5 ms and the end mark. The packets that wait are
  dropped.
- `append` adds whole 188-byte packets (each starting with 0x47) to the queue of the stream. The queue holds 6000
  packets; an append that does not fit is refused as a whole, so the host feeds at the useful bit rate. Null packets of
  the host are left out. Packets can wait in the queue before `start`.

Settings: `tx_bandwidth` (1.7, 5, 6, 7, 8, 10 MHz), `tx_fft` (1K to 32K), `tx_guard` (1/32, 1/16, 1/8, 1/4, 1/128,
19/128, 19/256), `tx_pilots` (PP1 to PP8), `tx_extended` (extended carriers, 8K to 32K), `tx_modulation` (QPSK to
256-QAM), `tx_rate` (1/2, 3/5, 2/3, 3/4, 4/5, 5/6), `tx_rotation` (rotated constellation), `tx_fec_frame` (64 800 or
16 200 bits), `tx_ti_blocks` (0 to 8 TI-blocks a frame; 0 is no time interleaving), `tx_frame_ms` (the longest T2-frame,
10 to 250 ms), `tx_l1_mod` (BPSK to 64-QAM for L1-post), `tx_cell_id`, `tx_network_id` and `tx_system_id` (0 to 65535).
The defaults are 8 MHz, 8K, guard 1/128, PP7, rotated 64-QAM 2/3 with 64 800-bit frames, three TI-blocks, frames of 100
ms at most and QPSK L1-post.

The frame: the T2-frame holds the most FEC blocks that fit `tx_frame_ms` (never more than the 250 ms of clause 8.3.1,
never less than the shortest frame of the FFT size), that PLP_NUM_BLOCKS can state (1023) and that the TI-blocks hold in
the time interleaver memory of 2^19 + 2^15 cells; then it takes the fewest data symbols that hold them. Without time
interleaving the frame holds the FEC blocks of one TI-block at most. The useful bit
rate is the FEC blocks × (K_bch − 80) over the frame time. Examples, with frames of 100 ms at most:

| Channel | FFT, guard, pilots | Constellation, rate, FEC frame | FEC blocks | Frame | Useful bit rate |
|---|---|---|---|---|---|
| 8 MHz | 8K, 1/128, PP7 | 64-QAM 2/3, 64 800 | 67 | 99.6 ms | 28.912 Mbit/s |
| 8 MHz | 2K, 1/8, PP2 | 16-QAM 3/4, 16 200 | 147 | 99.3 ms | 17.227 Mbit/s |
| 5 MHz | 2K, 1/8, PP2 | QPSK 1/2, 16 200 | 45 | 97.9 ms | 3.194 Mbit/s |
| 1.7 MHz | 2K, 1/8, PP2 | 16-QAM 3/4, 16 200 | 28 | 99.8 ms | 3.265 Mbit/s |
| 1.7 MHz | 2K, 1/8, PP2 | 16-QAM 1/2, 16 200 | 28 | 99.8 ms | 1.951 Mbit/s |
| 1.7 MHz | 2K, 1/8, PP2 | QPSK 1/2, 16 200 | 14 | 99.8 ms | 0.976 Mbit/s |

The stream: each packet slot takes the next packet of the queue; a slot that finds the queue empty carries a null packet
(PID 0x1FFF). With no input the signal is a valid multiplex of null packets; null packets are not deleted on air. Each
PCR leaves rewritten to the time of its slot on the time base of its PID, the slots at the useful bit rate. A PCR after
a discontinuity indicator, or far off its time base, starts the time base again. The host paces the stream: the sender
does not hold packets back to the times of their PCRs. The FEC blocks of a T2-frame are coded while the frame before it
is sent, a share with each symbol, so a packet goes on air one to two frames after its slot.

The signal: complex baseband at the session's rate, centred on the dial. The sender makes it at the elementary rate of
the width (131/71, 40/7, 48/7, 8, 64/7, 80/7 Msps for 1.7, 5, 6, 7, 8, 10 MHz); at another rate a resampler that is flat
over the occupied band changes the rate. A session rate below the occupied width over 0.9 is refused (8.452 Msps for 8
MHz, 1.706 Msps for 1.7 MHz, normal carriers). The receiver of the stack runs on the same session, so the session's rate
also fits the setting `bandwidth`. The rms is 0.25 of full scale (−12 dBFS). Every sample passes a soft limiter:
unchanged up to 0.9, above it 0.9 + 0.1 tanh((m − 0.9)/0.1), so no sample reaches full scale. The signal rises in 5 ms
at the start and falls in 5 ms from the end of the last frame. On a live radio whose host times bursts, the first sample
states its time on air. A real amplifier needs back-off: its mean output power about the peak to average ratio below its
1 dB compression point. Measured on 0.4 s of the defaults: rms 0.250, peak 0.967, peak to average ratio 11.8 dB at the
highest sample and 9.6 dB at a probability of 10⁻⁴, 0.0003 % of the samples above 0.9; the error that the limiter adds
is 89 dB below the signal; 7.54 MHz hold 99 % of the power (the carriers span 7.607 MHz), and the power beyond 4.2 MHz
from the centre is 48 dB below the total. The stack's own receiver locks in two T2-frames (0.2 s with frames of 100 ms)
and reads an SNR of 39 dB with noise 40 dB below the signal.

Cost: the sender takes 0.25 to 0.28 of one core for the defaults at 64/7 Msps and 0.02 for 2K QPSK 1/2 in 1.7 MHz at
its elementary rate (benchmarks `dvbt2.sender_8k_64qam_23_8mhz` and `dvbt2.sender_2k_qpsk_12_1700khz`, on a machine
with other work; 0.51 was read while it was busy). The work of a frame is spread over the frame: the part of a frame of
the defaults that takes longest takes 0.49 ms for 0.90 ms of signal. `start` builds the transmitter (2.7 ms) and codes
the first frame at once (15.7 ms); the frames after it are coded ahead. The benchmarks do not hold the rate change of a
session at another rate than the elementary rate. The sender runs in real time on one core and declares no stage of its
own. `cargo run --release -p sdrkit-stack-dvbt2 --example bench` prints the figures.

Results: `transmitting`, `tx_freq_hz`, `tx_bitrate_bps` (the useful bit rate of the settings, also before `start`; 0
while the settings cannot be sent), `tx_packets` (packets of the host sent), `tx_nulls` (null packets inserted),
`tx_queue`, `tx_queue_s` (the queue as time at the useful bit rate), `tx_pcr_restamped`, `tx_frame_s` (the length of a
T2-frame), `tx_fec_blocks` (the FEC blocks of a frame) and `tx_problem` (why the settings cannot be sent).

Not confirmed: extended carriers at 8K and 16K with PP7 or PP8. In these four cases some symbols have one data cell
more or less than table 48 of EN 302 755 states; the sender and the receiver of the stack agree with each other, and
another receiver can differ.

Not sent: T2-Lite, MISO, several PLPs, a time interleaver over several frames, high efficiency mode, null packet
deletion, ISSY, FEF parts and PAPR reduction. The receiver of the stack takes all of these but PAPR reduction by active
constellation extension.

Licence: television broadcasting on the broadcast bands needs a licence nearly everywhere. Amateur television exists in
the 70 cm (430 to 440 MHz in Region 1, 420 to 450 MHz in Region 2), 23 cm (1240 to 1300 MHz) and 13 cm (2300 to 2450
MHz) bands and higher, by national rules; amateur DVB-T2 is rare (not confirmed). These are facts for the user and the
region data, not gates in the library.
