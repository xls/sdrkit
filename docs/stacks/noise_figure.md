<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Noise figure (Y-factor) meter

The hot and cold plateaus of the noise power as a source is switched, the Y-factor and the noise figure for given ENR values.

Stack id: `noise_figure`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 200 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:noise_figure` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `enr_db` | Source ENR values | text (dB) | `5, 6.5, 15, 25` |  | Values from 0 to 40 dB separated by ','. |
| `agree_db` | A window joins a plateau within | number (dB) | 0.15 | 0.02 to 1 |  |
| `step_db` | Smallest step | number (dB) | 0.3 | 0.1 to 3 |  |
| `steps_kept` | Steps averaged | whole number | 4 | 1 to 32 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_y_db` | Y-factor | number (dB) | 10 | 0 to 40 |  |
| `gen_period_s` | Source toggles every | number (s) | 10 | 0.5 to 1000 |  |
| `gen_drift_db_s` | Gain drift | number (dB/s) | 0.01 | -1 to 1 |  |
| `gen_p0` | Receiver noise | number | 1 | 0 to 4 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `power_db` | Power now | number (dB) |
| `y_db` | Y-factor | number (dB) |
| `y_lin` | Y-factor ratio | number |
| `y_spread_db` | Spread of the steps | number (dB) |
| `steps` | Steps seen | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `noise_figure` | Noise figure | `enr_db` (number (dB)), `nf_db` (number (dB)), `negative` (yes or no) |
| `events` | Steps | `time_s` (duration (s)), `state` (choice), `from_db` (number (dB)), `to_db` (number (dB)), `y_db` (number (dB)), `y_lin` (number) |

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

Struct `NoiseFigureFields`. The result fields of `noise_figure`. Levels are relative and uncalibrated.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `power_db` | number (dB) | optional | 1.0 | The level of the present plateau of the power. |
| `y_db` | number (dB) | optional | 1.0 | The Y-factor: the mean of the kept steps. |
| `y_lin` | number | optional | 1.0 | The Y-factor as a ratio. |
| `y_spread_db` | number (dB) | optional | 1.0 | The sample standard deviation of the kept steps. |
| `steps` | whole number | always | 1.0 | The steps seen. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `noise_figure`

Struct `NoiseFigureRow`. A row of the noise figure table: one source ENR of the setting.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `enr_db` | number (dB) | always | 1.0 | The excess noise ratio of the source. |
| `nf_db` | number (dB) | optional | 1.0 | `ENR - 10 log10(Y - 1)`; no value when Y is 1 or less. |
| `negative` | yes or no | always | 1.0 | The noise figure is negative: the ENR is lower than the step implies. |

### Table `events`

Struct `NoiseFigureEventRow`. A row of the `noise_figure` log: one step.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `state` | choice | always | 1.0 | The state of the source after the step. |
| `from_db` | number (dB) | always | 1.0 | The plateau before. |
| `to_db` | number (dB) | always | 1.0 | The plateau after. |
| `y_db` | number (dB) | always | 1.0 | The size of the step. |
| `y_lin` | number | always | 1.0 | The size as a ratio. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_noise_figure_clear` | none | none |
| `reset` | `sdrkit_noise_figure_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `enr_db` | `sdrkit_noise_figure_get_enr_db` | `sdrkit_noise_figure_set_enr_db` |
| `agree_db` | `sdrkit_noise_figure_get_agree_db` | `sdrkit_noise_figure_set_agree_db` |
| `step_db` | `sdrkit_noise_figure_get_step_db` | `sdrkit_noise_figure_set_step_db` |
| `steps_kept` | `sdrkit_noise_figure_get_steps_kept` | `sdrkit_noise_figure_set_steps_kept` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_y_db` | `sdrkit_noise_figure_gen_get_y_db` | `sdrkit_noise_figure_gen_set_y_db` |
| `gen_period_s` | `sdrkit_noise_figure_gen_get_period_s` | `sdrkit_noise_figure_gen_set_period_s` |
| `gen_drift_db_s` | `sdrkit_noise_figure_gen_get_drift_db_s` | `sdrkit_noise_figure_gen_set_drift_db_s` |
| `gen_p0` | `sdrkit_noise_figure_gen_get_p0` | `sdrkit_noise_figure_gen_set_p0` |

## Try it

```sh
sdrkit run noise_figure --source synth:noise_figure
sdrkit run noise_figure --source <driver>:<address> --freq 0
```
<!-- generated: end -->
