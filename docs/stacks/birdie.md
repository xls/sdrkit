<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Persistent carrier finder

Narrow carriers that stay in the capture: frequency, level, drift, scatter, and whether each is bare or modulated.

Stack id: `birdie`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:birdie` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Peak over the noise | number (dB) | 10 | 6 to 30 |  |
| `presence_pct` | Present in at least | number (%) | 90 | 50 to 100 |  |
| `window_s` | Presence window | number (s) | 60 | 10 to 600 |  |
| `bare_share` | Bare carrier when the narrowness is at least | number | 0.75 | 0.5 to 0.95 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tones` | Carriers | text | `123456.7, 0.05, 0, 1, 0, 0.5; -75000.3, 0.02; 300000, 0.0015; -240000, 0.05, 4, 0.5` |  | Carriers separated by ';', each 'offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s, from s, to s'; the values after the amplitude can be left out. |
| `gen_fm_hz` | Modulated carrier offset | number (Hz) | 450000 | -100000000 to 100000000 |  |
| `gen_fm_amp` | Modulated carrier amplitude | number | 0.03 | 0 to 10 |  |
| `gen_fm_dev_hz` | Modulated carrier deviation | number (Hz) | 3000 | 0 to 1000000 | The RMS deviation; the modulation is white noise through a one-pole low-pass of 300 Hz. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `carriers` | Persistent carriers | whole number |
| `snapshots` | Snapshots | whole number |
| `snapshot_s` | Snapshot length | duration (s) |
| `bin_hz` | Bin width | number (Hz) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Persistent carriers | `frequency` (frequency), `level_db` (number (dB)), `level_sd_db` (number (dB)), `class` (choice), `drift_hz_min` (number (Hz/min)), `scatter_hz` (number (Hz)), `present` (whole number), `of` (whole number) |
| `carrier_events` | Carrier events | `time_s` (duration (s)), `kind` (choice), `frequency` (frequency), `level_db` (number (dB)), `class` (choice), `drift_hz_min` (number (Hz/min)), `scatter_hz` (number (Hz)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `birdie.event` | Carrier event | `time_s` (duration (s)), `kind` (choice), `frequency` (frequency), `level_db` (number (dB)), `class` (choice), `drift_hz_min` (number (Hz/min)), `scatter_hz` (number (Hz)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the tracks and the list; the log stays. |

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

Struct `BirdieFields`. The result fields of `birdie`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carriers` | whole number | always | 1.0 | The persistent carriers listed now. |
| `snapshots` | whole number | always | 1.0 | The snapshots taken. |
| `snapshot_s` | duration (s) | always | 1.0 | The time of a snapshot. |
| `bin_hz` | number (Hz) | always | 1.0 | The width of a bin. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `carriers`

Struct `BirdieCarrierRow`. A row of the list of persistent carriers, the strongest first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The smoothed frequency. |
| `level_db` | number (dB) | always | 1.0 | The mean level over the noise per bin. |
| `level_sd_db` | number (dB) | always | 1.0 | The standard deviation of the level. |
| `class` | choice | always | 1.0 | The class. |
| `drift_hz_min` | number (Hz/min) | always | 1.0 | The least-squares drift of the frequency. |
| `scatter_hz` | number (Hz) | always | 1.0 | The RMS of the frequency about the drift line. |
| `present` | whole number | always | 1.0 | The snapshots of the presence window in which it was seen. |
| `of` | whole number | always | 1.0 | The snapshots of the presence window. |

### Table `carrier_events`

Struct `BirdieEventRow`. A row of the carrier log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the row. |
| `kind` | choice | always | 1.0 | The kind. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `level_db` | number (dB) | always | 1.0 | The mean level over the noise. |
| `class` | choice | always | 1.0 | The class. |
| `drift_hz_min` | number (Hz/min) | always | 1.0 | The drift. |
| `scatter_hz` | number (Hz) | always | 1.0 | The scatter. |

### Event `birdie.event`

Struct `BirdieEvent`. A persistent carrier was listed, came back or went.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the row. |
| `kind` | choice | always | 1.0 | The kind. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `level_db` | number (dB) | always | 1.0 | The mean level over the noise. |
| `class` | choice | always | 1.0 | The class. |
| `drift_hz_min` | number (Hz/min) | always | 1.0 | The drift. |
| `scatter_hz` | number (Hz) | always | 1.0 | The scatter. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_birdie_clear` | none | none |
| `reset` | `sdrkit_birdie_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_birdie_get_threshold_db` | `sdrkit_birdie_set_threshold_db` |
| `presence_pct` | `sdrkit_birdie_get_presence_pct` | `sdrkit_birdie_set_presence_pct` |
| `window_s` | `sdrkit_birdie_get_window_s` | `sdrkit_birdie_set_window_s` |
| `bare_share` | `sdrkit_birdie_get_bare_share` | `sdrkit_birdie_set_bare_share` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tones` | `sdrkit_birdie_gen_get_tones` | `sdrkit_birdie_gen_set_tones` |
| `gen_fm_hz` | `sdrkit_birdie_gen_get_fm_hz` | `sdrkit_birdie_gen_set_fm_hz` |
| `gen_fm_amp` | `sdrkit_birdie_gen_get_fm_amp` | `sdrkit_birdie_gen_set_fm_amp` |
| `gen_fm_dev_hz` | `sdrkit_birdie_gen_get_fm_dev_hz` | `sdrkit_birdie_gen_set_fm_dev_hz` |

## Try it

```sh
sdrkit run birdie --source synth:birdie
sdrkit run birdie --source <driver>:<address> --freq 0
```
<!-- generated: end -->
