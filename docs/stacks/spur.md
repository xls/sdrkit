<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Spur and image finder

The radio's DC spike, the IQ images and harmonics of strong signals that follow their levels, and fixed spurs on a grid.

Stack id: `spur`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:spur` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `peak_db` | Peak over the noise | number (dB) | 8 | 4 to 30 |  |
| `dc_db` | DC spike over the noise | number (dB) | 6 | 3 to 30 |  |
| `spur_grid` | Fixed spur grid as a fraction of the rate | whole number (1/n) | 16 | 4 to 64 |  |
| `max_signals` | Strong signals used | whole number | 6 | 1 to 16 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_signals` | Signals | text | `130000, 0.2; -210000, 0.2` |  | Entries separated by ';', each 'offset Hz, amplitude'. |
| `gen_swing_db` | Level swing | number (dB) | 5 | 0 to 20 |  |
| `gen_image_db` | Image rejection | number (dB) | 34 | 0 to 120 |  |
| `gen_image_phase` | Image phase | number (rad) | 0.7 | -3.2 to 3.2 |  |
| `gen_dc` | DC offset | number | 0.01 | 0 to 1 |  |
| `gen_h2` | Second-order term | number | 0.5 | 0 to 10 |  |
| `gen_h3` | Third-order term | number | 1.5 | 0 to 10 |  |
| `gen_spur_hz` | Fixed tone | number (Hz) | 250000 | -10000000 to 10000000 |  |
| `gen_spur_amp` | Fixed tone amplitude | number | 0.005 | 0 to 1 |  |
| `gen_real_hz` | Real signal | number (Hz) | 0 | -10000000 to 10000000 |  |
| `gen_real_amp` | Real signal amplitude | number | 0 | 0 to 1 |  |
| `gen_shift_hz` | Shift of the signals | number (Hz) | 0 | -1000000 to 1000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dc_db` | DC spike over the noise | number (dB) |
| `dc_spike` | DC spike present | yes or no |
| `signals` | Strong signals | whole number |
| `images` | Images | whole number |
| `harmonics` | Harmonics | whole number |
| `fixed_spurs` | Fixed spurs | whole number |
| `snapshots` | Snapshots | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `findings` | Findings | `kind` (choice), `frequency` (frequency), `from_signal` (frequency), `below_db` (number (dB)), `verdict` (choice), `slope` (number), `correlation` (number), `image_rejection_db` (number (dB)), `gain_mismatch_db` (number (dB)), `phase_mismatch_deg` (number (°)) |
| `events` | Findings log | `time_s` (duration (s)), `kind` (choice), `frequency` (frequency), `from_signal` (frequency), `below_db` (number (dB)), `verdict` (choice), `slope` (number), `correlation` (number), `image_rejection_db` (number (dB)), `gain_mismatch_db` (number (dB)), `phase_mismatch_deg` (number (°)), `dc_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `images`, `harmonics`, `fixed_spurs`, `dc_db`, `signals`.

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

Struct `SpurFields`. The result fields of `spur`, from the last evaluation. The findings hold for this radio at this gain, rate and tuning.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dc_db` | number (dB) | optional | 1.0 | The largest bin within 2 bins of 0 Hz over the noise. |
| `dc_spike` | yes or no | optional | 1.0 | The DC spike is over its threshold. |
| `signals` | whole number | optional | 1.0 | The strong signals. |
| `images` | whole number | optional | 1.0 | Images not called independent. |
| `harmonics` | whole number | optional | 1.0 | Harmonics not called independent. |
| `fixed_spurs` | whole number | optional | 1.0 | Fixed spurs. |
| `snapshots` | whole number | always | 1.0 | The snapshots taken. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `findings`

Struct `SpurFindingRow`. A finding of `spur`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | The kind. |
| `frequency` | frequency | always | 1.0 | The frequency of the peak. |
| `from_signal` | frequency | optional | 1.0 | The signal that it belongs to; none for a fixed spur. |
| `below_db` | number (dB) | always | 1.0 | The mean level under the signal; for a fixed spur the mean level over the noise. |
| `verdict` | choice | always | 1.0 | The verdict. |
| `slope` | number | optional | 1.0 | The slope of its level against the signal's. |
| `correlation` | number | optional | 1.0 | The correlation. |
| `image_rejection_db` | number (dB) | optional | 1.0 | The image rejection (images). |
| `gain_mismatch_db` | number (dB) | optional | 1.0 | The gain mismatch alone that gives this image. |
| `phase_mismatch_deg` | number (°) | optional | 1.0 | The phase mismatch alone that gives this image. |

### Table `events`

Struct `SpurEventRow`. A row of the `spur` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | The kind. |
| `frequency` | frequency | always | 1.0 | The frequency of the peak. |
| `from_signal` | frequency | optional | 1.0 | The signal that it belongs to. |
| `below_db` | number (dB) | optional | 1.0 | The mean level under the signal, or over the noise. |
| `verdict` | choice | optional | 1.0 | The verdict. |
| `slope` | number | optional | 1.0 | The slope. |
| `correlation` | number | optional | 1.0 | The correlation. |
| `image_rejection_db` | number (dB) | optional | 1.0 | The image rejection. |
| `gain_mismatch_db` | number (dB) | optional | 1.0 | The gain mismatch alone. |
| `phase_mismatch_deg` | number (°) | optional | 1.0 | The phase mismatch alone. |
| `dc_db` | number (dB) | optional | 1.0 | The DC spike over the noise (kind dc). |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_spur_clear` | none | none |
| `reset` | `sdrkit_spur_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `peak_db` | `sdrkit_spur_get_peak_db` | `sdrkit_spur_set_peak_db` |
| `dc_db` | `sdrkit_spur_get_dc_db` | `sdrkit_spur_set_dc_db` |
| `spur_grid` | `sdrkit_spur_get_spur_grid` | `sdrkit_spur_set_spur_grid` |
| `max_signals` | `sdrkit_spur_get_max_signals` | `sdrkit_spur_set_max_signals` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_signals` | `sdrkit_spur_gen_get_signals` | `sdrkit_spur_gen_set_signals` |
| `gen_swing_db` | `sdrkit_spur_gen_get_swing_db` | `sdrkit_spur_gen_set_swing_db` |
| `gen_image_db` | `sdrkit_spur_gen_get_image_db` | `sdrkit_spur_gen_set_image_db` |
| `gen_image_phase` | `sdrkit_spur_gen_get_image_phase` | `sdrkit_spur_gen_set_image_phase` |
| `gen_dc` | `sdrkit_spur_gen_get_dc` | `sdrkit_spur_gen_set_dc` |
| `gen_h2` | `sdrkit_spur_gen_get_h2` | `sdrkit_spur_gen_set_h2` |
| `gen_h3` | `sdrkit_spur_gen_get_h3` | `sdrkit_spur_gen_set_h3` |
| `gen_spur_hz` | `sdrkit_spur_gen_get_spur_hz` | `sdrkit_spur_gen_set_spur_hz` |
| `gen_spur_amp` | `sdrkit_spur_gen_get_spur_amp` | `sdrkit_spur_gen_set_spur_amp` |
| `gen_real_hz` | `sdrkit_spur_gen_get_real_hz` | `sdrkit_spur_gen_set_real_hz` |
| `gen_real_amp` | `sdrkit_spur_gen_get_real_amp` | `sdrkit_spur_gen_set_real_amp` |
| `gen_shift_hz` | `sdrkit_spur_gen_get_shift_hz` | `sdrkit_spur_gen_set_shift_hz` |

## Try it

```sh
sdrkit run spur --source synth:spur
sdrkit run spur --source <driver>:<address> --freq 0
```
<!-- generated: end -->
