<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DVB-S/S2

Satellite television and data: DVB-S and DVB-S2 (transport streams, GSE and IP), services, guide and media. Transmits a DVB-S or DVB-S2 transport stream that the host gives.

Stack id: `dvbs`. Group: TV.

Signal types: DVB-S, DVB-S2.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.5 GHz |
| Tuning range | 10 MHz to 6 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dvbs` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 30 MHz | 0: automatic. |
| `standard` | Standard | choice | `auto` | `auto` (Auto), `dvbs` (DVB-S only), `dvbs2` (DVB-S2 only) | Auto tries DVB-S2 and DVB-S. Choose one to skip the other search on a weak signal. |
| `roll_off` | Roll-off | choice | `auto` | `auto` (Auto), `0.35`, `0.25`, `0.20`, `0.15`, `0.10`, `0.05` |  |
| `isi` | Input stream | whole number | -1 | -1 to 255 | -1: the first stream seen. |
| `pl_scrambling` | PL scrambling code | whole number | 0 | 0 to 262141 |  |
| `carrier_pick` | Carrier | text | empty |  | Offset in Hz, empty for automatic (set by clicking a carrier in the survey). |
| `ip_pcap` | PCAP output | yes or no | false |  | Send every IP packet as PCAP file bytes. The host writes the file. |
| `tx_polarisation` | Transponder | choice | `unset` | `unset` (Not set), `v` (Vertical), `h` (Horizontal) |  |
| `lnb_supply` | LNB supply | choice | `unset` | `unset` (Not set), `v` (Vertical (about 13 V)), `h` (Horizontal (about 18 V)) |  |
| `decode_video` | Decode video | yes or no | false |  | MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures. |
| `tx_symbol_rate` | Symbol rate | frequency | 5 MHz | 25 kHz to 60 MHz | The symbol rate sent. The session's rate must be at least 1.3 × (1 + roll-off) times it: 1.755 for roll-off 0.35. |
| `tx_rate` | Code rate | choice | `2/3` | `1/2`, `2/3`, `3/4`, `5/6`, `7/8` | DVB-S: the code rate of the convolutional code, 1/2, 2/3, 3/4, 5/6 or 7/8. DVB-S2 takes its code rate from tx_modcod. |
| `tx_inversion` | Spectral inversion | yes or no | false |  | Sends the spectrum mirrored (Q negated). |
| `tx_standard` | Standard | choice | `dvbs` | `dvbs2` (DVB-S2), `dvbs` (DVB-S) | The standard sent: DVB-S (EN 300 421) or DVB-S2 (EN 302 307-1). |
| `tx_modcod` | MODCOD | choice | `QPSK-2/3` | `QPSK-1/4` (QPSK 1/4), `QPSK-1/3` (QPSK 1/3), `QPSK-2/5` (QPSK 2/5), `QPSK-1/2` (QPSK 1/2), `QPSK-3/5` (QPSK 3/5), `QPSK-2/3` (QPSK 2/3), `QPSK-3/4` (QPSK 3/4), `QPSK-4/5` (QPSK 4/5), `QPSK-5/6` (QPSK 5/6), `QPSK-8/9` (QPSK 8/9), `QPSK-9/10` (QPSK 9/10), `8PSK-3/5` (8PSK 3/5), `8PSK-2/3` (8PSK 2/3), `8PSK-3/4` (8PSK 3/4), `8PSK-5/6` (8PSK 5/6), `8PSK-8/9` (8PSK 8/9), `8PSK-9/10` (8PSK 9/10), `16APSK-2/3` (16APSK 2/3), `16APSK-3/4` (16APSK 3/4), `16APSK-4/5` (16APSK 4/5), `16APSK-5/6` (16APSK 5/6), `16APSK-8/9` (16APSK 8/9), `16APSK-9/10` (16APSK 9/10), `32APSK-3/4` (32APSK 3/4), and others | DVB-S2: the modulation and the code rate of every frame. |
| `tx_frame` | FEC frame | choice | `normal` | `normal` (Normal), `short` (Short) | DVB-S2: normal frames (64 800 bits) or short frames (16 200 bits). Code rate 9/10 has no short frame. |
| `tx_pilots` | Pilots | yes or no | false |  | DVB-S2: a block of 36 pilot symbols after every 16 slots. |
| `tx_rolloff` | Roll-off | choice | `0.35` | `0.35`, `0.25`, `0.20` | DVB-S2: the roll-off of the pulse. DVB-S has 0.35. |
| `tx_pl_scrambling` | PL scrambling code | whole number | 0 | 0 to 262141 | DVB-S2: the number of the Gold code of the PL scrambling. 0 for broadcast. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `modcod_rule` | MODCODs taken | choice | `any` | `any` (Any), `fixed` (Fixed), `set` (Set) | Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded. |
| `modcod` | MODCOD | whole number | 0 | 0 to 255 |  |
| `modcods` | MODCOD set | text | empty |  | MODCOD numbers with commas between them, for the rule "set". |
| `frame_length` | Frame length | choice | `any` | `any` (Any), `normal` (Normal), `short` (Short) | Frames of the other length are counted and skipped. |
| `pilots` | Pilots | choice | `any` | `any` (Any), `on` (On), `off` (Off) | A check only: a frame with other pilots is counted and decoded. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_standard` | Standard | choice | `dvbs2` | `dvbs2` (DVB-S2), `dvbs` (DVB-S) |  |
| `gen_modulation` | Modulation | choice | `QPSK` | `QPSK`, `8PSK`, `16APSK`, `32APSK` |  |
| `gen_rate` | Code rate | choice | `2/3` | `1/4`, `1/3`, `2/5`, `1/2`, `3/5`, `2/3`, `3/4`, `4/5`, `5/6`, `7/8`, `8/9`, `9/10` |  |
| `gen_short` | Short frames | yes or no | false |  |  |
| `gen_pilots` | Pilots | yes or no | false |  |  |
| `gen_rolloff` | Roll-off | choice | `0.35` | `0.35`, `0.25`, `0.20` |  |
| `gen_symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 30 MHz | 0: 5 Msym/s or what the sample rate allows. |
| `gen_pl_scrambling` | PL scrambling code | whole number | 0 | 0 to 262141 |  |
| `gen_dummy_every` | A dummy frame every | whole number (frames) | 0 | 0 to 1000 |  |
| `gen_gse` | GSE (IP) instead of a transport stream | yes or no | false |  |  |
| `gen_multistream` | Two input streams (TS and GSE) | yes or no | false |  |  |
| `gen_payload` | Payload | choice | `programme` | `programme` (Test multiplex), `numbered` (Numbered packets) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `standard` | Standard | choice |
| `signal_note` | Note | choice |
| `symbol_rate_hz` | Symbol rate | frequency |
| `spectrum_rate` | Symbol rate from the spectrum | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `manual_rate` | Manual symbol rate | yes or no |
| `spectrum_snr_db` | Carrier over floor | number (dB) |
| `misfit` | Fit misfit | number |
| `edge_low` | Lower carrier edge | frequency |
| `edge_high` | Upper carrier edge | frequency |
| `offset_hz` | Offset | frequency |
| `inverted` | Spectrum inverted | yes or no |
| `roll_off` | Roll-off | number |
| `roll_off_source` | Roll-off from | choice |
| `modulation` | Modulation | choice |
| `code_rate` | Code rate | choice |
| `modcod` | MODCOD | whole number |
| `frame` | FEC frame | choice |
| `pilots` | Pilots | yes or no |
| `vcm` | VCM or ACM | yes or no |
| `single_stream` | Single stream | yes or no |
| `pl_scrambling_code` | PL scrambling code | whole number |
| `plframes` | PLFRAMEs | whole number |
| `dummy_frames` | Dummy frames | whole number |
| `unsupported_frames` | Frames not decoded | whole number |
| `other_modcod_frames` | Frames of another MODCOD | whole number |
| `pilot_mismatch` | Frames with other pilots | whole number |
| `bbheader_bad` | BBHEADER failures | whole number |
| `net_bit_rate` | Net bit rate | number (bit/s) |
| `esn0_db` | Es/N0 | number (dB) |
| `mer_db` | MER | number (dB) |
| `ebn0_db` | Eb/N0 | number (dB) |
| `margin_to_qef_db` | Margin to QEF | number (dB) |
| `ldpc_iterations` | LDPC iterations | number |
| `bch_ok` | BCH good | whole number |
| `bch_bad` | BCH failed | whole number |
| `rs_clean` | RS words clean | whole number |
| `rs_corrected` | RS words corrected | whole number |
| `rs_failed` | RS words failed | whole number |
| `carrier_lock` | Carrier lock | yes or no |
| `timing_lock` | Timing lock | yes or no |
| `frame_lock` | Frame lock | yes or no |
| `fec_lock` | FEC lock | yes or no |
| `ts_lock` | TS lock | yes or no |
| `level_dbfs` | Level | number (dBFS) |
| `lnb_mismatch` | LNB on the other polarisation | yes or no |
| `packets` | Packets | whole number |
| `flagged` | Uncorrectable packets | whole number |
| `scrambled` | Scrambled packets | whole number |
| `numbered_good` | Test packets good | whole number |
| `numbered_wrong` | Test packets wrong | whole number |
| `numbered_gaps` | Test packet gaps | whole number |
| `bitrate_bps` | Bit rate | number (bit/s) |
| `pat_seen` | PAT seen | yes or no |
| `services` | Services | text |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_bitrate_bps` | Useful bit rate | number (bit/s) |
| `tx_packets` | Packets sent | whole number |
| `tx_nulls` | Null packets inserted | whole number |
| `tx_queue` | Packets waiting | whole number |
| `tx_queue_s` | Queue length | duration (s) |
| `tx_pcr_restamped` | PCRs rewritten | whole number |
| `bbframe.frames_ok` | Frames good | whole number |
| `bbframe.frames_bad` | Frames bad | whole number |
| `bbframe.ts_packets` | TS packets | whole number |
| `bbframe.crc_errors` | CRC errors | whole number |
| `bbframe.pdus` | PDUs | whole number |
| `bbframe.non_ip` | Non-IP PDUs | whole number |
| `ip.ip_packets` | IP packets | whole number |
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
| `carriers` | Carriers | `offset` (frequency), `symbol_rate` (frequency), `roll_off` (number), `snr_db` (number (dB)), `level_db` (number (dB)), `decoded` (yes or no) |
| `streams` | Input streams | `isi` (whole number), `carries` (choice), `packet_bytes` (whole number (bytes)), `sync_byte` (whole number), `ccm` (yes or no), `frames` (whole number), `bits` (whole number) |
| `stream_modcods` | MODCODs of the input streams | `stream` (text), `modcod` (whole number), `modulation` (choice), `code_rate` (choice), `frames` (whole number), `share_pct` (number (%)) |
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
| `mer_history` | MER history | frame | dB |

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
| `start` | Start sending |  |  | Sends a DVB-S signal (EN 300 421) or a DVB-S2 signal (EN 302 307-1), as tx_standard says, with the tx_* settings, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. |
| `stop` | Stop sending |  |  | Ends the transmission after the group of eight packets on air (DVB-S) or the PLFRAME on air (DVB-S2) with a fall of 5 ms. The packets that wait are dropped. |
| `append` | Add packets | `packets` (bytes, required) |  | Adds transport packets to the queue of the stream sent. All or none: refused when the queue has no room, so the host feeds at the useful bit rate. |

## Typed interface

The stack is in the family `dvbcs`, version 1.5. The C header `sdrkit/dvbcs.h`, the registry record `registry/dvbcs.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Television and video (`television`). Media: sound (programme), video.

### Result fields

Struct `Dvbs2Fields`. The result fields of the DVB-S/S2 receiver: the carrier search, the frames, the code and the locks. A measurement of the carrier search has no value without a carrier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `standard` | choice | optional | 1.0 | The standard received. |
| `signal_note` | choice | optional | 1.0 | A note on the signal for the user. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate in use. No value before a rate is set or measured. |
| `spectrum_rate` | frequency | optional | 1.0 | The symbol rate from the spectrum (width or cyclic line). |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the symbol clock. |
| `manual_rate` | yes or no | always | 1.0 | The symbol rate was set by hand. |
| `spectrum_snr_db` | number (dB) | optional | 1.0 | The plateau of the carrier over the noise floor. |
| `misfit` | number | optional | 1.0 | The misfit of the raised cosine fit. |
| `edge_low` | frequency | optional | 1.0 | The lower -3 dB edge of the carrier, from the centre. |
| `edge_high` | frequency | optional | 1.0 | The upper -3 dB edge of the carrier, from the centre. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset from the centre. No value without a carrier. |
| `inverted` | yes or no | always | 1.0 | The spectrum is inverted. |
| `roll_off` | number | always | 1.0 | The roll-off. |
| `roll_off_source` | choice | always | 1.0 | Where the roll-off came from. |
| `modulation` | choice | optional | 1.0 | The modulation. |
| `code_rate` | choice | optional | 1.0 | The code rate. |
| `modcod` | whole number | optional | 1.0 | The MODCOD (DVB-S2). |
| `frame` | choice | optional | 1.0 | The FEC frame size (DVB-S2). |
| `pilots` | yes or no | always | 1.0 | Pilots are on. |
| `vcm` | yes or no | always | 1.0 | The MODCOD changes between frames. |
| `single_stream` | yes or no | always | 1.0 | The carrier has one input stream. |
| `pl_scrambling_code` | whole number | always | 1.0 | The PL scrambling code in use. |
| `plframes` | whole number | always | 1.0 | PLFRAMEs seen. |
| `dummy_frames` | whole number | always | 1.0 | Dummy PLFRAMEs seen. |
| `unsupported_frames` | whole number | always | 1.0 | Frames followed but not decoded. |
| `other_modcod_frames` | whole number | always | 1.0 | Frames skipped by the MODCOD rule or the frame length rule. |
| `pilot_mismatch` | whole number | always | 1.0 | Frames whose pilots differ from the setting. |
| `bbheader_bad` | whole number | always | 1.0 | BBHEADERs refused (CRC-8 or length). |
| `net_bit_rate` | number (bit/s) | always | 1.0 | The net bit rate of the data fields. |
| `esn0_db` | number (dB) | optional | 1.0 | Es/N0. |
| `mer_db` | number (dB) | optional | 1.0 | The modulation error ratio. |
| `ebn0_db` | number (dB) | optional | 1.0 | Eb/N0. |
| `margin_to_qef_db` | number (dB) | optional | 1.0 | Es/N0 above the quasi-error-free level of EN 302 307-1 table 13. |
| `ldpc_iterations` | number | always | 1.0 | LDPC iterations, recent mean. |
| `bch_ok` | whole number | always | 1.0 | BCH words good. |
| `bch_bad` | whole number | always | 1.0 | BCH words that failed. |
| `rs_clean` | whole number | always | 1.0 | Reed-Solomon words with no error (DVB-S). |
| `rs_corrected` | whole number | always | 1.0 | Reed-Solomon words corrected (DVB-S). |
| `rs_failed` | whole number | always | 1.0 | Reed-Solomon words that failed (DVB-S). |
| `carrier_lock` | yes or no | always | 1.0 | A carrier is found. |
| `timing_lock` | yes or no | always | 1.0 | The symbol timing runs. |
| `frame_lock` | yes or no | always | 1.0 | The carrier phase and the framing hold. |
| `fec_lock` | yes or no | always | 1.0 | The FEC decodes words. |
| `ts_lock` | yes or no | always | 1.0 | The transport stream is valid. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `lnb_mismatch` | yes or no | always | 1.0 | The LNB supply selects the other polarisation than the transponder's. |

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

### Result fields

Struct `DvbsTxFields`. The result fields of the sender `transmit` of `dvbs`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.2 | Whether a transmission is under way. |
| `tx_freq_hz` | frequency | optional | 1.2 | The transmit frequency: the centre of the carrier sent. No value while the stack does not transmit. |
| `tx_bitrate_bps` | number (bit/s) | always | 1.2 | The bit rate of the transport stream that the settings carry: the rate that the host feeds at most. |
| `tx_packets` | whole number | always | 1.2 | The packets of the host sent in this transmission, or in the last one. |
| `tx_nulls` | whole number | always | 1.2 | The null packets put in slots that found no packet of the host. |
| `tx_queue` | whole number | always | 1.2 | The packets that wait to be sent. |
| `tx_queue_s` | duration (s) | always | 1.2 | The time that the waiting packets take at the useful bit rate. |
| `tx_pcr_restamped` | whole number | always | 1.2 | The PCRs rewritten to the time of their slot. |

### Table `carriers`

Struct `Dvbs2CarrierRow`. A row of the carriers of the survey. The row key is the offset in kHz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset` | frequency | always | 1.0 | The offset of the carrier from the centre. |
| `symbol_rate` | frequency | always | 1.0 | Its symbol rate. |
| `roll_off` | number | always | 1.0 | Its roll-off. |
| `snr_db` | number (dB) | always | 1.0 | Its plateau over the floor. |
| `level_db` | number (dB) | always | 1.0 | Its level against the strongest. |
| `decoded` | yes or no | always | 1.0 | It is the carrier decoded. |

### Table `streams`

Struct `Dvbs2StreamRow`. A row of the input streams. The row key is the ISI.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `isi` | whole number | always | 1.0 | The input stream identifier. |
| `carries` | choice | always | 1.0 | What the stream carries. |
| `packet_bytes` | whole number (bytes) | optional | 1.0 | The user packet length. No value for a stream without user packets. |
| `sync_byte` | whole number | optional | 1.0 | The sync byte of the user packets. |
| `ccm` | yes or no | always | 1.0 | Constant coding and modulation. |
| `frames` | whole number | always | 1.0 | Frames of the stream. |
| `bits` | whole number | always | 1.0 | Data field bits of the stream. |

### Table `stream_modcods`

Struct `Dvbs2StreamModcodRow`. A row of the MODCODs of the input streams. The row key is `<isi>/<modcod>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `stream` | text | always | 1.0 | The input stream, a row key of the stream table. |
| `modcod` | whole number | always | 1.0 | The MODCOD. |
| `modulation` | choice | always | 1.0 | The modulation of the MODCOD. |
| `code_rate` | choice | optional | 1.0 | The code rate of the MODCOD. |
| `frames` | whole number | always | 1.0 | The frames of the stream with this MODCOD. |
| `share_pct` | number (%) | always | 1.0 | The share of the stream's frames with this MODCOD. |

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
| `clear` | `sdrkit_dvbs_clear` | none | none |
| `start` | `sdrkit_dvbs_start` | none | none |
| `stop` | `sdrkit_dvbs_stop` | none | none |
| `append` | `sdrkit_dvbs_append` | `sdrkit_dvbs_tx_append_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_dvbs_get_symbol_rate` | `sdrkit_dvbs_set_symbol_rate` |
| `standard` | `sdrkit_dvbs_get_standard` | `sdrkit_dvbs_set_standard` |
| `roll_off` | `sdrkit_dvbs_get_roll_off` | `sdrkit_dvbs_set_roll_off` |
| `isi` | `sdrkit_dvbs_get_isi` | `sdrkit_dvbs_set_isi` |
| `pl_scrambling` | `sdrkit_dvbs_get_pl_scrambling` | `sdrkit_dvbs_set_pl_scrambling` |
| `carrier_pick` | `sdrkit_dvbs_get_carrier_pick` | `sdrkit_dvbs_set_carrier_pick` |
| `modcod_rule` | `sdrkit_dvbs_get_modcod_rule` | `sdrkit_dvbs_set_modcod_rule` |
| `modcod` | `sdrkit_dvbs_get_modcod` | `sdrkit_dvbs_set_modcod` |
| `modcods` | `sdrkit_dvbs_get_modcods` | `sdrkit_dvbs_set_modcods` |
| `frame_length` | `sdrkit_dvbs_get_frame_length` | `sdrkit_dvbs_set_frame_length` |
| `pilots` | `sdrkit_dvbs_get_pilots` | `sdrkit_dvbs_set_pilots` |
| `ip_pcap` | `sdrkit_dvbs_get_ip_pcap` | `sdrkit_dvbs_set_ip_pcap` |
| `tx_polarisation` | `sdrkit_dvbs_get_tx_polarisation` | `sdrkit_dvbs_set_tx_polarisation` |
| `lnb_supply` | `sdrkit_dvbs_get_lnb_supply` | `sdrkit_dvbs_set_lnb_supply` |
| `decode_video` | `sdrkit_dvbs_get_decode_video` | `sdrkit_dvbs_set_decode_video` |
| `tx_symbol_rate` | `sdrkit_dvbs_get_tx_symbol_rate` | `sdrkit_dvbs_set_tx_symbol_rate` |
| `tx_rate` | `sdrkit_dvbs_get_tx_rate` | `sdrkit_dvbs_set_tx_rate` |
| `tx_inversion` | `sdrkit_dvbs_get_tx_inversion` | `sdrkit_dvbs_set_tx_inversion` |
| `tx_standard` | `sdrkit_dvbs_get_tx_standard` | `sdrkit_dvbs_set_tx_standard` |
| `tx_modcod` | `sdrkit_dvbs_get_tx_modcod` | `sdrkit_dvbs_set_tx_modcod` |
| `tx_frame` | `sdrkit_dvbs_get_tx_frame` | `sdrkit_dvbs_set_tx_frame` |
| `tx_pilots` | `sdrkit_dvbs_get_tx_pilots` | `sdrkit_dvbs_set_tx_pilots` |
| `tx_rolloff` | `sdrkit_dvbs_get_tx_rolloff` | `sdrkit_dvbs_set_tx_rolloff` |
| `tx_pl_scrambling` | `sdrkit_dvbs_get_tx_pl_scrambling` | `sdrkit_dvbs_set_tx_pl_scrambling` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_standard` | `sdrkit_dvbs_gen_get_standard` | `sdrkit_dvbs_gen_set_standard` |
| `gen_modulation` | `sdrkit_dvbs_gen_get_modulation` | `sdrkit_dvbs_gen_set_modulation` |
| `gen_rate` | `sdrkit_dvbs_gen_get_rate` | `sdrkit_dvbs_gen_set_rate` |
| `gen_short` | `sdrkit_dvbs_gen_get_short` | `sdrkit_dvbs_gen_set_short` |
| `gen_pilots` | `sdrkit_dvbs_gen_get_pilots` | `sdrkit_dvbs_gen_set_pilots` |
| `gen_rolloff` | `sdrkit_dvbs_gen_get_rolloff` | `sdrkit_dvbs_gen_set_rolloff` |
| `gen_symbol_rate` | `sdrkit_dvbs_gen_get_symbol_rate` | `sdrkit_dvbs_gen_set_symbol_rate` |
| `gen_pl_scrambling` | `sdrkit_dvbs_gen_get_pl_scrambling` | `sdrkit_dvbs_gen_set_pl_scrambling` |
| `gen_dummy_every` | `sdrkit_dvbs_gen_get_dummy_every` | `sdrkit_dvbs_gen_set_dummy_every` |
| `gen_gse` | `sdrkit_dvbs_gen_get_gse` | `sdrkit_dvbs_gen_set_gse` |
| `gen_multistream` | `sdrkit_dvbs_gen_get_multistream` | `sdrkit_dvbs_gen_set_multistream` |
| `gen_payload` | `sdrkit_dvbs_gen_get_payload` | `sdrkit_dvbs_gen_set_payload` |

## Try it

```sh
sdrkit run dvbs --source synth:dvbs
sdrkit run dvbs --source iiod:192.168.2.1 --freq 1500
```
<!-- generated: end -->

## Receive: DVB-S2 frames and carrier

The receiver takes every MODCOD of ETSI EN 302 307-1 (QPSK, 8PSK, 16APSK, 32APSK), normal and short frames, with and
without pilots, the roll-offs 0.35, 0.25 and 0.20, and both spectrum senses. It needs no setting for any of them.

Carrier survey. A hump of the averaged spectrum whose shape fits a raised cosine is listed as a carrier if its plateau
is no more than 4 dB under the noise floor (`snr_db` -4 or more). Under a plateau of 3 dB over the floor it is taken
for decoding only if the squared envelope also has a cyclic line near its width: the carriers of the low code rates of
QPSK stand that low, and noise has no such line. A hump without a line is passed over and the next carrier is tried.

Frames. The search looks for two PL headers one frame apart, by the products of adjacent symbols, which a carrier
offset does not disturb. The sign of those products of the start of frame gives the spectrum sense (`inverted`), for a
carrier within a quarter of the symbol rate of the centre. The PLS code (MODCOD, frame size, pilots) is the code whose
header correlates most with the 90 symbols as a whole; the frame length follows from it. Under an Es/N0 of about 1 dB
no single header stands over the noise in those products. Then the sums of the products of the headers are added over
several frames, for every PLFRAME length and every place, and the largest total gives the length, the place and the
spectrum sense; two of its headers must hold the same PLS code at the same carrier frequency. This needs frames of one
length (constant coding and modulation) and 2 to 4 of them in the window of 160 000 symbols. Once frames are
followed, the next header is looked for 8 symbols each way of its place, by its correlation at the carrier followed. If
no header shows there, the frame is taken as one more of the last kind at the phase expected; after three such frames
in a row the search starts again.

Carrier frequency. QPSK and 8PSK: the line of the fourth or eighth power of the data, searched within 1/180 of a cycle
per symbol of the carrier that the headers give.
16APSK and 32APSK have no such line. Their frequency comes from the known symbols of two headers, which leave a whole
number of turns over the frame open, and from the data between them: of the frequencies that fit both headers, the
right one puts the symbols nearest to the points of the constellation. After the lock the frequency follows each frame.

Carrier phase. Each header gives the phase at the start of its frame, so a frame starts without a phase ambiguity. In a
frame of QPSK or 8PSK the phase follows the decisions and the pilots. In a frame of 16APSK or 32APSK with pilots it is
the line between the phases of two neighbouring pilot fields, and no decision moves it; without pilots it follows the
decisions in a narrow loop of the second order. The ring radii are those of the code rate; the symbols are scaled by
the amplitude of the headers (and of the pilots), not by the mean power, which holds the noise.

Symbol clock. A symbol rate taken from the cyclic line of the carrier is right within a point of the line search, so
the clock loop searches within that distance of it (150 to 1000 ppm) with a narrow bandwidth. With a rate set by hand,
the clock starts from the
strongest cyclic line within 4 % of that rate. Without a line, the loop searches within 4 % of the rate set or of the
rate taken from the width of the carrier; a 16APSK or 32APSK carrier of roll-off 0.20 or 0.25 does not lock in that
wide search.

Sensitivity, normal frames without pilots at 2 samples per symbol, the symbol rate not set: the lowest Es/N0 at which
fewer than 1 frame in 100 is lost (20 carriers that differ in data, noise, phase, carrier offset up to 0.5 % of the
symbol rate and clock offset up to 100 ppm; steps of 0.25 dB), against the figure of table 13 of the standard.

| MODCOD | Es/N0 | Table 13 |
|---|---|---|
| QPSK 1/4 | -2.35 dB | -2.35 dB |
| QPSK 1/3 | -1.0 dB | -1.24 dB |
| QPSK 2/5 | -0.05 dB | -0.30 dB |
| QPSK 1/2 | 1.25 dB | 1.00 dB |
| QPSK 3/5 | 2.7 dB | 2.23 dB |
| QPSK 3/4 | 4.0 dB | 4.03 dB |
| QPSK 5/6 | 5.2 dB | 5.18 dB |
| 8PSK 2/3 | 6.9 dB | 6.62 dB |
| 8PSK 5/6 | 9.6 dB | 9.35 dB |
| 16APSK 2/3 | 9.2 dB | 8.97 dB |
| 16APSK 3/4 | 10.5 dB | 10.21 dB |
| 16APSK 9/10 | 13.1 dB | 13.13 dB |
| 32APSK 3/4 | 13.0 dB | 12.73 dB |
| 32APSK 4/5 | 14.1 dB | 13.64 dB |
| 32APSK 9/10 | 16.1 dB | 16.05 dB |

With pilots: QPSK 1/4 at -2.35 dB, 16APSK 2/3 at 9.0 dB, 32APSK 3/4 at 13.0 dB. Short frames without pilots: QPSK 1/4
at -2.35 dB, 16APSK 2/3 at 9.5 dB, 32APSK 3/4 at 13.2 dB. The LDPC decoder scales the minimum of a check by 15/16 for
code rate 1/4 and by 14/16 for 1/3, by 13/16 for the other rates.

Limits. The search over several frames needs frames of one length: a carrier that changes the frame length from frame
to frame is found only where single headers show, over an Es/N0 of about 1 dB. The first search has 65 000 symbols,
which hold 2 normal QPSK frames; under an Es/N0 of about -1 dB that is too few, and the lock comes with the next
search, 131 000 symbols later. About a fifth of the processor time of a search without a result is the search over
several frames. The frame decoder does not tell the frame follower that frames fail: a 16APSK or 32APSK
carrier without pilots that is taken at a frequency one turn per frame off stays so until the headers are lost. The
measurements above show no such case.

Lock time, after the survey (48 spectra) and the first search (about 65 000 symbols). The frames of the window that
lie before the header found are followed too, so the frames start near the start of the window. From the start of a
signal at 1 Msym/s and 2 Msps: from frame 3 of normal QPSK frames, frame 4 of 8PSK, frame 5 of 16APSK and frame 6 of
32APSK, the first of them out after 197 000 to 254 000 symbols of the signal.

Processor time per symbol at 2 samples per symbol on one core (release build, the front end, the frame stage and the
frame decoder): about 90 ns for QPSK 3/4 and 95 ns for 8PSK 2/3, 150 ns for 16APSK and 185 ns for 32APSK.

## Transmit

The stack `dvbs` sends a DVB-S signal (ETSI EN 300 421) or a DVB-S2 signal (ETSI EN 302 307-1) of a transport stream
that the host gives, for amateur digital television. The setting `tx_standard` selects the standard (`dvbs`, the
default, or `dvbs2`); the commands, the packet feed and the result fields are the same for both. The library has no
video encoder: the host, or the user, codes the pictures and the sound into an MPEG transport stream, and the stack
codes its packets. The engine setting `transmit_allowed` is the only switch; the stack adds no other gate.

Commands, with the keys, arguments and result fields of the DVB-T sender, so that a host that feeds one feeds the other:

- `start` sends with the `tx_*` settings until `stop`: DVB-S from the start of a group of eight packets, DVB-S2 from
  the start of a PLFRAME. A setting change applies at the next `start`. With the transmitter off it is refused
  (`SDRKIT_E_TRANSMIT_OFF`); a session rate below 1.3 × (1 + roll-off) times the symbol rate is refused with the rate
  it needs (1.755 times for roll-off 0.35); DVB-S2 short frames at code rate 9/10 are refused (the standard has none).
- `stop` ends the transmission after the group of eight packets on air (DVB-S) or after the PLFRAME on air (DVB-S2),
  with a fall of 5 ms and the end mark. The DVB-S2 fall runs over dummy PLFRAMEs, so no packet is cut. The packets that
  wait are dropped.
- `append` adds whole 188-byte packets (each starting with 0x47) to the queue. The queue holds 6000 packets; an append
  that does not fit, that is not whole packets, or that holds a packet without the sync byte is refused as a whole, so
  the host feeds at the useful bit rate. Null packets of the host are left out. Packets can wait in the queue before
  `start`.

Settings of both standards: `tx_standard`, `tx_symbol_rate` (25 000 to 60 000 000 sym/s, default 5 000 000; amateur
rates such as 125 000, 250 000 and 333 000 included), `tx_inversion` (the spectrum mirrored, default off).

Settings of DVB-S: `tx_rate` (1/2, 2/3, 3/4, 5/6, 7/8, default 2/3). The roll-off is 0.35.

Settings of DVB-S2: `tx_modcod` (the 28 MODCODs of the standard, `QPSK-1/4` to `32APSK-9/10`, default `QPSK-2/3`),
`tx_frame` (`normal`, 64 800 bits, or `short`, 16 200 bits), `tx_pilots` (default off), `tx_rolloff` (0.35, 0.25 or
0.20), `tx_pl_scrambling` (the Gold code number, 0 to 262 141, default 0). The signal has constant coding and
modulation and one input stream.

The stream: each packet slot takes the next packet of the queue; a slot that finds the queue empty carries a null packet
(PID 0x1FFF). With no input the signal is a valid stream of null packets. Each PCR leaves rewritten to the time of its
slot. The DVB-S2 sender sends no dummy PLFRAME between frames of data: the transport stream keeps its bit rate.

The coding. The test signal and the sender use one coder for each standard. DVB-S: energy dispersal with the inverted
sync byte at the start of each group of eight packets, Reed-Solomon (204,188, t = 8), the convolutional interleaver
(I = 12), the convolutional code (K = 7) punctured to the code rate, Gray-coded QPSK. DVB-S2: the baseband header with
its CRC-8, the sync byte of each packet replaced by the CRC-8 of the packet before it, baseband scrambling, BCH, LDPC,
the bit interleaver (8PSK and above), the constellation with the ring ratios of the code rate, the PL header (start of
frame and PLS code), the pilot blocks and the PL scrambling. Both: root raised cosine pulses made at the session's rate.

The useful bit rate (`tx_bitrate_bps`). DVB-S: `symbol rate × 2 × code rate × 188 / 204`: 38.015 Mbit/s for 3/4 at
27.5 Msym/s, 921.6 kbit/s for 1/2 at 1 Msym/s. DVB-S2: `symbol rate × (K_bch − 80) / symbols of a PLFRAME`, the header
and the pilots counted: at 1 Msym/s, QPSK 2/3 in normal frames carries 1.3222 Mbit/s (42 960 bits in 32 490 symbols)
and 1.2908 Mbit/s with pilots (33 282 symbols); 8PSK 9/10 in normal frames at 125 ksym/s carries 334.9 kbit/s;
16APSK 3/4 in short frames with pilots at 2 Msym/s carries 5.523 Mbit/s (11 632 bits in 4212 symbols).

Result fields: `transmitting`, `tx_freq_hz` (the dial, while sending), `tx_bitrate_bps` (the useful bit rate of the
settings, or of the transmission under way), `tx_packets`, `tx_nulls`, `tx_queue`, `tx_queue_s`, `tx_pcr_restamped`.

Level. DVB-S: rms 0.55 of full scale; the peak of QPSK with roll-off 0.35 stays near 0.87, under the soft limiter's
knee (0.9). DVB-S2: the rms follows the constellation and the roll-off, so that the peak stays under the knee:

| Constellation | Roll-off 0.35 | 0.25 | 0.20 | Peak over mean power (0.35, 0.20) |
|---|---|---|---|---|
| QPSK, 8PSK | 0.55 | 0.50 | 0.47 | 4.0 dB, 5.6 dB |
| 16APSK | 0.49 | 0.44 | 0.41 | 4.8 dB, 6.3 dB |
| 32APSK | 0.44 | 0.40 | 0.375 | 5.7 dB, 7.0 dB |

Spectrum of the clean signal: at twice the symbol rate from the centre 70 dB or more under the density in the band
(DVB-S: about 76 dB); the roll-off region follows the raised cosine of the roll-off within 0.6 dB.

Processor share of one core (the crate's benchmarks `dvbcs.dvbs_sender_*` and `dvbcs.dvbs2_sender_*`, release build).
DVB-S, QPSK 3/4: 5.3 % for 1 Msym/s at 2 Msps, 11 % for 2 Msym/s at 4 Msps. DVB-S2 at 1 Msym/s and 2 Msps in normal
frames: about 8 % for QPSK 3/4, about 10 % for 16APSK 3/4. The DVB-S2 sender codes a frame in one step (about 0.6 ms
for a normal frame) and shapes it in blocks of 512 symbols. The sender runs in the stack's graph; it needs no stage of
its own.
