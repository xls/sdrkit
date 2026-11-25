<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Noise floor logger

The noise floor of each sub-band of the span once a second over hours: rises over a baseline and changes of the whole band.

Stack id: `noise_floor`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:noise_floor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `sub_bands` | Sub-bands | whole number | 16 | 4 to 64 |  |
| `rise_db` | Rise over the baseline | number (dB) | 3 | 1 to 20 |  |
| `hold_s` | Rise must hold for | number (s) | 3 | 1 to 60 |  |
| `baseline_min` | Baseline window | number (min) | 10 | 1 to 60 |  |
| `whole_band_share` | Sub-bands that make a whole-band change | number | 0.75 | 0.5 to 1 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_extra_hz` | Added noise centre | number (Hz) | 300000 | -100000000 to 100000000 |  |
| `gen_extra_bw_hz` | Added noise width | number (Hz) | 250000 | 1 to 100000000 | Twice the cut-off of the one-pole low-pass that shapes the added noise. |
| `gen_extra_power` | Added noise power | number | 0.002 | 0 to 100 |  |
| `gen_extra_from_s` | Added noise from | number (s) | 90 | 0 to 1000000 |  |
| `gen_extra_to_s` | Added noise until | number (s) | 150 | 0 to 1000000 |  |
| `gen_tone_hz` | Carrier offset | number (Hz) | -400000 | -100000000 to 100000000 |  |
| `gen_tone_amp` | Carrier amplitude | number | 0.05 | 0 to 10 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `floor_db` | Noise floor | number (dB) |
| `floor_change_db` | Change over the baseline window | number (dB) |
| `raised` | Sub-bands raised | whole number |
| `elapsed_s` | Time measured | duration (s) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sub_bands` | Sub-bands | `frequency` (frequency), `now_db` (number (dB)), `baseline_db` (number (dB)), `p10_db` (number (dB)), `p50_db` (number (dB)), `p90_db` (number (dB)), `history_min` (number (min)), `raised` (yes or no) |
| `noise_events` | Noise events | `time_s` (duration (s)), `kind` (choice), `frequency` (frequency), `sub_band` (whole number), `rise_db` (number (dB)), `baseline_db` (number (dB)), `duration_s` (duration (s)), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `noise_floor.event` | Noise event | `time_s` (duration (s)), `kind` (choice), `frequency` (frequency), `sub_band` (whole number), `rise_db` (number (dB)), `baseline_db` (number (dB)), `duration_s` (duration (s)), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the histories of the sub-bands and their rise states; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `floor_db`, `floor_change_db`, `raised`.

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

Struct `NoiseFloorFields`. The result fields of `noise_floor`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `floor_db` | number (dB) | optional | 1.0 | The floor of the band: the upper median of the sub-bands now, dB re full scale per bin. |
| `floor_change_db` | number (dB) | optional | 1.0 | The floor now against the oldest kept value (the baseline window ago, or the start). |
| `raised` | whole number | always | 1.0 | The sub-bands raised now. |
| `elapsed_s` | duration (s) | always | 1.0 | The seconds measured. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `sub_bands`

Struct `NoiseFloorSubBandRow`. A row of the list of sub-bands.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The centre of the sub-band. |
| `now_db` | number (dB) | optional | 1.0 | The floor of the last second, dB re full scale per bin. |
| `baseline_db` | number (dB) | optional | 1.0 | The baseline: the 20th percentile of the baseline window. |
| `p10_db` | number (dB) | optional | 1.0 | The 10th percentile of the kept history. |
| `p50_db` | number (dB) | optional | 1.0 | The median of the kept history. |
| `p90_db` | number (dB) | optional | 1.0 | The 90th percentile of the kept history. |
| `history_min` | number (min) | always | 1.0 | The minutes of history kept. |
| `raised` | yes or no | always | 1.0 | The sub-band is raised. |

### Table `noise_events`

Struct `NoiseFloorEventRow`. A row of the noise floor log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the row. |
| `kind` | choice | always | 1.0 | The kind. |
| `frequency` | frequency | optional | 1.0 | The centre of the sub-band; no value for the whole band. |
| `sub_band` | whole number | optional | 1.0 | The sub-band, from 1; no value for the whole band. |
| `rise_db` | number (dB) | always | 1.0 | The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band. |
| `baseline_db` | number (dB) | optional | 1.0 | The baseline of the sub-band. |
| `duration_s` | duration (s) | optional | 1.0 | How long the rise lasted (rows of the fall back). |
| `level_db` | number (dB) | optional | 1.0 | The floor of the sub-band, or of the band, that second. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `noise_floor.event`

Struct `NoiseFloorEvent`. A row of the noise floor log was written.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the row. |
| `kind` | choice | always | 1.0 | The kind. |
| `frequency` | frequency | optional | 1.0 | The centre of the sub-band; no value for the whole band. |
| `sub_band` | whole number | optional | 1.0 | The sub-band, from 1; no value for the whole band. |
| `rise_db` | number (dB) | always | 1.0 | The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band. |
| `baseline_db` | number (dB) | optional | 1.0 | The baseline of the sub-band. |
| `duration_s` | duration (s) | optional | 1.0 | How long the rise lasted (rows of the fall back). |
| `level_db` | number (dB) | optional | 1.0 | The floor of the sub-band, or of the band, that second. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_noise_floor_clear` | none | none |
| `reset` | `sdrkit_noise_floor_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `sub_bands` | `sdrkit_noise_floor_get_sub_bands` | `sdrkit_noise_floor_set_sub_bands` |
| `rise_db` | `sdrkit_noise_floor_get_rise_db` | `sdrkit_noise_floor_set_rise_db` |
| `hold_s` | `sdrkit_noise_floor_get_hold_s` | `sdrkit_noise_floor_set_hold_s` |
| `baseline_min` | `sdrkit_noise_floor_get_baseline_min` | `sdrkit_noise_floor_set_baseline_min` |
| `whole_band_share` | `sdrkit_noise_floor_get_whole_band_share` | `sdrkit_noise_floor_set_whole_band_share` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_extra_hz` | `sdrkit_noise_floor_gen_get_extra_hz` | `sdrkit_noise_floor_gen_set_extra_hz` |
| `gen_extra_bw_hz` | `sdrkit_noise_floor_gen_get_extra_bw_hz` | `sdrkit_noise_floor_gen_set_extra_bw_hz` |
| `gen_extra_power` | `sdrkit_noise_floor_gen_get_extra_power` | `sdrkit_noise_floor_gen_set_extra_power` |
| `gen_extra_from_s` | `sdrkit_noise_floor_gen_get_extra_from_s` | `sdrkit_noise_floor_gen_set_extra_from_s` |
| `gen_extra_to_s` | `sdrkit_noise_floor_gen_get_extra_to_s` | `sdrkit_noise_floor_gen_set_extra_to_s` |
| `gen_tone_hz` | `sdrkit_noise_floor_gen_get_tone_hz` | `sdrkit_noise_floor_gen_set_tone_hz` |
| `gen_tone_amp` | `sdrkit_noise_floor_gen_get_tone_amp` | `sdrkit_noise_floor_gen_set_tone_amp` |

## Try it

```sh
sdrkit run noise_floor --source synth:noise_floor
sdrkit run noise_floor --source <driver>:<address> --freq 0
```
<!-- generated: end -->
