<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FM radio

FM broadcast stations in stereo, with the station name and radio text from RDS.

Stack id: `fm`. Group: Broadcast radio.

Signal types: FM broadcast.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 100 MHz |
| Tuning range | 65.8 MHz to 108 MHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 250 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fm` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `stereo` | Stereo | choice | `auto` | `auto` (Auto), `mono` (Mono), `stereo` (Stereo) | Auto plays stereo while the pilot tone is locked and moves towards mono as the signal gets weak. Mono always plays mono. Stereo plays full stereo whenever any pilot is there. |
| `squelch` | Squelch | number (dB) | 0 | 0 to 60 | Mutes the sound while the signal-to-noise measure is below this. |
| `bass` | Bass | number (dB) | 0 | -6 to 6 | Boost or cut below 200 Hz. |
| `treble` | Treble | number (dB) | 0 | -6 to 6 | Boost or cut above 6 kHz. |
| `tx_pi` | PI code | text | empty |  | Four hexadecimal digits, 0001 to FFFF, from the licence of the station. No default: the transmission does not start while it is empty. |
| `tx_ps` | Station name (PS) | text | empty |  | Up to 8 characters, padded with spaces. A change goes out from the next name cycle. |
| `tx_rt` | Radio text | text | empty |  | Up to 64 characters. Empty sends no radio text. A change goes out from the next text cycle with the A/B flag turned over, so that receivers clear the old text. |
| `tx_pty` | Programme type | whole number | 0 | 0 to 31 | The code of the programme type. The names of the codes differ between RDS and RBDS. |
| `tx_tp` | Traffic programme | yes or no | false |  | The station carries traffic announcements. |
| `tx_ta` | Traffic announcement now | yes or no | false |  | A traffic announcement is on air now. |
| `tx_music` | Music | yes or no | true |  | On for music, off for speech. |
| `tx_stereo` | Stereo | yes or no | true |  | Sends the pilot and the difference signal; off sends mono. Applies at the next start. |
| `tx_preemphasis` | Pre-emphasis | choice | `50` | `0` (None), `50` (50 µs), `75` (75 µs) | 50 µs in Europe, Asia, Africa and Oceania; 75 µs in the Americas and South Korea. Applies at the next start. |
| `tx_deviation` | Peak deviation | frequency | 75 kHz | 5 kHz to 100 kHz | The deviation that the whole multiplex never passes: audio, pilot and RDS together. 75 kHz is the broadcast value. Applies at the next start. |
| `tx_limiter` | Audio limiter | yes or no | true |  | Holds the audio peaks under the deviation with a gain that has no steps. Off clips each channel at full scale. Applies at the next start. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `force_mono` | Force mono | yes or no | false |  | Plays mono whatever the stereo choice. |
| `deemphasis` | De-emphasis | choice | `50` | `off` (Off), `50` (50 µs), `75` (75 µs) | Undoes the treble boost stations add. 50 µs in Europe, Asia, Africa and Australia. 75 µs in the Americas and South Korea. |
| `audio_bandwidth` | Audio bandwidth | choice | `0` | `0` (Full), `12000` (12 kHz), `8000` (8 kHz), `5000` (5 kHz) | Cuts the treble above this frequency. A lower value hides hiss on a weak station. |
| `warm` | Warm | yes or no | false |  | Takes 3 dB more off the treble, for a softer sound. |
| `channel_width` | Channel width | frequency | 0 Hz | 0 Hz to 1 MHz | The width of the channel filter. A narrower width keeps out a strong neighbour, at the cost of distortion on loud passages and a weaker RDS signal. At least 10 kHz. |
| `channel_filter` | Channel filter | choice | `standard` | `standard` (Standard), `wide` (Wide), `narrow` (Narrow), `custom` (Custom) | Standard is 220 kHz (or the channel width below when it is not 0). Wide is 300 kHz, for strong stations. Narrow is 150 kHz, to keep out a strong neighbour. Custom uses the channel width below. |
| `band_low_hz` | Band start | frequency | 87.5 MHz | 60 MHz to 110 MHz | The lowest frequency of the FM band. |
| `band_high_hz` | Band end | frequency | 108 MHz | 60 MHz to 110 MHz | The highest frequency of the FM band. |
| `first_hz` | First channel | frequency | 87.5 MHz | 60 MHz to 110 MHz | The first channel of the band. The channels are this frequency plus a whole number of steps. |
| `raster_hz` | Channel step | frequency | 100 kHz | 10 kHz to 1 MHz | The distance between two channels of the band. |
| `scan_low` | Scan from | number (MHz) | 87 | 76 to 108 | Where the band scan starts. The scan covers at least 0.5 MHz, within 76 to 108 MHz. |
| `scan_high` | Scan to | number (MHz) | 108 | 76 to 108 | Where the band scan ends. |
| `scan_full_rds` | Scan for full RDS | yes or no | false |  | The band scan stays on each station until its whole name, its radio text and its code have come in. Slower, but complete. |
| `station_finder` | Station finder | yes or no | false |  | For recordings and the test signal only: finds a station that is off the centre and tunes to it. |
| `rds_variant` | RDS meanings | choice | `auto` | `auto` (Auto), `rds` (RDS), `rbds` (RBDS) | How programme types and station codes are read. Auto reads RDS if the station sends a country code outside the Americas, else RBDS on the channel raster of RBDS (odd tenths of a MHz from 87.9 to 107.9 MHz) and RDS elsewhere. |
| `rds_correction` | RDS correction | choice | `2` | `0` (Off), `1` (Short bursts), `2` (Full) | How much the stack repairs damaged RDS data. Full gets the most from a weak station. A wrong character is still never shown. |
| `rds_voting` | RDS voting | yes or no | true |  | With full correction, also repairs data by comparing repeated copies. |
| `rds_charset` | RDS characters | choice | `1` | `0` (EBU Latin), `1` (Auto GB2312), `2` (GB2312) | How station names and texts are read. Auto reads Chinese (GB2312) text when it sees it. |
| `tx_af` | Alternative frequencies | text | empty |  | Up to 25 frequencies of the same programme, 87.6 to 107.9 MHz in steps of 0.1 MHz, with spaces between them. Empty sends no list. |
| `tx_ct` | Send the clock time | yes or no | false |  | Sends the time of the clock (UTC) in group 4A as each minute starts. |
| `tx_ct_offset` | Local time offset | whole number (half hours) | 0 | -24 to 24 | The local time less UTC, in half hours, sent with the clock time. |
| `tx_pilot` | Pilot level | number (%) | 9 | 0 to 20 | The pilot deviation as a share of the peak deviation. 8 to 10 % is the broadcast range. Applies at the next start. |
| `tx_rds_level` | RDS level | number (kHz) | 2 | 0.2 to 7.5 | The peak deviation by the RDS subcarrier; 2 to 3 kHz is usual. Applies at the next start. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_stereo` | Stereo | yes or no | true |  | Sends a stereo signal with its pilot tone. |
| `gen_left_hz` | Left tone | frequency | 1 kHz | 0 Hz to 15 kHz |  |
| `gen_left_amp` | Left level | number | 0.5 | 0 to 1 | 0 is silent. |
| `gen_right_hz` | Right tone | frequency | 3 kHz | 0 Hz to 15 kHz |  |
| `gen_right_amp` | Right level | number | 0.5 | 0 to 1 | 0 is silent. |
| `gen_pilot` | Pilot | number (%) | 9 | 0 to 15 | Pilot injection, as a percentage of 75 kHz. |
| `gen_rds` | RDS | yes or no | true |  |  |
| `gen_rds_khz` | RDS injection | number (kHz) | 2 | 0 to 10 |  |
| `gen_ps` | Station name | text | `SDRKIT ` |  | Up to 8 characters. |
| `gen_rt` | Radio text | text | `sdrkit FM test signal` |  | Up to 64 characters. |
| `gen_pi` | PI code | text | `4A21` |  | Four hexadecimal digits. |
| `gen_pty` | Programme type | whole number | 10 | 0 to 31 |  |
| `gen_tp` | Traffic programme | yes or no | true |  |  |
| `gen_ta` | Traffic announcement | yes or no | false |  |  |
| `gen_music` | Music | yes or no | true |  |  |
| `gen_preemphasis` | Pre-emphasis | choice | `50` | `0` (None), `50` (50 µs), `75` (75 µs) |  |
| `gen_extra_groups` | Extra RDS groups | text | empty |  | Blocks B, C and D of groups to send as well, in hexadecimal, for example "1000 00E2 B945; A400 524F 434B". |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `station` | Station | text |
| `station_partial` | Station so far | text |
| `text` | Text | text |
| `text_complete` | RadioText complete | yes or no |
| `programme` | Programme type | text |
| `pty` | PTY code | whole number |
| `pi` | PI | text |
| `call_letters` | Call letters | text |
| `rbds` | RBDS meanings | yes or no |
| `tp` | Traffic programme | yes or no |
| `ta` | Traffic announcement | yes or no |
| `music` | Music | yes or no |
| `ptyn` | Programme type name | text |
| `ecc` | Extended country code | whole number |
| `language` | Language code | whole number |
| `pin` | Programme item | text |
| `clock` | Station clock | time |
| `clock_offset` | Local time offset | duration (s) |
| `ert` | Enhanced RadioText | text |
| `item_running` | Item running | yes or no |
| `rds_sync` | RDS sync | yes or no |
| `rds_ok_pct` | RDS usable blocks | number (%) |
| `groups` | RDS groups | whole number |
| `blocks_good` | Good blocks | whole number |
| `blocks_corrected` | Corrected blocks | whole number |
| `blocks_bad` | Bad blocks | whole number |
| `blocks_voted` | Voted blocks | whole number |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `carrier` | Carrier | yes or no |
| `stereo` | Stereo | yes or no |
| `stereo_pct` | Stereo blend | number (%) |
| `pilot_pct` | Pilot | number (%) |
| `pilot_locked` | Pilot locked | yes or no |
| `deviation_hz` | Peak deviation | frequency |
| `multipath_pct` | Multipath | number (%) |
| `equaliser` | Equaliser | yes or no |
| `squelched` | Squelched | yes or no |
| `channel_width_hz` | Channel width | frequency |
| `mpx_top_hz` | MPX top | frequency |
| `signal_time` | Signal decoded | duration (s) |
| `band_start_hz` | Band start | frequency |
| `band_end_hz` | Band end | frequency |
| `band_step_hz` | Channel step | frequency |
| `band_first_hz` | First channel | frequency |
| `tmc_ltn` | TMC location table | whole number |
| `tmc_sid` | TMC service | whole number |
| `tmc_afi` | TMC alternative frequencies | yes or no |
| `tmc_enhanced_mode` | TMC enhanced mode | yes or no |
| `tmc_scope` | TMC scope bits | whole number |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_peak_hz` | Peak deviation sent | frequency |
| `tx_mpx_power_db` | Multiplex power | number (dB) |
| `tx_groups` | RDS groups sent | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `af` | Alternative frequencies | `freq_hz` (frequency) |
| `label_tags` | Text tags | `code` (whole number), `type` (text), `text` (text) |
| `oda` | Open data applications | `aid` (whole number), `group` (text) |
| `eon` | Other networks | `pi` (text), `name` (text), `tp` (yes or no), `ta` (yes or no), `pty` (whole number), `pin` (text) |
| `eon_af` | Other networks' frequencies | `network` (text), `freq_hz` (frequency) |
| `tmc` | Traffic messages | `event` (whole number), `location` (whole number), `negative_direction` (yes or no), `extent` (whole number), `diversion` (yes or no), `duration` (whole number), `repeats` (whole number), `first` (time), `last` (time), `expires` (time) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `mpx` | MPX spectrum | Hz | dB |
| `rds_symbols` | RDS symbols |  |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fm.station` | FM station named | `source` (text), `text` (text), `family` (choice) |
| `fm.text` | FM RadioText complete | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the station's name, text and the other RDS data. |
| `start` | Start sending |  |  | Sends the FM broadcast signal without end: the audio input in stereo, with the pilot and RDS from the transmit settings, until stop. The setting tx_pi must be set. |
| `stop` | Stop sending |  |  | Ends the transmission at once, with the fall of the carrier. |

## Typed interface

The stack is in the family `fm`, version 1.2. The C header `sdrkit/fm.h`, the registry record `registry/fm.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FmFields`. The result fields of the FM receiver, in the order a screen lists them. The display writes the band plan ([`BandPart`]) and the measurements and RDS data ([`RdsPart`]); the broadcast data publisher writes the RadioText and whether its item is running.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station` | text | always | 1.0 | The station name (PS): all 8 characters with trailing spaces, empty before the first name. |
| `station_partial` | text | always | 1.0 | The name being collected (`_` for a segment not yet received). |
| `text` | text | always | 1.0 | The RadioText. |
| `text_complete` | yes or no | always | 1.0 | Every RadioText segment has arrived. |
| `programme` | text | optional | 1.0 | The programme type's name in the table in force. |
| `pty` | whole number | optional | 1.0 | The programme type code. |
| `pi` | text | optional | 1.0 | The programme identification, four hexadecimal digits. |
| `call_letters` | text | optional | 1.0 | RBDS call letters. |
| `rbds` | yes or no | always | 1.0 | RBDS meanings apply. |
| `tp` | yes or no | always | 1.0 | Traffic programme. |
| `ta` | yes or no | always | 1.0 | Traffic announcement. |
| `music` | yes or no | always | 1.0 | Music (true) or speech. |
| `ptyn` | text | always | 1.0 | Programme type name (PTYN). |
| `ecc` | whole number | optional | 1.0 | Extended country code. |
| `language` | whole number | optional | 1.0 | Language code. |
| `pin` | text | optional | 1.0 | Programme item number, `DD HH:MM`. |
| `clock` | time | optional | 1.0 | The station's clock time. |
| `clock_offset` | duration (s) | optional | 1.0 | The station's local time offset. |
| `ert` | text | always | 1.0 | Enhanced RadioText. |
| `item_running` | yes or no | always | 1.0 | The RadioText+ item is running. |
| `rds_sync` | yes or no | always | 1.0 | RDS block sync. |
| `rds_ok_pct` | number (%) | always | 1.0 | RDS usable blocks. |
| `groups` | whole number | always | 1.0 | Accepted RDS groups. |
| `blocks_good` | whole number | always | 1.0 | Good RDS blocks. |
| `blocks_corrected` | whole number | always | 1.0 | Corrected RDS blocks (voted ones too). |
| `blocks_bad` | whole number | always | 1.0 | Bad RDS blocks. |
| `blocks_voted` | whole number | always | 1.0 | RDS blocks recovered by voting. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel. |
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier. |
| `carrier` | yes or no | always | 1.0 | A carrier is present. |
| `stereo` | yes or no | always | 1.0 | Stereo is reported. |
| `stereo_pct` | number (%) | always | 1.0 | The stereo blend while stereo is reported. |
| `pilot_pct` | number (%) | always | 1.0 | The pilot injection, percent of 75 kHz. |
| `pilot_locked` | yes or no | always | 1.0 | The pilot PLL is locked. |
| `deviation_hz` | frequency | always | 1.0 | The peak deviation. |
| `multipath_pct` | number (%) | always | 1.0 | The multipath ripple. |
| `equaliser` | yes or no | always | 1.0 | The equaliser is in use for the sound. |
| `squelched` | yes or no | always | 1.0 | The squelch is closed. |
| `channel_width_hz` | frequency | always | 1.0 | The channel filter width in use. |
| `mpx_top_hz` | frequency | always | 1.0 | The upper frequency of the `mpx` series. |
| `signal_time` | duration (s) | always | 1.0 | Signal decoded since the last reset. |
| `band_start_hz` | frequency | always | 1.0 | The lowest frequency of the band. |
| `band_end_hz` | frequency | always | 1.0 | The highest frequency of the band. |
| `band_step_hz` | frequency | always | 1.0 | The channel step. |
| `band_first_hz` | frequency | always | 1.0 | The first channel of the raster. |
| `tmc_ltn` | whole number | optional | 1.0 | TMC location table number. |
| `tmc_sid` | whole number | optional | 1.0 | TMC service identifier. |
| `tmc_afi` | yes or no | always | 1.0 | TMC alternative-frequency indicator. |
| `tmc_enhanced_mode` | yes or no | always | 1.0 | TMC enhanced mode. |
| `tmc_scope` | whole number | always | 1.0 | TMC geographical scope bits (I, N, R, U). |

### Result fields

Struct `FmTxFields`. The result fields of the FM sender. The measurements have no value while the stack does not transmit.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.2 | Whether the stack transmits now. |
| `tx_freq_hz` | frequency | optional | 1.2 | The transmit frequency: the tuned frequency. |
| `tx_peak_hz` | frequency | optional | 1.2 | The largest deviation of the multiplex over the last report. |
| `tx_mpx_power_db` | number (dB) | optional | 1.2 | The power of the multiplex over the last 60 s against that of a sine of ±19 kHz deviation (ITU-R BS.412): 0 dB is the limit. |
| `tx_groups` | whole number | always | 1.2 | The RDS groups sent in this transmission. |

### Table `af`

Struct `AfRow`. A row of the alternative frequencies. The row key is the place in the list (`00`).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq_hz` | frequency | always | 1.0 | A frequency. |

### Table `label_tags`

Struct `LabelTagRow`. A row of the tags of the RadioText, which the broadcast data publisher writes. The row key is the class name of the content type (`ITEM.TITLE`), or its code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | whole number | always | 1.0 | The content type code (0 to 127). |
| `type` | text | optional | 1.0 | What people call the content type ("Title"). |
| `text` | text | always | 1.0 | The tagged text. |

### Table `oda`

Struct `OdaRow`. A row of the open data applications. The row key is the AID in hexadecimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aid` | whole number | always | 1.0 | An open data application identifier. |
| `group` | text | always | 1.0 | The group type that carries an application ("11A"). |

### Table `eon`

Struct `EonRow`. A row of the other networks. The row key is the PI in hexadecimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pi` | text | always | 1.0 | The programme identification, four hexadecimal digits. |
| `name` | text | always | 1.0 | A name. |
| `tp` | yes or no | always | 1.0 | Traffic programme. |
| `ta` | yes or no | always | 1.0 | Traffic announcement. |
| `pty` | whole number | optional | 1.0 | The programme type code. |
| `pin` | text | optional | 1.0 | Programme item number, `DD HH:MM`. |

### Table `eon_af`

Struct `EonAfRow`. A row of the alternative frequencies of the other networks. The row key is `<PI>/<place>` (`4A21/00`).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `network` | text | always | 1.0 | The PI of the other network a row belongs to. |
| `freq_hz` | frequency | always | 1.0 | A frequency. |

### Table `tmc`

Struct `TmcRow`. A row of the traffic messages. The row key is the place, oldest first (`00`).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `event` | whole number | always | 1.0 | A TMC event code. |
| `location` | whole number | always | 1.0 | A TMC location code. |
| `negative_direction` | yes or no | always | 1.0 | The TMC direction bit. |
| `extent` | whole number | always | 1.0 | The TMC extent. |
| `diversion` | yes or no | always | 1.0 | The TMC diversion bit. |
| `duration` | whole number | always | 1.0 | The TMC duration code. |
| `repeats` | whole number | always | 1.0 | How often a message came. |
| `first` | time | always | 1.0 | When a message came first. |
| `last` | time | always | 1.0 | When a message came last. |
| `expires` | time | always | 1.0 | When a message expires. |

### Event `fm.station`

Struct `StationEvent`. The fields of the two events of the FM receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The PI, four hexadecimal digits. |
| `text` | text | always | 1.0 | The station name, or the RadioText. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `fm.text`

Struct `StationEvent`. The fields of the two events of the FM receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The PI, four hexadecimal digits. |
| `text` | text | always | 1.0 | The station name, or the RadioText. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fm_clear` | none | none |
| `start` | `sdrkit_fm_start` | none | none |
| `stop` | `sdrkit_fm_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `stereo` | `sdrkit_fm_get_stereo` | `sdrkit_fm_set_stereo` |
| `force_mono` | `sdrkit_fm_get_force_mono` | `sdrkit_fm_set_force_mono` |
| `deemphasis` | `sdrkit_fm_get_deemphasis` | `sdrkit_fm_set_deemphasis` |
| `squelch` | `sdrkit_fm_get_squelch` | `sdrkit_fm_set_squelch` |
| `audio_bandwidth` | `sdrkit_fm_get_audio_bandwidth` | `sdrkit_fm_set_audio_bandwidth` |
| `bass` | `sdrkit_fm_get_bass` | `sdrkit_fm_set_bass` |
| `treble` | `sdrkit_fm_get_treble` | `sdrkit_fm_set_treble` |
| `warm` | `sdrkit_fm_get_warm` | `sdrkit_fm_set_warm` |
| `channel_width` | `sdrkit_fm_get_channel_width` | `sdrkit_fm_set_channel_width` |
| `channel_filter` | `sdrkit_fm_get_channel_filter` | `sdrkit_fm_set_channel_filter` |
| `band_low_hz` | `sdrkit_fm_get_band_low_hz` | `sdrkit_fm_set_band_low_hz` |
| `band_high_hz` | `sdrkit_fm_get_band_high_hz` | `sdrkit_fm_set_band_high_hz` |
| `first_hz` | `sdrkit_fm_get_first_hz` | `sdrkit_fm_set_first_hz` |
| `raster_hz` | `sdrkit_fm_get_raster_hz` | `sdrkit_fm_set_raster_hz` |
| `scan_low` | `sdrkit_fm_get_scan_low` | `sdrkit_fm_set_scan_low` |
| `scan_high` | `sdrkit_fm_get_scan_high` | `sdrkit_fm_set_scan_high` |
| `scan_full_rds` | `sdrkit_fm_get_scan_full_rds` | `sdrkit_fm_set_scan_full_rds` |
| `station_finder` | `sdrkit_fm_get_station_finder` | `sdrkit_fm_set_station_finder` |
| `rds_variant` | `sdrkit_fm_get_rds_variant` | `sdrkit_fm_set_rds_variant` |
| `rds_correction` | `sdrkit_fm_get_rds_correction` | `sdrkit_fm_set_rds_correction` |
| `rds_voting` | `sdrkit_fm_get_rds_voting` | `sdrkit_fm_set_rds_voting` |
| `rds_charset` | `sdrkit_fm_get_rds_charset` | `sdrkit_fm_set_rds_charset` |
| `tx_pi` | `sdrkit_fm_get_tx_pi` | `sdrkit_fm_set_tx_pi` |
| `tx_ps` | `sdrkit_fm_get_tx_ps` | `sdrkit_fm_set_tx_ps` |
| `tx_rt` | `sdrkit_fm_get_tx_rt` | `sdrkit_fm_set_tx_rt` |
| `tx_pty` | `sdrkit_fm_get_tx_pty` | `sdrkit_fm_set_tx_pty` |
| `tx_tp` | `sdrkit_fm_get_tx_tp` | `sdrkit_fm_set_tx_tp` |
| `tx_ta` | `sdrkit_fm_get_tx_ta` | `sdrkit_fm_set_tx_ta` |
| `tx_music` | `sdrkit_fm_get_tx_music` | `sdrkit_fm_set_tx_music` |
| `tx_af` | `sdrkit_fm_get_tx_af` | `sdrkit_fm_set_tx_af` |
| `tx_ct` | `sdrkit_fm_get_tx_ct` | `sdrkit_fm_set_tx_ct` |
| `tx_ct_offset` | `sdrkit_fm_get_tx_ct_offset` | `sdrkit_fm_set_tx_ct_offset` |
| `tx_stereo` | `sdrkit_fm_get_tx_stereo` | `sdrkit_fm_set_tx_stereo` |
| `tx_preemphasis` | `sdrkit_fm_get_tx_preemphasis` | `sdrkit_fm_set_tx_preemphasis` |
| `tx_deviation` | `sdrkit_fm_get_tx_deviation` | `sdrkit_fm_set_tx_deviation` |
| `tx_pilot` | `sdrkit_fm_get_tx_pilot` | `sdrkit_fm_set_tx_pilot` |
| `tx_rds_level` | `sdrkit_fm_get_tx_rds_level` | `sdrkit_fm_set_tx_rds_level` |
| `tx_limiter` | `sdrkit_fm_get_tx_limiter` | `sdrkit_fm_set_tx_limiter` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_stereo` | `sdrkit_fm_gen_get_stereo` | `sdrkit_fm_gen_set_stereo` |
| `gen_left_hz` | `sdrkit_fm_gen_get_left_hz` | `sdrkit_fm_gen_set_left_hz` |
| `gen_left_amp` | `sdrkit_fm_gen_get_left_amp` | `sdrkit_fm_gen_set_left_amp` |
| `gen_right_hz` | `sdrkit_fm_gen_get_right_hz` | `sdrkit_fm_gen_set_right_hz` |
| `gen_right_amp` | `sdrkit_fm_gen_get_right_amp` | `sdrkit_fm_gen_set_right_amp` |
| `gen_pilot` | `sdrkit_fm_gen_get_pilot` | `sdrkit_fm_gen_set_pilot` |
| `gen_rds` | `sdrkit_fm_gen_get_rds` | `sdrkit_fm_gen_set_rds` |
| `gen_rds_khz` | `sdrkit_fm_gen_get_rds_khz` | `sdrkit_fm_gen_set_rds_khz` |
| `gen_ps` | `sdrkit_fm_gen_get_ps` | `sdrkit_fm_gen_set_ps` |
| `gen_rt` | `sdrkit_fm_gen_get_rt` | `sdrkit_fm_gen_set_rt` |
| `gen_pi` | `sdrkit_fm_gen_get_pi` | `sdrkit_fm_gen_set_pi` |
| `gen_pty` | `sdrkit_fm_gen_get_pty` | `sdrkit_fm_gen_set_pty` |
| `gen_tp` | `sdrkit_fm_gen_get_tp` | `sdrkit_fm_gen_set_tp` |
| `gen_ta` | `sdrkit_fm_gen_get_ta` | `sdrkit_fm_gen_set_ta` |
| `gen_music` | `sdrkit_fm_gen_get_music` | `sdrkit_fm_gen_set_music` |
| `gen_preemphasis` | `sdrkit_fm_gen_get_preemphasis` | `sdrkit_fm_gen_set_preemphasis` |
| `gen_extra_groups` | `sdrkit_fm_gen_get_extra_groups` | `sdrkit_fm_gen_set_extra_groups` |

## Try it

```sh
sdrkit run fm --source synth:fm
sdrkit run fm --source iiod:192.168.2.1 --freq 100
```
<!-- generated: end -->

## Transmit

The stack sends a complete FM broadcast signal without end: stereo audio from the host's audio input, the 19 kHz pilot,
the 38 kHz difference signal and RDS on 57 kHz. `start` begins the transmission and `stop` ends it at once. The engine's
transmit setting is the only switch: with it off, `start` is refused and nothing goes on air. `start` is also refused,
with an error that names the setting, while `tx_pi` is empty: the stack has no programme identification of its own.

The sender and the test signal are one multiplex (`mpx.rs`), so both send the same tables and the same RDS waveform.

- Audio: stereo, left then right, from the host audio input of the graph (`host.in`). A host that gives stereo audio
  is sent in stereo; mono audio of the host goes to both channels. Any rate is taken to 48 kHz.
- Chain: pre-emphasis (`tx_preemphasis`, 50 or 75 µs, the library's pre-emphasis filter), a low-pass to
  15 kHz that is 50 dB down at 19 kHz, the rate change to 384 kHz, then a look-ahead limiter (1 ms, release 100 ms) with
  one gain for both channels. Nothing after the limiter can raise a peak: the audio reaches the output rate by straight
  lines between limited samples. With `tx_limiter` off each channel is clipped at full scale instead.
- Deviation: `tx_deviation` (default 75 kHz) is the peak of the whole multiplex. The pilot (`tx_pilot`, 9 %) and RDS
  (`tx_rds_level`, 2 kHz) take their share, and the audio part gets the rest: 88.3 % at the defaults. So the deviation
  never passes the set value, with any audio. The specification's budget (audio 90 % plus pilot and RDS, 101.7 %) is not
  used. Mono (`tx_stereo` off) sends no pilot and gives its share to the audio.
- RDS: 1187.5 bit/s, differentially coded, as biphase symbols shaped by `cos(π f T / 4)` up to 2/T (IEC 62106), so that
  the data spectrum is zero beyond 2375 Hz from 57 kHz. The subcarrier is `cos 3θ` of the pilot `sin θ`. The groups are
  four 0A (station name, flags, AF) and one 2A (radio text) in turn; without radio text only 0A. The decoder
  identification sends d0 = stereo in segment 3 and 0 in the others. `tx_ct` adds group 4A in place of the first group
  that starts after each minute of the clock (UTC), with the local offset `tx_ct_offset`.
- Changes: PI, station name, programme type, the flags and AF take effect at the next station name cycle; the radio text
  at the next text cycle, with the A/B flag turned over. The levels, stereo, pre-emphasis and the limiter apply at the
  next `start`.
- Start and end: the carrier rises and falls with a raised cosine of 5 ms; its magnitude is 0.9 of full scale. On a live
  radio whose host gives a transmit lead, the first sample states its time on air. The last sample carries the end mark.
  The receive audio is muted while the stack sends and for 0.1 s after. The stated start and the mute come from the
  framework's shared sender parts and are not tested on this page's stack.

The results `transmitting`, `tx_freq_hz`, `tx_peak_hz` (the peak deviation of the last report), `tx_mpx_power_db` (the
multiplex power over 60 s against a sine of ±19 kHz, ITU-R BS.412; measured, not enforced) and `tx_groups` show the
transmission.

What the tests measure (the PI codes in tests are artificial):

| Check | Result |
|---|---|
| The worked examples of the specification: groups 0A (segments 0 and 3), 2A and 4A for PI 4A21 | every block and every 26-bit word with checkword and offset word as listed; block A after differential coding as listed; the receiver's block layer takes them with no correction |
| Own receiver at 2 Msps: PS, RT, PI, PTY, TP, TA, M/S, clock group | all as sent; the pilot found at 9 %; the clock gives the minute that started and the offset; a new radio text shows within two text cycles |
| 1 kHz on the left only at half of full scale, 50 µs | level within 0.01 dB of the expected, separation 71.6 dB, THD+N 0.046 % |
| Peak deviation from the phase steps of the output at 1 Msps, 75 kHz set | full-scale noise 72.2 kHz; noise and a square wave 12 dB over full scale 71.1 and 71.6 kHz; the same noise with the limiter off 74.3 kHz; never above 75 kHz |
| Pilot from the samples | 9.000 % of the deviation, 0.04 Hz off 19 kHz |
| RDS spectrum | the power more than 2.4 kHz from 57 kHz is 71.3 dB under the power within 2.4 kHz |
| `start` with `tx_pi` empty, and with the transmitter off | refused, naming `tx_pi`; refused as transmit off; nothing on air |

### Licence

Broadcasting on the FM band needs a licence from the national regulator nearly everywhere. Some countries allow very low
power without a licence (for example the United States, 47 CFR 15.239). The library does not check this; it is a
regional rule for the host. The FM plan data (`data/plans/fm.json`) does not yet carry the note.
