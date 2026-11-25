<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ADS-L

ADS-L traffic, status and ground uplink of light aircraft on 868.2, 868.4 and 869.525 MHz from one capture, and FLARM from the same capture.

Stack id: `adsl`. Group: Aircraft and drones.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.875 MHz |
| Tuning range | 860 MHz to 930 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:adsl` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `m1_hz` | M-band 1 | frequency | 868.2 MHz | 100 MHz to 2 GHz | The first Manchester channel. The default is the frequency of the test signal. |
| `m2_hz` | M-band 2 | frequency | 868.4 MHz | 100 MHz to 2 GHz | The second Manchester channel. The default is the frequency of the test signal. |
| `o_hz` | O-band | frequency | 869.525 MHz | 100 MHz to 2 GHz | The channel of the low and the high data rate. The default is the frequency of the test signal. |
| `expiry_s` | Forget aircraft after | number (s) | 60 | 5 to 3600 |  |
| `with_flarm` | Decode FLARM from the same capture | yes or no | true |  |  |
| `wall_clock` | Key time from this computer's clock | yes or no | true |  | On for a radio: the packets' key time is the computer's clock (keep it right to a few seconds). Off, or for a recording or the test signal: the start time plus signal time. |
| `start_time` | Start time (unix seconds) | number (s) | 0 | 0 to 4000000000 | The key time of the first sample when the computer's clock is not used. 0: the start of playback. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tone_chips` | M-band chip detector by tone energy | yes or no | true |  | Off selects the FM discriminator, for comparisons. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre` | Test signal centre | position | 54.7, 25.3 |  |  |
| `gen_aircraft` | Test signal aircraft | whole number | 4 | 1 to 4 |  |
| `gen_key_index` | Test signal key index | whole number | 0 | 0 to 3 |  |
| `gen_fec` | Test signal with Reed-Solomon | yes or no | false |  |  |
| `gen_preamble` | Test signal M-band preamble | whole number | 8 | 5 to 12 |  |
| `gen_bad_bit` | Wrong bit in every n-th packet | whole number | 0 | 0 to 100 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `packets_868_2` | Packets on 868.2 MHz | whole number |
| `packets_868_4` | Packets on 868.4 MHz | whole number |
| `packets_ldr` | Packets on O-band LDR | whole number |
| `packets_hdr` | Packets on O-band HDR | whole number |
| `on_868_2` | 868.2 MHz received | yes or no |
| `on_868_4` | 868.4 MHz received | yes or no |
| `on_ldr` | O-band LDR received | yes or no |
| `on_hdr` | O-band HDR received | yes or no |
| `traffic` | Traffic packets | whole number |
| `status` | Status packets | whole number |
| `uplink` | Uplink packets | whole number |
| `fis_b` | FIS-B packets | whole number |
| `remote_id` | Remote ID packets | whole number |
| `other` | Other packets | whole number |
| `encrypted` | Encrypted packets | whole number |
| `signed` | Signed packets | whole number |
| `corrected` | Repaired packets | whole number |
| `syncs` | Syncs | whole number |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `packets_per_s` | Packets per second | number |
| `aircraft` | Aircraft | whole number |
| `with_position` | With a position | whole number |
| `max_range_km` | Maximum range | number (km) |
| `flarm_packets` | FLARM packets | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `aircraft` | Aircraft | `source` (text), `address_source` (text), `category` (text), `flight_state` (text), `emergency` (text), `position` (position), `alt_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `climb_m_s` (number (m/s)), `level_db` (number (dB)), `distance_km` (number (km)), `bearing_deg` (number (°)), `channel_hz` (frequency), `channel` (text), `relayed` (yes or no), `nic` (whole number), `hfom` (whole number), `vfom` (whole number), `velocity_accuracy` (whole number), `capabilities` (text), `private` (yes or no), `urgency` (choice), `last_heard` (time), `heard` (whole number) |
| `aircraft_tracks` | Tracks | `aircraft` (text), `position` (position), `time_s` (number (s)) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `adsl.aircraft` | ADS-L aircraft | `source` (text), `address_source` (text), `category` (text), `position` (position), `private` (yes or no), `family` (choice) |
| `adsl.emergency` | ADS-L emergency | `source` (text), `text` (text), `position` (position), `private` (yes or no), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 2.0. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `AdslFields`. The result fields of ADS-L.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `packets_868_2` | whole number | always | 1.0 | Packets with good parity on the band. |
| `packets_868_4` | whole number | always | 1.0 | Packets with good parity on the band. |
| `packets_ldr` | whole number | always | 1.0 | Packets with good parity on the band. |
| `packets_hdr` | whole number | always | 1.0 | Packets with good parity on the band. |
| `on_868_2` | yes or no | always | 1.0 | The band is inside the capture and received. |
| `on_868_4` | yes or no | always | 1.0 | The band is inside the capture and received. |
| `on_ldr` | yes or no | always | 1.0 | The band is inside the capture and received. |
| `on_hdr` | yes or no | always | 1.0 | The band is inside the capture and received. |
| `traffic` | whole number | always | 1.0 | Traffic packets. |
| `status` | whole number | always | 1.0 | Status packets. |
| `uplink` | whole number | always | 1.0 | Traffic Uplink packets. |
| `fis_b` | whole number | always | 1.0 | FIS-B Uplink packets. |
| `remote_id` | whole number | always | 1.0 | Remote Identification packets. |
| `other` | whole number | always | 1.0 | Packets of other payload types. |
| `encrypted` | whole number | always | 1.0 | Packets with a reserved key index: their content is not read. |
| `signed` | whole number | always | 1.0 | Packets with the signature flag. The signature is not checked: no register of public keys exists. |
| `corrected` | whole number | always | 1.0 | Packets that needed a repair. |
| `syncs` | whole number | always | 1.0 | Syncs taken. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | Noise floor, dBFS. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `packets_per_s` | number | always | 1.0 | Good packets a second, over the last second. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the table. |
| `with_position` | whole number | always | 1.0 | Aircraft with a position. |
| `max_range_km` | number (km) | always | 1.0 | The farthest range seen. |
| `flarm_packets` | whole number | always | 1.0 | FLARM packets decoded from the same capture. |

### Table `aircraft`

Struct `AdslAircraftRow`. An aircraft heard on ADS-L.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address_source` | text | always | 1.0 | Where the address of an aircraft comes from. |
| `category` | text | always | 1.0 | The aircraft category. |
| `flight_state` | text | always | 1.0 | The flight state. |
| `emergency` | text | always | 1.0 | The emergency status. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | optional | 1.0 | The drone's altitude. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `climb_m_s` | number (m/s) | optional | 1.0 | Climb. |
| `level_db` | number (dB) | always | 1.0 | Level over the noise. |
| `distance_km` | number (km) | optional | 1.0 | Distance from the station. |
| `bearing_deg` | number (°) | optional | 1.0 | Bearing from the station. |
| `channel_hz` | frequency | always | 1.0 | The radio channel. |
| `channel` | text | always | 1.0 | The channel of the last packet. |
| `relayed` | yes or no | always | 1.0 | Last heard through a ground station or a relay. |
| `nic` | whole number | always | 1.0 | The navigation integrity category. |
| `hfom` | whole number | always | 1.0 | The horizontal position accuracy category. |
| `vfom` | whole number | always | 1.0 | The vertical position accuracy category. |
| `velocity_accuracy` | whole number | always | 1.0 | The velocity accuracy category. |
| `capabilities` | text | always | 1.0 | The capabilities of a Status packet. |
| `private` | yes or no | always | 1.0 | The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text. |
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

### Event `adsl.aircraft`

Struct `AdslAircraftEvent`. An aircraft heard on ADS-L.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `address_source` | text | always | 1.0 | Where the address of an aircraft comes from. |
| `category` | text | always | 1.0 | The aircraft category. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `private` | yes or no | always | 1.0 | The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `adsl.emergency`

Struct `AdslEmergencyEvent`. An ADS-L aircraft in an emergency.

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
| `clear` | `sdrkit_adsl_clear` | none | none |
| `remove` | `sdrkit_adsl_remove` | `sdrkit_drones_aircraft_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `m1_hz` | `sdrkit_adsl_get_m1_hz` | `sdrkit_adsl_set_m1_hz` |
| `m2_hz` | `sdrkit_adsl_get_m2_hz` | `sdrkit_adsl_set_m2_hz` |
| `o_hz` | `sdrkit_adsl_get_o_hz` | `sdrkit_adsl_set_o_hz` |
| `expiry_s` | `sdrkit_adsl_get_expiry_s` | `sdrkit_adsl_set_expiry_s` |
| `with_flarm` | `sdrkit_adsl_get_with_flarm` | `sdrkit_adsl_set_with_flarm` |
| `tone_chips` | `sdrkit_adsl_get_tone_chips` | `sdrkit_adsl_set_tone_chips` |
| `wall_clock` | `sdrkit_adsl_get_wall_clock` | `sdrkit_adsl_set_wall_clock` |
| `start_time` | `sdrkit_adsl_get_start_time` | `sdrkit_adsl_set_start_time` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre` | `sdrkit_adsl_gen_get_centre` | `sdrkit_adsl_gen_set_centre` |
| `gen_aircraft` | `sdrkit_adsl_gen_get_aircraft` | `sdrkit_adsl_gen_set_aircraft` |
| `gen_key_index` | `sdrkit_adsl_gen_get_key_index` | `sdrkit_adsl_gen_set_key_index` |
| `gen_fec` | `sdrkit_adsl_gen_get_fec` | `sdrkit_adsl_gen_set_fec` |
| `gen_preamble` | `sdrkit_adsl_gen_get_preamble` | `sdrkit_adsl_gen_set_preamble` |
| `gen_bad_bit` | `sdrkit_adsl_gen_get_bad_bit` | `sdrkit_adsl_gen_set_bad_bit` |

## Try it

```sh
sdrkit run adsl --source synth:adsl
sdrkit run adsl --source iiod:192.168.2.1 --freq 868.875
```
<!-- generated: end -->
