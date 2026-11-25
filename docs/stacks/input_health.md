<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SDR input health meter

Level, peak, crest factor, clipping, DC offset and blind IQ mismatch of the raw samples, with a level verdict.

Stack id: `input_health`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:input_health` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `block_s` | Reading every | number (s) | 1 | 0.1 to 10 |  |
| `clip_level` | Clip at (of full scale) | number | 0.985 | 0.9 to 1 |  |
| `overload_ppm` | Overload above | number (ppm) | 100 | 1 to 10000 |  |
| `high_dbfs` | High above | number (dBFS) | -10 | -80 to 0 |  |
| `low_dbfs` | Low under | number (dBFS) | -50 | -80 to 0 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_power` | Noise power | number | 0.0025 | 0 to 4 |  |
| `gen_tones` | Tones | text | `123456, 0.3; -300000, 0.3` |  | Entries separated by ';', each 'offset Hz, amplitude' with the amplitude in units of the square root of the noise power. |
| `gen_clip` | Clip at | number | 1 | 0.01 to 10 |  |
| `gen_scale` | Gain after the clip | number | 1 | 0 to 100 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `rms_dbfs` | Level | number (dBFS) |
| `peak_dbfs` | Peak | number (dBFS) |
| `crest_db` | Crest factor | number (dB) |
| `clipping_ppm` | Samples near full scale | number (ppm) |
| `dc_i_pct` | DC of I | number (%) |
| `dc_q_pct` | DC of Q | number (%) |
| `dc_pct` | DC offset | number (%) |
| `dc_under_rms_db` | DC under the level | number (dB) |
| `iq_gain_db` | IQ gain mismatch, Q against I | number (dB) |
| `iq_phase_deg` | IQ phase error | number (°) |
| `image_rejection_db` | Image rejection from the mismatch | number (dB) |
| `verdict` | Level verdict | choice |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Changes | `time_s` (duration (s)), `kind` (choice), `verdict` (choice), `rms_dbfs` (number (dBFS)), `peak_dbfs` (number (dBFS)), `clipping_ppm` (number (ppm)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `verdict`, `rms_dbfs`, `peak_dbfs`, `clipping_ppm`, `dc_pct`, `image_rejection_db`.

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

Struct `InputHealthFields`. The result fields of `input_health`, from the last block of raw samples. Levels are against a full scale of 1.0 per component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rms_dbfs` | number (dBFS) | optional | 1.0 | The RMS level, DC included. |
| `peak_dbfs` | number (dBFS) | optional | 1.0 | The largest of `\|I\|` and `\|Q\|`. |
| `crest_db` | number (dB) | optional | 1.0 | The peak over the RMS level. |
| `clipping_ppm` | number (ppm) | optional | 1.0 | The samples with `\|I\|` or `\|Q\|` at the clip level or over it. |
| `dc_i_pct` | number (%) | optional | 1.0 | The mean of I. |
| `dc_q_pct` | number (%) | optional | 1.0 | The mean of Q. |
| `dc_pct` | number (%) | optional | 1.0 | The length of the mean vector. |
| `dc_under_rms_db` | number (dB) | optional | 1.0 | The power of the mean vector under the RMS level. |
| `iq_gain_db` | number (dB) | optional | 1.0 | `10 log10(V_Q / V_I)`. |
| `iq_phase_deg` | number (°) | optional | 1.0 | `asin` of the correlation of I and Q. |
| `image_rejection_db` | number (dB) | optional | 1.0 | The image rejection that the mismatch implies, at most 120 dB. |
| `verdict` | choice | optional | 1.0 | The level verdict. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `InputHealthEventRow`. A row of the `input_health` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | What the row reports. |
| `verdict` | choice | always | 1.0 | The verdict. |
| `rms_dbfs` | number (dBFS) | always | 1.0 | The RMS level. |
| `peak_dbfs` | number (dBFS) | always | 1.0 | The peak. |
| `clipping_ppm` | number (ppm) | always | 1.0 | The samples near full scale. |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_input_health_clear` | none | none |
| `reset` | `sdrkit_input_health_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `block_s` | `sdrkit_input_health_get_block_s` | `sdrkit_input_health_set_block_s` |
| `clip_level` | `sdrkit_input_health_get_clip_level` | `sdrkit_input_health_set_clip_level` |
| `overload_ppm` | `sdrkit_input_health_get_overload_ppm` | `sdrkit_input_health_set_overload_ppm` |
| `high_dbfs` | `sdrkit_input_health_get_high_dbfs` | `sdrkit_input_health_set_high_dbfs` |
| `low_dbfs` | `sdrkit_input_health_get_low_dbfs` | `sdrkit_input_health_set_low_dbfs` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_power` | `sdrkit_input_health_gen_get_power` | `sdrkit_input_health_gen_set_power` |
| `gen_tones` | `sdrkit_input_health_gen_get_tones` | `sdrkit_input_health_gen_set_tones` |
| `gen_clip` | `sdrkit_input_health_gen_get_clip` | `sdrkit_input_health_gen_set_clip` |
| `gen_scale` | `sdrkit_input_health_gen_get_scale` | `sdrkit_input_health_gen_set_scale` |

## Try it

```sh
sdrkit run input_health --source synth:input_health
sdrkit run input_health --source <driver>:<address> --freq 0
```
<!-- generated: end -->
