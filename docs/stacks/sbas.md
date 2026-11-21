<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SBAS

WAAS, EGNOS, MSAS, GAGAN and other SBAS satellites: PRN masks, fast and long-term corrections, ionosphere, GEO navigation and time. Shown, never applied.

Stack id: `sbas`. Group: Satellite navigation.

Signal types: SBAS.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sbas` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `search_half_width` | Search half width | frequency | 10 kHz | 1 kHz to 200 kHz | The first frequency window while nothing is found. |
| `frequency_hint` | Radio frequency error | frequency | 0 Hz | -200 kHz to 200 kHz | The remembered frequency error of this radio, as seen at 1575.42 MHz. |
| `frequency_hint_valid` | Use the remembered error | yes or no | false |  |  |
| `acquisition_effort` | Acquisition effort | number (FFT/ms) | 10 | 1 to 100 | Search work per millisecond of signal. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `data_valid` | Corrections held | yes or no |
| `sbas_messages` | SBAS messages | whole number |
| `messages_missed` | Messages missed | whole number |
| `help` | Help | text |
| `selected` | Selected PRN | whole number |
| `geo_iodn` | GEO IODN | whole number |
| `geo_t0_s` | GEO t0 | number (s) |
| `geo_ura` | GEO URA | whole number |
| `geo_radius_m` | GEO radius | number (m) |
| `geo_longitude_deg` | GEO longitude | number (°) |
| `sbas_week` | Network week | whole number |
| `sbas_tow_s` | Network TOW | whole number (s) |
| `sbas_leap_s` | Leap seconds | whole number (s) |
| `sbas_leap_next_s` | Next leap seconds | whole number (s) |
| `utc_a0_s` | A0 | number (s) |
| `utc_a1` | A1 | number (s/s) |
| `utc_id` | UTC identifier | whole number |
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
| `rx.centre_hz` | Centre | frequency |
| `rx.input_rate` | Input rate | frequency |
| `rx.search_system` | Search system | choice |
| `rx.search_prn` | Search PRN | whole number |
| `rx.search_progress_pct` | Search progress | number (%) |
| `rx.search_round` | Search round | whole number |
| `rx.search_centre` | Search centre | frequency |
| `rx.search_half_width` | Search half width | frequency |
| `rx.search_integration_ms` | Search integration | number (ms) |
| `rx.frequency_error` | Frequency error | frequency |
| `rx.frequency_error_ppm` | Frequency error | number (ppm) |
| `rx.frequency_source` | Frequency error source | choice |
| `rx.first_lock` | First lock | duration (s) |
| `rx.frames_ok` | Frames good | whole number |
| `rx.frames_bad` | Frames bad | whole number |
| `rx.glonass_tau_gps_s` | GLONASS tau GPS | number (s) |
| `rx.glonass_tau_c_s` | GLONASS tau c | number (s) |
| `rx.bdt_gps_s` | BDT - GPS (less 14 s) | number (s) |
| `rx.almanac_satellites` | Almanac | whole number |
| `rx.iono_broadcast` | Ionosphere from the satellites | yes or no |
| `rx.leap_seconds` | Leap seconds | whole number (s) |
| `rx.leap_seconds_broadcast` | Leap seconds from the satellites | yes or no |
| `rx.level_dbfs` | Level | number (dBFS) |
| `rx.clipped_pct` | Clipped | number (%) |
| `rx.dc_i` | DC I | number |
| `rx.dc_q` | DC Q | number |
| `rx.signal` | Signal time | duration (s) |
| `rx.progress` | Progress | text |
| `rx.position` | Position | position |
| `rx.height_m` | Height | number (m) |
| `rx.error_h_m` | Horizontal error | number (m) |
| `rx.error_v_m` | Vertical error | number (m) |
| `rx.hdop` | HDOP | number |
| `rx.vdop` | VDOP | number |
| `rx.pdop` | PDOP | number |
| `rx.satellites_used` | Satellites used | whole number |
| `rx.satellites_tracked` | Satellites tracked | whole number |
| `rx.utc` | UTC | time |
| `rx.gps_week` | GPS week | whole number |
| `rx.gps_tow_s` | GPS time of week | number (s) |
| `rx.speed_kt` | Speed | number (kt) |
| `rx.course_deg` | Course | number (°) |
| `rx.clock_bias_m` | Clock bias | number (m) |
| `rx.first_fix` | First fix | duration (s) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sbas_satellites` | SBAS satellites | `prn` (whole number), `service` (choice), `state` (choice), `cn0_dbhz` (number (dB-Hz)), `doppler` (frequency), `messages` (whole number), `messages_missed` (whole number) |
| `sbas_satellite_types` | Types by satellite | `prn` (whole number), `message_type` (whole number) |
| `types` | Types | `message_type` (whole number), `name` (text), `count` (whole number) |
| `messages` | Messages | `at` (duration (s)), `prn` (whole number), `message_type` (whole number), `text` (text) |
| `fast_corrections` | Fast corrections | `slot` (whole number), `system` (choice), `prn` (whole number), `correction_m` (number (m)), `udrei` (whole number), `udre_sigma_m` (number (m)), `iodf` (whole number), `iodp` (whole number), `iodp_current` (yes or no), `degradation` (number (m/s²)) |
| `ionosphere` | Ionosphere | `band` (whole number), `iodi` (whole number), `points` (whole number), `blocks_seen` (whole number), `blocks_needed` (whole number), `values` (whole number), `not_monitored` (whole number), `min_m` (number (m)), `mean_m` (number (m)), `max_m` (number (m)), `worst_givei` (whole number) |
| `interference_lines` | Interference lines | `offset` (frequency), `level_db` (number (dB)), `known` (yes or no) |
| `interference_events` | Interference events | `at` (duration (s)), `level` (choice), `text` (text) |
| `rx.systems` | Systems | `system` (choice), `wanted` (yes or no), `active` (yes or no), `searched` (yes or no) |
| `rx.fix_systems` | Systems in the fix | `system` (choice), `satellites` (whole number), `clock_m` (number (m)) |
| `rx.satellites` | Satellites | `system` (choice), `prn` (whole number), `glo_channel` (whole number), `tracked` (yes or no), `state` (choice), `cn0_dbhz` (number (dB-Hz)), `doppler` (frequency), `code_chips` (number (chips)), `azimuth_deg` (number (°)), `elevation_deg` (number (°)), `locked_for` (duration (s)), `frames_good` (whole number), `frames` (whole number), `residual_m` (number (m)), `healthy` (yes or no), `used` (yes or no), `ephemeris` (choice), `parts` (whole number), `parts_of` (whole number), `seconds_left` (duration (s)), `iod` (whole number), `data_age` (duration (s)), `week` (whole number), `tow_s` (number (s)), `af0_s` (number (s)), `sub_point` (position (°)), `height_km` (number (km)), `orbit` (choice) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Band spectrum | bin | dB |
| `history` | Noise floor | s | dBFS/Hz |
| `rx.acquisition` | Search correlation |  |  |
| `rx.scatter` | Prompt scatter |  |  |
| `rx.level` | Input level | s | dBFS |
| `rx.best_cn0` | Best C/N0 | s | dB-Hz |
| `rx.clock_offset` | Clock offset | s | ms |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `reset` | Reset |  |  | Drops every channel and search. The navigation data stays. |
| `select` | Select | `row` (text, required) |  | Shows the corrections of this SBAS satellite. |
| `clear` | Clear |  |  | Forgets every message decoded so far. |

## Typed interface

The stack is in the family `gnss`, version 1.0. The C header `sdrkit/gnss.h`, the registry record `registry/gnss.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SbasFields`. The result fields of the SBAS content decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `data_valid` | yes or no | always | 1.0 | A PRN mask and fast corrections are held. |
| `sbas_messages` | whole number | always | 1.0 | The SBAS messages with a good CRC. |
| `messages_missed` | whole number | always | 1.0 | The messages missed. |
| `help` | text | optional | 1.0 | Help for people while nothing is heard. |
| `selected` | whole number | optional | 1.0 | The PRN whose corrections the tables show. |
| `geo_iodn` | whole number | optional | 1.0 | GEO navigation of the selected PRN: the issue of data. |
| `geo_t0_s` | number (s) | optional | 1.0 | GEO navigation: the time of applicability. |
| `geo_ura` | whole number | optional | 1.0 | GEO navigation: the accuracy index. |
| `geo_radius_m` | number (m) | optional | 1.0 | GEO navigation: the distance from the centre of the earth. |
| `geo_longitude_deg` | number (°) | optional | 1.0 | GEO navigation: the longitude. |
| `sbas_week` | whole number | optional | 1.0 | Network time of the selected PRN: the week. |
| `sbas_tow_s` | whole number (s) | optional | 1.0 | Network time: the time of week. |
| `sbas_leap_s` | whole number (s) | optional | 1.0 | Network time: the leap seconds. |
| `sbas_leap_next_s` | whole number (s) | optional | 1.0 | Network time: the leap seconds after the next change. |
| `utc_a0_s` | number (s) | optional | 1.0 | Network time: the UTC offset A0. |
| `utc_a1` | number (s/s) | optional | 1.0 | Network time: the UTC drift A1. |
| `utc_id` | whole number | optional | 1.0 | Network time: the identifier of the UTC standard. |

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

### Table `sbas_satellites`

Struct `SbasSatelliteRow`. A row of the SBAS satellite table. The row key is the PRN.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `prn` | whole number | always | 1.0 | The PRN. |
| `service` | choice | optional | 1.0 | The service of the PRN. |
| `state` | choice | optional | 1.0 | The state of the channel of the satellite. |
| `cn0_dbhz` | number (dB-Hz) | always | 1.0 | The carrier to noise density. |
| `doppler` | frequency | always | 1.0 | The Doppler at the carrier, with the error of the radio. |
| `messages` | whole number | always | 1.0 | The messages received. |
| `messages_missed` | whole number | always | 1.0 | The messages missed. |

### Table `sbas_satellite_types`

Struct `SbasSatelliteTypeRow`. A message type that an SBAS satellite sent. The row key is the PRN and the type, as `123/9`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `prn` | whole number | always | 1.0 | The SBAS satellite. |
| `message_type` | whole number | always | 1.0 | The message type. |

### Table `types`

Struct `SbasTypeRow`. The count of one message type. The row key is the type.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `message_type` | whole number | always | 1.0 | The message type. |
| `name` | text | optional | 1.0 | The name of the type. |
| `count` | whole number | always | 1.0 | The messages of the type. |

### Table `messages`

Struct `SbasMessageRow`. A row of the message log. The row key is the number of the message since the start.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of the message. |
| `prn` | whole number | always | 1.0 | The SBAS satellite. |
| `message_type` | whole number | always | 1.0 | The message type. |
| `text` | text | always | 1.0 | The content in words. |

### Table `fast_corrections`

Struct `SbasFastCorrectionRow`. A fast correction of the selected PRN. The row key is the slot.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `slot` | whole number | always | 1.0 | The slot of the PRN mask. |
| `system` | choice | optional | 1.0 | The system of the corrected satellite. |
| `prn` | whole number | optional | 1.0 | The PRN of the corrected satellite. |
| `correction_m` | number (m) | optional | 1.0 | The fast correction. |
| `udrei` | whole number | always | 1.0 | The UDRE indicator. |
| `udre_sigma_m` | number (m) | optional | 1.0 | The UDRE as one sigma. No value for "not monitored" and "do not use". |
| `iodf` | whole number | always | 1.0 | The issue of data of the fast corrections. |
| `iodp` | whole number | always | 1.0 | The issue of data of the PRN mask. |
| `iodp_current` | yes or no | always | 1.0 | The correction belongs to the PRN mask held. |
| `degradation` | number (m/s²) | optional | 1.0 | The degradation factor. |

### Table `ionosphere`

Struct `SbasIonosphereRow`. An ionosphere band of the selected PRN. The row key is the band.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | whole number | always | 1.0 | The band. |
| `iodi` | whole number | always | 1.0 | The issue of data of the ionosphere. |
| `points` | whole number | always | 1.0 | The grid points. |
| `blocks_seen` | whole number | always | 1.0 | The blocks seen. |
| `blocks_needed` | whole number | always | 1.0 | The blocks needed. |
| `values` | whole number | always | 1.0 | The delays held. |
| `not_monitored` | whole number | always | 1.0 | The points not monitored. |
| `min_m` | number (m) | always | 1.0 | The smallest vertical delay. |
| `mean_m` | number (m) | always | 1.0 | The mean vertical delay. |
| `max_m` | number (m) | always | 1.0 | The largest vertical delay. |
| `worst_givei` | whole number | always | 1.0 | The worst GIVE indicator. |

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

### Table `rx.systems`

Struct `GnssSystemRow`. A satellite system. The row key is the key of the system.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system` | choice | always | 1.0 | A satellite system. |
| `wanted` | yes or no | always | 1.0 | This system is selected for reception. |
| `active` | yes or no | always | 1.0 | The receiver has a band for this system at the present tune. |
| `searched` | yes or no | always | 1.0 | The receiver has searched or tracks this system. |

### Table `rx.fix_systems`

Struct `GnssFixSystemRow`. A satellite system in the fix. The row key is the key of the system.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system` | choice | always | 1.0 | A satellite system. |
| `satellites` | whole number | always | 1.0 | Satellites of a system in the fix. |
| `clock_m` | number (m) | always | 1.0 | The receiver clock of a system in the fix. |

### Table `rx.satellites`

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

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `reset` | `sdrkit_sbas_reset` | none | none |
| `select` | `sdrkit_sbas_select` | `sdrkit_sbas_select_args` | none |
| `clear` | `sdrkit_sbas_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `search_half_width` | `sdrkit_sbas_get_search_half_width` | `sdrkit_sbas_set_search_half_width` |
| `frequency_hint` | `sdrkit_sbas_get_frequency_hint` | `sdrkit_sbas_set_frequency_hint` |
| `frequency_hint_valid` | `sdrkit_sbas_get_frequency_hint_valid` | `sdrkit_sbas_set_frequency_hint_valid` |
| `acquisition_effort` | `sdrkit_sbas_get_acquisition_effort` | `sdrkit_sbas_set_acquisition_effort` |

## Try it

```sh
sdrkit run sbas --source synth:sbas
sdrkit run sbas --source iiod:192.168.2.1 --freq 1575.42
```
<!-- generated: end -->
