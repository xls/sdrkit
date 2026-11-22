<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# VDL Mode 2

The VHF digital data link of airliners: ACARS, ATN and link frames on every VDL2 channel in the capture.

Stack id: `vdl2`. Group: Aviation.

Signal types: VDL Mode 2.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 118 MHz to 140 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 400 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:vdl2` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | text | empty |  | Frequencies in MHz separated by spaces. Empty watches the common signalling channel 136.975 MHz and the tuned frequency. The host fills it from a channel file (data/avdata/datalink_channels.csv). Tune between the channels. |
| `track_tuning` | Track tuning | yes or no | true |  | Measures the radio's tuning error from the bursts and takes it out. |
| `tune_error_hz` | Tuning error guess | frequency | 0 Hz | -6 kHz to 6 kHz | The radio's tuning error to start from. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_aircraft` | Test aircraft | whole number | 6 | 1 to 24 | Aircraft in the test signal. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `bursts_bad` | Bad bursts | whole number |
| `octets_corrected` | Octets corrected | whole number |
| `headers_fixed` | Headers corrected | whole number |
| `fcs_bad` | Bad FCS | whole number |
| `positions` | Positions | whole number |
| `tuning_error` | Tuning error | frequency |
| `aircraft` | Aircraft | whole number |
| `ground_stations` | Ground stations | whole number |
| `messages` | Frames | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Frames | `time` (time), `signal_s` (number (s)), `frequency` (frequency), `level_db` (number (dB)), `snr_db` (number (dB)), `from` (text), `from_type` (whole number), `to` (text), `to_type` (whole number), `frame_kind` (text), `frame` (text), `response` (yes or no), `protocol` (text), `summary` (text), `registration` (text), `flight` (text), `label` (text), `text` (text), `acars_ok` (yes or no), `blocks` (whole number), `reading_kind` (text), `reading_category` (text), `cpdlc_imi` (text), `cpdlc_ground` (text), `cpdlc_aircraft` (text), `cpdlc_crc_ok` (yes or no), `cpdlc_parsed` (yes or no), `cpdlc_text` (text), `position` (position), `alt_ft` (number (ft)), `detail` (text) |
| `aircraft` | Aircraft | `address` (text), `registration` (text), `country` (text), `flight` (text), `label` (text), `messages` (whole number), `position` (position), `alt_ft` (number (ft)), `track_deg` (number (deg)), `gs_kt` (number (kt)), `vs_fpm` (number (ft/min)), `last_heard` (time), `count` (whole number), `frequency` (frequency), `ground_station` (text), `estimate` (position), `trail_points` (whole number) |
| `ground_stations` | Ground stations | `address` (text), `kind` (text), `count` (whole number), `frequency` (frequency), `level_db` (number (dB)), `last_heard` (time), `airports` (text), `position` (position) |
| `channels` | Channels | `frequency` (frequency), `in_band` (yes or no), `active` (yes or no), `level_db` (number (dB)), `snr_db` (number (dB)), `found` (whole number), `good` (whole number), `bad` (whole number), `frames` (whole number), `offset_hz` (frequency) |

## Map points

The stack puts points on a map: Aircraft and ground stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `vdl2.message` | VDL2 frame | `source` (text), `target` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `from_air` (yes or no), `protocol` (text), `frame` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `avdata`, version 1.3. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: text.

### Result fields

Struct `Vdl2Fields`. The result fields of VDL Mode 2.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Good frames. |
| `bursts_bad` | whole number | always | 1.0 | Bursts that failed. |
| `octets_corrected` | whole number | always | 1.0 | Octets the Reed-Solomon code corrected. |
| `headers_fixed` | whole number | always | 1.0 | Headers that needed their FEC. |
| `fcs_bad` | whole number | always | 1.0 | Frames with a bad FCS. |
| `positions` | whole number | always | 1.0 | Frames with a position. |
| `tuning_error` | frequency | optional | 1.0 | How far the radio is off the channel grid. No value before the first report of the channels. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the table. |
| `ground_stations` | whole number | always | 1.0 | Ground stations in the table. |
| `messages` | whole number | always | 1.0 | Frames in the list. |

### Table `messages`

Struct `Vdl2MessageRow`. A row of the frames. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The wall-clock time. No value for a recording. |
| `signal_s` | number (s) | always | 1.0 | The signal time. |
| `frequency` | frequency | always | 1.0 | The channel. |
| `level_db` | number (dB) | always | 1.0 | The level of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the burst. |
| `from` | text | always | 1.0 | The source address, six hex digits. |
| `from_type` | whole number | always | 1.0 | The type of the source address. |
| `to` | text | always | 1.0 | The destination address, six hex digits. |
| `to_type` | whole number | always | 1.0 | The type of the destination address. |
| `frame_kind` | text | optional | 1.0 | I, S or U. |
| `frame` | text | optional | 1.0 | The name of the frame. |
| `response` | yes or no | always | 1.0 | The C/R bit of the source. |
| `protocol` | text | optional | 1.0 | The deepest layer read. |
| `summary` | text | optional | 1.0 | One line. |
| `registration` | text | optional | 1.0 | The registration of an ACARS message. |
| `flight` | text | optional | 1.0 | The flight id of an ACARS message. |
| `label` | text | optional | 1.0 | The label of an ACARS message. |
| `text` | text | optional | 1.0 | The text of an ACARS message. |
| `acars_ok` | yes or no | always | 1.0 | The ACARS check sequence passed. |
| `blocks` | whole number | optional | 1.0 | Blocks joined into the ACARS message. |
| `reading_kind` | text | optional | 1.0 | The kind of the reading. |
| `reading_category` | text | optional | 1.0 | The category of the reading. |
| `cpdlc_imi` | text | optional | 1.0 | The CPDLC envelope: AT1, CR1, CC1 or DR1. |
| `cpdlc_ground` | text | optional | 1.0 | The CPDLC ground facility. |
| `cpdlc_aircraft` | text | optional | 1.0 | The aircraft named in the CPDLC envelope. |
| `cpdlc_crc_ok` | yes or no | optional | 1.0 | The CPDLC envelope check passed. |
| `cpdlc_parsed` | yes or no | optional | 1.0 | The CPDLC message body decoded completely. |
| `cpdlc_text` | text | optional | 1.0 | The decoded CPDLC message. |
| `position` | position | optional | 1.0 | The position the frame gives. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude the frame gives. |
| `detail` | text | optional | 1.0 | Every layer, one line each, joined by " \| ". |

The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`.

### Table `aircraft`

Struct `Vdl2AircraftRow`. A row of the aircraft. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | optional | 1.0 | The 24-bit address, six hex digits. |
| `registration` | text | optional | 1.0 | The registration. |
| `country` | text | optional | 1.0 | The State the address is allocated to. |
| `flight` | text | optional | 1.0 | The flight id. |
| `label` | text | optional | 1.0 | The label of the last message. |
| `messages` | whole number | always | 1.0 | Messages of the aircraft. |
| `position` | position | optional | 1.0 | The last position. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude of the last position. |
| `track_deg` | number (deg) | optional | 1.0 | The track. |
| `gs_kt` | number (kt) | optional | 1.0 | The ground speed. |
| `vs_fpm` | number (ft/min) | optional | 1.0 | The vertical rate. |
| `last_heard` | time | optional | 1.0 | When the aircraft was last heard. |
| `count` | whole number | always | 1.0 | Frames to or from the aircraft. |
| `frequency` | frequency | always | 1.0 | The channel of the last frame. |
| `ground_station` | text | optional | 1.0 | The ground station the aircraft last talked to. |
| `estimate` | position | optional | 1.0 | The position now, by dead reckoning from the last fix. |
| `trail_points` | whole number | always | 1.0 | The points of the trail. |

The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.

### Table `ground_stations`

Struct `Vdl2StationRow`. A row of the ground stations. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address, six hex digits. |
| `kind` | text | always | 1.0 | The kind of the station. |
| `count` | whole number | always | 1.0 | Frames the station sent. |
| `frequency` | frequency | always | 1.0 | The channel. |
| `level_db` | number (dB) | always | 1.0 | The level of its last frame. |
| `last_heard` | time | optional | 1.0 | When the station was last heard. |
| `airports` | text | optional | 1.0 | The airports the station covers. |
| `position` | position | optional | 1.0 | The position from its GSIF. |

The column `address` names a row. Place of a row: the column `position`.

### Table `channels`

Struct `Vdl2ChannelRow`. A row of the channels heard now. The row key is the frequency in hertz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The channel. |
| `in_band` | yes or no | always | 1.0 | The channel is inside the band of the input. |
| `active` | yes or no | always | 1.0 | A good burst in the last 2 s. |
| `level_db` | number (dB) | always | 1.0 | The level of the channel. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the channel. |
| `found` | whole number | always | 1.0 | Bursts found. |
| `good` | whole number | always | 1.0 | Good bursts. |
| `bad` | whole number | always | 1.0 | Bad bursts. |
| `frames` | whole number | always | 1.0 | Frames. |
| `offset_hz` | frequency | always | 1.0 | The offset of the last burst. |

### Event `vdl2.message`

Struct `Vdl2MessageEvent`. A VDL2 frame. The source and the target are the addresses of the sender and the receiver, six hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address of the sender. |
| `target` | text | always | 1.0 | The address of the receiver. |
| `text` | text | always | 1.0 | The summary of the frame. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the burst. |
| `family` | choice | always | 1.0 | What the event is about: an aircraft. |
| `from_air` | yes or no | always | 1.0 | An aircraft sent the frame. |
| `protocol` | text | always | 1.0 | The deepest layer read. |
| `frame` | text | always | 1.0 | The name of the frame. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_vdl2_clear` | none | none |
| `remove` | `sdrkit_vdl2_remove` | `sdrkit_vdl2_station_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_vdl2_get_channels` | `sdrkit_vdl2_set_channels` |
| `track_tuning` | `sdrkit_vdl2_get_track_tuning` | `sdrkit_vdl2_set_track_tuning` |
| `tune_error_hz` | `sdrkit_vdl2_get_tune_error_hz` | `sdrkit_vdl2_set_tune_error_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_aircraft` | `sdrkit_vdl2_gen_get_aircraft` | `sdrkit_vdl2_gen_set_aircraft` |

## Try it

```sh
sdrkit run vdl2 --source synth:vdl2
sdrkit run vdl2 --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
