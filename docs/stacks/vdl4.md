<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# VDL Mode 4

The VHF data link used for surveillance and position reports, on the VDL4 channels in the capture.

Stack id: `vdl4`. Group: Aviation.

Signal types: VDL Mode 4.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 118 MHz to 140 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 400 kHz |
| Signal bandwidth | 400 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:vdl4` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | text | empty |  | Frequencies in MHz separated by spaces. Empty watches the tuned frequency. The host fills it from a channel file (data/avdata/datalink_channels.csv). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames` | Frames | whole number |
| `bad` | Bad | whole number |
| `aircraft` | Aircraft | whole number |
| `ground_stations` | Ground stations | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `time` (time), `signal_s` (number (s)), `frequency` (frequency), `level_db` (number (dB)), `snr_db` (number (dB)), `address` (text), `address_kind` (text), `message` (text), `summary` (text), `dls_text` (text), `position` (position), `alt_ft` (number (ft)) |
| `aircraft` | Aircraft | `address` (text), `registration` (text), `country` (text), `flight` (text), `label` (text), `messages` (whole number), `position` (position), `alt_ft` (number (ft)), `track_deg` (number (deg)), `gs_kt` (number (kt)), `vs_fpm` (number (ft/min)), `last_heard` (time), `count` (whole number), `frequency` (frequency), `trail_points` (whole number) |
| `ground_stations` | Ground stations | `address` (text), `count` (whole number), `frequency` (frequency), `last_heard` (time), `position` (position) |
| `channels` | Channels | `frequency` (frequency), `in_band` (yes or no), `found` (whole number), `good` (whole number), `bad` (whole number), `level_db` (number (dB)), `snr_db` (number (dB)) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `vdl4.message` | VDL4 message | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `message` (text), `position` (position) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `avdata`, version 1.4. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: text.

### Result fields

Struct `Vdl4Fields`. The result fields of VDL Mode 4.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames` | whole number | always | 1.0 | Frames with a good FCS. |
| `bad` | whole number | always | 1.0 | Bursts that failed on the channels now heard. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the table. |
| `ground_stations` | whole number | always | 1.0 | Ground stations in the table. |

### Table `messages`

Struct `Vdl4MessageRow`. A row of the messages. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The wall-clock time. No value for a recording. |
| `signal_s` | number (s) | always | 1.0 | The signal time. |
| `frequency` | frequency | always | 1.0 | The channel. |
| `level_db` | number (dB) | always | 1.0 | The level of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the burst. |
| `address` | text | always | 1.0 | The address of the sender, six hex digits. |
| `address_kind` | text | always | 1.0 | The kind of the address. |
| `message` | text | optional | 1.0 | The name of the message. |
| `summary` | text | optional | 1.0 | One line. |
| `dls_text` | text | optional | 1.0 | The text of the DLS. |
| `position` | position | optional | 1.0 | The position the message gives. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude the message gives. |

The rows are the messages of the stack. The column `address` names a row. Place of a row: the column `position`. Height: `alt_ft`. Sender: `address`. Text: `summary`. Time: `time`.

### Table `aircraft`

Struct `Vdl4AircraftRow`. A row of the aircraft. The row key is the address.

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
| `count` | whole number | always | 1.0 | Messages heard from it on the link. |
| `frequency` | frequency | always | 1.0 | The channel of the last burst. |
| `trail_points` | whole number | always | 1.0 | The points of the trail. |

The rows are the stations or nodes of the stack. The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`. Time: `last_heard`.

### Table `ground_stations`

Struct `Vdl4StationRow`. A row of the ground stations. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address, six hex digits. |
| `count` | whole number | always | 1.0 | Messages of the station. |
| `frequency` | frequency | always | 1.0 | The channel. |
| `last_heard` | time | optional | 1.0 | When the station was last heard. |
| `position` | position | optional | 1.0 | The position the station gave. |

The column `address` names a row. Place of a row: the column `position`.

### Table `channels`

Struct `Vdl4ChannelRow`. A row of the channels heard now. The row key is the frequency in hertz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The channel. |
| `in_band` | yes or no | always | 1.0 | The channel is inside the band of the input. |
| `found` | whole number | always | 1.0 | Bursts found. |
| `good` | whole number | always | 1.0 | Good bursts. |
| `bad` | whole number | always | 1.0 | Bad bursts. |
| `level_db` | number (dB) | always | 1.0 | The level of the channel. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the channel. |

### Event `vdl4.message`

Struct `Vdl4MessageEvent`. A VDL4 message. The source is the address of the sender, six hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address of the sender. |
| `text` | text | always | 1.0 | The summary of the message. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the burst. |
| `family` | choice | always | 1.0 | What the event is about: an aircraft. |
| `message` | text | always | 1.0 | The name of the message. |
| `position` | position | optional | 1.0 | The position the message gives. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_vdl4_clear` | none | none |
| `remove` | `sdrkit_vdl4_remove` | `sdrkit_vdl4_station_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_vdl4_get_channels` | `sdrkit_vdl4_set_channels` |

## Try it

```sh
sdrkit run vdl4 --source synth:vdl4
sdrkit run vdl4 --source <driver>:<address> --freq 0
```
<!-- generated: end -->
