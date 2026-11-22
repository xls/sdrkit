<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Carrier frequency accuracy meter

The offset of a known carrier in Hz and ppm, its C/N0, drift, Allan deviation, steps, loss and return.

Stack id: `freq_accuracy`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 16 kHz |
| Signal bandwidth | 6 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:freq_accuracy` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `search_hz` | Search half width | number (Hz) | 2800 | 100 to 2800 |  |
| `present_db` | Carrier over the noise per bin | number (dB) | 16 | 6 to 40 |  |
| `step_sigmas` | Step threshold in scatter widths | number | 8 | 3 to 20 |  |
| `step_min_hz` | Smallest step | number (Hz) | 0.5 | 0.01 to 100 |  |
| `row_every_s` | Frequency row every | number (s) | 10 | 1 to 600 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_steps` | Steps | text | empty |  | Steps separated by ';', each 'time s, frequency Hz': from that time the carrier is that far from its start. |
| `gen_gaps` | Gaps | text | empty |  | Stretches separated by ';', each 'from s, to s': the carrier is off. |
| `gen_fm_noise_hz` | Frequency noise | number (Hz) | 0 | 0 to 1000 | Standard deviation of a frequency held constant over each hold; 0 for none. |
| `gen_fm_hold_s` | Frequency noise hold | number (s) | 1.024 | 0.001 to 100 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `offset_hz` | Carrier offset (mean of the last 10) | frequency |
| `scatter_hz` | Scatter of the last 10 | frequency |
| `error_ppm` | Error | number (ppm) |
| `cn0_dbhz` | Carrier to noise density | number (dB-Hz) |
| `drift_hz_s` | Drift | number (Hz/s) |
| `drift_ppm_h` | Drift | number (ppm/h) |
| `readings` | Readings | whole number |
| `window_s` | Reading window | duration (s) |
| `carrier` | Carrier present | yes or no |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `allan` | Allan deviation | `tau_s` (duration (s)), `adev` (number), `adev_hz` (frequency), `groups` (whole number) |
| `events` | Events | `time_s` (duration (s)), `kind` (choice), `offset_hz` (frequency), `step_hz` (frequency), `cn0_dbhz` (number (dB-Hz)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `freq_accuracy.event` | Event | `time_s` (duration (s)), `kind` (choice), `offset_hz` (frequency), `step_hz` (frequency), `cn0_dbhz` (number (dB-Hz)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

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

Struct `FreqAccuracyFields`. The result fields of `freq_accuracy`. The readings of the carrier have no value while the carrier is absent.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | optional | 1.0 | The mean offset of the last 10 readings from the tuned frequency. |
| `scatter_hz` | frequency | optional | 1.0 | The sample standard deviation of the last 10 readings. |
| `error_ppm` | number (ppm) | optional | 1.0 | The offset over the tuned frequency; no value without the tuned frequency. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | The carrier to noise density of the last window. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The least-squares slope of the last 120 readings; from 8 readings. |
| `drift_ppm_h` | number (ppm/h) | optional | 1.0 | The drift over the tuned frequency. |
| `readings` | whole number | always | 1.0 | The readings kept. |
| `window_s` | duration (s) | optional | 1.0 | The length of one reading window. |
| `carrier` | yes or no | optional | 1.0 | The carrier was present in the last window. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `allan`

Struct `FreqAccuracyAllanRow`. A row of the Allan deviation table: one averaging time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tau_s` | duration (s) | always | 1.0 | The averaging time. |
| `adev` | number | optional | 1.0 | The fractional Allan deviation; no value without the tuned frequency. |
| `adev_hz` | frequency | always | 1.0 | The Allan deviation of the offset. |
| `groups` | whole number | always | 1.0 | The groups it is from. |

### Table `events`

Struct `FreqAccuracyEventRow`. A row of the `freq_accuracy` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | The kind. |
| `offset_hz` | frequency | always | 1.0 | The offset of the reading (for a loss, the last reading). |
| `step_hz` | frequency | optional | 1.0 | The size of a step. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | The carrier to noise density of the reading. |

### Event `freq_accuracy.event`

Struct `FreqAccuracyEvent`. A row of the `freq_accuracy` log was written.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | The kind. |
| `offset_hz` | frequency | always | 1.0 | The offset of the reading (for a loss, the last reading). |
| `step_hz` | frequency | optional | 1.0 | The size of a step. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | The carrier to noise density of the reading. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_freq_accuracy_clear` | none | none |
| `reset` | `sdrkit_freq_accuracy_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `search_hz` | `sdrkit_freq_accuracy_get_search_hz` | `sdrkit_freq_accuracy_set_search_hz` |
| `present_db` | `sdrkit_freq_accuracy_get_present_db` | `sdrkit_freq_accuracy_set_present_db` |
| `step_sigmas` | `sdrkit_freq_accuracy_get_step_sigmas` | `sdrkit_freq_accuracy_set_step_sigmas` |
| `step_min_hz` | `sdrkit_freq_accuracy_get_step_min_hz` | `sdrkit_freq_accuracy_set_step_min_hz` |
| `row_every_s` | `sdrkit_freq_accuracy_get_row_every_s` | `sdrkit_freq_accuracy_set_row_every_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_steps` | `sdrkit_freq_accuracy_gen_get_steps` | `sdrkit_freq_accuracy_gen_set_steps` |
| `gen_gaps` | `sdrkit_freq_accuracy_gen_get_gaps` | `sdrkit_freq_accuracy_gen_set_gaps` |
| `gen_fm_noise_hz` | `sdrkit_freq_accuracy_gen_get_fm_noise_hz` | `sdrkit_freq_accuracy_gen_set_fm_noise_hz` |
| `gen_fm_hold_s` | `sdrkit_freq_accuracy_gen_get_fm_hold_s` | `sdrkit_freq_accuracy_gen_set_fm_hold_s` |

## Try it

```sh
sdrkit run freq_accuracy --source synth:freq_accuracy
sdrkit run freq_accuracy --source <driver>:<address> --freq 0
```
<!-- generated: end -->
