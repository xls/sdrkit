<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Spectrum occupancy monitor

Busy and idle time of every channel of a grid over the captured span: duty cycle, transmissions, the longest one, and the band occupancy.

Stack id: `occupancy`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:occupancy` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `grid_khz` | Channel width | number (kHz) | 25 | 1 to 1000 | The width of each channel of the grid, rounded to whole FFT bins (at least 2). |
| `on_db` | Busy above (over the floor) | number (dB) | 6 | 2 to 30 |  |
| `off_db` | Idle below (over the floor) | number (dB) | 4 | 2 to 30 |  |
| `floor_share` | Floor quantile of the channels | number | 0.2 | 0.05 to 0.5 | The floor is the channel power at this quantile of the sorted channels. |
| `min_log_s` | Shortest transmission listed | number (s) | 0.05 | 0.01 to 10 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tones` | Carriers | text | `-289507.8, 1, 2, 0.5, 0.3; -60976.6, 1, 1, 0.2, 0.1; 167539.1, 1, 5, 0.9, 0.7; 396054.7, 1, 1.6, 0.35, 0.45` |  | Carriers separated by ';', each 'offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s, from s, to s'; the values after the amplitude can be left out. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `channels` | Channels in the grid | whole number |
| `channel_khz` | Channel width | number (kHz) |
| `span_mhz` | Span covered | number (MHz) |
| `floor_db` | Noise floor (relative, per bin) | number (dB) |
| `busy_now` | Channels busy now | whole number |
| `band_occupancy_pct` | Busy share of channel-time since the start | number (%) |
| `elapsed_s` | Time measured | duration (s) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Busy channels | `frequency` (frequency), `duty_pct` (number (%)), `transmissions` (whole number), `longest_s` (duration (s)), `busy` (yes or no) |
| `activity` | Activity | `time_s` (duration (s)), `frequency` (frequency), `length_s` (duration (s)), `peak_over_floor_db` (number (dB)), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `occupancy.activity` | Activity | `time_s` (duration (s)), `frequency` (frequency), `length_s` (duration (s)), `peak_over_floor_db` (number (dB)), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Learns the floor again and drops the channel states; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 1.1. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

### Result fields

Struct `WatchFields`. The status of a spectrum watcher: the headline, the state, the counts of readings and the input level.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The headline. |
| `state` | choice | always | 1.0 | The state. |
| `good` | whole number | always | 1.0 | The good readings: what the stack counts as a measurement. |
| `bad` | whole number | always | 1.0 | The bad readings: what the stack counts as a fault of the watched band. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level of the last report, relative to full scale. |
| `note` | text | always | 1.0 | What the stack measures. |

### Result fields

Struct `OccupancyFields`. The result fields of `occupancy`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channels` | whole number | always | 1.0 | The channels of the grid. |
| `channel_khz` | number (kHz) | always | 1.0 | The width of a channel: whole bins. |
| `span_mhz` | number (MHz) | always | 1.0 | The span that the channels cover. |
| `floor_db` | number (dB) | optional | 1.0 | The noise floor of a channel, relative, per bin; no value before the first decision. |
| `busy_now` | whole number | always | 1.0 | The channels busy now. |
| `band_occupancy_pct` | number (%) | optional | 1.0 | The busy share of the channel-time since the end of the warm-up; no value before it. |
| `elapsed_s` | duration (s) | always | 1.0 | The time measured after the warm-up. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `channels`

Struct `OccupancyChannelRow`. A row of the list of busy channels: a channel that has been busy, the busiest first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The centre of the channel (an offset from the centre when the tuned frequency is not known). |
| `duty_pct` | number (%) | always | 1.0 | The busy time over the time measured. |
| `transmissions` | whole number | always | 1.0 | The transmissions that ended. |
| `longest_s` | duration (s) | always | 1.0 | The longest transmission. |
| `busy` | yes or no | always | 1.0 | The channel is busy now. |

### Table `activity`

Struct `OccupancyActivityRow`. A row of the activity log: one transmission of at least the shortest listed length.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The end of the transmission, signal time. |
| `frequency` | frequency | always | 1.0 | The centre of the channel. |
| `length_s` | duration (s) | always | 1.0 | The length of the transmission. |
| `peak_over_floor_db` | number (dB) | always | 1.0 | The highest channel power over the floor. |
| `level_db` | number (dB) | always | 1.0 | The highest channel power, relative, per bin. |

### Event `occupancy.activity`

Struct `OccupancyActivityEvent`. A transmission of a channel ended.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The end of the transmission, signal time. |
| `frequency` | frequency | always | 1.0 | The centre of the channel. |
| `length_s` | duration (s) | always | 1.0 | The length of the transmission. |
| `peak_over_floor_db` | number (dB) | always | 1.0 | The highest channel power over the floor. |
| `level_db` | number (dB) | always | 1.0 | The highest channel power, relative, per bin. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_occupancy_clear` | none | none |
| `reset` | `sdrkit_occupancy_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `grid_khz` | `sdrkit_occupancy_get_grid_khz` | `sdrkit_occupancy_set_grid_khz` |
| `on_db` | `sdrkit_occupancy_get_on_db` | `sdrkit_occupancy_set_on_db` |
| `off_db` | `sdrkit_occupancy_get_off_db` | `sdrkit_occupancy_set_off_db` |
| `floor_share` | `sdrkit_occupancy_get_floor_share` | `sdrkit_occupancy_set_floor_share` |
| `min_log_s` | `sdrkit_occupancy_get_min_log_s` | `sdrkit_occupancy_set_min_log_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tones` | `sdrkit_occupancy_gen_get_tones` | `sdrkit_occupancy_gen_set_tones` |

## Try it

```sh
sdrkit run occupancy --source synth:occupancy
sdrkit run occupancy --source <driver>:<address> --freq 0
```
<!-- generated: end -->
