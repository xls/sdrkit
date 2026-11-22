<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Spectral kurtosis RFI finder

The bins of a wide capture that hold non-Gaussian power, steady or pulsed, by spectral kurtosis; the clean share of the band.

Stack id: `kurtosis`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:kurtosis` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `fft` | Frame length | whole number (points) | 1024 | 256 to 8192 |  |
| `frames` | Frames per estimate | whole number | 128 | 16 to 1024 |  |
| `blocks` | Estimates averaged | whole number | 8 | 1 to 64 |  |
| `sigmas` | Flag threshold | number (sigma) | 4.5 | 3 to 10 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tones` | Tones | text | `300000, 8, 0, 1; -400000, 40, 0.004, 0.1` |  | Entries separated by ';', each 'offset Hz, amplitude, period s, duty'; a period of 0 is always on. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `clean_pct` | Clean bins | number (%) |
| `sigma` | Expected spread of SK | number |
| `regions` | Flagged regions | whole number |
| `block_ms` | Evaluation time | number (ms) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `regions` | Regions | `low` (frequency), `high` (frequency), `kind` (choice), `sk` (number), `full_power_pct` (number (%)), `over_noise_db` (number (dB)) |
| `events` | Changes | `time_s` (duration (s)), `kind` (choice), `centre` (frequency), `low` (frequency), `high` (frequency), `sk` (number), `over_noise_db` (number (dB)) |

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

Struct `KurtosisFields`. The result fields of `kurtosis`, from the last evaluation.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `clean_pct` | number (%) | optional | 1.0 | The share of the used bins whose SK is within the limits. |
| `sigma` | number | always | 1.0 | The expected standard deviation of SK for Gaussian noise. |
| `regions` | whole number | optional | 1.0 | The flagged regions. |
| `block_ms` | number (ms) | always | 1.0 | The time of one evaluation. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `regions`

Struct `KurtosisRegionRow`. A flagged region of `kurtosis`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `low` | frequency | always | 1.0 | The low edge. |
| `high` | frequency | always | 1.0 | The high edge. |
| `kind` | choice | always | 1.0 | The kind. |
| `sk` | number | always | 1.0 | The mean SK of its bins. |
| `full_power_pct` | number (%) | optional | 1.0 | The share of frames at full power, `1 / (SK + 1)`, for a pulsed region with SK over 1.5. |
| `over_noise_db` | number (dB) | always | 1.0 | The mean power over the noise per bin. |

### Table `events`

Struct `KurtosisEventRow`. A row of the `kurtosis` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | What happened. |
| `centre` | frequency | always | 1.0 | The centre of the region. |
| `low` | frequency | always | 1.0 | The low edge. |
| `high` | frequency | always | 1.0 | The high edge. |
| `sk` | number | always | 1.0 | The mean SK. |
| `over_noise_db` | number (dB) | always | 1.0 | The mean power over the noise. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_kurtosis_clear` | none | none |
| `reset` | `sdrkit_kurtosis_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `fft` | `sdrkit_kurtosis_get_fft` | `sdrkit_kurtosis_set_fft` |
| `frames` | `sdrkit_kurtosis_get_frames` | `sdrkit_kurtosis_set_frames` |
| `blocks` | `sdrkit_kurtosis_get_blocks` | `sdrkit_kurtosis_set_blocks` |
| `sigmas` | `sdrkit_kurtosis_get_sigmas` | `sdrkit_kurtosis_set_sigmas` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tones` | `sdrkit_kurtosis_gen_get_tones` | `sdrkit_kurtosis_gen_set_tones` |

## Try it

```sh
sdrkit run kurtosis --source synth:kurtosis
sdrkit run kurtosis --source <driver>:<address> --freq 0
```
<!-- generated: end -->
