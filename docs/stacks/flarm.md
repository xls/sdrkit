<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FLARM

Gliders and light aircraft on 868 MHz: the old FLARM protocol's positions, privacy flags respected.

Stack id: `flarm`. Group: Aircraft and drones.

Signal types: FLARM.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.3 MHz |
| Tuning range | 860 MHz to 930 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 400 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:flarm` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `wall_clock` | Key time from this computer's clock | yes or no | true |  | On for a radio: the packets' key time is the computer's clock (keep it right to a few seconds). Off, or for a recording or the test signal: the start time plus signal time. |
| `start_time` | Start time (unix seconds) | number (s) | 0 | 0 to 4000000000 | The key time of the first sample when the computer's clock is not used. 0: the start of playback. |
| `expiry_s` | Forget aircraft after | number (s) | 60 | 5 to 3600 | Silence after which an aircraft leaves the table. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre` | Test signal centre | position | 54.7, 25.3 |  | The position the simulated aircraft circle. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `v6_decoded` | Packets decoded | whole number |
| `frames_bad` | Frames bad | whole number |
| `syncs` | Syncs | whole number |
| `frames_868_2` | Frames on 868.2 MHz | whole number |
| `frames_868_4` | Frames on 868.4 MHz | whole number |
| `newer_packets` | Newer-protocol packets | whole number |
| `newer_devices` | Newer-protocol devices | whole number |
| `other_type` | Other packets | whole number |
| `privacy_aircraft` | Privacy-flagged aircraft | whole number |
| `privacy_packets` | Privacy-flagged packets | whole number |
| `aircraft` | Aircraft | whole number |
| `with_position` | With a position | whole number |
| `farthest_km` | Farthest | number (km) |
| `packets_per_s` | Packets per second | number |
| `key_time` | Key time | choice |
| `needs_station` | Needs the station position | yes or no |
| `snr_db` | SNR | number (dB) |
| `noise_dbfs` | Noise floor | number (dBFS) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `aircraft` | Aircraft | `source` (text), `address_type` (whole number), `type` (whole number), `position` (position), `alt_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `climb_m_s` (number (m/s)), `distance_km` (number (km)), `bearing_deg` (number (°)), `channel_hz` (frequency), `level_db` (number (dB)), `airborne` (yes or no), `urgency` (choice), `last_heard` (time), `heard` (whole number) |
| `aircraft_tracks` | Tracks | `aircraft` (text), `position` (position), `time_s` (number (s)) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `flarm.aircraft` | Aircraft | `source` (text), `type` (whole number), `position` (position), `alt_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `climb_m_s` (number (m/s)), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 1.4. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `FlarmFields`. The result fields of FLARM.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `v6_decoded` | whole number | always | 1.0 | Old-protocol packets decoded. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `syncs` | whole number | always | 1.0 | Syncs taken. |
| `frames_868_2` | whole number | always | 1.0 | Good frames on 868.2 MHz. |
| `frames_868_4` | whole number | always | 1.0 | Good frames on 868.4 MHz. |
| `newer_packets` | whole number | always | 1.0 | Newer-protocol packets. |
| `newer_devices` | whole number | always | 1.0 | Newer-protocol devices. |
| `other_type` | whole number | always | 1.0 | Packets of other types. |
| `privacy_aircraft` | whole number | always | 1.0 | Privacy-flagged aircraft. |
| `privacy_packets` | whole number | always | 1.0 | Privacy-flagged packets. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the table. |
| `with_position` | whole number | always | 1.0 | Aircraft with a position. |
| `farthest_km` | number (km) | always | 1.0 | The farthest range seen. |
| `packets_per_s` | number | always | 1.0 | Packets a second, over the last second. |
| `key_time` | choice | always | 1.0 | Where the key time comes from. |
| `needs_station` | yes or no | always | 1.0 | The station position is missing: without it the packets give no positions. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | Noise floor, dBFS. |

### Table `aircraft`

Struct `FlarmAircraftRow`. An aircraft heard on FLARM.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address_type` | whole number | always | 1.0 | Address type code (0 random, 1 ICAO, 2 FLARM, 3 anonymous). |
| `type` | whole number | always | 1.0 | Aircraft type code (0 to 15). |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | always | 1.0 | Altitude. |
| `speed_kt` | number (kt) | always | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | always | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `climb_m_s` | number (m/s) | always | 1.0 | Climb. |
| `distance_km` | number (km) | optional | 1.0 | Distance from the station. |
| `bearing_deg` | number (°) | optional | 1.0 | Bearing from the station. |
| `channel_hz` | frequency | always | 1.0 | The radio channel. |
| `level_db` | number (dB) | always | 1.0 | Level over the noise. |
| `airborne` | yes or no | always | 1.0 | Airborne. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `heard` | whole number | optional | 1.0 | Times heard. |

The column `source` names a row. Place of a row: the column `position`. Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.

### Table `aircraft_tracks`

Struct `AircraftTrackRow`. A track point of an aircraft. The row key is the aircraft key and the signal time of the point.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aircraft` | text | always | 1.0 | The aircraft a track point belongs to. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `time_s` | number (s) | always | 1.0 | The signal time of a track point. |

Place of a row: the column `position`.

### Event `flarm.aircraft`

Struct `FlarmAircraftEvent`. An aircraft heard on FLARM.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `type` | whole number | always | 1.0 | Aircraft type code (0 to 15). |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | always | 1.0 | Altitude. |
| `speed_kt` | number (kt) | always | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | always | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `climb_m_s` | number (m/s) | always | 1.0 | Climb. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_flarm_clear` | none | none |
| `remove` | `sdrkit_flarm_remove` | `sdrkit_drones_aircraft_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `wall_clock` | `sdrkit_flarm_get_wall_clock` | `sdrkit_flarm_set_wall_clock` |
| `start_time` | `sdrkit_flarm_get_start_time` | `sdrkit_flarm_set_start_time` |
| `expiry_s` | `sdrkit_flarm_get_expiry_s` | `sdrkit_flarm_set_expiry_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre` | `sdrkit_flarm_gen_get_centre` | `sdrkit_flarm_gen_set_centre` |

## Try it

```sh
sdrkit run flarm --source synth:flarm
sdrkit run flarm --source iiod:192.168.2.1 --freq 868.3
```

## Test recordings

- `tests/vectors/drones/flarm_868_1m.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
