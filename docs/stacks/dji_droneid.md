<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DJI drones

DJI drones: DroneID packets (serial number, position, home and pilot) and drone video links recognised by their shape. Nothing is decrypted.

Stack id: `dji_droneid`. Group: Aircraft and drones.

Signal types: DJI DroneID.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.44 GHz |
| Tuning range | 2.3 GHz to 6 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 16 MHz |
| Signal bandwidth | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dji_droneid` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `decode_droneid` | Decode DroneID | yes or no | true |  | Try every 0.5 to 1.5 ms burst that is 7 to 11 MHz wide as a DroneID burst: Zadoff-Chu synchronisation, turbo decoding, CRC-24. |
| `wall_clock` | Stamp with this computer's clock | yes or no | true |  | Stamp rows and bursts of live samples with the computer's clock as well as signal time. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scenario` | Test signal | choice | `0` | `0` (everything), `1` (DroneID only), `2` (OcuSync-like only), `3` (Lightbridge-like only), `4` (Wi-Fi-like only), `5` (DroneID with collisions) |  |
| `gen_period_s` | DroneID period | number (s) | 0.64 | 0.01 to 10 |  |
| `gen_eight_symbols` | 8-symbol DroneID | yes or no | false |  |  |
| `gen_wide` | 20 MHz OcuSync-like | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bursts_measured` | Bursts measured | whole number |
| `bursts_ofdm_like` | OFDM-like bursts | whole number |
| `droneid_decoded` | DroneID packets | whole number |
| `droneid_crc_failed` | DroneID CRC failures | whole number |
| `droneid_tried` | DroneID attempts | whole number |
| `centre_hz` | Capture centre | frequency |
| `burst_cfo_hz` | DroneID carrier offset | frequency |
| `drones` | Drones | whole number |
| `signal` | Signal | text |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `signals` | Signals | `track` (whole number), `class` (choice), `confidence_pct` (number (%)), `freq_hz` (frequency), `offset_hz` (frequency), `width_hz` (frequency), `clipped` (yes or no), `burst` (duration (s)), `period` (duration (s)), `duty_pct` (number (%)), `regularity_pct` (number (%)), `ofdm_pct` (number (%)), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `bursts` (whole number), `zc_seen` (yes or no), `decoded` (whole number), `failed` (whole number), `band` (choice), `reason` (choice), `near_droneid` (yes or no), `last_s` (number (s)) |
| `drones` | Drones | `source` (text), `position` (position), `alt_m` (number (m)), `height_m` (number (m)), `alt_raw` (whole number), `height_raw` (whole number), `speed_kt` (number (kt)), `course_deg` (number (°)), `home` (position), `pilot` (position), `seq` (whole number), `state_info` (whole number), `version` (whole number), `inner_crc_ok` (yes or no), `v_north` (whole number (cm/s)), `v_east` (whole number (cm/s)), `v_up` (whole number (cm/s)), `yaw` (whole number), `gps_time` (time), `device_type` (whole number), `uuid` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `track` (whole number), `trail_points` (whole number), `last_heard` (time), `heard` (whole number) |
| `bursts` | Bursts | `time_s` (number (s)), `time` (time), `track` (whole number), `offset_hz` (frequency), `width_hz` (frequency), `burst` (duration (s)), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `ofdm_pct` (number (%)), `droneid` (choice) |
| `drone_tracks` | Tracks | `drone` (text), `position` (position), `time_s` (number (s)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level` | Level | s | dBFS |

## Map points

The stack puts points on a map: Drones.

## Events

| Kind | Label | Fields |
|---|---|---|
| `dji.droneid` | DroneID packet | `source` (text), `position` (position), `alt_m` (number (m)), `height_m` (number (m)), `alt_raw` (whole number), `height_raw` (whole number), `speed_kt` (number (kt)), `course_deg` (number (°)), `home` (position), `pilot` (position), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 1.2. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `DjiDroneidFields`. The result fields of DJI drones.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts_measured` | whole number | always | 1.0 | Bursts measured. |
| `bursts_ofdm_like` | whole number | always | 1.0 | OFDM-like bursts. |
| `droneid_decoded` | whole number | always | 1.0 | DroneID packets decoded. |
| `droneid_crc_failed` | whole number | always | 1.0 | DroneID bursts with the symbols whose CRC failed. |
| `droneid_tried` | whole number | always | 1.0 | DroneID attempts. |
| `centre_hz` | frequency | optional | 1.0 | The capture centre. |
| `burst_cfo_hz` | frequency | optional | 1.0 | The carrier offset of the last DroneID burst from the 0.5 MHz grid. |
| `drones` | whole number | always | 1.0 | Drones in the table. |
| `signal` | text | optional | 1.0 | The track with the most likely DJI shape: a row key of the `signals` table. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | Noise floor, dBFS. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | optional | 1.0 | Carrier offset from where it was expected, Hz. |

### Table `signals`

Struct `DjiDroneidSignalRow`. A drone link recognised by its shape.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `track` | whole number | always | 1.0 | The track number. |
| `class` | choice | always | 1.0 | The class of a track. |
| `confidence_pct` | number (%) | always | 1.0 | The confidence of the class. |
| `freq_hz` | frequency | optional | 1.0 | The radio frequency. |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `width_hz` | frequency | always | 1.0 | The width. |
| `clipped` | yes or no | always | 1.0 | The last burst was clipped. |
| `burst` | duration (s) | always | 1.0 | The burst length. |
| `period` | duration (s) | optional | 1.0 | The burst period. |
| `duty_pct` | number (%) | always | 1.0 | The duty cycle. |
| `regularity_pct` | number (%) | always | 1.0 | The share of gaps that are whole periods. |
| `ofdm_pct` | number (%) | always | 1.0 | How OFDM-like the bursts are. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `bursts` | whole number | always | 1.0 | Bursts seen. |
| `zc_seen` | yes or no | always | 1.0 | The DroneID symbols were seen. |
| `decoded` | whole number | always | 1.0 | DroneID packets decoded on the track. |
| `failed` | whole number | always | 1.0 | DroneID CRC failures on the track. |
| `band` | choice | optional | 1.0 | The band. |
| `reason` | choice | always | 1.0 | Why the track has its class. |
| `near_droneid` | yes or no | always | 1.0 | The track is within 3 MHz of a frequency reported for DroneID. |
| `last_s` | number (s) | always | 1.0 | The signal time of the last burst. |

Default order: the column `snr_db`, highest first.

### Table `drones`

Struct `DjiDroneidDroneRow`. A DJI drone heard on DroneID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | always | 1.0 | The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet. |
| `height_m` | number (m) | always | 1.0 | The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude. |
| `alt_raw` | whole number | always | 1.2 | The altitude field as received (a signed 16-bit number), whose unit is not confirmed. |
| `height_raw` | whole number | always | 1.2 | The height field as received (a signed 16-bit number), whose unit is not confirmed. |
| `speed_kt` | number (kt) | always | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | always | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `home` | position | optional | 1.0 | The home position. |
| `pilot` | position | optional | 1.0 | The pilot's position. |
| `seq` | whole number | always | 1.0 | The packet sequence number. |
| `state_info` | whole number | always | 1.0 | The state information bits. |
| `version` | whole number | always | 1.0 | The packet version. |
| `inner_crc_ok` | yes or no | always | 1.0 | The inner CRC passed. |
| `v_north` | whole number (cm/s) | always | 1.0 | Velocity north. |
| `v_east` | whole number (cm/s) | always | 1.0 | Velocity east. |
| `v_up` | whole number (cm/s) | always | 1.0 | Velocity up. |
| `yaw` | whole number | always | 1.0 | The raw yaw. |
| `gps_time` | time | always | 1.0 | The GPS time of the packet. |
| `device_type` | whole number | always | 1.0 | The device type code. |
| `uuid` | text | optional | 1.0 | The UUID. |
| `freq_hz` | frequency | optional | 1.0 | The radio frequency. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `track` | whole number | always | 1.0 | The track number. |
| `trail_points` | whole number | always | 1.0 | Trail points. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `heard` | whole number | optional | 1.0 | Times heard. |

The column `source` names a row. Place of a row: the column `position`. Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.

### Table `bursts`

Struct `DjiDroneidBurstRow`. A burst in the burst log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of a burst. |
| `time` | time | optional | 1.0 | The wall time of a burst. |
| `track` | whole number | always | 1.0 | The track number. |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `width_hz` | frequency | always | 1.0 | The width. |
| `burst` | duration (s) | always | 1.0 | The burst length. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `ofdm_pct` | number (%) | always | 1.0 | How OFDM-like the bursts are. |
| `droneid` | choice | always | 1.0 | The DroneID outcome of a burst. |

### Table `drone_tracks`

Struct `DroneTrackRow`. A track point of a drone. The row key is the drone key and the signal time of the point.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `drone` | text | always | 1.0 | The drone a track point belongs to. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `time_s` | number (s) | always | 1.0 | The signal time of a track point. |

Place of a row: the column `position`.

### Event `dji.droneid`

Struct `DjiDroneidEvent`. A DroneID packet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | always | 1.0 | The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet. |
| `height_m` | number (m) | always | 1.0 | The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude. |
| `alt_raw` | whole number | always | 1.2 | The altitude field as received (a signed 16-bit number), whose unit is not confirmed. |
| `height_raw` | whole number | always | 1.2 | The height field as received (a signed 16-bit number), whose unit is not confirmed. |
| `speed_kt` | number (kt) | always | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | always | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `home` | position | optional | 1.0 | The home position. |
| `pilot` | position | optional | 1.0 | The pilot's position. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dji_droneid_clear` | none | none |
| `remove` | `sdrkit_dji_droneid_remove` | `sdrkit_drones_drone_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `decode_droneid` | `sdrkit_dji_droneid_get_decode_droneid` | `sdrkit_dji_droneid_set_decode_droneid` |
| `wall_clock` | `sdrkit_dji_droneid_get_wall_clock` | `sdrkit_dji_droneid_set_wall_clock` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scenario` | `sdrkit_dji_droneid_gen_get_scenario` | `sdrkit_dji_droneid_gen_set_scenario` |
| `gen_period_s` | `sdrkit_dji_droneid_gen_get_period_s` | `sdrkit_dji_droneid_gen_set_period_s` |
| `gen_eight_symbols` | `sdrkit_dji_droneid_gen_get_eight_symbols` | `sdrkit_dji_droneid_gen_set_eight_symbols` |
| `gen_wide` | `sdrkit_dji_droneid_gen_get_wide` | `sdrkit_dji_droneid_gen_set_wide` |

## Try it

```sh
sdrkit run dji_droneid --source synth:dji_droneid
sdrkit run dji_droneid --source iiod:192.168.2.1 --freq 2440
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Altitude and height

The altitude and the height above home are read as 0.01 m. No public source confirms the unit: the NDSS 2023 receiver reads
the same fields as feet. The columns `alt_raw` and `height_raw` hold the fields as received.
