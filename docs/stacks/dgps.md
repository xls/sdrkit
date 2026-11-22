<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DGPS beacon

Differential GPS beacons: RTCM corrections, the base station's position, the beacon almanac and text messages.

Stack id: `dgps`. Group: Navigation.

Signal types: DGPS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 304 kHz |
| Tuning range | 283.5 kHz to 325 kHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 1 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dgps` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bitrate` | Bit rate | choice | `0` | `0` (Automatic), `50` (50 bps), `100` (100 bps), `200` (200 bps) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bitrate` | Test signal: bit rate | choice | `200` | `50`, `100`, `200` |  |
| `gen_station` | Test signal: station id | whole number | 333 | 0 to 1023 |  |
| `gen_lat` | Test signal: latitude | number (°) | -33.86 | -90 to 90 |  |
| `gen_lon` | Test signal: longitude | number (°) | 151.21 | -180 to 180 |  |
| `gen_text` | Test signal: text | text | `SDRKIT TEST BEACON` |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `station_id` | Station | whole number |
| `health` | Health | choice |
| `zcount` | Z-count | duration (s) |
| `symbol_rate_hz` | Symbol rate | frequency |
| `locked` | Locked | yes or no |
| `good_words` | Good words | whole number |
| `bad_words` | Bad words | whole number |
| `frames_ok` | Frames good | whole number |
| `base_station` | Base station | position |
| `base_height_m` | Base station height | number (m) |
| `text` | Text | text |
| `unhealthy` | Unhealthy satellites | text |
| `corrections` | Corrections | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `peak_dbfs` | Peak level | number (dBFS) |
| `msk50.clock_ppm` | Clock error | number (ppm) |
| `msk50.offset_hz` | Offset | frequency |
| `msk100.clock_ppm` | Clock error | number (ppm) |
| `msk100.offset_hz` | Offset | frequency |
| `msk200.clock_ppm` | Clock error | number (ppm) |
| `msk200.offset_hz` | Offset | frequency |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `station` | Station | `station_id` (whole number), `health` (choice), `position` (position), `height_m` (number (m)) |
| `messages` | Messages | `msg_type` (whole number), `count` (whole number) |
| `corrections` | Corrections | `prn` (whole number), `prc_m` (number (m)), `rrc_m_s` (number (m/s)), `iod` (whole number), `udre` (whole number) |
| `almanac` | Beacon almanac | `station_id` (whole number), `freq_hz` (frequency), `bit_rate_hz` (frequency), `position` (position), `range_km` (number (km)), `health` (whole number) |

## Map points

The stack puts points on a map: Base station.

## Events

| Kind | Label | Fields |
|---|---|---|
| `rtcm.frame` | RTCM frame | `source` (text), `text` (text), `family` (choice), `msg_type` (whole number), `station_id` (whole number), `zcount_s` (number (s)), `sequence` (whole number), `health` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avnav`, version 1.4. The C header `sdrkit/avnav.h`, the registry record `registry/avnav.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Navigation aids (`navigation`). Media: none.

### Result fields

Struct `DgpsFields`. The result fields of DGPS. A field of the station has no value before the first frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station_id` | whole number | optional | 1.0 | The base station id. |
| `health` | choice | optional | 1.0 | The health of the station. |
| `zcount` | duration (s) | optional | 1.0 | The last modified Z-count: the time into the hour. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The bit rate decoded. No value before the first report of the link. |
| `locked` | yes or no | always | 1.0 | The framer of that bit rate holds the frame sync. |
| `good_words` | whole number | always | 1.0 | Words with good parity. |
| `bad_words` | whole number | always | 1.0 | Words with bad parity. |
| `frames_ok` | whole number | always | 1.0 | Frames decoded. |
| `base_station` | position | optional | 1.0 | The position of the base station. No value before a message of type 3. |
| `base_height_m` | number (m) | optional | 1.0 | The height of the base station above the ellipsoid. No value before a message of type 3. |
| `text` | text | optional | 1.0 | The last text message. No value before a message of type 16. |
| `unhealthy` | text | optional | 1.0 | The satellites flagged unhealthy, by PRN. No value before a message of type 5. |
| `corrections` | whole number | always | 1.0 | The corrections held. |

### Result fields

Struct `DgpsMeterFields`. The levels of the channel, from its power meter.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The mean level. No value before the first tenth of a second. |
| `peak_dbfs` | number (dBFS) | optional | 1.0 | The peak level. No value before the first tenth of a second. |

### Table `station`

Struct `DgpsStationRow`. The row of the station table: the base station of the frames received. The row key is the station id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station_id` | whole number | always | 1.4 | The station id. |
| `health` | choice | optional | 1.4 | The health of the station. |
| `position` | position | optional | 1.4 | The position of the station. No value before a message of type 3. |
| `height_m` | number (m) | optional | 1.4 | The height of the station above the ellipsoid. No value before a message of type 3. |

The column `station_id` names a row. Place of a row: the column `position`. Height: `height_m`.

### Table `messages`

Struct `DgpsMessageRow`. A row of the frames by message type. The row key is the type in two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `msg_type` | whole number | always | 1.0 | The message type. |
| `count` | whole number | always | 1.0 | The frames of the type. |

The column `msg_type` names a row.

### Table `corrections`

Struct `DgpsCorrectionRow`. A row of the pseudorange corrections. The row key is the PRN in two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `prn` | whole number | always | 1.0 | The satellite. |
| `prc_m` | number (m) | always | 1.0 | The pseudorange correction. |
| `rrc_m_s` | number (m/s) | always | 1.0 | The range-rate correction. |
| `iod` | whole number | always | 1.0 | The issue of data. |
| `udre` | whole number | always | 1.0 | The user differential range error. |

The column `prn` names a row. Default order: the column `prn`, lowest first.

### Table `almanac`

Struct `DgpsAlmanacRow`. A row of the beacon almanac. The row key is the station id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station_id` | whole number | always | 1.0 | The station id. |
| `freq_hz` | frequency | always | 1.0 | The frequency of the beacon. |
| `bit_rate_hz` | frequency | always | 1.0 | The bit rate of the beacon. |
| `position` | position | always | 1.0 | The position of the beacon. |
| `range_km` | number (km) | always | 1.0 | The range of the beacon. |
| `health` | whole number | always | 1.0 | The health of the beacon, 0 (healthy) to 3. |

The column `station_id` names a row. Place of a row: the column `position`.

### Event `rtcm.frame`

Struct `RtcmFrameEvent`. The fields of an RTCM frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The station id. |
| `text` | text | always | 1.0 | A short text of the message. |
| `family` | choice | always | 1.0 | The family of the source: other. |
| `msg_type` | whole number | always | 1.0 | The message type. |
| `station_id` | whole number | always | 1.0 | The station id. |
| `zcount_s` | number (s) | always | 1.0 | The modified Z-count, seconds into the hour. |
| `sequence` | whole number | always | 1.0 | The sequence number. |
| `health` | choice | always | 1.0 | The health of the station. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dgps_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bitrate` | `sdrkit_dgps_get_bitrate` | `sdrkit_dgps_set_bitrate` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bitrate` | `sdrkit_dgps_gen_get_bitrate` | `sdrkit_dgps_gen_set_bitrate` |
| `gen_station` | `sdrkit_dgps_gen_get_station` | `sdrkit_dgps_gen_set_station` |
| `gen_lat` | `sdrkit_dgps_gen_get_lat` | `sdrkit_dgps_gen_set_lat` |
| `gen_lon` | `sdrkit_dgps_gen_get_lon` | `sdrkit_dgps_gen_set_lon` |
| `gen_text` | `sdrkit_dgps_gen_get_text` | `sdrkit_dgps_gen_set_text` |

## Try it

```sh
sdrkit run dgps --source synth:dgps
sdrkit run dgps --source <driver>:<address> --freq 0.304
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
