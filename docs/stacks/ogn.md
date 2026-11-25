<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# OGN tracker

Position packets of Open Glider Network trackers on the channels of one capture: address, position, altitude, speed, heading, climb and aircraft type. Packets with an encrypted position are counted. Experimental: the sync word, the code table, the whitening and the field layout are not confirmed by a public document.

Stack id: `ogn`. Group: Aircraft and drones.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.3 MHz |
| Tuning range | 860 MHz to 930 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 600 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ogn` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | whole number | 2 | 1 to 80 | The count of channels of the plan of your area. The tuned frequency is the middle of the plan. The default is the plan of the test signal. |
| `channel_step_hz` | Channel spacing | frequency | 200 kHz | 100 kHz to 2 MHz | The distance between two channels. The default is the plan of the test signal. |
| `expiry_s` | Forget trackers after | number (s) | 600 | 30 to 86400 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre` | Test signal centre | position | 54.7, 25.3 |  |  |
| `gen_trackers` | Test signal trackers | whole number | 4 | 1 to 4 |  |
| `gen_bad_bits` | Wrong bits in every third packet | whole number | 0 | 0 to 4 |  |
| `gen_emergency` | Test signal with an emergency | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `syncs` | Syncs | whole number |
| `corrected` | Repaired packets | whole number |
| `encrypted` | Encrypted packets | whole number |
| `trackers` | Trackers | whole number |
| `channels_on` | Channels received | whole number |
| `channels` | Channels | text |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `note` | Note | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `packets` | Packets | `time_s` (number (s)), `kind` (choice), `id` (text), `decoded` (text), `position` (position), `channel_hz` (frequency), `repaired` (yes or no), `hex` (text) |
| `aircraft` | Aircraft | `source` (text), `address_type` (text), `aircraft_type` (text), `position` (position), `alt_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `climb_m_s` (number (m/s)), `turn_deg_s` (number (°/s)), `relay` (whole number), `stealth` (yes or no), `fix_quality` (whole number), `second` (whole number), `level_db` (number (dB)), `channel_hz` (frequency), `private` (yes or no), `urgency` (choice), `heard` (whole number), `first_heard` (time), `last_heard` (time) |
| `aircraft_tracks` | Tracks | `aircraft` (text), `position` (position), `time_s` (number (s)) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `ogn.tracker` | OGN tracker | `source` (text), `address_type` (text), `aircraft_type` (text), `position` (position), `private` (yes or no), `family` (choice) |
| `ogn.emergency` | OGN tracker emergency | `source` (text), `text` (text), `position` (position), `private` (yes or no), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 2.0. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `OgnFields`. The result fields of the OGN tracker.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `syncs` | whole number | always | 1.0 | Sync words found. |
| `corrected` | whole number | always | 1.0 | Packets that needed a repair. |
| `encrypted` | whole number | always | 1.0 | Packets with an encrypted position. |
| `trackers` | whole number | always | 1.0 | Trackers in the table. |
| `channels_on` | whole number | always | 1.0 | The channels of the plan inside the capture. |
| `channels` | text | always | 1.0 | The channels of the plan and which are received. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | Noise floor, dBFS. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `note` | text | always | 1.0 | What no public document confirms. |

### Table `packets`

Struct `OgnPacketRow`. A packet in the packet log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time at the end of a packet. |
| `kind` | choice | always | 1.0 | What a packet is. |
| `id` | text | always | 1.0 | The address and the name of its type. |
| `decoded` | text | always | 1.0 | A packet in one line. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `channel_hz` | frequency | always | 1.0 | The radio channel. |
| `repaired` | yes or no | always | 1.0 | The repair changed a bit of the packet. |
| `hex` | text | always | 1.0 | The 26 bytes of a packet after the repair. |

The column `hex` names a row. Place of a row: the column `position`.

### Table `aircraft`

Struct `OgnAircraftRow`. An aircraft heard on OGN.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address_type` | text | always | 1.0 | The name of the address type. |
| `aircraft_type` | text | optional | 1.0 | The aircraft type. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | optional | 1.0 | The drone's altitude. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `climb_m_s` | number (m/s) | optional | 1.0 | Climb. |
| `turn_deg_s` | number (°/s) | optional | 1.0 | The turn rate. |
| `relay` | whole number | always | 1.0 | The count of relays. |
| `stealth` | yes or no | optional | 1.0 | The sender does not want to be shown by other trackers. |
| `fix_quality` | whole number | optional | 1.0 | The fix quality, 0 to 3. |
| `second` | whole number | optional | 1.0 | The second of the minute of the last position. |
| `level_db` | number (dB) | always | 1.0 | Level over the noise. |
| `channel_hz` | frequency | always | 1.0 | The radio channel. |
| `private` | yes or no | always | 1.0 | The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `first_heard` | time | optional | 1.0 | Wall-clock time when first heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

The column `source` names a row. Place of a row: the column `position`. Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.

### Table `aircraft_tracks`

Struct `AircraftTrackRow`. A track point of an aircraft. The row key is the aircraft key and the signal time of the point.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aircraft` | text | always | 1.0 | The aircraft a track point belongs to. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `time_s` | number (s) | always | 1.0 | The signal time of a track point. |

Place of a row: the column `position`.

### Event `ogn.tracker`

Struct `OgnTrackerEvent`. An OGN tracker heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address_type` | text | always | 1.0 | The name of the address type. |
| `aircraft_type` | text | always | 1.0 | The aircraft type. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `private` | yes or no | always | 1.0 | The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `ogn.emergency`

Struct `OgnEmergencyEvent`. An OGN tracker in an emergency.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `private` | yes or no | always | 1.0 | The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ogn_clear` | none | none |
| `remove` | `sdrkit_ogn_remove` | `sdrkit_drones_aircraft_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_ogn_get_channels` | `sdrkit_ogn_set_channels` |
| `channel_step_hz` | `sdrkit_ogn_get_channel_step_hz` | `sdrkit_ogn_set_channel_step_hz` |
| `expiry_s` | `sdrkit_ogn_get_expiry_s` | `sdrkit_ogn_set_expiry_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre` | `sdrkit_ogn_gen_get_centre` | `sdrkit_ogn_gen_set_centre` |
| `gen_trackers` | `sdrkit_ogn_gen_get_trackers` | `sdrkit_ogn_gen_set_trackers` |
| `gen_bad_bits` | `sdrkit_ogn_gen_get_bad_bits` | `sdrkit_ogn_gen_set_bad_bits` |
| `gen_emergency` | `sdrkit_ogn_gen_get_emergency` | `sdrkit_ogn_gen_set_emergency` |

## Try it

```sh
sdrkit run ogn --source synth:ogn
sdrkit run ogn --source iiod:192.168.2.1 --freq 868.3
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
