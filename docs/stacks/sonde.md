<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Radiosonde

Weather balloons: RS41, RS92, DFM, M10, M20 and LMS6 positions, heights, sensors and landing predictions.

Stack id: `sonde`. Group: Science, time and utility.

Signal types: Radiosonde.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 403 MHz |
| Tuning range | 380 MHz to 1.7 GHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 6 MHz |
| Radio filter | 7 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sonde` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Sondes followed at once | whole number | 8 | 1 to 8 | Each channel costs processing time. |
| `landing_window_s` | Landing smoothing | duration (s) | 10 s | 0 to 60 |  |
| `landing_layers` | Use the ascent winds | yes or no | true |  |  |
| `landing_layer_m` | Layer | number (m) | 500 | 100 to 2000 |  |
| `landing_ascent_weight` | Ascent wind weight | number | 0.6 | 0 to 1 |  |
| `ground_m` | Ground height | number (m) | 0 | -500 to 5000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_count` | Test signal: sondes | whole number | 3 | 1 to 7 |  |
| `gen_types` | Test signal: types | text | `rs41,dfm,m10` |  | The keys of the sonde families, separated by commas (rs41, rs92, dfm, m10, m20, lms6). |
| `gen_drift_ms` | Test signal: east drift | number (m/s) | 10 | -50 to 50 |  |
| `gen_rs41b_khz` | Test signal: second RS41 offset | number (kHz) | 450 | -3000 to 3000 |  |
| `gen_lband` | Test signal: 1680 MHz (LMS6) | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `sondes` | Sondes | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `searches` | Searches | whole number |
| `channels_used` | Channels used | whole number |
| `channels_allowed` | Channels allowed | whole number |
| `band` | Band searched | frequency |
| `narrow` | Narrow capture | yes or no |
| `selected` | Selected sonde | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sondes` | Radiosondes | `type` (choice), `subtype` (text), `serial` (text), `channel` (whole number), `frequency` (frequency), `offset` (frequency), `snr_db` (number (dB)), `frame` (whole number), `time` (time), `position` (position), `height_m` (number (m)), `max_alt_m` (number (m)), `climb_m_s` (number (m/s)), `speed_kt` (number (kt)), `course_deg` (number (°)), `sats` (whole number), `temp_c` (number (°C)), `humidity_pct` (number (%)), `pressure_hpa` (number (hPa)), `battery_v` (number (V)), `kill` (duration (s)), `cal_held` (whole number), `cal_needed` (whole number), `frames_ok` (whole number), `frames_bad` (whole number), `ok_pct` (number (%)), `landing` (position), `landing_in` (duration (s)), `landing_distance_m` (number (m)), `landing_bearing_deg` (number (°)), `heard` (whole number), `last_heard` (time) |
| `carriers` | Carriers | `offset` (frequency), `frequency` (frequency), `level_db` (number (dB)), `channel` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `alt_time` | Altitude | s | m |
| `temp_alt` | Temperature by altitude | °C | m |
| `track` | Track | ° | ° |

## Map points

The stack puts points on a map: Radiosondes.

## Events

| Kind | Label | Fields |
|---|---|---|
| `sonde.frame` | Radiosonde frame | `source` (text), `text` (text), `family` (choice), `position` (position), `height_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `type` (choice), `serial` (text), `temp_c` (number (°C)), `humidity_pct` (number (%)), `climb_m_s` (number (m/s)), `burst_killer` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `select` | Select | `row` (text, required) |  | Shows the plots of one sonde. |

## Typed interface

The stack is in the family `science`, version 1.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SondeFields`. The result fields of `sonde`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sondes` | whole number | always | 1.0 | The number of sondes in the table. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `searches` | whole number | always | 1.0 | Carrier searches so far. |
| `channels_used` | whole number | always | 1.0 | Channels in use. |
| `channels_allowed` | whole number | always | 1.0 | Channels allowed. |
| `band` | frequency | always | 1.0 | The band searched (the input rate). |
| `narrow` | yes or no | always | 1.0 | The capture is narrower than 2 MHz: only ±0.45 of the rate is searched. |
| `selected` | text | optional | 1.0 | The row key of the sonde the plots show. |

### Table `sondes`

Struct `SondeRow`. A row of the table `sondes`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `type` | choice | always | 1.0 | The family. |
| `subtype` | text | optional | 1.0 | The subtype. |
| `serial` | text | always | 1.0 | The serial. |
| `channel` | whole number | optional | 1.0 | The receiving channel. |
| `frequency` | frequency | optional | 1.0 | The absolute frequency. |
| `offset` | frequency | always | 1.0 | The offset from the centre. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `frame` | whole number | optional | 1.0 | The frame counter. |
| `time` | time | optional | 1.0 | UTC from the sonde. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `height_m` | number (m) | optional | 1.0 | The height of the object that the event is about, metres above mean sea level. |
| `max_alt_m` | number (m) | optional | 1.0 | The highest altitude. |
| `climb_m_s` | number (m/s) | optional | 1.0 | The vertical speed. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `sats` | whole number | optional | 1.0 | The satellites used. |
| `temp_c` | number (°C) | optional | 1.0 | The temperature. |
| `humidity_pct` | number (%) | optional | 1.0 | The relative humidity. |
| `pressure_hpa` | number (hPa) | optional | 1.0 | The pressure. |
| `battery_v` | number (V) | optional | 1.0 | The battery. |
| `kill` | duration (s) | optional | 1.0 | The burst or kill timer. |
| `cal_held` | whole number | optional | 1.0 | The calibration parts held. |
| `cal_needed` | whole number | optional | 1.0 | The calibration parts needed. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `ok_pct` | number (%) | always | 1.0 | Good frames as a share of all frames. |
| `landing` | position | optional | 1.0 | The predicted landing point. |
| `landing_in` | duration (s) | optional | 1.0 | The time to the landing. |
| `landing_distance_m` | number (m) | optional | 1.0 | The distance from the station to the landing point. |
| `landing_bearing_deg` | number (°) | optional | 1.0 | The bearing from the station to the landing point. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

### Table `carriers`

Struct `SondeCarrierRow`. A carrier of the last search. The row key is the place in the list, two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset` | frequency | always | 1.0 | The offset from the centre. |
| `frequency` | frequency | optional | 1.0 | The absolute frequency. |
| `level_db` | number (dB) | always | 1.0 | The carrier level. |
| `channel` | whole number | optional | 1.0 | The receiving channel. |

### Event `sonde.frame`

Struct `SondeFrameEvent`. A sonde frame, every 5 s for each sonde. `source` is the serial, `text` the sonde for people.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `height_m` | number (m) | optional | 1.0 | The height of the object that the event is about, metres above mean sea level. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `type` | choice | always | 1.0 | The family. |
| `serial` | text | always | 1.0 | The serial. |
| `temp_c` | number (°C) | optional | 1.0 | The temperature. |
| `humidity_pct` | number (%) | optional | 1.0 | The relative humidity. |
| `climb_m_s` | number (m/s) | optional | 1.0 | The vertical speed. |
| `burst_killer` | yes or no | optional | 1.0 | The sonde's burst or kill timer runs. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sonde_clear` | none | none |
| `remove` | `sdrkit_sonde_remove` | `sdrkit_sonde_row_args` | none |
| `select` | `sdrkit_sonde_select` | `sdrkit_sonde_row_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_sonde_get_channels` | `sdrkit_sonde_set_channels` |
| `landing_window_s` | `sdrkit_sonde_get_landing_window_s` | `sdrkit_sonde_set_landing_window_s` |
| `landing_layers` | `sdrkit_sonde_get_landing_layers` | `sdrkit_sonde_set_landing_layers` |
| `landing_layer_m` | `sdrkit_sonde_get_landing_layer_m` | `sdrkit_sonde_set_landing_layer_m` |
| `landing_ascent_weight` | `sdrkit_sonde_get_landing_ascent_weight` | `sdrkit_sonde_set_landing_ascent_weight` |
| `ground_m` | `sdrkit_sonde_get_ground_m` | `sdrkit_sonde_set_ground_m` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_count` | `sdrkit_sonde_gen_get_count` | `sdrkit_sonde_gen_set_count` |
| `gen_types` | `sdrkit_sonde_gen_get_types` | `sdrkit_sonde_gen_set_types` |
| `gen_drift_ms` | `sdrkit_sonde_gen_get_drift_ms` | `sdrkit_sonde_gen_set_drift_ms` |
| `gen_rs41b_khz` | `sdrkit_sonde_gen_get_rs41b_khz` | `sdrkit_sonde_gen_set_rs41b_khz` |
| `gen_lband` | `sdrkit_sonde_gen_get_lband` | `sdrkit_sonde_gen_set_lband` |

## Try it

```sh
sdrkit run sonde --source synth:sonde
sdrkit run sonde --source iiod:192.168.2.1 --freq 403
```

## Test recordings

- `tests/vectors/science/sonde_rs41_250k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
