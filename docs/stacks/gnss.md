<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GNSS

GPS, Galileo, BeiDou, GLONASS, QZSS and SBAS: satellites tracked, position and time, the sky, interference and spoofing watch. Needs an active antenna (bias-tee on).

Stack id: `gnss`. Group: Satellite navigation.

Signal types: GNSS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.57542 GHz |
| Tuning range | 1.55 GHz to 1.61 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 2.046 MHz |
| Signal bandwidth | 2.046 MHz |
| Radio filter | 2.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gnss` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tune` | Tune | choice | `l1` | `l1` (L1 / E1 1575.42 MHz), `b1i` (BeiDou B1I 1561.098 MHz), `glo` (GLONASS L1 1602 MHz), `all` (All systems 1582 MHz), `custom` (Custom) | Which band the radio is tuned to. A tune that needs another sample rate restarts the radio. |
| `sys_gps` | GPS | yes or no | true |  | Search and track this system's satellites. |
| `sys_qzss` | QZSS | yes or no | true |  | Search and track this system's satellites. |
| `sys_sbas` | SBAS | yes or no | true |  | Search and track this system's satellites. |
| `sys_galileo` | Galileo | yes or no | true |  | Search and track this system's satellites. |
| `sys_beidou` | BeiDou | yes or no | true |  | Search and track this system's satellites. |
| `sys_glonass` | GLONASS | yes or no | true |  | Search and track this system's satellites. |
| `elevation_mask` | Elevation mask | number (°) | 5 | 0 to 30 | Satellites lower than this are tracked but left out of the position. |
| `search_half_width` | Search half width | frequency | 10 kHz | 1 kHz to 200 kHz | The first frequency window while nothing is found. |
| `frequency_hint` | Radio frequency error | frequency | 0 Hz | -200 kHz to 200 kHz | The remembered frequency error of this radio, as seen at 1575.42 MHz. |
| `frequency_hint_valid` | Use the remembered error | yes or no | false |  |  |
| `acquisition_effort` | Acquisition effort | number (FFT/ms) | 10 | 1 to 100 | Search work per millisecond of signal. |
| `known_week` | Known week | whole number | 0 | 0 to 9999 | The GPS week used to resolve broadcast weeks. 0 takes it from the computer's clock. |
| `clock_compare` | Compare the computer clock | yes or no | false |  | On for a live radio: the computer's clock against GPS time. |
| `galileo_codes` | Galileo E1-B code table | text | empty |  | A text file with the E1-B memory codes of the Galileo OS SIS ICD, Annex C (one hexadecimal code a line). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_gps` | GPS | yes or no | true |  |  |
| `gen_qzss` | QZSS | yes or no | true |  |  |
| `gen_sbas` | SBAS | yes or no | true |  |  |
| `gen_galileo` | Galileo | yes or no | true |  |  |
| `gen_beidou` | BeiDou | yes or no | true |  |  |
| `gen_glonass` | GLONASS | yes or no | true |  |  |
| `gen_sats` | Satellites (0: all) | whole number | 0 | 0 to 12 |  |
| `gen_cold` | Cold start | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `centre_hz` | Centre | frequency |
| `input_rate` | Input rate | frequency |
| `search_system` | Search system | choice |
| `search_prn` | Search PRN | whole number |
| `search_progress_pct` | Search progress | number (%) |
| `search_round` | Search round | whole number |
| `search_centre` | Search centre | frequency |
| `search_half_width` | Search half width | frequency |
| `search_integration_ms` | Search integration | number (ms) |
| `frequency_error` | Frequency error | frequency |
| `frequency_error_ppm` | Frequency error | number (ppm) |
| `frequency_source` | Frequency error source | choice |
| `first_lock` | First lock | duration (s) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `sbas_messages` | SBAS messages | whole number |
| `glonass_tau_gps_s` | GLONASS tau GPS | number (s) |
| `glonass_tau_c_s` | GLONASS tau c | number (s) |
| `bdt_gps_s` | BDT - GPS (less 14 s) | number (s) |
| `almanac_satellites` | Almanac | whole number |
| `iono_broadcast` | Ionosphere from the satellites | yes or no |
| `leap_seconds` | Leap seconds | whole number (s) |
| `leap_seconds_broadcast` | Leap seconds from the satellites | yes or no |
| `level_dbfs` | Level | number (dBFS) |
| `clipped_pct` | Clipped | number (%) |
| `dc_i` | DC I | number |
| `dc_q` | DC Q | number |
| `signal` | Signal time | duration (s) |
| `progress` | Progress | text |
| `position` | Position | position |
| `height_m` | Height | number (m) |
| `error_h_m` | Horizontal error | number (m) |
| `error_v_m` | Vertical error | number (m) |
| `hdop` | HDOP | number |
| `vdop` | VDOP | number |
| `pdop` | PDOP | number |
| `satellites_used` | Satellites used | whole number |
| `satellites_tracked` | Satellites tracked | whole number |
| `utc` | UTC | time |
| `gps_week` | GPS week | whole number |
| `gps_tow_s` | GPS time of week | number (s) |
| `speed_kt` | Speed | number (kt) |
| `course_deg` | Course | number (°) |
| `clock_bias_m` | Clock bias | number (m) |
| `first_fix` | First fix | duration (s) |
| `interference` | Interference | choice |
| `swept` | Swept | yes or no |
| `pulsed` | Pulsed | yes or no |
| `narrowband` | Narrowband | yes or no |
| `wideband` | Wideband | yes or no |
| `interference_reason` | Reason | text |
| `noise_floor_dbfs` | Noise floor | number (dBFS/Hz) |
| `band_power_dbfs` | Band power | number (dBFS) |
| `flatness` | Flatness | number |
| `spoofing` | Spoofing | yes or no |
| `spoofing_reason` | Spoofing reason | text |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `systems` | Systems | `system` (choice), `wanted` (yes or no), `active` (yes or no), `searched` (yes or no) |
| `fix_systems` | Systems in the fix | `system` (choice), `satellites` (whole number), `clock_m` (number (m)) |
| `satellites` | Satellites | `system` (choice), `prn` (whole number), `glo_channel` (whole number), `tracked` (yes or no), `state` (choice), `cn0_dbhz` (number (dB-Hz)), `doppler` (frequency), `code_chips` (number (chips)), `azimuth_deg` (number (°)), `elevation_deg` (number (°)), `locked_for` (duration (s)), `frames_good` (whole number), `frames` (whole number), `residual_m` (number (m)), `healthy` (yes or no), `used` (yes or no), `ephemeris` (choice), `parts` (whole number), `parts_of` (whole number), `seconds_left` (duration (s)), `iod` (whole number), `data_age` (duration (s)), `week` (whole number), `tow_s` (number (s)), `af0_s` (number (s)), `sub_point` (position (°)), `height_km` (number (km)), `orbit` (choice) |
| `interference_lines` | Interference lines | `offset` (frequency), `level_db` (number (dB)), `known` (yes or no) |
| `interference_events` | Interference events | `at` (duration (s)), `level` (choice), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `acquisition` | Search correlation |  |  |
| `scatter` | Prompt scatter |  |  |
| `level` | Input level | s | dBFS |
| `best_cn0` | Best C/N0 | s | dB-Hz |
| `clock_offset` | Clock offset | s | ms |
| `spectrum` | Band spectrum | bin | dB |
| `history` | Noise floor | s | dBFS/Hz |

## Map points

The stack puts points on a map: Satellites.

## Events

| Kind | Label | Fields |
|---|---|---|
| `gnss.fix` | Fix | `family` (choice), `position` (position), `height_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `satellites_used` (whole number) |
| `text.nmea` | NMEA sentence | `family` (choice), `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `reset` | Reset |  |  | Drops every channel and search. The navigation data stays. |

## Typed interface

The stack is in the family `gnss`, version 1.0. The C header `sdrkit/gnss.h`, the registry record `registry/gnss.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `GnssFields`. The result fields of the receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `centre_hz` | frequency | always | 1.0 | The centre frequency of the input. |
| `input_rate` | frequency | always | 1.0 | The sample rate of the input. |
| `search_system` | choice | optional | 1.0 | The system that the search works on now. |
| `search_prn` | whole number | optional | 1.0 | The PRN that the search works on now. |
| `search_progress_pct` | number (%) | optional | 1.0 | The part of the search round done. |
| `search_round` | whole number | optional | 1.0 | The number of the search round, from 1. |
| `search_centre` | frequency | optional | 1.0 | The centre of the search window (the radio's frequency error at L1). |
| `search_half_width` | frequency | optional | 1.0 | The half width of the search window. |
| `search_integration_ms` | number (ms) | optional | 1.0 | The integration of the search. |
| `frequency_error` | frequency | optional | 1.0 | The radio's frequency error at L1, measured by the fix. |
| `frequency_error_ppm` | number (ppm) | optional | 1.0 | The same error in parts per million of 1575.42 MHz. |
| `frequency_source` | choice | always | 1.0 | Where the frequency error comes from. |
| `first_lock` | duration (s) | optional | 1.0 | Signal time of the first lock. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `sbas_messages` | whole number | always | 1.0 | SBAS messages with a good CRC. |
| `glonass_tau_gps_s` | number (s) | optional | 1.0 | GLONASS time minus GPS time. |
| `glonass_tau_c_s` | number (s) | optional | 1.0 | GLONASS time minus UTC(SU). |
| `bdt_gps_s` | number (s) | optional | 1.0 | BeiDou time minus GPS time, less 14 s. |
| `almanac_satellites` | whole number | always | 1.0 | GPS satellites with an almanac entry. |
| `iono_broadcast` | yes or no | always | 1.0 | Whether the ionosphere model comes from the satellites. |
| `leap_seconds` | whole number (s) | optional | 1.0 | GPS time minus UTC: broadcast, else from the station's leap-second table. |
| `leap_seconds_broadcast` | yes or no | always | 1.0 | Whether the satellites broadcast the leap seconds. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `clipped_pct` | number (%) | always | 1.0 | Input samples clipped. |
| `dc_i` | number | always | 1.0 | Input DC, in phase (full scale 1). |
| `dc_q` | number | always | 1.0 | Input DC, quadrature (full scale 1). |
| `signal` | duration (s) | always | 1.0 | Signal time fed. |
| `progress` | text | always | 1.0 | What the receiver waits for while it has no fix, for people, one item a line. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `height_m` | number (m) | optional | 1.0 | The height of the object that the event is about, metres above mean sea level. |
| `error_h_m` | number (m) | optional | 1.0 | Horizontal error, 1 sigma. |
| `error_v_m` | number (m) | optional | 1.0 | Vertical error, 1 sigma. |
| `hdop` | number | optional | 1.0 | Horizontal dilution of precision. |
| `vdop` | number | optional | 1.0 | Vertical dilution of precision. |
| `pdop` | number | optional | 1.0 | Position dilution of precision. |
| `satellites_used` | whole number | optional | 1.0 | Satellites in the fix. |
| `satellites_tracked` | whole number | always | 1.0 | Satellites tracked. |
| `utc` | time | optional | 1.0 | UTC of the fix. |
| `gps_week` | whole number | optional | 1.0 | GPS week of the fix. |
| `gps_tow_s` | number (s) | optional | 1.0 | GPS seconds of the week of the fix. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `clock_bias_m` | number (m) | optional | 1.0 | The receiver's clock bias. |
| `first_fix` | duration (s) | optional | 1.0 | Signal time of the first fix. |

### Result fields

Struct `InterferenceFields`. The result fields of the interference monitor.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `interference` | choice | always | 1.0 | The monitor's verdict. |
| `swept` | yes or no | always | 1.0 | A tone that sweeps through the band. |
| `pulsed` | yes or no | always | 1.0 | Pulses. |
| `narrowband` | yes or no | always | 1.0 | A narrow line that the receiver does not know. |
| `wideband` | yes or no | always | 1.0 | A raised noise floor. |
| `interference_reason` | text | always | 1.0 | Why the monitor says so, for people. |
| `noise_floor_dbfs` | number (dBFS/Hz) | always | 1.0 | The noise floor, dBFS in 1 Hz. |
| `band_power_dbfs` | number (dBFS) | always | 1.0 | The band power. |
| `flatness` | number | always | 1.0 | Spectral flatness, 0 to 1. |
| `spoofing` | yes or no | always | 1.0 | Whether spoofing is suspected. |
| `spoofing_reason` | text | optional | 1.0 | Why spoofing is suspected, for people. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `systems`

Struct `GnssSystemRow`. A satellite system. The row key is the key of the system.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system` | choice | always | 1.0 | A satellite system. |
| `wanted` | yes or no | always | 1.0 | This system is selected for reception. |
| `active` | yes or no | always | 1.0 | The receiver has a band for this system at the present tune. |
| `searched` | yes or no | always | 1.0 | The receiver has searched or tracks this system. |

### Table `fix_systems`

Struct `GnssFixSystemRow`. A satellite system in the fix. The row key is the key of the system.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system` | choice | always | 1.0 | A satellite system. |
| `satellites` | whole number | always | 1.0 | Satellites of a system in the fix. |
| `clock_m` | number (m) | always | 1.0 | The receiver clock of a system in the fix. |

### Table `satellites`

Struct `GnssSatelliteRow`. A satellite: what the channels, the navigation data, the sky and the orbits say of it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system` | choice | optional | 1.0 | A satellite system. |
| `prn` | whole number | optional | 1.0 | A PRN or slot number. |
| `glo_channel` | whole number | optional | 1.0 | The frequency channel of a GLONASS satellite. |
| `tracked` | yes or no | always | 1.0 | A channel tracks the satellite. |
| `state` | choice | optional | 1.0 | The state of the satellite's channel. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | Carrier to noise density. |
| `doppler` | frequency | optional | 1.0 | Doppler at the signal's carrier, with the radio's error. |
| `code_chips` | number (chips) | optional | 1.0 | Code phase. |
| `azimuth_deg` | number (°) | optional | 1.0 | Azimuth, degrees from true north. |
| `elevation_deg` | number (°) | optional | 1.0 | Elevation. |
| `locked_for` | duration (s) | optional | 1.0 | Time since lock was gained, seconds. |
| `frames_good` | whole number | optional | 1.0 | Frames that passed their check. |
| `frames` | whole number | optional | 1.0 | Frames received. |
| `residual_m` | number (m) | optional | 1.0 | The residual of the satellite in the fix. |
| `healthy` | yes or no | optional | 1.0 | The satellite says it is healthy. |
| `used` | yes or no | always | 1.0 | The satellite is in the fix. |
| `ephemeris` | choice | optional | 1.0 | What the receiver holds of the ephemeris. |
| `parts` | whole number | optional | 1.0 | Ephemeris parts collected. |
| `parts_of` | whole number | optional | 1.0 | Ephemeris parts needed. |
| `seconds_left` | duration (s) | optional | 1.0 | About how long until the ephemeris is complete. |
| `iod` | whole number | optional | 1.0 | The issue of data (IODE, IODnav, AODE or the GLONASS tb index). |
| `data_age` | duration (s) | optional | 1.0 | The age of the data from its epoch (negative: ahead). |
| `week` | whole number | optional | 1.0 | The week of the data. |
| `tow_s` | number (s) | optional | 1.0 | The time of week of the last unit. |
| `af0_s` | number (s) | optional | 1.0 | The satellite clock af0. |
| `sub_point` | position (°) | optional | 1.0 | The point below the satellite. |
| `height_km` | number (km) | optional | 1.0 | The height of the satellite. |
| `orbit` | choice | optional | 1.0 | Where the satellite's position comes from. |

### Table `interference_lines`

Struct `InterferenceLineRow`. A line of the interference spectrum.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset` | frequency | always | 1.0 | The offset of a line from the centre. |
| `level_db` | number (dB) | always | 1.0 | A level above the floor. |
| `known` | yes or no | always | 1.0 | A line that the receiver has identified (a satellite or the radio's own). |

### Table `interference_events`

Struct `InterferenceEventRow`. An interference event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | Signal time of an event. |
| `level` | choice | always | 1.0 | The level of an event. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Event `gnss.fix`

Struct `GnssFixEvent`. A fix.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `height_m` | number (m) | always | 1.0 | The height of the object that the event is about, metres above mean sea level. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `satellites_used` | whole number | always | 1.0 | Satellites in the fix. |

### Event `text.nmea`

Struct `NmeaEvent`. An NMEA 0183 sentence.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The family of the event. |
| `text` | text | always | 1.0 | The sentence. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `reset` | `sdrkit_gnss_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `tune` | `sdrkit_gnss_get_tune` | `sdrkit_gnss_set_tune` |
| `sys_gps` | `sdrkit_gnss_get_sys_gps` | `sdrkit_gnss_set_sys_gps` |
| `sys_qzss` | `sdrkit_gnss_get_sys_qzss` | `sdrkit_gnss_set_sys_qzss` |
| `sys_sbas` | `sdrkit_gnss_get_sys_sbas` | `sdrkit_gnss_set_sys_sbas` |
| `sys_galileo` | `sdrkit_gnss_get_sys_galileo` | `sdrkit_gnss_set_sys_galileo` |
| `sys_beidou` | `sdrkit_gnss_get_sys_beidou` | `sdrkit_gnss_set_sys_beidou` |
| `sys_glonass` | `sdrkit_gnss_get_sys_glonass` | `sdrkit_gnss_set_sys_glonass` |
| `elevation_mask` | `sdrkit_gnss_get_elevation_mask` | `sdrkit_gnss_set_elevation_mask` |
| `search_half_width` | `sdrkit_gnss_get_search_half_width` | `sdrkit_gnss_set_search_half_width` |
| `frequency_hint` | `sdrkit_gnss_get_frequency_hint` | `sdrkit_gnss_set_frequency_hint` |
| `frequency_hint_valid` | `sdrkit_gnss_get_frequency_hint_valid` | `sdrkit_gnss_set_frequency_hint_valid` |
| `acquisition_effort` | `sdrkit_gnss_get_acquisition_effort` | `sdrkit_gnss_set_acquisition_effort` |
| `known_week` | `sdrkit_gnss_get_known_week` | `sdrkit_gnss_set_known_week` |
| `clock_compare` | `sdrkit_gnss_get_clock_compare` | `sdrkit_gnss_set_clock_compare` |
| `galileo_codes` | `sdrkit_gnss_get_galileo_codes` | `sdrkit_gnss_set_galileo_codes` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_gps` | `sdrkit_gnss_gen_get_gps` | `sdrkit_gnss_gen_set_gps` |
| `gen_qzss` | `sdrkit_gnss_gen_get_qzss` | `sdrkit_gnss_gen_set_qzss` |
| `gen_sbas` | `sdrkit_gnss_gen_get_sbas` | `sdrkit_gnss_gen_set_sbas` |
| `gen_galileo` | `sdrkit_gnss_gen_get_galileo` | `sdrkit_gnss_gen_set_galileo` |
| `gen_beidou` | `sdrkit_gnss_gen_get_beidou` | `sdrkit_gnss_gen_set_beidou` |
| `gen_glonass` | `sdrkit_gnss_gen_get_glonass` | `sdrkit_gnss_gen_set_glonass` |
| `gen_sats` | `sdrkit_gnss_gen_get_sats` | `sdrkit_gnss_gen_set_sats` |
| `gen_cold` | `sdrkit_gnss_gen_get_cold` | `sdrkit_gnss_gen_set_cold` |

## Try it

```sh
sdrkit run gnss --source synth:gnss
sdrkit run gnss --source iiod:192.168.2.1 --freq 1575.42
```

## Test recordings

- `tests/vectors/gnss/gnss_l1_multi_4m.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
