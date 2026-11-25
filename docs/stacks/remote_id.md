<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Drone Remote ID

The identification drones broadcast openly on Bluetooth LE and Wi-Fi (ASTM F3411 / EN 4709-002): serial, position, operator.

Stack id: `remote_id`. Group: Aircraft and drones.

Signal types: Bluetooth LE, Wi-Fi.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.432 GHz |
| Tuning range | 2.4 GHz to 2.5 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 20 MHz |
| Signal bandwidth | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:remote_id` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `band` | Band | choice | `2432` | `2432` (BLE 38 + Wi-Fi 6 (2432 MHz)), `2437` (Wi-Fi 6 (2437 MHz)), `2412` (Wi-Fi 1 (2412 MHz)), `2462` (Wi-Fi 11 (2462 MHz)), `2403` (BLE 37 + Wi-Fi 1 (2403 MHz)), `2426` (BLE 38 (2426 MHz)), `2478` (BLE 39 (2478 MHz)) | One 20 MHz band of 2.4 GHz is heard at a time. Drones advertise on all three BLE channels, and most send Wi-Fi beacons on channel 6, so 2432 MHz hears both. |
| `ble` | Bluetooth LE | yes or no | true |  | Run the Bluetooth LE decoder when an advertising channel is in the band. |
| `wifi_ofdm` | Wi-Fi OFDM beacons | yes or no | true |  | Run the OFDM beacon path beside the DSSS one. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `shared_radio` | Shared-radio check | yes or no | false |  | Flags drones as suspicious when several broadcast addresses keep one carrier offset (within 1 kHz, on the same carrier) while they are heard: one radio behind several drones. Two real drones can share an offset by chance, so the check is off by default. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_carriers` | Test signal carriers | choice | `both` | `both`, `ble` (BLE only), `wifi` (Wi-Fi only) |  |
| `gen_centre_mhz` | Test signal radio centre | number (MHz) | 2432 | 0 to 2500 | 0 places both carriers at 0 Hz (as a file without a frequency). |
| `gen_wifi_phy` | Test signal Wi-Fi PHY | choice | `dsss` | `dsss` (DSSS 1 Mbit/s), `ofdm` (OFDM 6 Mbit/s) |  |
| `gen_spoofer` | Test signal spoofer | yes or no | false |  | Adds a drone on Bluetooth LE channel 38 whose position jumps 2 km at each Location message, so that the trust check flags it as spoofed. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `ble_channel` | Bluetooth LE channel | whole number |
| `ble_offset_hz` | Bluetooth LE offset | frequency |
| `wifi_channel` | Wi-Fi channel | whole number |
| `wifi_offset_hz` | Wi-Fi offset | frequency |
| `band` | Band | choice |
| `drones` | Drones | whole number |
| `flagged` | Flagged drones | whole number |
| `messages` | Messages | whole number |
| `messages_refused` | Messages refused | whole number |
| `fields_out_of_range` | Values out of range | whole number |
| `ble_good` | BLE packets | whole number |
| `ble_remote_id` | BLE packets with Remote ID | whole number |
| `wifi_good` | Wi-Fi frames | whole number |
| `wifi_remote_id` | Wi-Fi frames with Remote ID | whole number |
| `frames_bad` | Frames bad | whole number |
| `basic_id_messages` | Basic ID | whole number |
| `location_messages` | Location | whole number |
| `authentication_messages` | Authentication | whole number |
| `self_id_messages` | Self-ID | whole number |
| `system_messages` | System | whole number |
| `operator_id_messages` | Operator ID | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `drones` | Drones | `source` (text), `address` (text), `verdict` (choice), `reasons` (text), `serial` (text), `session_id` (text), `caa_registration` (text), `utm_uuid` (text), `ua_type` (whole number), `status` (whole number), `position` (position), `alt_geo_m` (number (m)), `alt_baro_m` (number (m)), `height_m` (number (m)), `height_type` (whole number), `speed_kt` (number (kt)), `vspeed_m_s` (number (m/s)), `course_deg` (number (°)), `horiz_acc` (whole number), `vert_acc` (whole number), `speed_acc` (whole number), `timestamp_ds` (whole number (0.1 s)), `operator_position` (position), `operator_alt_m` (number (m)), `location_type` (whole number), `classification` (whole number), `category` (whole number), `class` (whole number), `system_timestamp_s` (whole number (s)), `operator_id` (text), `self_id` (text), `authentication` (whole number), `authentication_pages` (whole number), `authentication_length` (whole number (B)), `authentication_complete` (yes or no), `basic_id_messages` (whole number), `location_messages` (whole number), `authentication_messages` (whole number), `self_id_messages` (whole number), `system_messages` (whole number), `operator_id_messages` (whole number), `track_points` (whole number), `via` (choice), `level_dbfs` (number (dBFS)), `last_heard` (time), `heard` (whole number) |
| `messages` | Messages | `time_s` (number (s)), `time` (time), `via` (choice), `radio_channel` (whole number), `address` (text), `level_dbfs` (number (dBFS)), `type` (choice), `message` (text) |
| `drone_tracks` | Tracks | `drone` (text), `position` (position), `time_s` (number (s)) |

## Map points

The stack puts points on a map: Drones.

## Events

| Kind | Label | Fields |
|---|---|---|
| `remote_id.drone` | Drone | `source` (text), `address` (text), `position` (position), `alt_geo_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `operator_position` (position), `urgency` (choice), `family` (choice) |
| `remote_id.verdict` | Trust verdict | `source` (text), `address` (text), `verdict` (choice), `reasons` (text), `position` (position), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 1.4. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

Key readings: `drones`, `flagged`.

### Result fields

Struct `RemoteIdFields`. The result fields of Drone Remote ID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ble_channel` | whole number | optional | 1.0 | The BLE advertising channel in the band (0 for a file without a centre). |
| `ble_offset_hz` | frequency | optional | 1.0 | The offset of the BLE advertising channel from the centre. |
| `wifi_channel` | whole number | optional | 1.0 | The Wi-Fi channel in the band (0 for a file without a centre). |
| `wifi_offset_hz` | frequency | always | 1.0 | The offset of the Wi-Fi channel from the centre. |
| `band` | choice | optional | 1.0 | The band of the present centre, when it is one of the bands that the stack offers. |
| `drones` | whole number | always | 1.0 | Drones in the table. |
| `flagged` | whole number | always | 1.2 | Drones in the table whose trust verdict is suspicious or spoofed. |
| `messages` | whole number | always | 1.0 | Messages decoded. |
| `messages_refused` | whole number | always | 1.2 | Messages refused: members of a Message Pack that are packs or of a reserved type, authentication pages that fail their checks. |
| `fields_out_of_range` | whole number | always | 1.2 | Messages with a value outside its range (a Location timestamp above 36000, a latitude or longitude out of range), which the tables show as absent. |
| `ble_good` | whole number | always | 1.0 | Good BLE packets. |
| `ble_remote_id` | whole number | always | 1.0 | Good BLE packets with Remote ID. |
| `wifi_good` | whole number | always | 1.0 | Good Wi-Fi frames. |
| `wifi_remote_id` | whole number | always | 1.0 | Good Wi-Fi frames with Remote ID. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `basic_id_messages` | whole number | always | 1.0 | Basic ID messages (a field: all drones; a column: one drone). |
| `location_messages` | whole number | always | 1.0 | Location messages. |
| `authentication_messages` | whole number | always | 1.0 | Authentication messages. |
| `self_id_messages` | whole number | always | 1.0 | Self-ID messages. |
| `system_messages` | whole number | always | 1.0 | System messages. |
| `operator_id_messages` | whole number | always | 1.0 | Operator ID messages. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | optional | 1.0 | Carrier offset from where it was expected, Hz. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |

### Table `drones`

Struct `RemoteIdDroneRow`. A drone heard on Remote ID. The row key is the broadcast address. The source is the shown ID: the serial number, else the session ID, else the CAA registration, else the UTM UUID, else the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address` | text | always | 1.0 | The broadcast address. |
| `verdict` | choice | optional | 1.2 | The trust verdict of the broadcasts heard since the stack started: authenticated, plausible, suspicious or spoofed. No value before the first packet since the start (a drone of a restored state). |
| `reasons` | text | always | 1.2 | The active reasons of the trust verdict: their stable names (`teleport`, `counter_conflict`, ...) joined by ", ". Empty when there is none. |
| `serial` | text | optional | 1.0 | Serial number. |
| `session_id` | text | optional | 1.0 | Session ID. |
| `caa_registration` | text | optional | 1.0 | CAA registration. |
| `utm_uuid` | text | optional | 1.0 | UTM assigned UUID. |
| `ua_type` | whole number | optional | 1.0 | UA type code (ASTM F3411-22a, Basic ID message). |
| `status` | whole number | optional | 1.0 | Operational status code (ASTM F3411-22a, Location message). |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_geo_m` | number (m) | optional | 1.0 | Geodetic altitude. |
| `alt_baro_m` | number (m) | optional | 1.0 | Pressure altitude. |
| `height_m` | number (m) | optional | 1.0 | Height above take-off or ground, as the column `height_type` says. |
| `height_type` | whole number | optional | 1.0 | What the height is above: 0 take-off, 1 ground. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `vspeed_m_s` | number (m/s) | optional | 1.0 | Vertical speed, up. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `horiz_acc` | whole number | optional | 1.0 | Horizontal accuracy code. |
| `vert_acc` | whole number | optional | 1.0 | Vertical accuracy code. |
| `speed_acc` | whole number | optional | 1.0 | Speed accuracy code. |
| `timestamp_ds` | whole number (0.1 s) | optional | 1.0 | Tenths of a second since the hour. |
| `operator_position` | position | optional | 1.0 | The operator's position. |
| `operator_alt_m` | number (m) | optional | 1.0 | The operator's altitude. |
| `location_type` | whole number | optional | 1.0 | Operator location type code. |
| `classification` | whole number | optional | 1.0 | Classification type code. |
| `category` | whole number | optional | 1.0 | EU category code. |
| `class` | whole number | optional | 1.0 | EU class code (1 to 7 are C0 to C6). |
| `system_timestamp_s` | whole number (s) | optional | 1.0 | System time stamp, seconds since 2019-01-01. |
| `operator_id` | text | optional | 1.0 | Operator ID. |
| `self_id` | text | optional | 1.0 | Self-ID text. |
| `authentication` | whole number | optional | 1.0 | Authentication type code. |
| `authentication_pages` | whole number | optional | 1.0 | Authentication pages. |
| `authentication_length` | whole number (B) | optional | 1.2 | Authentication data length in bytes, from page 0. |
| `authentication_complete` | yes or no | optional | 1.2 | Every page of the present authentication was heard. No signature is verified. |
| `basic_id_messages` | whole number | always | 1.0 | Basic ID messages (a field: all drones; a column: one drone). |
| `location_messages` | whole number | always | 1.0 | Location messages. |
| `authentication_messages` | whole number | always | 1.0 | Authentication messages. |
| `self_id_messages` | whole number | always | 1.0 | Self-ID messages. |
| `system_messages` | whole number | always | 1.0 | System messages. |
| `operator_id_messages` | whole number | always | 1.0 | Operator ID messages. |
| `track_points` | whole number | always | 1.0 | Trail points. |
| `via` | choice | always | 1.0 | How the drone was heard. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `heard` | whole number | optional | 1.0 | Times heard. |

The column `address` names a row. Place of a row: the column `position`. Height: `alt_geo_m`. Course: `course_deg`. Speed: `speed_kt`.

### Table `messages`

Struct `RemoteIdMessageRow`. A line of the message log. The row key is the sequence number of the line.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time of a log line. |
| `time` | time | optional | 1.0 | Wall time of a log line. |
| `via` | choice | always | 1.0 | The carrier of a log line. |
| `radio_channel` | whole number | optional | 1.0 | The radio channel of a log line (BLE 37 to 39, Wi-Fi 1 to 13; none when unknown). |
| `address` | text | always | 1.0 | The broadcast address. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `type` | choice | always | 1.0 | The message type. |
| `message` | text | always | 1.0 | The summary line of a message. |

The column `address` names a row.

### Table `drone_tracks`

Struct `DroneTrackRow`. A track point of a drone. The row key is the drone key and the signal time of the point.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `drone` | text | always | 1.0 | The drone a track point belongs to. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `time_s` | number (s) | always | 1.0 | The signal time of a track point. |

Place of a row: the column `position`.

### Event `remote_id.drone`

Struct `RemoteIdDroneEvent`. A drone with a position, at most every 5 s for each drone. The urgency is emergency when the drone states the emergency status.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address` | text | always | 1.0 | The broadcast address. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `alt_geo_m` | number (m) | optional | 1.0 | Geodetic altitude. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `operator_position` | position | optional | 1.0 | The operator's position. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `remote_id.verdict`

Struct `RemoteIdVerdictEvent`. A change of the trust verdict of a Remote ID drone. The urgency is notice when the verdict is suspicious or spoofed, else none.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.2 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address` | text | always | 1.2 | The broadcast address. |
| `verdict` | choice | always | 1.2 | The new trust verdict. |
| `reasons` | text | always | 1.2 | The active reasons of the verdict: their stable names joined by ", ". Empty when there is none. |
| `position` | position | optional | 1.2 | The position of the object that the event is about. |
| `urgency` | choice | always | 1.2 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `family` | choice | always | 1.2 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_remote_id_clear` | none | none |
| `remove` | `sdrkit_remote_id_remove` | `sdrkit_drones_drone_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `band` | `sdrkit_remote_id_get_band` | `sdrkit_remote_id_set_band` |
| `ble` | `sdrkit_remote_id_get_ble` | `sdrkit_remote_id_set_ble` |
| `wifi_ofdm` | `sdrkit_remote_id_get_wifi_ofdm` | `sdrkit_remote_id_set_wifi_ofdm` |
| `shared_radio` | `sdrkit_remote_id_get_shared_radio` | `sdrkit_remote_id_set_shared_radio` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_carriers` | `sdrkit_remote_id_gen_get_carriers` | `sdrkit_remote_id_gen_set_carriers` |
| `gen_centre_mhz` | `sdrkit_remote_id_gen_get_centre_mhz` | `sdrkit_remote_id_gen_set_centre_mhz` |
| `gen_wifi_phy` | `sdrkit_remote_id_gen_get_wifi_phy` | `sdrkit_remote_id_gen_set_wifi_phy` |
| `gen_spoofer` | `sdrkit_remote_id_gen_get_spoofer` | `sdrkit_remote_id_gen_set_spoofer` |

## Try it

```sh
sdrkit run remote_id --source synth:remote_id
sdrkit run remote_id --source iiod:192.168.2.1 --freq 2432
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Trust verdicts

Each drone's broadcasts go to the trust engine of the `remoteid` crate. The column `verdict` of the drone table holds its
verdict: `authenticated`, `plausible`, `suspicious` or `spoofed`; the column `reasons` holds the reasons behind it by their
stable names (`teleport`, `counter_conflict`, `location_clock_drift`, ...). The event `remote_id.verdict` comes at each change
of a verdict, with urgency notice for suspicious and spoofed. The field `flagged` counts the drones that are suspicious or
spoofed now. The verdict is advice: nothing is dropped.

- The receiver's UTC clock is given to the engine only for live samples. A recording and the test signal have no UTC clock,
  so the checks of transmitter clocks against UTC do not run on them. On a live radio, a host clock that is more than 30 s
  wrong makes every drone suspicious (`location_clock_drift`).
- The signal level is not given: it is in dBFS and steps with the radio gain, so a gain change would count as a signal jump.
- Signatures are not verified, so no drone is `authenticated`.
- The engine judges a position jump only when the fix before it is at least 50 ms old, and its speed and course checks need
  a fix at least 2 s old among the last 16. A transmitter that sends Location messages faster than 20 a second is never
  judged for a jump, and one that sends more than 8 a second never for its speed and course.
- The setting `shared_radio` (off by default) flags drones whose packets keep one carrier offset within 1 kHz on the same
  carrier: one radio behind several broadcast addresses. Two real drones can share an offset by chance.
- Verdicts are not saved. A drone of a restored state has no verdict until it is heard again.
