<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Intermodulation finder

Strong signals of a wide capture, the peaks at their product frequencies and whether each follows the intermodulation law.

Stack id: `intermod`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:intermod` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `snapshot_s` | Snapshot length | number (s) | 0.25 | 0.1 to 2 |  |
| `peak_db` | Peak over the noise | number (dB) | 10 | 6 to 30 |  |
| `input_db` | Strong signal over the noise (mean) | number (dB) | 20 | 10 to 60 |  |
| `max_inputs` | Strong signals used | whole number | 8 | 2 to 16 |  |
| `second_order` | Also f2 - f1 | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_f1_hz` | First tone | number (Hz) | 120000 | -10000000 to 10000000 |  |
| `gen_f2_hz` | Second tone | number (Hz) | 190000 | -10000000 to 10000000 |  |
| `gen_amp` | Tone amplitude | number | 0.2 | 0 to 1 |  |
| `gen_swing_db` | Level swing | number (dB) | 5 | 0 to 20 |  |
| `gen_swing_hz` | Swing rates | text (Hz) | `0.071, 0.113` |  | The rates of the two swings separated by ','. |
| `gen_k` | Cubic term of the receiver | number | 0.8 | 0 to 10 |  |
| `gen_extra_hz` | Independent tone | number (Hz) | 0 | -10000000 to 10000000 |  |
| `gen_extra_amp` | Independent tone amplitude | number | 0 | 0 to 1 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `inputs` | Strong signals | whole number |
| `products` | Peaks at product frequencies | whole number |
| `products_follow` | Following the law | whole number |
| `products_independent` | Independent | whole number |
| `products_unknown` | Unknown | whole number |
| `snapshots` | Snapshots | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `inputs` | Strong signals | `frequency` (frequency), `level_db` (number (dB)) |
| `products` | Products | `frequency` (frequency), `kind` (choice), `parent_a` (frequency), `parent_b` (frequency), `below_db` (number (dB)), `verdict` (choice), `slope` (number), `correlation` (number), `points` (whole number) |
| `events` | Verdicts | `time_s` (duration (s)), `frequency` (frequency), `kind` (choice), `parent_a` (frequency), `parent_b` (frequency), `below_db` (number (dB)), `verdict` (choice), `slope` (number), `correlation` (number), `points` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `intermod.product` | Product | `time_s` (duration (s)), `frequency` (frequency), `kind` (choice), `parent_a` (frequency), `parent_b` (frequency), `below_db` (number (dB)), `verdict` (choice), `slope` (number), `correlation` (number), `points` (whole number) |

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

Struct `IntermodFields`. The result fields of `intermod`, from the last evaluation.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `inputs` | whole number | optional | 1.0 | The strong signals used as parents. |
| `products` | whole number | optional | 1.0 | The peaks at product frequencies. |
| `products_follow` | whole number | optional | 1.0 | Those that follow the law. |
| `products_independent` | whole number | optional | 1.0 | Those called independent. |
| `products_unknown` | whole number | optional | 1.0 | Those unclear or that cannot be told. |
| `snapshots` | whole number | always | 1.0 | The snapshots taken. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `inputs`

Struct `IntermodInputRow`. A strong signal of `intermod`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The frequency (an offset from the centre when the tuned frequency is not known). |
| `level_db` | number (dB) | always | 1.0 | The mean level over the noise. |

### Table `products`

Struct `IntermodProductRow`. A peak at a product frequency.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The frequency of the peak. |
| `kind` | choice | always | 1.0 | The kind of product. |
| `parent_a` | frequency | always | 1.0 | The lower parent. |
| `parent_b` | frequency | always | 1.0 | The upper parent. |
| `below_db` | number (dB) | always | 1.0 | The mean level under the weaker parent. |
| `verdict` | choice | always | 1.0 | The verdict of the level fit. |
| `slope` | number | optional | 1.0 | The slope of its level against the law. |
| `correlation` | number | optional | 1.0 | The correlation of its level with the law. |
| `points` | whole number | always | 1.0 | The snapshots of the fit. |

### Table `events`

Struct `IntermodEventRow`. A row of the `intermod` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `frequency` | frequency | always | 1.0 | The frequency of the peak. |
| `kind` | choice | always | 1.0 | The kind of product. |
| `parent_a` | frequency | always | 1.0 | The lower parent. |
| `parent_b` | frequency | always | 1.0 | The upper parent. |
| `below_db` | number (dB) | always | 1.0 | The mean level under the weaker parent. |
| `verdict` | choice | always | 1.0 | The verdict. |
| `slope` | number | optional | 1.0 | The slope. |
| `correlation` | number | optional | 1.0 | The correlation. |
| `points` | whole number | always | 1.0 | The snapshots of the fit. |

### Event `intermod.product`

Struct `IntermodProductEvent`. A verdict of a product that held for two evaluations.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `frequency` | frequency | always | 1.0 | The frequency of the peak. |
| `kind` | choice | always | 1.0 | The kind of product. |
| `parent_a` | frequency | always | 1.0 | The lower parent. |
| `parent_b` | frequency | always | 1.0 | The upper parent. |
| `below_db` | number (dB) | always | 1.0 | The mean level under the weaker parent. |
| `verdict` | choice | always | 1.0 | The verdict. |
| `slope` | number | optional | 1.0 | The slope. |
| `correlation` | number | optional | 1.0 | The correlation. |
| `points` | whole number | always | 1.0 | The snapshots of the fit. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_intermod_clear` | none | none |
| `reset` | `sdrkit_intermod_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `snapshot_s` | `sdrkit_intermod_get_snapshot_s` | `sdrkit_intermod_set_snapshot_s` |
| `peak_db` | `sdrkit_intermod_get_peak_db` | `sdrkit_intermod_set_peak_db` |
| `input_db` | `sdrkit_intermod_get_input_db` | `sdrkit_intermod_set_input_db` |
| `max_inputs` | `sdrkit_intermod_get_max_inputs` | `sdrkit_intermod_set_max_inputs` |
| `second_order` | `sdrkit_intermod_get_second_order` | `sdrkit_intermod_set_second_order` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_f1_hz` | `sdrkit_intermod_gen_get_f1_hz` | `sdrkit_intermod_gen_set_f1_hz` |
| `gen_f2_hz` | `sdrkit_intermod_gen_get_f2_hz` | `sdrkit_intermod_gen_set_f2_hz` |
| `gen_amp` | `sdrkit_intermod_gen_get_amp` | `sdrkit_intermod_gen_set_amp` |
| `gen_swing_db` | `sdrkit_intermod_gen_get_swing_db` | `sdrkit_intermod_gen_set_swing_db` |
| `gen_swing_hz` | `sdrkit_intermod_gen_get_swing_hz` | `sdrkit_intermod_gen_set_swing_hz` |
| `gen_k` | `sdrkit_intermod_gen_get_k` | `sdrkit_intermod_gen_set_k` |
| `gen_extra_hz` | `sdrkit_intermod_gen_get_extra_hz` | `sdrkit_intermod_gen_set_extra_hz` |
| `gen_extra_amp` | `sdrkit_intermod_gen_get_extra_amp` | `sdrkit_intermod_gen_set_extra_amp` |

## Try it

```sh
sdrkit run intermod --source synth:intermod
sdrkit run intermod --source <driver>:<address> --freq 0
```
<!-- generated: end -->
