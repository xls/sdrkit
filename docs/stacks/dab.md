<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DAB / DAB+

Digital radio ensembles: DAB+ and DAB audio with labels, slideshow, programme guide, Journaline and DMB video.

Stack id: `dab`. Group: Broadcast.

Signal types: DAB / DAB+.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 174 MHz to 1.492 GHz |
| Sample rate | 2.048 MHz |
| Smallest sample rate | 1.6 MHz |
| Signal bandwidth | 1.7 MHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dab` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `detection` | Detection | choice | `auto` | `auto` (Automatic), `differential` (Differential) | Automatic uses the channel estimate when it is reliable. Differential forces plain differential detection. |
| `centring` | Centre the ensemble | yes or no | true |  | Moves an ensemble found off centre to the middle before the stack locks. |
| `impulse_blanker` | Impulse blanker | yes or no | true |  | Sets samples far above the running mean power to zero. |
| `early_start` | Early start | yes or no | true |  | DAB+ decodes its first frames from 10 CIFs instead of 16. |
| `aac_packaging` | AAC packaging | choice | `raw` | `raw` (Raw access units), `latm` (LATM) | How DAB+ audio is given to the host's decoder. |
| `channels` | Channels | text | empty |  | The centre frequencies in Hz of the channels that service following may tune to, separated by ";". At most 128 entries. A frequency within 50 kHz of a block of EN 300 401 takes the name of the block. Empty: every block of Band III and L-band. |
| `data_subchannel` | Data sub-channel | whole number | -1 | -1 to 63 | A further sub-channel to decode for data (packet mode SPI or Journaline, DMB video). −1 decodes none. |
| `tii` | Transmitter identification | yes or no | true |  | Analyses the null symbols for transmitter identifiers. |
| `announcement_types` | Announcement types | whole number | 63 | 0 to 65535 | The announcement types (a bit mask, TS 101 756 table 14) that apply to the service playing. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tii` | Transmitter identification | yes or no | false |  | Three transmitters (main 1, sub 1, 3 and 7 at 0, −5 and −11 dB). |
| `gen_fig2` | Extended labels | yes or no | false |  | Labels as FIG 2 (UTF-8). |
| `gen_pty` | Programme type | whole number | 0 | 0 to 31 | 0 for none. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `level_dbfs` | Level | number (dBFS) |
| `ensemble_label` | Ensemble | text |
| `ensemble_short_label` | Ensemble short label | text |
| `eid` | EId | whole number |
| `ecc` | Extended country code | whole number |
| `country` | Country id | whole number |
| `utc` | Ensemble time | time |
| `local_time_offset` | Local time offset | duration (s) |
| `ensemble_valid` | Ensemble complete | yes or no |
| `service_count` | Services | whole number |
| `sync` | Frame sync found | yes or no |
| `prs_peak` | Sync symbol peak | number |
| `frames` | Frames | whole number |
| `fib_ok` | FIBs good | whole number |
| `fib_bad` | FIBs bad | whole number |
| `fib_ok_last_frame` | FIBs good in the last frame | whole number |
| `fic_fill_pct` | FIC fill | number (%) |
| `quiet` | Quiet time | duration (s) |
| `sync_losses` | Sync losses | whole number |
| `relocks` | Relocks | whole number |
| `erased_cifs` | Erased CIFs | whole number |
| `impulses_blanked` | Impulses blanked | whole number |
| `mirrored` | Spectrum mirrored | yes or no |
| `signal_time` | Signal time | duration (s) |
| `detection` | Detection | choice |
| `selected_service` | Service playing | text |
| `selected_subchannel` | Sub-channel playing | whole number |
| `selected_label` | Service playing | text |
| `selected_kind` | Kind playing | choice |
| `selected_bitrate_kbps` | Bit rate playing | whole number (kbit/s) |
| `selected_protection` | Protection playing | text |
| `playing` | Audio | choice |
| `audio_codec` | Audio codec | choice |
| `audio_rate` | Audio sample rate | frequency |
| `audio_channels` | Audio channels | whole number |
| `audio_sbr` | SBR | yes or no |
| `audio_ps` | Parametric stereo | yes or no |
| `audio_units_per_superframe` | Access units per superframe | whole number |
| `audio_logical_frames` | Logical frames | whole number |
| `audio_units_ok` | Access units good | whole number |
| `audio_units_bad` | Access units bad | whole number |
| `audio_superframes_ok` | Superframes good | whole number |
| `audio_superframes_bad` | Superframes bad | whole number |
| `audio_rs_corrected` | RS bytes corrected | whole number |
| `audio_rs_uncorrectable` | RS words beyond repair | whole number |
| `audio_mp2_frames` | Layer II frames | whole number |
| `audio_mp2_bad` | Layer II frames bad | whole number |
| `audio_concealed_pct` | Concealed | number (%) |
| `label_charset` | Label character set | whole number |
| `slide_sequence` | Slide number | whole number |
| `slide_width` | Slide width | whole number (px) |
| `slide_height` | Slide height | whole number (px) |
| `slide_bytes` | Slide size | whole number (bytes) |
| `timing_lock` | Locked at | duration (s) |
| `timing_first_fib` | First good FIB at | duration (s) |
| `timing_ensemble_valid` | Ensemble complete at | duration (s) |
| `timing_all_labels` | Every label at | duration (s) |
| `timing_first_access_unit` | First audio at | duration (s) |
| `timing_label` | First label at | duration (s) |
| `timing_slide` | First slide at | duration (s) |
| `announcement_cluster` | Announcement cluster | whole number |
| `announcement_type` | Announcement type | whole number |
| `announcement_name` | Announcement | text |
| `announcement_subchannel` | Announcement sub-channel | whole number |
| `dmb_ts_packets` | DMB packets | whole number |
| `dmb_rs_corrected` | DMB RS words corrected | whole number |
| `dmb_rs_failed` | DMB RS words failed | whole number |
| `dmb_video_units` | DMB video units | whole number |
| `dmb_audio_units` | DMB audio units | whole number |
| `dmb_video_width` | DMB video width | whole number (px) |
| `dmb_video_height` | DMB video height | whole number (px) |
| `fic_records` | FIG records | whole number |
| `spi_objects` | Guide objects | whole number |
| `journaline_received` | Journaline objects received | whole number |
| `tii_analysed` | Null symbols analysed | whole number |
| `health_sync` | Sync | choice |
| `health_prs` | Sync symbol | choice |
| `health_fic` | FIC | choice |
| `health_ensemble` | Ensemble | choice |
| `health_msc` | Sub-channel | choice |
| `health_audio` | Audio | choice |
| `health_concealed` | Concealment | choice |
| `quality` | Signal quality | number |
| `quality_text` | Signal quality | choice |
| `text` | Text | text |
| `item_running` | Item running | yes or no |
| `slide_name` | Slide | text |
| `slide_category` | Slide category | text |
| `slide_link` | Slide link | text |
| `slide_alert` | Emergency slide | yes or no |
| `concealed` | Concealed | duration (s) |
| `muted` | Muted | duration (s) |
| `concealments` | Concealments | whole number |
| `underruns` | Underruns | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `services` | Services | `sid` (whole number), `label` (text), `short_label` (text), `kind` (choice), `subchannel` (whole number), `bitrate_kbps` (whole number (kbit/s)), `protection` (text), `component_type` (whole number), `pty` (whole number), `pty_name` (text), `pty_static` (yes or no), `language` (whole number), `language_name` (text), `scrambled` (yes or no), `announcement_support` (whole number), `now_playing` (text), `playing` (yes or no) |
| `subchannels` | Sub-channels | `id` (whole number), `start_cu` (whole number (CU)), `size_cu` (whole number (CU)), `protection` (text), `bitrate_kbps` (whole number (kbit/s)) |
| `service_components` | Service components | `service` (text), `tmid` (whole number), `component_type` (whole number), `subchannel` (whole number), `packet_address` (whole number), `scids` (whole number), `primary` (yes or no), `scrambled` (yes or no), `label` (text) |
| `user_applications` | User applications | `service` (text), `scids` (whole number), `app_type` (whole number), `name` (text) |
| `service_clusters` | Announcement clusters | `service` (text), `cluster` (whole number) |
| `tii_database` | Transmitter database | `main` (whole number), `sub` (whole number), `position` (position), `delay_us` (whole number (µs)) |
| `following` | Service following | `freq` (frequency), `channel` (text), `eid` (whole number) |
| `announcements` | Announcements | `cluster` (whole number), `asw` (whole number), `subchannel` (whole number) |
| `transmitters` | Transmitters | `main` (whole number), `sub` (whole number), `level_db` (number (dB)), `position` (position), `delay_us` (whole number (µs)) |
| `journaline` | Journaline | `id` (whole number), `kind` (choice), `title` (text), `body` (text) |
| `journaline_links` | Journaline menu entries | `object` (text), `target` (whole number), `text` (text) |
| `journaline_items` | Journaline list items | `object` (text), `text` (text) |
| `fig_types` | FIG types | `fig` (text), `count` (whole number), `bytes` (whole number) |
| `label_tags` | Text tags | `code` (whole number), `type` (text), `text` (text) |
| `guide_services` | Guide services | `name` (text), `short_name` (text), `description` (text), `sid` (whole number), `eid` (whole number), `genres` (text), `logo_url` (text) |
| `guide_programmes` | Programmes | `crid` (text), `name` (text), `start` (time (s)), `duration` (duration (s)), `description` (text), `genres` (text) |
| `guide_groups` | Programme groups | `name` (text) |
| `slide_categories` | Slide categories | `category` (whole number), `title` (text), `slide` (whole number), `name` (text) |
| `logos` | Logos | `png` (bytes), `jpeg` (bytes) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `impulse_response` | Impulse response | sample |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `dab.ensemble` | Ensemble complete | `source` (text), `text` (text), `family` (choice) |
| `dab.services` | Service list changed | `source` (text), `count` (whole number), `family` (choice) |
| `dab.announcement_started` | Announcement started | `source` (text), `cluster` (whole number), `asw` (whole number), `subchannel` (whole number), `text` (text), `urgency` (choice), `family` (choice) |
| `dab.announcement_ended` | Announcement ended | `source` (text), `cluster` (whole number), `family` (choice) |
| `dab.sync_lost` | Sync lost | `family` (choice) |
| `dab.label` | Dynamic label | `source` (text), `text` (text), `family` (choice) |
| `dab.slide` | Slide | `source` (text), `slide_sequence` (whole number), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |

## Typed interface

The stack is in the family `dab`, version 1.1. The C header `sdrkit/dab.h`, the registry record `registry/dab.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Broadcast radio (`broadcast`). Media: sound (programme), pictures.

### Result fields

Struct `SignalFields`. The measurements of the signal. No SNR before the lock, and no offset and clock error while the receiver searches.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `snr_db` | number (dB) | optional | 1.0 | The SNR measure. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset, without the centring shift. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The sample clock error. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The input level. |

### Result fields

Struct `EnsembleFields`. The ensemble. No value for an item that the ensemble has not signalled yet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ensemble_label` | text | optional | 1.0 | The ensemble label. |
| `ensemble_short_label` | text | optional | 1.0 | The ensemble short label. |
| `eid` | whole number | optional | 1.0 | The ensemble identifier (EId). |
| `ecc` | whole number | optional | 1.0 | The extended country code (FIG 0/9). |
| `country` | whole number | optional | 1.0 | The country id (top 4 bits of the EId). |
| `utc` | time | optional | 1.0 | The ensemble time. |
| `local_time_offset` | duration (s) | optional | 1.0 | The local time offset (FIG 0/9). |
| `ensemble_valid` | yes or no | always | 1.0 | Every label and the service organisation are received. |
| `service_count` | whole number | always | 1.0 | Services in the ensemble. |

### Result fields

Struct `SyncFields`. The synchronisation, the FIC and the front end.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sync` | yes or no | always | 1.0 | The last synchronisation symbol (PRS) was found. |
| `prs_peak` | number | optional | 1.0 | The PRS correlation peak over its median. No value while the receiver searches. |
| `frames` | whole number | always | 1.0 | Frames demodulated. |
| `fib_ok` | whole number | always | 1.0 | FIBs with a good CRC. |
| `fib_bad` | whole number | always | 1.0 | FIBs with a bad CRC. |
| `fib_ok_last_frame` | whole number | always | 1.0 | Good FIBs in the last frame. |
| `fic_fill_pct` | number (%) | always | 1.0 | The share of the FIC that carries FIGs. |
| `quiet` | duration (s) | always | 1.0 | The quiet time of the null symbol. |
| `sync_losses` | whole number | always | 1.0 | Sync losses. |
| `relocks` | whole number | always | 1.0 | Relocks. |
| `erased_cifs` | whole number | always | 1.0 | CIFs erased after lost samples. |
| `impulses_blanked` | whole number | always | 1.0 | Impulses blanked. |
| `mirrored` | yes or no | always | 1.0 | The spectrum was mirrored. |
| `signal_time` | duration (s) | always | 1.0 | Signal time. |
| `detection` | choice | optional | 1.0 | The detection in use. No value while the receiver searches. |

### Result fields

Struct `PlayingFields`. The service playing. No value while no service plays.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `selected_service` | text | optional | 1.0 | The row key of the service playing in [`SERVICES`]. |
| `selected_subchannel` | whole number | optional | 1.0 | The sub-channel playing. |
| `selected_label` | text | optional | 1.0 | The label of the service playing. |
| `selected_kind` | choice | optional | 1.0 | The kind of the service playing. |
| `selected_bitrate_kbps` | whole number (kbit/s) | optional | 1.0 | The bit rate playing. |
| `selected_protection` | text | optional | 1.0 | The protection playing ("EEP 3-A", "UEP 12"). |
| `playing` | choice | always | 1.0 | What the audio of the service playing does. |

### Result fields

Struct `AudioFields`. The audio of the service playing. No codec, rate, channels, SBR or PS before the audio configuration is known, and no concealed share before the first access unit.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `audio_codec` | choice | optional | 1.0 | The audio codec. |
| `audio_rate` | frequency | optional | 1.0 | The audio sample rate. |
| `audio_channels` | whole number | optional | 1.0 | Audio channels. |
| `audio_sbr` | yes or no | optional | 1.0 | SBR in use. |
| `audio_ps` | yes or no | optional | 1.0 | Parametric stereo in use. |
| `audio_units_per_superframe` | whole number | optional | 1.0 | Access units in a DAB+ superframe. |
| `audio_logical_frames` | whole number | always | 1.0 | Logical frames of the sub-channel playing. |
| `audio_units_ok` | whole number | always | 1.0 | Good access units. |
| `audio_units_bad` | whole number | always | 1.0 | Bad access units. |
| `audio_superframes_ok` | whole number | always | 1.0 | Good superframes. |
| `audio_superframes_bad` | whole number | always | 1.0 | Bad superframes. |
| `audio_rs_corrected` | whole number | always | 1.0 | Reed-Solomon bytes corrected. |
| `audio_rs_uncorrectable` | whole number | always | 1.0 | Reed-Solomon words beyond repair. |
| `audio_mp2_frames` | whole number | always | 1.0 | Layer II frames. |
| `audio_mp2_bad` | whole number | always | 1.0 | Bad Layer II frames. |
| `audio_concealed_pct` | number (%) | optional | 1.0 | The share of access units concealed. |

### Result fields

Struct `PadFields`. The dynamic label and the slide of the service playing. No value before the first label or slide.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `label_charset` | whole number | optional | 1.0 | The character set code of the dynamic label (TS 101 756 table 1). |
| `slide_sequence` | whole number | optional | 1.0 | The slide number: a new number is a new picture. |
| `slide_width` | whole number (px) | optional | 1.0 | The slide width. |
| `slide_height` | whole number (px) | optional | 1.0 | The slide height. |
| `slide_bytes` | whole number (bytes) | optional | 1.0 | The slide size. |

### Result fields

Struct `TimingFields`. When things were first reported, after the start. No value before it happened.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `timing_lock` | duration (s) | optional | 1.0 | Lock. |
| `timing_first_fib` | duration (s) | optional | 1.0 | The first good FIB. |
| `timing_ensemble_valid` | duration (s) | optional | 1.0 | The ensemble complete. |
| `timing_all_labels` | duration (s) | optional | 1.0 | Every label known. |
| `timing_first_access_unit` | duration (s) | optional | 1.0 | The first audio of the service playing. |
| `timing_label` | duration (s) | optional | 1.0 | The first label. |
| `timing_slide` | duration (s) | optional | 1.0 | The first slide. |

### Result fields

Struct `AnnouncementFields`. The announcement that applies to the service playing. No value while none applies.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `announcement_cluster` | whole number | optional | 1.0 | The cluster of the announcement. |
| `announcement_type` | whole number | optional | 1.0 | The type of the announcement (TS 101 756 table 14: 0 alarm, 1 road traffic ...). |
| `announcement_name` | text | optional | 1.0 | The name of that type. |
| `announcement_subchannel` | whole number | optional | 1.0 | The sub-channel of the announcement. |

### Result fields

Struct `DmbFields`. The DMB video sub-channel. No value before the DMB decoder reports, and no size before a sequence parameter set.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dmb_ts_packets` | whole number | optional | 1.0 | DMB transport stream packets. |
| `dmb_rs_corrected` | whole number | optional | 1.0 | DMB Reed-Solomon words corrected. |
| `dmb_rs_failed` | whole number | optional | 1.0 | DMB Reed-Solomon words beyond repair. |
| `dmb_video_units` | whole number | optional | 1.0 | DMB video access units. |
| `dmb_audio_units` | whole number | optional | 1.0 | DMB audio access units. |
| `dmb_video_width` | whole number (px) | optional | 1.0 | The width of the DMB video, from its sequence parameter set. |
| `dmb_video_height` | whole number (px) | optional | 1.0 | The height of the DMB video. |

### Result fields

Struct `DataFields`. The counts of the data decoders. No value before a decoder reports.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `fic_records` | whole number | always | 1.0 | FIG records kept for the analyser. |
| `spi_objects` | whole number | optional | 1.0 | Guide objects decoded. |
| `journaline_received` | whole number | optional | 1.0 | Journaline objects received. |
| `tii_analysed` | whole number | optional | 1.0 | Null symbols analysed for transmitter identification. |

### Result fields

Struct `HealthFields`. The health indicators and the signal quality.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `health_sync` | choice | always | 1.0 | The receiver is locked. |
| `health_prs` | choice | always | 1.0 | The synchronisation symbol peak. |
| `health_fic` | choice | always | 1.0 | The FIBs of the last frame. |
| `health_ensemble` | choice | always | 1.0 | The ensemble. |
| `health_msc` | choice | always | 1.0 | The sub-channel playing. |
| `health_audio` | choice | always | 1.0 | The audio. |
| `health_concealed` | choice | always | 1.0 | The concealment. |
| `quality` | number | optional | 1.0 | The signal quality, 0 to 1: (SNR − 4 dB) / 18 dB. No value before the lock. |
| `quality_text` | choice | optional | 1.0 | The signal quality in words. No value before the lock. |

### Result fields

Struct `LabelFields`. The radio text and whether its item is running.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | optional | 1.0 | The radio text. |
| `item_running` | yes or no | optional | 1.0 | The tagged item (a song) is playing. |

### Result fields

Struct `SlideFields`. The slide shown.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `slide_name` | text | optional | 1.0 | The name of the slide (its MOT ContentName). |
| `slide_category` | text | optional | 1.0 | The category title of the slide. |
| `slide_link` | text | optional | 1.0 | The click-through URL of the slide. |
| `slide_alert` | yes or no | always | 1.0 | The slide is an emergency alert. |

### Result fields

Struct `PlayoutFields`. The counts of the playout since the start or the last change of frequency, written once a second while sound plays. No value before the first report.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `concealed` | duration (s) | optional | 1.0 | Time filled by concealment above silence. |
| `muted` | duration (s) | optional | 1.0 | Time of silence (faded concealment, or waiting to start). |
| `concealments` | whole number | optional | 1.0 | Concealments started. |
| `underruns` | whole number | optional | 1.0 | Times the queue ran empty. |

### Table `services`

Struct `ServiceRow`. A service of the ensemble. The row key is the SId in hexadecimal: 4 digits, 8 for a data service.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sid` | whole number | always | 1.0 | The service identifier (SId). |
| `label` | text | optional | 1.0 | The label. |
| `short_label` | text | optional | 1.0 | The short label. |
| `kind` | choice | always | 1.0 | What the service carries. |
| `subchannel` | whole number | optional | 1.0 | The sub-channel of the primary component. |
| `bitrate_kbps` | whole number (kbit/s) | optional | 1.0 | The bit rate of that sub-channel. |
| `protection` | text | optional | 1.0 | The protection of that sub-channel ("EEP 3-A", "UEP 12"). |
| `component_type` | whole number | optional | 1.0 | The ASCTy of the audio component, or the DSCTy of a data component. |
| `pty` | whole number | optional | 1.0 | The programme type code. |
| `pty_name` | text | optional | 1.0 | The name of the programme type. |
| `pty_static` | yes or no | always | 1.0 | The programme type is static. |
| `language` | whole number | optional | 1.0 | The language code (TS 101 756 tables 9 and 10). |
| `language_name` | text | optional | 1.0 | The name of the language. |
| `scrambled` | yes or no | always | 1.0 | Scrambled (conditional access). |
| `announcement_support` | whole number | always | 1.0 | The announcement support flags (ASu). |
| `now_playing` | text | optional | 1.0 | The dynamic label of the service, while it plays. |
| `playing` | yes or no | always | 1.0 | The service plays. |

The column `label` names a row.

### Table `subchannels`

Struct `SubchannelRow`. A sub-channel. The row key is its id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The sub-channel id. |
| `start_cu` | whole number (CU) | always | 1.0 | The first capacity unit. |
| `size_cu` | whole number (CU) | always | 1.0 | The size in capacity units. |
| `protection` | text | optional | 1.0 | The protection ("EEP 3-A", "UEP 12"). |
| `bitrate_kbps` | whole number (kbit/s) | always | 1.0 | The bit rate. |

### Table `service_components`

Struct `ServiceComponentRow`. A service component. The row key is `<service>/<n>`; n counts from 0 in the order of FIG 0/2 and FIG 0/8.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service` | text | always | 1.0 | The row key of the service. |
| `tmid` | whole number | always | 1.0 | The transport mechanism (TMId: 0 audio stream, 1 data stream, 3 packet data). |
| `component_type` | whole number | always | 1.0 | The ASCTy of an audio component, or the DSCTy of a data component. |
| `subchannel` | whole number | optional | 1.0 | The sub-channel. |
| `packet_address` | whole number | optional | 1.0 | The packet address. |
| `scids` | whole number | optional | 1.0 | The service component identifier within the service (SCIdS). |
| `primary` | yes or no | always | 1.0 | The primary component. |
| `scrambled` | yes or no | always | 1.0 | Scrambled (conditional access). |
| `label` | text | optional | 1.0 | The component label. |

The column `label` names a row.

### Table `user_applications`

Struct `UserApplicationRow`. A user application of a service (FIG 0/13). The row key is `<service>/<SCIdS>/<type>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service` | text | always | 1.0 | The row key of the service. |
| `scids` | whole number | always | 1.0 | The service component identifier within the service (SCIdS). |
| `app_type` | whole number | always | 1.0 | The user application type (TS 101 756 table 16). |
| `name` | text | always | 1.0 | The name of the type. |

The column `name` names a row.

### Table `service_clusters`

Struct `ServiceClusterRow`. An announcement cluster of a service (FIG 0/18). The row key is `<service>/<cluster>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service` | text | always | 1.0 | The row key of the service. |
| `cluster` | whole number | always | 1.0 | The announcement cluster. |

### Table `tii_database`

Struct `TiiEntryRow`. A transmitter of the database of FIG 0/22. The row key is `<main>.<sub>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `main` | whole number | always | 1.0 | The transmitter main identifier. |
| `sub` | whole number | always | 1.0 | The transmitter sub-identifier. |
| `position` | position | always | 1.0 | The position. |
| `delay_us` | whole number (µs) | always | 1.0 | The delay. |

Place of a row: the column `position`.

### Table `following`

Struct `FollowingRow`. A frequency where the service playing can also be received. The row key is the frequency in Hz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq` | frequency | always | 1.0 | The frequency. |
| `channel` | text | always | 1.0 | The channel name. |
| `eid` | whole number | always | 1.0 | The ensemble identifier. |

### Table `announcements`

Struct `AnnouncementRow`. An announcement running. The row key is its cluster.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cluster` | whole number | always | 1.0 | The announcement cluster. |
| `asw` | whole number | always | 1.0 | The announcement switching flags (ASw). |
| `subchannel` | whole number | always | 1.0 | The sub-channel of the announcement. |

### Table `transmitters`

Struct `TransmitterRow`. A transmitter identified. The row key is `<main>.<sub>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `main` | whole number | always | 1.0 | The transmitter main identifier. |
| `sub` | whole number | always | 1.0 | The transmitter sub-identifier. |
| `level_db` | number (dB) | always | 1.0 | The level relative to the strongest transmitter. |
| `position` | position | optional | 1.0 | The position, from the transmitter database. |
| `delay_us` | whole number (µs) | optional | 1.0 | The delay, from the transmitter database. |

Place of a row: the column `position`.

### Table `journaline`

Struct `JournalineRow`. A Journaline object. The row key is its id in hexadecimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The object id. |
| `kind` | choice | always | 1.0 | The kind of the object. |
| `title` | text | always | 1.0 | The title. |
| `body` | text | optional | 1.0 | The body text. |

The column `title` names a row.

### Table `journaline_links`

Struct `JournalineLinkRow`. A menu entry of a Journaline object. The row key is `<object>/<n>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `object` | text | always | 1.0 | The row key of the object. |
| `target` | whole number | always | 1.0 | The object the entry leads to. |
| `text` | text | always | 1.0 | The text of the entry. |

### Table `journaline_items`

Struct `JournalineItemRow`. A list item of a Journaline object. The row key is `<object>/<n>`. Columns are separated by a tab.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `object` | text | always | 1.0 | The row key of the object. |
| `text` | text | always | 1.0 | The text of the item. |

### Table `fig_types`

Struct `FigTypeRow`. The statistics of a FIG type. The row key is the FIG type ("0/2").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `fig` | text | always | 1.0 | The FIG type. |
| `count` | whole number | always | 1.0 | FIGs received. |
| `bytes` | whole number | always | 1.0 | Bytes received. |

### Table `label_tags`

Struct `LabelTagRow`. A tag of the radio text. The row key is the class name of the content type (`ITEM.TITLE`), or its code for a reserved type.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | whole number | always | 1.0 | The content type code (0 to 127). |
| `type` | text | optional | 1.0 | What people call the content type ("Title"). |
| `text` | text | always | 1.0 | The tagged text. |

### Table `guide_services`

Struct `GuideServiceRow`. A service of the service information. The row key is the SId in hexadecimal, or the place of a service without a bearer.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | optional | 1.0 | The name. |
| `short_name` | text | optional | 1.0 | The short name. |
| `description` | text | optional | 1.0 | The description. |
| `sid` | whole number | optional | 1.0 | The service identifier of the first bearer. |
| `eid` | whole number | optional | 1.0 | The ensemble identifier of the first bearer. |
| `genres` | text | optional | 1.0 | The genre links, separated by spaces. |
| `logo_url` | text | optional | 1.0 | The URL of the first logo. |

### Table `guide_programmes`

Struct `GuideProgrammeRow`. A programme of the programme information. The row key is `<crid>@<start>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `crid` | text | always | 1.0 | The programme id (CRID). |
| `name` | text | optional | 1.0 | The name. |
| `start` | time (s) | always | 1.0 | The start time. |
| `duration` | duration (s) | always | 1.0 | The duration. |
| `description` | text | optional | 1.0 | The description. |
| `genres` | text | optional | 1.0 | The genre links, separated by spaces. |

### Table `guide_groups`

Struct `GuideGroupRow`. A programme group. The row key is its name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name. |

### Table `slide_categories`

Struct `SlideCategoryRow`. A slide of a categorised slideshow. The row key is `<CategoryID>/<SlideID>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `category` | whole number | always | 1.0 | The CategoryID. |
| `title` | text | optional | 1.0 | The CategoryTitle. |
| `slide` | whole number | always | 1.0 | The SlideID. |
| `name` | text | always | 1.0 | The name of the slide. |

### Table `logos`

Struct `LogoRow`. A logo. The row key is its MOT ContentName. A row holds the picture in the member of its format.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `png` | bytes | optional | 1.0 | A PNG picture. |
| `jpeg` | bytes | optional | 1.0 | A JPEG picture. |

### Event `dab.ensemble`

Struct `EnsembleEvent`. The fields of the event of a complete ensemble.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The EId in hexadecimal. |
| `text` | text | always | 1.0 | The ensemble label. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `dab.services`

Struct `ServicesEvent`. The fields of the event of a changed service list.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The EId in hexadecimal. |
| `count` | whole number | always | 1.0 | The number of services. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `dab.announcement_started`

Struct `AnnouncementStartedEvent`. The fields of the event of a started announcement.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The EId in hexadecimal. |
| `cluster` | whole number | always | 1.0 | The announcement cluster. |
| `asw` | whole number | always | 1.0 | The announcement switching flags (ASw). |
| `subchannel` | whole number | always | 1.0 | The sub-channel of the announcement. |
| `text` | text | always | 1.0 | The name of the first type of the flags. |
| `urgency` | choice | always | 1.0 | Urgent for an alarm announcement (type 0), else a notice. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `dab.announcement_ended`

Struct `AnnouncementEndedEvent`. The fields of the event of an ended announcement.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The EId in hexadecimal. |
| `cluster` | whole number | always | 1.0 | The announcement cluster. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `dab.sync_lost`

Struct `SyncLostEvent`. The fields of the event of a lost lock.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The family of the sender: the receiver. |

### Event `dab.label`

Struct `LabelEvent`. The fields of the event of a new dynamic label.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The SId of the service playing, in hexadecimal. |
| `text` | text | always | 1.0 | The label. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `dab.slide`

Struct `SlideEvent`. The fields of the event of a new slide.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The SId of the service playing, in hexadecimal. |
| `slide_sequence` | whole number | always | 1.0 | The slide number. |
| `text` | text | optional | 1.0 | The name of the slide. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `service.select` | `sdrkit_dab_service_select` | `sdrkit_dab_service_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `detection` | `sdrkit_dab_get_detection` | `sdrkit_dab_set_detection` |
| `centring` | `sdrkit_dab_get_centring` | `sdrkit_dab_set_centring` |
| `impulse_blanker` | `sdrkit_dab_get_impulse_blanker` | `sdrkit_dab_set_impulse_blanker` |
| `early_start` | `sdrkit_dab_get_early_start` | `sdrkit_dab_set_early_start` |
| `aac_packaging` | `sdrkit_dab_get_aac_packaging` | `sdrkit_dab_set_aac_packaging` |
| `channels` | `sdrkit_dab_get_channels` | `sdrkit_dab_set_channels` |
| `data_subchannel` | `sdrkit_dab_get_data_subchannel` | `sdrkit_dab_set_data_subchannel` |
| `tii` | `sdrkit_dab_get_tii` | `sdrkit_dab_set_tii` |
| `announcement_types` | `sdrkit_dab_get_announcement_types` | `sdrkit_dab_set_announcement_types` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tii` | `sdrkit_dab_gen_get_tii` | `sdrkit_dab_gen_set_tii` |
| `gen_fig2` | `sdrkit_dab_gen_get_fig2` | `sdrkit_dab_gen_set_fig2` |
| `gen_pty` | `sdrkit_dab_gen_get_pty` | `sdrkit_dab_gen_set_pty` |

## Try it

```sh
sdrkit run dab --source synth:dab
sdrkit run dab --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
