<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DVB-C2

Second-generation cable TV (DVB-C2, ETSI EN 302 769): one PLP with its common PLP put back into its transport stream, or a PLP bundled over several slices, 16- to 4096-QAM, transport stream, GSE, GFPS or GCS.

Stack id: `dvbc2`. Group: TV.

Signal types: DVB-C2.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 346 MHz |
| Tuning range | 47 MHz to 1.002 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 6 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dvbc2` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `plp` | PLP | whole number | -1 | -1 to 255 | PLP_ID of the PLP to decode, as the table of PLPs lists it; -1: the first data PLP of the first data slice inside the receive window. The common PLP of its group is decoded with it. |
| `slice` | Data slice | whole number | -1 | -1 to 255 | DSLICE_ID of the data slice; -1: the first slice inside the receive window that carries the PLP. |
| `centre_known` | Centre frequency known | yes or no | true |  | Off: the absolute carrier index is found by a full search over every carrier position (about a second once). |
| `raster` | Channel raster | choice | `8` | `8` (8 MHz), `6` (6 MHz) | 8 MHz: 64/7 Msps, carriers 2232 Hz apart; 6 MHz: 48/7 Msps, 1674 Hz. Transmitter and receiver must agree. |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `ldpc_iterations` | LDPC iterations | whole number | 50 | 1 to 200 |  |
| `timing_metric` | Timing metric | number | 0.22 | 0.01 to 1 | The mean of \|C\| / E of the guard correlation folded over 8 symbol periods. |
| `preamble_ratio` | Preamble ratio | number | 3 | 1 to 10 | The energy of the strongest scattered pilot phase against the median: about 5 on a data symbol. |
| `index_metric` | Carrier index metric | number | 0.35 | 0.05 to 1 |  |
| `index_span` | Carrier index span | whole number | 64 | 0 to 1000 |  |
| `header_candidate` | Header candidate | number | 0.45 | 0.05 to 1 |  |
| `header_agree` | Header agreement | whole number | 22 | 16 to 32 |  |
| `header_misses` | Header misses | whole number | 2 | 1 to 100 |  |
| `block_misses` | Block misses | whole number | 8 | 1 to 1000 |  |
| `preamble_misses` | Preamble misses | whole number | 3 | 1 to 100 |  |
| `ti_full_width` | Time interleaver over every carrier | yes or no | true |  | On: N_DSLICE_WIDTH = K_DS,max - K_DS,min + 1, one column per carrier from K_DS,min to K_DS,max (clause 9.4.1 makes K_DS,max the last carrier of the slice). Off: K_DS,max - K_DS,min as clause 9.4.4 prints it, and the last carrier of the slice carries no data. |
| `rm_scrambling` | Header scrambling word | whole number | 495795187 | 0 to 4294967295 | w^RM of clause 7.2.2.3 as a number, w^RM_0 the most significant bit; default 0x1D8D3BF3. Transmitter and receiver must agree. |
| `bundle_hold` | Bundled PLP hold | whole number (frames) | 16 | 1 to 1000 | The BBFRAMEs of a PLP bundled over several slices wait here until they can go out in the order of their ISCR. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_frequency` | Centre frequency | frequency | 346 MHz | 47 MHz to 1.002 GHz | The system is centred on the carrier below this frequency that is a multiple of D_X. |
| `gen_modcod` | Coding | choice | `256_5_6` | `256_5_6` (256-QAM 5/6), `16_4_5` (16-QAM 4/5), `64_2_3` (64-QAM 2/3), `256_3_4` (256-QAM 3/4), `64_9_10` (64-QAM 9/10), `256_9_10` (256-QAM 9/10), `1024_3_4` (1024-QAM 3/4), `4096_5_6` (4096-QAM 5/6) |  |
| `gen_short_frames` | 16K FEC frames | yes or no | false |  |  |
| `gen_guard` | Guard interval | choice | `1_64` | `1_64` (1/64), `1_128` (1/128) |  |
| `gen_slice_type` | Data slice type | choice | `type1` | `type1` (Type 1), `type2_robust` (Type 2, robust header), `type2_efficient` (Type 2, high efficiency header) |  |
| `gen_ti_depth` | Time interleaving | choice | `none` | `none` (None), `4` (4 symbols), `8` (8 symbols), `16` (16 symbols) |  |
| `gen_l1_ti` | L1 time interleaving | choice | `none` | `none` (None), `best_fit` (Best fit), `4` (4 symbols), `8` (8 symbols) |  |
| `gen_notch` | Narrowband notch | yes or no | false |  | 35 carriers at guard 1/64, 23 at 1/128. |
| `gen_reserved_tones` | Reserved tones | yes or no | false |  |  |
| `gen_system` | Slices and PLPs | choice | `single` | `single` (One slice, one PLP), `two_plps` (One slice, two PLPs), `common` (Data PLP and common PLP), `bundled` (PLP bundled over two slices), `two_slices` (Two slices, two L1 blocks wide), `mixed_slices` (A type 1 and a type 2 slice), `group` (Two data PLPs and their common PLP), `far_slice` (Group over two L1 blocks) | two_plps and common use a type 2 slice; bundled, group and far_slice use type 2 slices only; mixed_slices has one slice of each type. group and far_slice carry numbered packets. two_slices and far_slice are 15.2 MHz wide (8 MHz raster): two_slices is centred on gen_frequency and needs 20 Msps to be seen whole; far_slice has the centre of its lower L1 block there, and the window at the tuning position of the slice of PLP 2 holds that slice and the common PLP. |
| `gen_stream` | Stream | choice | `ts` | `ts` (Transport stream), `gse` (GSE), `gfps` (GFPS), `gcs` (GCS) |  |
| `gen_hem` | High efficiency mode | yes or no | false |  | Transport packets without sync byte and CRC-8, or GSE, in BBFRAMEs of MODE 1. |
| `gen_payload` | Payload | choice | `programme` | `programme` (Test multiplex), `numbered` (Numbered packets) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `guard` | Guard interval | choice |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `window_carrier` | Window centre carrier | whole number |
| `window_hz` | Window centre | frequency |
| `esn0_db` | Es/N0 | number (dB) |
| `mer_db` | MER | number (dB) |
| `network_id` | Network id | whole number |
| `system_id` | C2 system id | whole number |
| `start_frequency_hz` | Start frequency | frequency |
| `bandwidth_hz` | System width | frequency |
| `change_counter` | L1 change counter | whole number |
| `ews` | Early warning | yes or no |
| `c2_version` | C2 version | whole number |
| `reserved_tones` | Reserved tones | yes or no |
| `blocks_ok` | FEC blocks good | whole number |
| `blocks_bad` | FEC blocks bad | whole number |
| `l1_bad` | L1 failures | whole number |
| `sync_losses` | Sync losses | whole number |
| `pre_ber` | BER before LDPC | number |
| `mean_iterations` | LDPC iterations | number |
| `c2_frames` | C2 frames | whole number |
| `chosen_slice` | Data slice | whole number |
| `chosen_plp` | PLP | whole number |
| `common_plp` | Common PLP | whole number |
| `bundled_slices` | Bundled slices | whole number |
| `retune_hz` | Tune to | frequency |
| `outside_slice` | Slice out of the window | whole number |
| `outside_hz` | Out of the window by | frequency |
| `generic_packets` | Generic stream packets | whole number |
| `generic_bytes` | Generic stream bytes | whole number |
| `note` | Note | text |
| `ts_packets` | TS packets | whole number |
| `pdus` | GSE PDUs | whole number |
| `ip_packets` | IP packets | whole number |
| `packets` | Packets | whole number |
| `flagged` | Uncorrectable packets | whole number |
| `scrambled` | Scrambled packets | whole number |
| `numbered_good` | Test packets good | whole number |
| `numbered_wrong` | Test packets wrong | whole number |
| `numbered_gaps` | Test packet gaps | whole number |
| `bitrate_bps` | Bit rate | number (bit/s) |
| `pat_seen` | PAT seen | yes or no |
| `services` | Services | text |
| `bbframe.frames_ok` | Frames good | whole number |
| `bbframe.frames_bad` | Frames bad | whole number |
| `bbframe.crc_errors` | CRC errors | whole number |
| `bbframe.non_ip` | Non-IP PDUs | whole number |
| `ip.ip_bad` | Bad IP packets | whole number |
| `ip.reassembled` | Reassembled | whole number |
| `ip.flows` | Flows | whole number |
| `ip.encrypted_flows` | Encrypted flows | whole number |
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
| `slices` | Data slices | `slice_id` (whole number), `slice_type` (choice), `header_type` (choice), `tune_hz` (frequency), `start_hz` (frequency), `end_hz` (frequency), `ti_depth` (whole number (symbols)), `plp_count` (whole number), `in_window` (yes or no), `width_hz` (frequency) |
| `plps` | PLPs | `plp_slice` (whole number), `plp_id` (whole number), `plp_type` (choice), `payload` (choice), `fec_frame_bits` (whole number (bits)), `modulation` (choice), `code_rate` (choice), `ts_id` (whole number), `on_id` (whole number), `chosen` (yes or no), `group` (whole number), `bundled` (yes or no) |
| `notches` | Notches | `notch_start_hz` (frequency), `notch_end_hz` (frequency) |
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
| `clear` | Clear |  |  | Forgets every count. |

## Typed interface

The stack is in the family `dvbcs`, version 1.3. The C header `sdrkit/dvbcs.h`, the registry record `registry/dvbcs.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `Dvbc2Fields`. The result fields of the receiver of `dvbc2`: the signal, the L1 signalling and the FEC. A value of the L1 has no value before the L1 is read.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | What the receiver does now. |
| `guard` | choice | optional | 1.0 | The guard interval. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset from the centre. No value while the receiver searches. |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the sample clock. |
| `window_carrier` | whole number | optional | 1.0 | The carrier at the centre of the receive window. |
| `window_hz` | frequency | optional | 1.0 | The frequency of the centre of the receive window. |
| `esn0_db` | number (dB) | optional | 1.0 | Es/N0. |
| `mer_db` | number (dB) | optional | 1.0 | The modulation error ratio. |
| `network_id` | whole number | optional | 1.0 | The network id of the L1. |
| `system_id` | whole number | optional | 1.0 | The C2 system id of the L1. |
| `start_frequency_hz` | frequency | optional | 1.0 | The start frequency of the C2 system. |
| `bandwidth_hz` | frequency | optional | 1.0 | The width of the C2 system. |
| `change_counter` | whole number | optional | 1.0 | The change counter of the L1. |
| `ews` | yes or no | optional | 1.0 | The early warning flag of the L1. |
| `c2_version` | whole number | optional | 1.0 | The C2 version of the L1. |
| `reserved_tones` | yes or no | optional | 1.0 | Reserved tones are in use. |
| `blocks_ok` | whole number | always | 1.0 | FEC blocks good. |
| `blocks_bad` | whole number | always | 1.0 | FEC blocks that failed. |
| `l1_bad` | whole number | always | 1.0 | L1 blocks that failed. |
| `sync_losses` | whole number | always | 1.0 | Losses of the frame sync. |
| `pre_ber` | number | optional | 1.0 | The bit error ratio before the LDPC decoder. |
| `mean_iterations` | number | optional | 1.0 | LDPC iterations, recent mean. |
| `c2_frames` | whole number | always | 1.0 | C2 frames received. |
| `chosen_slice` | whole number | optional | 1.0 | The data slice decoded. |
| `chosen_plp` | whole number | optional | 1.0 | The PLP decoded. |
| `common_plp` | whole number | optional | 1.0 | The common PLP of the PLP decoded. |
| `bundled_slices` | whole number | optional | 1.0 | The data slices that carry the PLP, when more than one. |
| `retune_hz` | frequency | optional | 1.0 | The frequency to tune to for the chosen data slice, when it is outside the window. |
| `outside_slice` | whole number | optional | 1.3 | The data slice that the decoding needs and that reaches out of the receive window. |
| `outside_hz` | frequency | optional | 1.3 | How far that data slice reaches beyond the edge of the receive window. |
| `generic_packets` | whole number | always | 1.0 | Packets of a generic stream. |
| `generic_bytes` | whole number | always | 1.0 | Bytes of a generic stream. |
| `note` | text | always | 1.0 | A note on what the receiver decodes. |

### Result fields

Struct `Dvbc2StreamFields`. The counts of the stream adaptation and the IP layer that `dvbc2` shows under its own keys.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ts_packets` | whole number | always | 1.0 | Transport packets of the chosen PLP. |
| `pdus` | whole number | always | 1.0 | GSE PDUs. |
| `ip_packets` | whole number | always | 1.0 | IP packets. |

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

### Table `slices`

Struct `Dvbc2SliceRow`. A row of the data slices of the L1. The row key is DSLICE_ID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `slice_id` | whole number | always | 1.0 | DSLICE_ID. |
| `slice_type` | choice | always | 1.0 | DSLICE_TYPE. |
| `header_type` | choice | always | 1.0 | The FECFrame header of a type 2 slice. |
| `tune_hz` | frequency | always | 1.0 | The tuning position of the slice. |
| `start_hz` | frequency | optional | 1.0 | The first carrier of the slice. |
| `end_hz` | frequency | optional | 1.0 | The last carrier of the slice. |
| `ti_depth` | whole number (symbols) | always | 1.0 | The depth of the time interleaver. |
| `plp_count` | whole number | always | 1.0 | The PLPs of the slice. |
| `in_window` | yes or no | always | 1.0 | The slice is inside the receive window. |
| `width_hz` | frequency | optional | 1.3 | The width of the slice. |

### Table `plps`

Struct `Dvbc2PlpRow`. A row of the PLPs of the L1. The row key is `<slice>/<PLP_ID>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plp_slice` | whole number | always | 1.0 | The data slice of the PLP. |
| `plp_id` | whole number | always | 1.0 | PLP_ID. |
| `plp_type` | choice | always | 1.0 | PLP_TYPE. |
| `payload` | choice | always | 1.0 | PLP_PAYLOAD_TYPE. |
| `fec_frame_bits` | whole number (bits) | optional | 1.0 | The length of the FEC frame. |
| `modulation` | choice | optional | 1.0 | The constellation. |
| `code_rate` | choice | optional | 1.0 | The code rate. |
| `ts_id` | whole number | optional | 1.0 | The transport stream id. No value for a PLP with PSI reprocessing. |
| `on_id` | whole number | optional | 1.0 | The original network id. No value for a PLP with PSI reprocessing. |
| `chosen` | yes or no | always | 1.0 | It is the PLP decoded. |
| `group` | whole number | optional | 1.3 | PLP_GROUP_ID. No value for a normal data PLP. |
| `bundled` | yes or no | always | 1.3 | PLP_BUNDLED: the PLP is spread over several data slices. |

### Table `notches`

Struct `Dvbc2NotchRow`. A row of the notches of the L1. The row key is the number of the notch.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `notch_start_hz` | frequency | optional | 1.0 | The first carrier of the notch. |
| `notch_end_hz` | frequency | optional | 1.0 | The last carrier of the notch. |

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
| `clear` | `sdrkit_dvbc2_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `plp` | `sdrkit_dvbc2_get_plp` | `sdrkit_dvbc2_set_plp` |
| `slice` | `sdrkit_dvbc2_get_slice` | `sdrkit_dvbc2_set_slice` |
| `centre_known` | `sdrkit_dvbc2_get_centre_known` | `sdrkit_dvbc2_set_centre_known` |
| `raster` | `sdrkit_dvbc2_get_raster` | `sdrkit_dvbc2_set_raster` |
| `ldpc_iterations` | `sdrkit_dvbc2_get_ldpc_iterations` | `sdrkit_dvbc2_set_ldpc_iterations` |
| `timing_metric` | `sdrkit_dvbc2_get_timing_metric` | `sdrkit_dvbc2_set_timing_metric` |
| `preamble_ratio` | `sdrkit_dvbc2_get_preamble_ratio` | `sdrkit_dvbc2_set_preamble_ratio` |
| `index_metric` | `sdrkit_dvbc2_get_index_metric` | `sdrkit_dvbc2_set_index_metric` |
| `index_span` | `sdrkit_dvbc2_get_index_span` | `sdrkit_dvbc2_set_index_span` |
| `header_candidate` | `sdrkit_dvbc2_get_header_candidate` | `sdrkit_dvbc2_set_header_candidate` |
| `header_agree` | `sdrkit_dvbc2_get_header_agree` | `sdrkit_dvbc2_set_header_agree` |
| `header_misses` | `sdrkit_dvbc2_get_header_misses` | `sdrkit_dvbc2_set_header_misses` |
| `block_misses` | `sdrkit_dvbc2_get_block_misses` | `sdrkit_dvbc2_set_block_misses` |
| `preamble_misses` | `sdrkit_dvbc2_get_preamble_misses` | `sdrkit_dvbc2_set_preamble_misses` |
| `ti_full_width` | `sdrkit_dvbc2_get_ti_full_width` | `sdrkit_dvbc2_set_ti_full_width` |
| `rm_scrambling` | `sdrkit_dvbc2_get_rm_scrambling` | `sdrkit_dvbc2_set_rm_scrambling` |
| `bundle_hold` | `sdrkit_dvbc2_get_bundle_hold` | `sdrkit_dvbc2_set_bundle_hold` |
| `decode_video` | `sdrkit_dvbc2_get_decode_video` | `sdrkit_dvbc2_set_decode_video` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_frequency` | `sdrkit_dvbc2_gen_get_frequency` | `sdrkit_dvbc2_gen_set_frequency` |
| `gen_modcod` | `sdrkit_dvbc2_gen_get_modcod` | `sdrkit_dvbc2_gen_set_modcod` |
| `gen_short_frames` | `sdrkit_dvbc2_gen_get_short_frames` | `sdrkit_dvbc2_gen_set_short_frames` |
| `gen_guard` | `sdrkit_dvbc2_gen_get_guard` | `sdrkit_dvbc2_gen_set_guard` |
| `gen_slice_type` | `sdrkit_dvbc2_gen_get_slice_type` | `sdrkit_dvbc2_gen_set_slice_type` |
| `gen_ti_depth` | `sdrkit_dvbc2_gen_get_ti_depth` | `sdrkit_dvbc2_gen_set_ti_depth` |
| `gen_l1_ti` | `sdrkit_dvbc2_gen_get_l1_ti` | `sdrkit_dvbc2_gen_set_l1_ti` |
| `gen_notch` | `sdrkit_dvbc2_gen_get_notch` | `sdrkit_dvbc2_gen_set_notch` |
| `gen_reserved_tones` | `sdrkit_dvbc2_gen_get_reserved_tones` | `sdrkit_dvbc2_gen_set_reserved_tones` |
| `gen_system` | `sdrkit_dvbc2_gen_get_system` | `sdrkit_dvbc2_gen_set_system` |
| `gen_stream` | `sdrkit_dvbc2_gen_get_stream` | `sdrkit_dvbc2_gen_set_stream` |
| `gen_hem` | `sdrkit_dvbc2_gen_get_hem` | `sdrkit_dvbc2_gen_set_hem` |
| `gen_payload` | `sdrkit_dvbc2_gen_get_payload` | `sdrkit_dvbc2_gen_set_payload` |

## Try it

```sh
sdrkit run dvbc2 --source synth:dvbc2
sdrkit run dvbc2 --source iiod:192.168.2.1 --freq 346
```
<!-- generated: end -->

## Description

DVB-C2 carries a cable network's multiplexes in one wide OFDM system: 4096 carriers in an 8 MHz raster (the carrier spacing is
2232 Hz; 1674 Hz in the 6 MHz raster), guard 1/64 or 1/128, and carrier indices counted from 0 Hz. A frame is one or more preamble symbols and 448 data
symbols. The preamble repeats the L1 signalling in every L1 block of 3408 carriers, so any receive window of 7.61 MHz holds all
of it. The data symbols carry data slices; each slice carries one or more PLPs, coded with BCH and 16K or 64K LDPC (the DVB-T2
codes) and mapped onto 16- to 4096-QAM.

The receiver needs the cable network as input, not an antenna. It works in these steps:

1. The samples are resampled to 64/7 Msps (48/7 Msps on the 6 MHz raster); the sample clock error is corrected in the
   resampler. When the chosen slice lies outside the window but inside the samples, the samples are first shifted in
   frequency so that the window sits on the slice's tuning position.
2. The guard correlation, folded over 8 symbol periods, gives the symbol timing, the guard interval and the fractional carrier
   offset.
3. A preamble shows by its pilots: every sixth carrier, not boosted, against the boosted scattered pilots of data symbols.
4. The preamble pilots give the absolute carrier index of the window: within 64 carriers of the tuned frequency, or, with
   `centre_known` off, by a search over every carrier position (about a second once).
5. The preamble header (Reed-Muller coded) gives the size of the L1 signalling; the L1 block cells are combined over their
   repetitions, de-interleaved, decoded (LDPC, BCH, CRC-32) and read.
6. Each data symbol: the common phase step of the absolute carrier grid is undone; the scattered, continual and edge pilots
   give the channel on a grid of D_X carriers, averaged over symbols and interpolated; the common phase and the timing slope
   drive the carrier and timing loops and the clock correction; the cells of the chosen data slice are equalised and frequency
   de-interleaved; the time de-interleaver gives the slice's cell stream.
7. The Data Slice Packets of the chosen PLP: in a type 1 slice the FEC blocks follow one another from PLP_START; in a type 2 slice
   each packet has a FECFrame header, found by its code structure and decoded with the Reed-Muller soft decoder. Several
   slices are decoded at once: every slice in the window that carries a bundled PLP, and the slice of the common PLP of a
   grouped data PLP.
8. A helper thread demaps each FEC block (per axis, with the weight of each cell), decodes LDPC and BCH and descrambles the
   BBFRAME. The BBFRAMEs of a bundled PLP are put back in the order of the ISCR in their headers.
9. Out of `dvbc2.receiver`: the BBFRAMEs of a transport stream or GSE PLP and of the common PLP of its group (`bbframes`,
   MATYPE-2 naming the PLP, to the mode adaptation `ipcast.bbframe` of `sdrkit-proto-ipcast`, normal or high efficiency
   mode), the pair of those PLPs (`plps`), and a generic stream (`generic`): the user packets of a GFPS (normal mode, CRC-8
   checked) or the data fields of a GCS.
10. `ipcast.bbframe` puts the common PLP back into the transport stream of the data PLP (EN 302 755 annex D): a null packet
   of the data PLP takes the packet of the common PLP in the same slot. The streams line up by packet count; when a slot holds
   a packet in both, the merge waits for the one offset that puts no two packets in a slot over 16 packets of the common PLP.

Slices outside the window. A slice is at most 3408 carriers wide (clause 9.4.1), so a window of 3521 carriers at its tuning
position always holds it. The decoding needs the slice of the chosen PLP and, for a grouped data PLP, the slice of the common
PLP of its group. When one of them reaches out of the window, the receiver tries the tuning positions that the L1 gives for
the two slices (DSLICE_TUNE_POS of the data slice first) and takes the first whose window holds both. It moves its window
there when the samples hold it (a sample rate wide enough for the system, such as 20 Msps for a system two L1 blocks wide);
otherwise it publishes the frequency (`retune_hz`) and the stack asks the host once to tune to it. The samples hold a position
only when the radio's filter passes it: with a filter of 8 MHz the move fails, and after a move that ends in a loss before an
L1 is decoded the receiver does not move again and asks for the tuning. When no tuning position of the L1 holds both slices,
the stack asks for nothing: `outside_slice` names the slice that reaches out of the window and `outside_hz` how far, and the
data PLP is decoded without its common PLP when its own slice is inside the window (its transport stream then has null
packets in the slots of the common PLP). Bundled slices outside the window are not decoded (a log line names them). The
line-up of the common PLP by ISSY is not done. GFPS and GCS in high efficiency mode are not read. After a move of the window the data symbols of the frame whose
L1 named the slice are lost (they come before the window has moved), so the stream starts one C2 frame later. Slices next to a broadband notch use the same carrier rules as other notches;
no test covers them.

Standard: ETSI EN 302 769 V1.3.1; ETSI EN 302 755 for the LDPC and BCH codes it shares.

## Tuning: notes

Tune to the tuning position of the data slice (DSLICE_TUNE_POS), or to any frequency whose 7.61 MHz window holds the slice.
The receiver uses the carriers within 1760 of the carrier at the window centre (3.93 MHz either side). The
stack must know the RF centre frequency of its samples, since carrier indices, pilots, scrambling and L1 blocks are absolute;
without it (`centre_known` off) the full search runs. Cable networks, their START_FREQUENCY and the tuning positions are learned
from the L1; the stack holds no channel list.

## Settings: notes

| Key | Label | Kind | Default | Range or options |
|---|---|---|---|---|
| `plp` | PLP | whole number | -1 | PLP_ID 0 to 255, as the table `plps` lists it; -1 the first data PLP of the first slice inside the window |
| `slice` | Data slice | whole number | -1 | DSLICE_ID 0 to 255; -1 the first slice inside the window that carries the PLP |
| `centre_known` | Centre frequency known | bool | true | off: the full carrier index search |
| `raster` | Channel raster | choice | `8` | `8`, `6`; the test signal follows it |

Receiver thresholds (advanced; no public document gives them, see below):

| Key | Label | Default | Range |
|---|---|---|---|
| `ldpc_iterations` | LDPC iterations | 50 | 1 to 200 |
| `timing_metric` | Timing metric | 0.22 | 0.01 to 1 |
| `preamble_ratio` | Preamble ratio | 3 | 1 to 10 |
| `index_metric` | Carrier index metric | 0.35 | 0.05 to 1 |
| `index_span` | Carrier index span | 64 | 0 to 1000 |
| `header_candidate` | Header candidate | 0.45 | 0.05 to 1 |
| `header_agree` | Header agreement | 22 | 16 to 32 |
| `header_misses` | Header misses | 2 | 1 to 100 |
| `block_misses` | Block misses | 8 | 1 to 1000 |
| `preamble_misses` | Preamble misses | 3 | 1 to 100 |
| `ti_full_width` | Time interleaver over every carrier | true | off: one column fewer |
| `rm_scrambling` | Header scrambling word | 0x1D8D3BF3 (495795187) | 0 to 2^32 - 1 |
| `bundle_hold` | Bundled PLP hold | 16 frames | 1 to 1000 |

The test signal (`gen_*`):

| Key | Label | Default | Options |
|---|---|---|---|
| `gen_frequency` | Centre frequency | 346 MHz | 47 to 1002 MHz |
| `gen_modcod` | Coding | `256_5_6` | `256_5_6`, `16_4_5`, `64_2_3`, `256_3_4`, `64_9_10`, `256_9_10`, `1024_3_4`, `4096_5_6` |
| `gen_short_frames` | 16K FEC frames | false | |
| `gen_guard` | Guard interval | `1_64` | `1_64`, `1_128` |
| `gen_slice_type` | Data slice type | `type1` | `type1`, `type2_robust`, `type2_efficient` |
| `gen_ti_depth` | Time interleaving | `none` | `none`, `4`, `8`, `16` |
| `gen_l1_ti` | L1 time interleaving | `none` | `none`, `best_fit`, `4`, `8` |
| `gen_notch` | Narrowband notch | false | 35 carriers at 1/64, 23 at 1/128, at half the tuning position |
| `gen_reserved_tones` | Reserved tones | false | |
| `gen_system` | Slices and PLPs | `single` | `single`; `two_plps` (one type 2 slice, PLP 1 and 2 alternate); `common` (one type 2 slice: grouped data PLP 1 and common PLP 9); `bundled` (two type 2 slices that both carry PLP 1, high efficiency mode with ISSY); `two_slices` (two L1 blocks wide, PLP 1 in the lower slice, PLP 2 in the upper) |
| `gen_stream` | Stream | `ts` | `ts`, `gse` (UDP packets of 40 to 3000 bytes), `gfps` (96-byte packets, sync byte 0xB8, numbered), `gcs` (32-bit counters) |
| `gen_hem` | High efficiency mode | false | transport stream and GSE |
| `gen_payload` | Payload | `programme` | `programme`, `numbered` |

## Results

The tables and events of the transport stream layer, the counters of the transport stream monitor, and:

| Key | Label | Kind |
|---|---|---|
| `state` | State: searching, timing found, decoding the L1, data | choice |
| `guard` | Guard interval | choice |
| `offset_hz` | Carrier offset | Hz |
| `clock_ppm` | Clock error (the correction applied) | ppm |
| `window_carrier`, `window_hz` | The carrier at the window centre, and its frequency | whole number, Hz |
| `esn0_db` | Es/N0 of the data cells, from the pilots | dB |
| `mer_db` | MER of the decoded cells against the nearest points | dB |
| `network_id`, `system_id` | NETWORK_ID, C2_SYSTEM_ID | whole number |
| `start_frequency_hz`, `bandwidth_hz` | START_FREQUENCY and the system width | Hz |
| `change_counter`, `c2_version` | L1_PART2_CHANGE_COUNTER, C2_VERSION | whole number |
| `ews`, `reserved_tones` | EWS, RESERVED_TONE | bool |
| `blocks_ok`, `blocks_bad` | FEC blocks good, bad | whole number |
| `l1_bad` | L1 failures | whole number |
| `sync_losses` | Returns to a search and losses of the packet alignment | whole number |
| `pre_ber` | Bit error ratio before LDPC | number |
| `mean_iterations` | LDPC iterations (mean) | number |
| `c2_frames` | C2 frames received | whole number |
| `chosen_slice`, `chosen_plp` | The slice and PLP decoded | whole number |
| `common_plp` | The common PLP decoded with the chosen PLP | whole number |
| `bundled_slices` | The slices that carry the chosen PLP when it is bundled | whole number |
| `retune_hz` | The frequency to tune to when the chosen slice lies outside the samples | Hz |
| `generic_packets`, `generic_bytes` | Packets and bytes of the generic stream output | whole number |
| `ts_packets`, `pdus`, `ip_packets` | Transport packets, GSE PDUs and IP packets of the mode adaptation | whole number |
| `note` | What is decoded and which values no public document confirms | text |

Tables: `slices` (id, type, header type, tuning position, start and end, time interleaving depth, PLP count, inside the
window), `plps` (slice, PLP id, type, payload type, FEC frame, constellation and code rate from the L1 or the last FECFrame
header, TS id, ON id, decoded) and `notches` (start and end).

## Values that no public document confirms

These values are settings with their default; the `note` field names them:

- The guard correlation metric 0.22 over 8 symbol periods (`timing_metric`).
- The scattered pilot energy ratio 3 below which a symbol is a preamble (`preamble_ratio`).
- The carrier index: metric 0.35 (`index_metric`) over 64 carriers either side of the nominal index (`index_span`).
- The FECFrame header: candidate measure 0.45 (`header_candidate`), 22 of 32 code bits agreeing (`header_agree`), 2 failed
  headers in a row lose the lock (`header_misses`). The search takes a header only when the packet it names is followed by
  another header that passes the same two tests: cells of data pass them now and then, and a header taken from them would
  cost the packets up to the next loss of the lock.
- 8 failed FEC blocks in a row drop the alignment of a type 1 slice (`block_misses`).
- 3 frames without a preamble start the timing search again (`preamble_misses`).
- At most 50 LDPC iterations (`ldpc_iterations`).
- 16 BBFRAMEs of a bundled PLP held to restore their ISCR order (`bundle_hold`).
- The time interleaver of a slice has one column per carrier, N_DSLICE_WIDTH = K_DS,max - K_DS,min + 1 (`ti_full_width`).
  Clause 9.4.4 prints K_DS,max - K_DS,min, but clause 9.4.1 makes K_DS,max the last carrier of the slice (it subtracts 1),
  an empty slice has OFFSET_LEFT = OFFSET_RIGHT, and the memory holds every carrier of the slice: so the width is
  D_X (OFFSET_RIGHT - OFFSET_LEFT). `ti_full_width` off follows the printed formula, and the last carrier carries no data.
- The word w^RM that scrambles the lower branch of the Reed-Muller headers (`rm_scrambling`): 0x1D8D3BF3, the order of the
  bits as the printed page of clause 7.2.2.3 shows them. Only one source was read.

The scrambling sequences are those of clauses 9.2.1 and 9.2.2 (figures 31 and 32): w from all ones with feedback stage 11 xor
stage 9 (w_(n+11) = w_(n+2) xor w_n; the register of the DVB-T and DVB-T2 pilots), w' from all ones with feedback stage 10 xor
stage 7 (w'_(n+10) = w'_(n+3) xor w'_n). w starts 11111111111000000000110000000111, w' starts 11111111110000000111000011111101.
The lower branch of a Reed-Muller header is the code word delayed cyclically by 2, u_((i+2) mod 32) = lambda_i, then scrambled
with w^RM.

Choices of the stack that EN 302 769 does not state:

- The LDPC address tables of annexes A and B are taken to equal those of DVB-T2 and DVB-S2 (only the first row of table A.1
  was compared).
- Tables 28 and 29 (L1 shortening and puncturing) are taken to equal the DVB-T2 L1-post 16-QAM orders, and table 32 the DVB-T2
  4K frequency interleaver permutations.
- The tuning range 47 to 1002 MHz and the default frequency 346 MHz.
- The default sample rate 10 Msps, the least rate 8 Msps (6 Msps on the 6 MHz raster), the clock correction in the resampler.
- The receiver thresholds and algorithms above.
- The target quality figures: the implementation guidelines (TS 102 991) were not read.

Interpretations: PLP_START counts the slice's cells after time de-interleaving from the first data cell of the frame; in a
preamble symbol the 32 header cells and the L1 row repeat together until the 2840 cells of the L1 block are filled; L1 time
interleaving with more FEC blocks than its depth is refused; a failed L1 keeps the last good one for one frame, and a second
failure in a row starts the preamble search. A bundled PLP is ordered by the ISCR (ISSY in the BBHEADER of high efficiency mode);
frames without ISSY go out as they come. The common PLP of a grouped data PLP is the PLP of type 00 with the same PLP_GROUP_ID.
None of these rules for bundling and the common PLP was checked against the standard's text.

The carrier at 0 Hz of the samples carries what is left of the DC offset: its cell is erased and its pilot is not used.

## Test signal

One system one L1 block wide (3409 carriers; two blocks with `gen_system` `two_slices` and `far_slice`), centred on the carrier at or below the
frequency of `gen_frequency` that is a multiple of D_X, with the slices and PLPs of `gen_system`. The data PLPs carry the test
multiplex, or numbered packets, or GSE with UDP packets of 40 to 3000 bytes, or a GFPS or GCS. For the common PLP about one
transport packet in four (by a SplitMix64 hash of its index) moves to PLP 9 and leaves a null packet in PLP 1. A bundled PLP's BBFRAMEs go to the two slices as their cells
run out, with ISCRlong values that rise by 1000 per BBFRAME.

`mixed_slices` has a type 1 slice with PLP 1 and a type 2 slice with PLP 2, half an L1 block each; `gen_slice_type` chooses the
header of the type 2 slice. `group` has a type 2 slice with the grouped data PLPs 1 and 2 and a type 2 slice with the common
PLP 9 of their group. `far_slice` is that group over two L1 blocks: PLP 1 over the lower block, whose centre is at
`gen_frequency`; PLP 9 in the first 432 carriers of the upper block and PLP 2 in the rest of it. The window at the tuning
position of the slice of PLP 2 holds that slice and the common PLP; no window holds the slices of PLP 1 and PLP 9. In `group`
and `far_slice` each data PLP has an input stream of numbered packets on its own PID (0x0020, 0x0021) with, in about one slot
in four, the numbered packets of PID 0x002F that both streams share; these move to PLP 9. The three PLPs send one FEC block
each per round, so that they run at one packet rate, and stuffing packets fill what a slice has left. The generator
places the signal itself: its samples are centred on `gen_frequency`. Impairments come from the framework's `gen_*`
settings; the stated symbol rate makes `gen_esn0_db` the Es/N0 of a data cell.

## Speed

The receiver's thread resamples, synchronises, equalises and de-interleaves; a helper thread demaps and decodes the FEC blocks
behind a bounded queue. The demapper works per axis in a few operations per bit (`sdrkit_algo_modem::graydemap`), and a FEC
block whose LDPC result is a BCH code word needs no BCH decoding. `sdrkit bench dvbc2` measures the receiver on its test
signal at 10 Msps (64K 256-QAM 5/6, 30 dB).

## Try it: notes

```sh
sdrkit run dvbc2 --source synth:dvbc2
sdrkit run dvbc2 gen_modcod=4096_5_6 gen_short_frames=true gen_slice_type=type2_robust --source synth:dvbc2
sdrkit run dvbc2 raster=6 --source synth:dvbc2
sdrkit run dvbc2 gen_system=bundled --source synth:dvbc2
sdrkit run dvbc2 gen_system=group plp=2 --source synth:dvbc2
sdrkit run dvbc2 gen_system=far_slice plp=2 --source synth:dvbc2
sdrkit verify dvbc2
```
