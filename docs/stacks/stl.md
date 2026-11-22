<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# STL link

Studio-to-transmitter links of broadcast stations: the sound and RDS of an analogue FM link; a digital link detected and measured, not decoded.

Stack id: `stl`. Group: Broadcast radio.

Signal types: FM broadcast, Wide FM, Wide digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 947.5 MHz |
| Tuning range | 100 MHz to 6 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 280 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:stl` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `stereo` | Stereo | choice | `auto` | `auto` (Auto), `mono` (Mono), `stereo` (Stereo) | Auto plays stereo while the pilot tone is locked and moves towards mono as the signal gets weak. Mono always plays mono. Stereo plays full stereo whenever any pilot is there. |
| `deviation` | Full deviation | frequency | 75 kHz | 10 kHz to 200 kHz | The deviation of full modulation of the link: 75 kHz, or 50 kHz where a region uses that (ITU-R BS.450). It sets the sound level only; the measurements stay in Hz. |
| `auto_width` | Automatic width | yes or no | true |  | The channel width follows the link: twice the peak deviation plus the highest multiplex frequency (60 kHz, 99 kHz with an SCA subcarrier), or the occupied width if that is more, with 35 % and 10 kHz of margin. Each change restarts the channel filter. |
| `width` | Channel width | frequency | 300 kHz | 10 kHz to 400 kHz | The channel width when the automatic width is off. At most 400 kHz and 0.9 times the sample rate. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `deemphasis` | De-emphasis | choice | `50` | `off` (Off), `50` (50 µs), `75` (75 µs) | Undoes the treble boost stations add. 50 µs in Europe, Asia, Africa and Australia. 75 µs in the Americas and South Korea. |
| `rds_variant` | RDS meanings | choice | `auto` | `auto` (Auto), `rds` (RDS), `rbds` (RBDS) | How programme types and station codes are read. Auto reads RDS if the station sends a country code outside the Americas, else RBDS on the channel raster of RBDS (odd tenths of a MHz from 87.9 to 107.9 MHz) and RDS elsewhere. |
| `rds_correction` | RDS correction | choice | `2` | `0` (Off), `1` (Short bursts), `2` (Full) | How much the stack repairs damaged RDS data. Full gets the most from a weak station. A wrong character is still never shown. |
| `rds_voting` | RDS voting | yes or no | true |  | With full correction, also repairs data by comparing repeated copies. |
| `rds_charset` | RDS characters | choice | `1` | `0` (EBU Latin), `1` (Auto GB2312), `2` (GB2312) | How station names and texts are read. Auto reads Chinese (GB2312) text when it sees it. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_link` | Link | choice | `composite` | `composite` (Composite stereo), `mono` (Mono), `digital` (Digital-like) | Composite sends the stereo multiplex with pilot and RDS, mono sends programme audio without a pilot, digital-like sends band-limited noise. |
| `gen_deviation` | Full deviation | frequency | 75 kHz | 10 kHz to 200 kHz | The deviation of a full-scale multiplex. |
| `gen_sca` | SCA subcarriers | yes or no | false |  | Adds subcarriers at 67 and 92 kHz, each at 10 % of full deviation and frequency modulated by a 1 kHz tone. |
| `gen_width` | Digital width | frequency | 220 kHz | 20 kHz to 400 kHz | The occupied width of the digital-like link. |
| `gen_left_hz` | Left tone | frequency | 1 kHz | 0 Hz to 15 kHz |  |
| `gen_left_amp` | Left level | number | 0.5 | 0 to 1 | 0 is silent. |
| `gen_right_hz` | Right tone | frequency | 3 kHz | 0 Hz to 15 kHz |  |
| `gen_right_amp` | Right level | number | 0.5 | 0 to 1 | 0 is silent. |
| `gen_rds` | RDS | yes or no | true |  |  |
| `gen_ps` | Station name | text | `SDRKIT ` |  | Up to 8 characters. |
| `gen_rt` | Radio text | text | `sdrkit FM test signal` |  | Up to 64 characters. |
| `gen_pi` | PI code | text | `4A21` |  | Four hexadecimal digits. |
| `gen_pty` | Programme type | whole number | 10 | 0 to 31 |  |
| `gen_preemphasis` | Pre-emphasis | choice | `50` | `0` (None), `50` (50 µs), `75` (75 µs) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `link` | Link | choice |
| `peak_deviation_hz` | Peak deviation | frequency |
| `modulation_pct` | Modulation | number (%) |
| `pilot_hz` | Pilot level | frequency |
| `width_auto` | Automatic width | yes or no |
| `floor_measured` | Floor measured | yes or no |
| `occupied_width_hz` | Occupied width | frequency |
| `over_floor_db` | Over the floor | number (dB) |
| `flatness_db` | Flatness | number (dB) |
| `sca67_snr_db` | SCA 67 kHz | number (dB) |
| `sca92_snr_db` | SCA 92 kHz | number (dB) |
| `sound_peak_dbfs` | Sound peak | number (dBFS) |
| `sound_rms_dbfs` | Sound RMS | number (dBFS) |
| `station` | Station | text |
| `station_partial` | Station so far | text |
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
| `tmc_ltn` | TMC location table | whole number |
| `tmc_sid` | TMC service | whole number |
| `tmc_afi` | TMC alternative frequencies | yes or no |
| `tmc_enhanced_mode` | TMC enhanced mode | yes or no |
| `tmc_scope` | TMC scope bits | whole number |
| `text` | Text | text |
| `item_running` | Item running | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `links` | Link changes | `time_s` (duration (s)), `link` (choice), `text` (text), `over_floor_db` (number (dB)) |
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
| `stl.link` | STL link changed | `link` (choice), `text` (text), `over_floor_db` (number (dB)) |
| `fm.station` | FM station named | `source` (text), `text` (text), `family` (choice) |
| `fm.text` | FM RadioText complete | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the station's name, text and the other RDS data. |

## Typed interface

The stack is in the family `fm`, version 1.3. The C header `sdrkit/fm.h`, the registry record `registry/fm.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Broadcast radio (`broadcast`). Media: sound (programme).

### Result fields

Struct `StlFields`. The result fields of the STL link, beside the reception fields of the FM components.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | choice | always | 1.1 | What the link is. A change holds after two reports of 0.5 s in a row. |
| `peak_deviation_hz` | frequency | optional | 1.1 | The peak deviation over the last 0.5 s, noise included. No value without a carrier. |
| `modulation_pct` | number (%) | optional | 1.1 | The peak deviation against the deviation of full modulation. No value without a carrier. |
| `pilot_hz` | frequency | optional | 1.1 | The pilot level, as deviation. No value while the pilot is not locked. |
| `width_auto` | yes or no | always | 1.1 | The channel width follows the link (the automatic width). |
| `floor_measured` | yes or no | always | 1.1 | The spectrum of the capture reaches far enough past the channel to measure the floor. Without it the digital check is off. |
| `occupied_width_hz` | frequency | optional | 1.1 | The width of the occupied region. No value while none stands out. |
| `over_floor_db` | number (dB) | optional | 1.1 | The mean of the occupied region over the floor. |
| `flatness_db` | number (dB) | optional | 1.1 | The flatness of the occupied region: its 10th percentile against its mean (0 dB is flat). |
| `sca67_snr_db` | number (dB) | optional | 1.1 | The SNR of the SCA subcarrier at 67 kHz. No value while it is absent. |
| `sca92_snr_db` | number (dB) | optional | 1.1 | The SNR of the SCA subcarrier at 92 kHz. No value while it is absent. |
| `sound_peak_dbfs` | number (dBFS) | optional | 1.1 | The peak of the sound over the last 0.5 s. No value for silence. |
| `sound_rms_dbfs` | number (dBFS) | optional | 1.1 | The RMS of the sound over the last 0.5 s. No value for silence. |

### Result fields

Struct `RdsPart`. The measurements of the FM receiver and the RDS data: a part of [`FmFields`], and a result set of its own in a stack that runs the FM receiver without the band plan.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station` | text | always | 1.1 | The station name (PS): all 8 characters with trailing spaces, empty before the first name. |
| `station_partial` | text | always | 1.1 | The name being collected (`_` for a segment not yet received). |
| `text_complete` | yes or no | always | 1.1 | Every RadioText segment has arrived. |
| `programme` | text | optional | 1.1 | The programme type's name in the table in force. |
| `pty` | whole number | optional | 1.1 | The programme type code. |
| `pi` | text | optional | 1.1 | The programme identification, four hexadecimal digits. |
| `call_letters` | text | optional | 1.1 | RBDS call letters. |
| `rbds` | yes or no | always | 1.1 | RBDS meanings apply. |
| `tp` | yes or no | always | 1.1 | Traffic programme. |
| `ta` | yes or no | always | 1.1 | Traffic announcement. |
| `music` | yes or no | always | 1.1 | Music (true) or speech. |
| `ptyn` | text | always | 1.1 | Programme type name (PTYN). |
| `ecc` | whole number | optional | 1.1 | Extended country code. |
| `language` | whole number | optional | 1.1 | Language code. |
| `pin` | text | optional | 1.1 | Programme item number, `DD HH:MM`. |
| `clock` | time | optional | 1.1 | The station's clock time. |
| `clock_offset` | duration (s) | optional | 1.1 | The station's local time offset. |
| `ert` | text | always | 1.1 | Enhanced RadioText. |
| `rds_sync` | yes or no | always | 1.1 | RDS block sync. |
| `rds_ok_pct` | number (%) | always | 1.1 | RDS usable blocks. |
| `groups` | whole number | always | 1.1 | Accepted RDS groups. |
| `blocks_good` | whole number | always | 1.1 | Good RDS blocks. |
| `blocks_corrected` | whole number | always | 1.1 | Corrected RDS blocks (voted ones too). |
| `blocks_bad` | whole number | always | 1.1 | Bad RDS blocks. |
| `blocks_voted` | whole number | always | 1.1 | RDS blocks recovered by voting. |
| `snr_db` | number (dB) | always | 1.1 | The signal to noise ratio. |
| `level_dbfs` | number (dBFS) | always | 1.1 | The level of the channel. |
| `offset_hz` | frequency | always | 1.1 | The offset of the carrier. |
| `carrier` | yes or no | always | 1.1 | A carrier is present. |
| `stereo` | yes or no | always | 1.1 | Stereo is reported. |
| `stereo_pct` | number (%) | always | 1.1 | The stereo blend while stereo is reported. |
| `pilot_pct` | number (%) | always | 1.1 | The pilot injection, percent of 75 kHz. |
| `pilot_locked` | yes or no | always | 1.1 | The pilot PLL is locked. |
| `deviation_hz` | frequency | always | 1.1 | The peak deviation. |
| `multipath_pct` | number (%) | always | 1.1 | The multipath ripple. |
| `equaliser` | yes or no | always | 1.1 | The equaliser is in use for the sound. |
| `squelched` | yes or no | always | 1.1 | The squelch is closed. |
| `channel_width_hz` | frequency | always | 1.1 | The channel filter width in use. |
| `mpx_top_hz` | frequency | always | 1.1 | The upper frequency of the `mpx` series. |
| `signal_time` | duration (s) | always | 1.1 | Signal decoded since the last reset. |
| `tmc_ltn` | whole number | optional | 1.1 | TMC location table number. |
| `tmc_sid` | whole number | optional | 1.1 | TMC service identifier. |
| `tmc_afi` | yes or no | always | 1.1 | TMC alternative-frequency indicator. |
| `tmc_enhanced_mode` | yes or no | always | 1.1 | TMC enhanced mode. |
| `tmc_scope` | whole number | always | 1.1 | TMC geographical scope bits (I, N, R, U). |

### Result fields

Struct `LabelFields`. The radio text and whether its item is running.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | optional | 1.1 | The radio text. |
| `item_running` | yes or no | optional | 1.1 | The tagged item (a song) is playing. |

### Table `links`

Struct `LinkRow`. A held change of the link. The row key is the count of changes (`0001`).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.1 | The signal time of the change. |
| `link` | choice | always | 1.1 | What the link became. |
| `text` | text | always | 1.1 | The details: pilot, peak deviation and offset of an FM link, or level and width of a signal that is not FM. |
| `over_floor_db` | number (dB) | optional | 1.1 | The occupied region over the floor. No value without a region. |

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

The column `name` names a row.

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

### Event `stl.link`

Struct `LinkEvent`. The fields of a change of the link.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | choice | always | 1.1 | What the link became. |
| `text` | text | always | 1.1 | The details, as in the table of changes. |
| `over_floor_db` | number (dB) | optional | 1.1 | The occupied region over the floor. No value without a region. |

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
| `clear` | `sdrkit_stl_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `stereo` | `sdrkit_stl_get_stereo` | `sdrkit_stl_set_stereo` |
| `deemphasis` | `sdrkit_stl_get_deemphasis` | `sdrkit_stl_set_deemphasis` |
| `deviation` | `sdrkit_stl_get_deviation` | `sdrkit_stl_set_deviation` |
| `auto_width` | `sdrkit_stl_get_auto_width` | `sdrkit_stl_set_auto_width` |
| `width` | `sdrkit_stl_get_width` | `sdrkit_stl_set_width` |
| `rds_variant` | `sdrkit_stl_get_rds_variant` | `sdrkit_stl_set_rds_variant` |
| `rds_correction` | `sdrkit_stl_get_rds_correction` | `sdrkit_stl_set_rds_correction` |
| `rds_voting` | `sdrkit_stl_get_rds_voting` | `sdrkit_stl_set_rds_voting` |
| `rds_charset` | `sdrkit_stl_get_rds_charset` | `sdrkit_stl_set_rds_charset` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_link` | `sdrkit_stl_gen_get_link` | `sdrkit_stl_gen_set_link` |
| `gen_deviation` | `sdrkit_stl_gen_get_deviation` | `sdrkit_stl_gen_set_deviation` |
| `gen_sca` | `sdrkit_stl_gen_get_sca` | `sdrkit_stl_gen_set_sca` |
| `gen_width` | `sdrkit_stl_gen_get_width` | `sdrkit_stl_gen_set_width` |
| `gen_left_hz` | `sdrkit_stl_gen_get_left_hz` | `sdrkit_stl_gen_set_left_hz` |
| `gen_left_amp` | `sdrkit_stl_gen_get_left_amp` | `sdrkit_stl_gen_set_left_amp` |
| `gen_right_hz` | `sdrkit_stl_gen_get_right_hz` | `sdrkit_stl_gen_set_right_hz` |
| `gen_right_amp` | `sdrkit_stl_gen_get_right_amp` | `sdrkit_stl_gen_set_right_amp` |
| `gen_rds` | `sdrkit_stl_gen_get_rds` | `sdrkit_stl_gen_set_rds` |
| `gen_ps` | `sdrkit_stl_gen_get_ps` | `sdrkit_stl_gen_set_ps` |
| `gen_rt` | `sdrkit_stl_gen_get_rt` | `sdrkit_stl_gen_set_rt` |
| `gen_pi` | `sdrkit_stl_gen_get_pi` | `sdrkit_stl_gen_set_pi` |
| `gen_pty` | `sdrkit_stl_gen_get_pty` | `sdrkit_stl_gen_set_pty` |
| `gen_preemphasis` | `sdrkit_stl_gen_get_preemphasis` | `sdrkit_stl_gen_set_preemphasis` |

## Try it

```sh
sdrkit run stl --source synth:stl
sdrkit run stl --source iiod:192.168.2.1 --freq 947.5
```
<!-- generated: end -->

## Description

Standards: ITU-R BS.450 (deviation, pre-emphasis, the pilot-tone stereo multiplex), IEC 62106 and NRSC-4-B (RDS on 57 kHz and its meanings), 47 CFR 74.502 (the aural STL band of the United States).

## What it does

A studio-to-transmitter link (STL) carries the programme of a broadcast station from the studio to the transmitter site. The stack receives only.

An analogue link is a wide FM carrier. A composite link carries the whole stereo multiplex of the transmitter: the mono sum to 15 kHz, the 19 kHz pilot, the difference signal on a suppressed 38 kHz carrier, RDS on 57 kHz, and in the United States SCA subcarriers at 67 and 92 kHz. A mono link carries programme audio without a pilot. The stack plays the sound and reads RDS with the channel filter, demodulator, stereo decoder, de-emphasis and RDS decoder of the FM stack.

A digital link (QAM or OFDM with vendor frames, often encrypted) is detected and measured: its occupied width, its level over the floor and its flatness. Nothing inside it is decoded: no public document describes its frames. The headline then ends with "not decoded".

Every 0.5 s the stack decides what the link is:

| Link | When |
|---|---|
| Composite (stereo multiplex) | the FM demodulator sees a carrier and the pilot is locked |
| Mono | a carrier without a pilot (the stack cannot tell a programme link from a mono multiplex) |
| Digital or unknown (not decoded) | no carrier; an occupied region at least 6 dB over the floor, flatter than -3 dB and at least 60 kHz wide |
| Unknown (not FM) | no carrier; another region at least 6 dB over the floor |
| Nothing | anything else |

The first reading holds at once; a change holds when the same new kind comes in two reports in a row. Each held change adds a row to the table `links` and sends the event `stl.link`.

The floor is the median of the spectrum from 0.75 × max(100 kHz, the channel width) to 0.4 × the sample rate from the centre. The comparison needs at least 24 bins there, in practice a sample rate of about 1.9 times the channel width or more. Without it the field `floor_measured` is false and the digital check is off.

The channel width follows the link (setting `auto_width`): twice the peak deviation hold plus the highest multiplex frequency (60 kHz, 99 kHz with an SCA subcarrier), or the occupied width if that is more, times 1.35 plus 10 kHz. It starts at 400 kHz, widens at once, and narrows after 1.5 s the first time and 30 s later. Each change restarts the channel filter, which clicks the sound. The channel centre moves to the centre of the occupied region when the two are more than 4 kHz apart; this move is phase continuous.

The setting `deviation` is the deviation of full modulation of the link. It sets the sound level only; the measurements stay in Hz. The sound is silent while the demodulator sees no carrier.

## Settings: notes

The channel is at most 400 kHz wide: the FM demodulator works at a channel rate near 500 kHz. A link of twenty 25 kHz segments (500 kHz) is received in 400 kHz, which holds a fully modulated composite link with SCA subcarriers (2 × (75 + 99) kHz by Carson's rule).

The defaults of `deemphasis` (50 µs) and `rds_variant` (automatic) are those of the FM stack. In the United States set 75 µs and RBDS.

## Region data

`data/plans/stl.json` holds the aural STL band of the United States (group `stl_us`, listed in the file for the country `US`): 944 to 952 MHz, a raster of 25 kHz segments, the first centred on 944.0125 MHz. A link stacks segments to at most 300 kHz, or 500 kHz for twenty segments. No published band was found for other regions, so the file has none.

## What no public document confirms

- The SCA subcarriers at 67 and 92 kHz (United States practice), and their level and deviation in the test signal (10 % of full deviation each, ±6 kHz by a 1 kHz tone).
- The rules that decide the link kind, the automatic width and the link centre.
- The frames of digital links.
- The plan file records its source as the text of 47 CFR 74.502 that a law library site publishes, not the eCFR.

## Not done

The sound of the SCA subcarriers is not played: the SCA meter measures their SNR only.

## Test signal

The test link (`gen_link`) is composite (default), mono or digital-like. The composite link is the FM test transmitter at the deviation `gen_deviation`, with the pilot at 9 %, RDS and pre-emphasis; `gen_sca` adds the SCA subcarriers. The mono link sends the tones without a pilot and without RDS. The digital-like link is band-limited Gaussian noise `gen_width` wide. Each is at unit power.
