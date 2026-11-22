<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FANET

Paraglider, hang glider and glider tracking on one LoRa channel: positions, names, messages, weather stations and thermals.

Stack id: `fanet`. Group: IoT and mesh.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 860 MHz to 930 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 500 kHz |
| Offset from the signal | 600 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fanet` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bandwidth` | Bandwidth | choice | `250` | `125` (125 kHz), `250` (250 kHz), `500` (500 kHz) | The bandwidth that the stations of your area use. The default is the one of the test signal. |
| `expiry_s` | Forget stations after | number (s) | 600 | 30 to 86400 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre` | Test signal centre | position | 54.7, 25.3 |  |  |
| `gen_stations` | Test signal stations | whole number | 5 | 1 to 20 |  |
| `gen_bad_crc` | Wrong CRC in every n-th frame | whole number | 0 | 0 to 100 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `listening` | Channel | text |
| `stations_heard` | Stations | whole number |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `kind` (choice), `source` (text), `target` (text), `decoded` (text), `position` (position), `level_db` (number (dB)), `snr_db` (number (dB)) |
| `stations` | Stations | `source` (text), `name` (text), `kind` (choice), `aircraft` (text), `position` (position), `alt_m` (number (m)), `speed_kt` (number (kt)), `climb_m_s` (number (m/s)), `course_deg` (number (°)), `weather` (text), `heard` (whole number), `first_heard` (time), `last_heard` (time) |
| `station_tracks` | Tracks | `station` (text), `position` (position), `time_s` (number (s)) |

## Map points

The stack puts points on a map: Stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `fanet.station` | FANET station | `source` (text), `family` (choice) |
| `fanet.message` | FANET message | `source` (text), `target` (text), `text` (text), `family` (choice) |
| `fanet.distress` | FANET distress | `source` (text), `text` (text), `position` (position), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `lora`, version 3.2. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `FanetFields`. The result fields of the FANET stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames with a good CRC. |
| `frames_bad` | whole number | always | 1.0 | Frames whose CRC failed. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last frame, while frames come. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame, while frames come. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the bank reports. |
| `listening` | text | always | 1.0 | The channel of the plan. |
| `stations_heard` | whole number | always | 1.0 | The stations in the table. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `frames`

Struct `FanetFrameRow`. A row of the FANET frame log. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `kind` | choice | always | 1.0 | What the frame is. |
| `source` | text | always | 1.0 | The sender. |
| `target` | text | always | 1.0 | The addressee of a unicast frame, else empty. |
| `decoded` | text | always | 1.0 | The frame in one line. |
| `position` | position | optional | 1.0 | The position that the frame gives, if it gives one. |
| `level_db` | number (dB) | always | 1.0 | The level of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |

The column `source` names a row. Place of a row: the column `position`.

### Table `stations`

Struct `FanetStationRow`. A row of the FANET stations. The row key and `source` are the station id "MM:DDDD".

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The station id. |
| `name` | text | always | 1.0 | The name that the station sent. |
| `kind` | choice | always | 1.0 | What the station is. |
| `aircraft` | text | optional | 1.0 | The aircraft type, from a tracking frame. |
| `position` | position | optional | 1.0 | The last position. |
| `alt_m` | number (m) | optional | 1.0 | The altitude, from a tracking frame. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed, from a tracking frame. |
| `climb_m_s` | number (m/s) | optional | 1.0 | The climb rate, from a tracking frame. |
| `course_deg` | number (°) | optional | 1.0 | The heading, from a tracking frame. |
| `weather` | text | always | 1.0 | The last service frame in one line. |
| `heard` | whole number | optional | 1.0 | The frames of the station. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The column `name` names a row. Place of a row: the column `position`. Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.

### Table `station_tracks`

Struct `FanetTrackRow`. A track point of a FANET station. The row key is the station id and the signal time of the point.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station` | text | always | 1.0 | The station that the point belongs to. |
| `position` | position | always | 1.0 | The position. |
| `time_s` | number (s) | always | 1.0 | The signal time of the point. |

The column `station` names a row. Place of a row: the column `position`.

### Event `fanet.station`

Struct `FanetStationEvent`. The fields of a station heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The station id. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `fanet.message`

Struct `FanetMessageEvent`. The fields of a message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender. |
| `target` | text | always | 1.0 | The addressee of a unicast message, else empty. |
| `text` | text | always | 1.0 | The message. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `fanet.distress`

Struct `FanetDistressEvent`. The fields of a ground tracker that requests help.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The station id. |
| `text` | text | always | 1.0 | The state of the tracker in words. |
| `position` | position | optional | 1.0 | The position of the tracker, if the frame gives one. |
| `urgency` | choice | always | 1.0 | How urgent the request is. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fanet_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bandwidth` | `sdrkit_fanet_get_bandwidth` | `sdrkit_fanet_set_bandwidth` |
| `expiry_s` | `sdrkit_fanet_get_expiry_s` | `sdrkit_fanet_set_expiry_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre` | `sdrkit_fanet_gen_get_centre` | `sdrkit_fanet_gen_set_centre` |
| `gen_stations` | `sdrkit_fanet_gen_get_stations` | `sdrkit_fanet_gen_set_stations` |
| `gen_bad_crc` | `sdrkit_fanet_gen_get_bad_crc` | `sdrkit_fanet_gen_set_bad_crc` |

## Try it

```sh
sdrkit run fanet --source synth:fanet
sdrkit run fanet --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
