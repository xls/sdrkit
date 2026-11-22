<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FSK parameter analyser

Of an unknown FSK or GFSK signal: symbol rate, 2 or 4 levels, shift, levels, modulation index, centre and eye SNR.

Stack id: `fsk_analyser`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 100 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fsk_analyser` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `block_s` | Block length | number (s) | 2 | 0.5 to 10 |  |
| `min_baud` | Lowest symbol rate | number (Bd) | 100 | 20 to 5000 |  |
| `min_eye_db` | Lowest eye SNR shown | number (dB) | 6 | 0 to 20 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_levels` | Levels | whole number | 2 | 2 to 4 |  |
| `gen_baud` | Symbol rate | number (Bd) | 9600 | 10 to 100000 |  |
| `gen_deviation_hz` | Deviation | number (Hz) | 2400 | 0 to 50000 |  |
| `gen_bt` | Gaussian BT | number | 0.5 | 0 to 2 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `levels` | Levels | choice |
| `msk_like` | Modulation index 0.5 | yes or no |
| `symbol_rate_bd` | Symbol rate | number (Bd) |
| `shift_hz` | Shift | frequency |
| `outer_hz` | Outer distance | frequency |
| `mod_index` | Modulation index | number |
| `centre_hz` | Centre | frequency |
| `spacing_ratio` | Spacing ratio | number |
| `spacing_equal` | Equal spacing | yes or no |
| `eye_snr_db` | Eye SNR | number (dB) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `level_freqs` | Levels | `index` (whole number), `frequency_hz` (frequency) |
| `events` | Readings | `time_s` (duration (s)), `levels` (choice), `symbol_rate_bd` (number (Bd)), `shift_hz` (frequency), `mod_index` (number), `centre_hz` (frequency), `eye_snr_db` (number (dB)) |

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

Struct `FskAnalyserFields`. The result fields of `fsk_analyser`, from the last block. They have no value while no FSK signal is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `levels` | choice | optional | 1.0 | The number of levels. |
| `msk_like` | yes or no | optional | 1.0 | Two levels with a modulation index of 0.5. |
| `symbol_rate_bd` | number (Bd) | optional | 1.0 | The symbol rate. |
| `shift_hz` | frequency | optional | 1.0 | Two levels: the distance of the tones; four: the mean distance of neighbours. |
| `outer_hz` | frequency | optional | 1.0 | The distance of the outer levels. |
| `mod_index` | number | optional | 1.0 | The outer distance over the symbol rate. |
| `centre_hz` | frequency | optional | 1.0 | The centre of the levels from the tuned frequency. |
| `spacing_ratio` | number | optional | 1.0 | Four levels: the mean outer spacing over the inner spacing. |
| `spacing_equal` | yes or no | optional | 1.0 | Four levels: the spacing ratio is within 0.35 of 1. |
| `eye_snr_db` | number (dB) | optional | 1.0 | The level spacing over the spread inside a level. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `level_freqs`

Struct `FskAnalyserLevelRow`. A row of the level table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `index` | whole number | always | 1.0 | The place of the level from the lowest, from 0. |
| `frequency_hz` | frequency | always | 1.0 | The frequency of the level from the tuned frequency. |

### Table `events`

Struct `FskAnalyserEventRow`. A row of the `fsk_analyser` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `levels` | choice | always | 1.0 | The number of levels. |
| `symbol_rate_bd` | number (Bd) | always | 1.0 | The symbol rate. |
| `shift_hz` | frequency | always | 1.0 | The shift. |
| `mod_index` | number | always | 1.0 | The modulation index. |
| `centre_hz` | frequency | always | 1.0 | The centre. |
| `eye_snr_db` | number (dB) | always | 1.0 | The eye SNR. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fsk_analyser_clear` | none | none |
| `reset` | `sdrkit_fsk_analyser_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `block_s` | `sdrkit_fsk_analyser_get_block_s` | `sdrkit_fsk_analyser_set_block_s` |
| `min_baud` | `sdrkit_fsk_analyser_get_min_baud` | `sdrkit_fsk_analyser_set_min_baud` |
| `min_eye_db` | `sdrkit_fsk_analyser_get_min_eye_db` | `sdrkit_fsk_analyser_set_min_eye_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_levels` | `sdrkit_fsk_analyser_gen_get_levels` | `sdrkit_fsk_analyser_gen_set_levels` |
| `gen_baud` | `sdrkit_fsk_analyser_gen_get_baud` | `sdrkit_fsk_analyser_gen_set_baud` |
| `gen_deviation_hz` | `sdrkit_fsk_analyser_gen_get_deviation_hz` | `sdrkit_fsk_analyser_gen_set_deviation_hz` |
| `gen_bt` | `sdrkit_fsk_analyser_gen_get_bt` | `sdrkit_fsk_analyser_gen_set_bt` |

## Try it

```sh
sdrkit run fsk_analyser --source synth:fsk_analyser
sdrkit run fsk_analyser --source <driver>:<address> --freq 0
```
<!-- generated: end -->
