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
| `decode_droneid` | Decode DroneID | yes or no | true |  | Try every 0.5 to 1.5 ms burst that is 7 to 11 MHz wide as a DroneID burst: Zadoff-Chu synchronisation, turbo decoding, CRC-24. Also search the DroneID frequencies inside the capture for the two Zadoff-Chu symbols all the time, so a burst near the noise floor or cut by the edge of the capture is found. |
| `wall_clock` | Stamp with this computer's clock | yes or no | true |  | Stamp rows and bursts of live samples with the computer's clock as well as signal time. |
| `droneid_tuning` | DroneID tuning | choice | `fixed` | `fixed` (stay on the tuned frequency), `step` (step through the DroneID frequencies) | The newest aircraft send DroneID on fixed frequencies (2414.5, 2429.5 and 2444.5 MHz were seen), about one burst each 3 s on a frequency. With `step` the stack asks the host for the DroneID frequencies of the band it is tuned in: each in turn for the dwell, or one centre when the capture covers them all. With `fixed` the tuned frequency stays. |
| `droneid_dwell_s` | DroneID dwell | duration (s) | 10 s | 10 to 300 | The time on each frequency while stepping. A burst comes each 3 to 6 s, so the least is 10 s. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scenario` | Test signal | choice | `0` | `0` (everything), `1` (DroneID only), `2` (OcuSync-like only), `3` (Lightbridge-like only), `4` (Wi-Fi-like only), `5` (DroneID with collisions) |  |
| `gen_period_s` | DroneID period | number (s) | 0.64 | 0.01 to 10 |  |
| `gen_eight_symbols` | 8-symbol DroneID | yes or no | false |  |  |
| `gen_wide` | 20 MHz OcuSync-like | yes or no | false |  |  |
| `gen_frame` | DroneID frame | choice | `readable` | `readable` (readable packet), `encrypted` (encrypted format), `empty` (empty frame) |  |
| `gen_droneid_frequency` | On a DroneID frequency | yes or no | false |  | Send the DroneID bursts on the DroneID frequency nearest to the radio centre, one each 5 periods (3.2 s with the period of 0.64 s), without the carriers outside the capture. |
| `gen_two_aircraft` | Two aircraft | yes or no | false |  | Every second DroneID burst comes from a second aircraft. |
| `gen_centre_hz` | Test signal radio centre | frequency | 0 Hz | 0 Hz to 6 GHz | The radio centre for `gen_droneid_frequency`. 0 takes the tuned frequency. |

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
| `aircraft` | Aircraft | whole number |
| `droneid_encrypted` | DroneID encrypted frames | whole number |
| `droneid_empty` | DroneID empty frames | whole number |
| `droneid_other` | DroneID frames of an unknown layout | whole number |
| `droneid_refused` | Empty frames refused | whole number |
| `droneid_pairs` | DroneID symbol pairs | whole number |
| `droneid_bursts` | DroneID bursts | whole number |
| `droneid_searched` | DroneID frequencies searched | whole number |
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
| `aircraft` | Aircraft | `aircraft` (text), `session_key` (text), `drone` (text), `format` (choice), `frame_type` (whole number), `frames` (whole number), `first_s` (number (s)), `last_s` (number (s)), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `freq_hz` (frequency), `first_heard` (time), `last_heard` (time) |

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
| `dji.aircraft` | New DJI aircraft | `source` (text), `session_key` (text), `format` (choice), `frame_type` (whole number), `position` (position), `snr_db` (number (dB)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 2.0. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

Key readings: `aircraft`, `droneid_decoded`, `droneid_encrypted`, `droneid_empty`.

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
| `aircraft` | whole number | always | 1.0 | Aircraft in the aircraft table: the drones with a readable packet, the aircraft that send the encrypted format, and the sender of empty frames. |
| `droneid_encrypted` | whole number | always | 1.0 | DroneID frames of the encrypted format: the aircraft is present, its position and serial number are not readable. |
| `droneid_empty` | whole number | always | 1.0 | Empty DroneID frames: the aircraft is present and sends no data in this state. |
| `droneid_other` | whole number | always | 1.0 | DroneID frames whose CRC-24 passed and whose layout is not known. |
| `droneid_refused` | whole number | always | 1.0 | Frames of zeros that were refused: the received bits did not agree with an empty frame. |
| `droneid_pairs` | whole number | always | 1.0 | Pairs of the two DroneID symbols that the continuous search found. |
| `droneid_bursts` | whole number | always | 1.0 | Bursts of the continuous search whose two symbols passed the check at the full rate. |
| `droneid_searched` | whole number | always | 1.0 | The DroneID frequencies inside the capture that the continuous search covers. |
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
| `alt_raw` | whole number | always | 1.0 | The altitude field as received (a signed 16-bit number), whose unit is not confirmed. |
| `height_raw` | whole number | always | 1.0 | The height field as received (a signed 16-bit number), whose unit is not confirmed. |
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

### Table `aircraft`

Struct `DjiDroneidAircraftRow`. A DJI aircraft heard on DroneID, in any format.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aircraft` | text | always | 1.0 | The name of the row: the serial number of a drone with a readable packet, `session-` and the session key of an aircraft that sends the encrypted format, `empty frames` for the sender of empty frames. |
| `session_key` | text | optional | 1.0 | The 4 bytes of the encrypted format that are constant for one aircraft, as 8 hex digits. They tell two aircraft apart within a session. They are not a serial number and do not identify the aircraft. |
| `drone` | text | optional | 1.0 | The drone of a readable packet: a row key of the `drones` table. |
| `format` | choice | always | 1.0 | The format of the last frame. |
| `frame_type` | whole number | optional | 1.0 | The type byte of the last frame of the encrypted format (0x10, 0x11 and 0x13 were seen). |
| `frames` | whole number | always | 1.0 | The frames received. |
| `first_s` | number (s) | always | 1.0 | The signal time of the first frame. |
| `last_s` | number (s) | always | 1.0 | The signal time of the last frame. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `freq_hz` | frequency | optional | 1.0 | The radio frequency. |
| `first_heard` | time | optional | 1.0 | Wall-clock time when first heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

The rows are the stations or nodes of the stack. The column `aircraft` names a row. Default order: the column `aircraft`, lowest first. Time: `last_heard`.

### Event `dji.droneid`

Struct `DjiDroneidEvent`. A DroneID packet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | always | 1.0 | The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet. |
| `height_m` | number (m) | always | 1.0 | The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude. |
| `alt_raw` | whole number | always | 1.0 | The altitude field as received (a signed 16-bit number), whose unit is not confirmed. |
| `height_raw` | whole number | always | 1.0 | The height field as received (a signed 16-bit number), whose unit is not confirmed. |
| `speed_kt` | number (kt) | always | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | always | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `home` | position | optional | 1.0 | The home position. |
| `pilot` | position | optional | 1.0 | The pilot's position. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `dji.aircraft`

Struct `DjiDroneidAircraftEvent`. A DJI aircraft that was not heard before.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `session_key` | text | optional | 1.0 | The 4 bytes of the encrypted format that are constant for one aircraft, as 8 hex digits. They tell two aircraft apart within a session. They are not a serial number and do not identify the aircraft. |
| `format` | choice | always | 1.0 | The format of the frame. |
| `frame_type` | whole number | optional | 1.0 | The type byte of a frame of the encrypted format. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
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
| `droneid_tuning` | `sdrkit_dji_droneid_get_droneid_tuning` | `sdrkit_dji_droneid_set_droneid_tuning` |
| `droneid_dwell_s` | `sdrkit_dji_droneid_get_droneid_dwell_s` | `sdrkit_dji_droneid_set_droneid_dwell_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scenario` | `sdrkit_dji_droneid_gen_get_scenario` | `sdrkit_dji_droneid_gen_set_scenario` |
| `gen_period_s` | `sdrkit_dji_droneid_gen_get_period_s` | `sdrkit_dji_droneid_gen_set_period_s` |
| `gen_eight_symbols` | `sdrkit_dji_droneid_gen_get_eight_symbols` | `sdrkit_dji_droneid_gen_set_eight_symbols` |
| `gen_wide` | `sdrkit_dji_droneid_gen_get_wide` | `sdrkit_dji_droneid_gen_set_wide` |
| `gen_frame` | `sdrkit_dji_droneid_gen_get_frame` | `sdrkit_dji_droneid_gen_set_frame` |
| `gen_droneid_frequency` | `sdrkit_dji_droneid_gen_get_droneid_frequency` | `sdrkit_dji_droneid_gen_set_droneid_frequency` |
| `gen_two_aircraft` | `sdrkit_dji_droneid_gen_get_two_aircraft` | `sdrkit_dji_droneid_gen_set_two_aircraft` |
| `gen_centre_hz` | `sdrkit_dji_droneid_gen_get_centre_hz` | `sdrkit_dji_droneid_gen_set_centre_hz` |

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

## What can be read from an aircraft

The stack reads the DroneID burst of an aircraft. What the burst holds depends on the aircraft.

| The aircraft sends | The stack shows | Position and serial number |
|---|---|---|
| The readable packet (older aircraft, OcuSync 2) | A row in `drones` and a row in `aircraft` with the format `readable` | Readable: serial number, position, altitude, home and pilot |
| The encrypted format (the newest link generation: a DJI Neo 2 and a DJI Mini 4 Pro were measured) | A row in `aircraft` with the format `encrypted` and the frame type. Status: "DJI DroneID, encrypted format (type 0x10): aircraft present, position and serial not readable" | Not readable |
| An empty frame (seen from a Neo 2 indoors with no GPS fix and the motors off; the cause is not known) | The row `empty frames` in `aircraft`. Status: "DJI DroneID present, empty frame (no data sent in this state)" | Not sent |

Each of the three identifies a DJI aircraft with high confidence: the burst has the two DroneID symbols and its CRC-24 is
good.

The encrypted format has a readable head and a body that is not readable. The stack does not decrypt the body and does not
guess its content. From the head it reads the frame type and 4 bytes that are constant for one aircraft and differ between
aircraft. The column `session_key` shows these 4 bytes as hex. Use the session key only to tell two aircraft apart within a
session. It is not a serial number, and nothing shows that it stays the same for one aircraft over time.

Empty frames hold nothing that tells aircraft apart, so all of them are one row. A frame of zeros passes the CRC-24 whatever
was received. The stack accepts an empty frame only when the two DroneID symbols are found and at least 70 % of the received
bits agree with the coded bits of the frame. The field `droneid_refused` counts the frames of zeros that did not pass.

The counters keep the formats apart: `droneid_decoded` (readable packets), `droneid_encrypted`, `droneid_empty` and
`droneid_other` (a good CRC-24 and a layout that is not known). `droneid_crc_failed` counts only bursts whose CRC-24 failed.

## Where the newest aircraft send

The newest aircraft do not send DroneID on the channel of the video link. They send it on fixed frequencies: 2414.5, 2429.5
and 2444.5 MHz were seen. About one burst comes each 3 s on a frequency, with gaps of up to 6 s. Close to a Wi-Fi router
the burst can be near the noise floor.

The stack searches the DroneID frequencies inside the capture all the time for the two DroneID symbols. This search does
not need the burst to stand out of the noise, and it finds a burst that the edge of the capture cuts when at least about a
third of its carriers are inside. The field `droneid_searched` shows how many frequencies the capture covers. At the default
frequency of 2440 MHz with 20 MHz, the capture covers 2444.5 MHz and a part of 2429.5 MHz.

The setting `droneid_tuning` chooses how the stack tunes:

- `fixed` (default): the tuned frequency stays. Tune to 2440 MHz or to one DroneID frequency and wait at least 10 s.
- `step`: the stack asks the host for the DroneID frequencies of the band in turn, each for `droneid_dwell_s` (at least
  10 s). When the capture is wide enough for all of them, it asks for one centre instead.

Older aircraft send the burst each 0.64 s on the channel of the video link. The stack finds these bursts with the burst
detector anywhere in the capture, as before.

To replay your own recordings of the DroneID frequencies:

```sh
SDRKIT_DRONEID_RECORDINGS=<file or folder> cargo run --release -p sdrkit-stack-drones --example droneid_replay
```
