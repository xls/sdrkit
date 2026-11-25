<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Crest factor meter

Mean power, peak, crest factor and the power CCDF at 10, 1, 0.1 and 0.01 % of the span, read against Gaussian noise.

Stack id: `papr`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 500 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:papr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `keep_samples` | Samples in the statistics | whole number | 1048576 | 65536 to 16777216 |  |
| `every_blocks` | Evaluate every (blocks of 16384) | whole number | 16 | 1 to 256 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_kind` | Signal | choice | `noise` | `noise` (Noise), `fm` (FM carrier), `qpsk` (QPSK), `clipped_noise` (Clipped noise), `noise_with_impulses` (Noise with impulses) |  |
| `gen_power` | Power | number | 0.01 | 0 to 4 |  |
| `gen_clip` | Magnitude limit | number | 1 | 0.0001 to 2 |  |
| `gen_impulse_every` | Impulse every | whole number (samples) | 500 | 2 to 10000000 |  |
| `gen_impulse_amp` | Impulse size | number | 2 | 0 to 10 |  |
| `gen_symbol_rate` | Symbol rate | number (Bd) | 25000 | 100 to 1000000 |  |
| `gen_rolloff` | Roll-off | number | 0.35 | 0.05 to 1 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `mean_dbfs` | Mean power | number (dBFS) |
| `peak_dbfs` | Peak | number (dBFS) |
| `crest_db` | Crest factor | number (dB) |
| `ccdf_10_db` | Level at 10 % | number (dB) |
| `ccdf_1_db` | Level at 1 % | number (dB) |
| `ccdf_01_db` | Level at 0.1 % | number (dB) |
| `ccdf_001_db` | Level at 0.01 % | number (dB) |
| `samples` | Samples | whole number |
| `reading` | Shape | choice |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Readings | `time_s` (duration (s)), `ccdf_10_db` (number (dB)), `ccdf_1_db` (number (dB)), `ccdf_01_db` (number (dB)), `ccdf_001_db` (number (dB)), `crest_db` (number (dB)), `mean_dbfs` (number (dBFS)), `samples` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `crest_db`, `ccdf_01_db`, `mean_dbfs`, `peak_dbfs`.

### Result fields

Struct `WatchFields`. The status of a spectrum watcher: the state, the counts of readings and the input level. The headline is the summary of the status.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The state. |
| `good` | whole number | always | 1.0 | The good readings: what the stack counts as a measurement. |
| `bad` | whole number | always | 1.0 | The bad readings: what the stack counts as a fault of the watched band. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level of the last report, relative to full scale. |
| `note` | text | always | 1.0 | What the stack measures. |

### Result fields

Struct `PaprFields`. The result fields of `papr`, from the last evaluation over the kept samples.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mean_dbfs` | number (dBFS) | optional | 1.0 | The mean power, relative to full scale. |
| `peak_dbfs` | number (dBFS) | optional | 1.0 | The largest instantaneous power. |
| `crest_db` | number (dB) | optional | 1.0 | The peak over the mean power. |
| `ccdf_10_db` | number (dB) | optional | 1.0 | The level over the mean exceeded by 10 % of the samples. |
| `ccdf_1_db` | number (dB) | optional | 1.0 | The level exceeded by 1 %. |
| `ccdf_01_db` | number (dB) | optional | 1.0 | The level exceeded by 0.1 %. |
| `ccdf_001_db` | number (dB) | optional | 1.0 | The level exceeded by 0.01 %; only with a million samples or more. |
| `samples` | whole number | optional | 1.0 | The samples in the statistics. |
| `reading` | choice | optional | 1.0 | The shape against Gaussian noise. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `PaprEventRow`. A row of the `papr` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `ccdf_10_db` | number (dB) | always | 1.0 | The level at 10 %. |
| `ccdf_1_db` | number (dB) | always | 1.0 | The level at 1 %. |
| `ccdf_01_db` | number (dB) | always | 1.0 | The level at 0.1 %. |
| `ccdf_001_db` | number (dB) | optional | 1.0 | The level at 0.01 %. |
| `crest_db` | number (dB) | always | 1.0 | The crest factor. |
| `mean_dbfs` | number (dBFS) | always | 1.0 | The mean power. |
| `samples` | whole number | always | 1.0 | The samples in the statistics. |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_papr_clear` | none | none |
| `reset` | `sdrkit_papr_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `keep_samples` | `sdrkit_papr_get_keep_samples` | `sdrkit_papr_set_keep_samples` |
| `every_blocks` | `sdrkit_papr_get_every_blocks` | `sdrkit_papr_set_every_blocks` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_kind` | `sdrkit_papr_gen_get_kind` | `sdrkit_papr_gen_set_kind` |
| `gen_power` | `sdrkit_papr_gen_get_power` | `sdrkit_papr_gen_set_power` |
| `gen_clip` | `sdrkit_papr_gen_get_clip` | `sdrkit_papr_gen_set_clip` |
| `gen_impulse_every` | `sdrkit_papr_gen_get_impulse_every` | `sdrkit_papr_gen_set_impulse_every` |
| `gen_impulse_amp` | `sdrkit_papr_gen_get_impulse_amp` | `sdrkit_papr_gen_set_impulse_amp` |
| `gen_symbol_rate` | `sdrkit_papr_gen_get_symbol_rate` | `sdrkit_papr_gen_set_symbol_rate` |
| `gen_rolloff` | `sdrkit_papr_gen_get_rolloff` | `sdrkit_papr_gen_set_rolloff` |

## Try it

```sh
sdrkit run papr --source synth:papr
sdrkit run papr --source <driver>:<address> --freq 0
```
<!-- generated: end -->
