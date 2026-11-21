<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Beacon propagation monitor

One beacon: its level over the noise, the Doppler spread of its line and its level swing; the path steady, fading, aurora-like or none.

Stack id: `beacon_monitor`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 28 MHz to 450 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 4 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:beacon_monitor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `present_db` | Line over the noise | number (dB) | 10 | 3 to 30 |  |
| `aurora_hz` | Aurora-like above | number (Hz) | 40 | 10 to 200 |  |
| `fading_db` | Fading swing above | number (dB) | 6 | 2 to 20 |  |
| `spectra` | Spectra per decision | whole number | 5 | 3 to 20 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_fade_hz` | Fading rate | number (Hz) | 0 | 0 to 10 |  |
| `gen_fade_depth_db` | Fading depth | number (dB) | 14 | 0 to 40 |  |
| `gen_spread_hz` | Aurora spread | number (Hz) | 0 | 0 to 500 |  |
| `gen_off` | No beacon | text | empty |  | Spans separated by ';', each 'from s, to s'. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `class` | Path | choice |
| `level_db` | Beacon over the noise in a bin | number (dB) |
| `spread_hz` | Doppler spread of the line | number (Hz) |
| `swing_db` | Level swing over the last 12 s | number (dB) |
| `line_offset_hz` | Line from the tuned frequency | frequency |
| `changes` | Class changes | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Class changes | `time_s` (duration (s)), `from_class` (choice), `to_class` (choice), `level_db` (number (dB)), `spread_hz` (number (Hz)), `swing_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `beacon_monitor.change` | Path change | `time_s` (duration (s)), `from_class` (choice), `to_class` (choice), `level_db` (number (dB)), `spread_hz` (number (Hz)), `swing_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 1.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `BeaconMonitorFields`. The result fields of `beacon_monitor`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `class` | choice | always | 1.0 | The class of the path. |
| `level_db` | number (dB) | optional | 1.0 | The line over the noise in a bin, last spectrum. |
| `spread_hz` | number (Hz) | optional | 1.0 | The Doppler spread of the line (a Gaussian full width at half height), last spectrum. |
| `swing_db` | number (dB) | optional | 1.0 | The largest minus the smallest level of the last 12 s, at the last decision. |
| `line_offset_hz` | frequency | optional | 1.0 | The line from the tuned frequency, last spectrum. |
| `changes` | whole number | always | 1.0 | The class changes. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `BeaconMonitorEventRow`. A change of class of `beacon_monitor`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `from_class` | choice | always | 1.0 | The class before. |
| `to_class` | choice | always | 1.0 | The class after. |
| `level_db` | number (dB) | optional | 1.0 | The last level. |
| `spread_hz` | number (Hz) | optional | 1.0 | The median spread of the decision. |
| `swing_db` | number (dB) | optional | 1.0 | The swing of the decision. |

### Event `beacon_monitor.change`

Struct `BeaconMonitorChangeEvent`. A change of class of the path.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `from_class` | choice | always | 1.0 | The class before. |
| `to_class` | choice | always | 1.0 | The class after. |
| `level_db` | number (dB) | optional | 1.0 | The last level. |
| `spread_hz` | number (Hz) | optional | 1.0 | The median spread of the decision. |
| `swing_db` | number (dB) | optional | 1.0 | The swing of the decision. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_beacon_monitor_clear` | none | none |
| `reset` | `sdrkit_beacon_monitor_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `present_db` | `sdrkit_beacon_monitor_get_present_db` | `sdrkit_beacon_monitor_set_present_db` |
| `aurora_hz` | `sdrkit_beacon_monitor_get_aurora_hz` | `sdrkit_beacon_monitor_set_aurora_hz` |
| `fading_db` | `sdrkit_beacon_monitor_get_fading_db` | `sdrkit_beacon_monitor_set_fading_db` |
| `spectra` | `sdrkit_beacon_monitor_get_spectra` | `sdrkit_beacon_monitor_set_spectra` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_fade_hz` | `sdrkit_beacon_monitor_gen_get_fade_hz` | `sdrkit_beacon_monitor_gen_set_fade_hz` |
| `gen_fade_depth_db` | `sdrkit_beacon_monitor_gen_get_fade_depth_db` | `sdrkit_beacon_monitor_gen_set_fade_depth_db` |
| `gen_spread_hz` | `sdrkit_beacon_monitor_gen_get_spread_hz` | `sdrkit_beacon_monitor_gen_set_spread_hz` |
| `gen_off` | `sdrkit_beacon_monitor_gen_get_off` | `sdrkit_beacon_monitor_gen_set_off` |

## Try it

```sh
sdrkit run beacon_monitor --source synth:beacon_monitor
sdrkit run beacon_monitor --source <driver>:<address> --freq 0
```
<!-- generated: end -->
