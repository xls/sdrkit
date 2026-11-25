<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Frequency-hopper analyser

One frequency hopper in the span: hop rate, dwell, guard, channel count, spacing and span.

Stack id: `hopper`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hopper` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `present_db` | Strongest bin over the frame median | number (dB) | 13 | 6 to 30 |  |
| `min_dwells` | Dwells before a report | whole number | 40 | 20 to 400 |  |
| `min_channels` | Fewest channels | whole number | 3 | 3 to 16 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_channels` | Channels | whole number | 16 | 0 to 256 |  |
| `gen_spacing_hz` | Channel spacing | number (Hz) | 50000 | 1000 to 10000000 |  |
| `gen_dwell_s` | Dwell | number (s) | 0.002 | 0.0001 to 10 |  |
| `gen_guard_s` | Guard | number (s) | 0.0003 | 0 to 10 |  |
| `gen_jitter` | Hop clock jitter | number | 0 | 0 to 0.5 |  |
| `gen_fsk_hz` | FSK deviation | number (Hz) | 3000 | 0 to 1000000 |  |
| `gen_fsk_bps` | FSK bit rate | number (bit/s) | 10000 | 1 to 10000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `hop_rate` | Hop rate | number (1/s) |
| `hop_period_ms` | Hop period | number (ms) |
| `dwell_ms` | Dwell | number (ms) |
| `guard_ms` | Guard | number (ms) |
| `channels` | Channels | whole number |
| `spacing_khz` | Channel spacing | number (kHz) |
| `grid` | Regular grid | yes or no |
| `span_khz` | Span | number (kHz) |
| `lowest` | Lowest channel | frequency |
| `highest` | Highest channel | frequency |
| `level_db` | Level over the median | number (dB) |
| `dwells` | Dwells | whole number |
| `dwells_window` | Dwells in the window | whole number |
| `bin_khz` | Bin width | number (kHz) |
| `frame_us` | Frame step | number (us) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `hoppers` | Hoppers | `time_s` (duration (s)), `hop_rate` (number (1/s)), `channels` (whole number), `spacing_khz` (number (kHz)), `span_khz` (number (kHz)), `lowest` (frequency), `highest` (frequency), `dwell_ms` (number (ms)), `guard_ms` (number (ms)), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hopper.changed` | Hopper | `time_s` (duration (s)), `hop_rate` (number (1/s)), `channels` (whole number), `spacing_khz` (number (kHz)), `span_khz` (number (kHz)), `lowest` (frequency), `highest` (frequency), `dwell_ms` (number (ms)), `guard_ms` (number (ms)), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the dwells and the readings; the table stays. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `hop_rate`, `channels`, `spacing_khz`, `dwell_ms`, `span_khz`.

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

Struct `HopperFields`. The result fields of `hopper`. The readings of the hopper have no value until an evaluation finds one.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `hop_rate` | number (1/s) | optional | 1.0 | The hop rate: one over the median start-to-start time. |
| `hop_period_ms` | number (ms) | optional | 1.0 | The median start-to-start time. |
| `dwell_ms` | number (ms) | optional | 1.0 | The median dwell. |
| `guard_ms` | number (ms) | optional | 1.0 | The median silence between dwells. |
| `channels` | whole number | optional | 1.0 | The channels found. |
| `spacing_khz` | number (kHz) | optional | 1.0 | The median distance of neighbouring channels. |
| `grid` | yes or no | optional | 1.0 | At least 90 % of the channels lie on a grid of the spacing. |
| `span_khz` | number (kHz) | optional | 1.0 | The highest minus the lowest channel plus one spacing. |
| `lowest` | frequency | optional | 1.0 | The lowest channel. |
| `highest` | frequency | optional | 1.0 | The highest channel. |
| `level_db` | number (dB) | optional | 1.0 | The mean dwell level over the frame median. |
| `dwells` | whole number | always | 1.0 | The dwells since the start. |
| `dwells_window` | whole number | always | 1.0 | The dwells kept for the evaluation. |
| `bin_khz` | number (kHz) | always | 1.0 | The width of a bin. |
| `frame_us` | number (us) | always | 1.0 | The frame step. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `hoppers`

Struct `HopperRow`. A row of the hopper log: an evaluation that found a hopper or a change of it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the evaluation. |
| `hop_rate` | number (1/s) | always | 1.0 | The hop rate. |
| `channels` | whole number | always | 1.0 | The channels. |
| `spacing_khz` | number (kHz) | always | 1.0 | The channel spacing. |
| `span_khz` | number (kHz) | always | 1.0 | The span. |
| `lowest` | frequency | always | 1.0 | The lowest channel. |
| `highest` | frequency | always | 1.0 | The highest channel. |
| `dwell_ms` | number (ms) | always | 1.0 | The median dwell. |
| `guard_ms` | number (ms) | always | 1.0 | The median guard. |
| `level_db` | number (dB) | always | 1.0 | The mean dwell level over the frame median. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `hopper.changed`

Struct `HopperChangedEvent`. A hopper was found, or its rate or its channel count changed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the evaluation. |
| `hop_rate` | number (1/s) | always | 1.0 | The hop rate. |
| `channels` | whole number | always | 1.0 | The channels. |
| `spacing_khz` | number (kHz) | always | 1.0 | The channel spacing. |
| `span_khz` | number (kHz) | always | 1.0 | The span. |
| `lowest` | frequency | always | 1.0 | The lowest channel. |
| `highest` | frequency | always | 1.0 | The highest channel. |
| `dwell_ms` | number (ms) | always | 1.0 | The median dwell. |
| `guard_ms` | number (ms) | always | 1.0 | The median guard. |
| `level_db` | number (dB) | always | 1.0 | The mean dwell level over the frame median. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hopper_clear` | none | none |
| `reset` | `sdrkit_hopper_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `present_db` | `sdrkit_hopper_get_present_db` | `sdrkit_hopper_set_present_db` |
| `min_dwells` | `sdrkit_hopper_get_min_dwells` | `sdrkit_hopper_set_min_dwells` |
| `min_channels` | `sdrkit_hopper_get_min_channels` | `sdrkit_hopper_set_min_channels` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_channels` | `sdrkit_hopper_gen_get_channels` | `sdrkit_hopper_gen_set_channels` |
| `gen_spacing_hz` | `sdrkit_hopper_gen_get_spacing_hz` | `sdrkit_hopper_gen_set_spacing_hz` |
| `gen_dwell_s` | `sdrkit_hopper_gen_get_dwell_s` | `sdrkit_hopper_gen_set_dwell_s` |
| `gen_guard_s` | `sdrkit_hopper_gen_get_guard_s` | `sdrkit_hopper_gen_set_guard_s` |
| `gen_jitter` | `sdrkit_hopper_gen_get_jitter` | `sdrkit_hopper_gen_set_jitter` |
| `gen_fsk_hz` | `sdrkit_hopper_gen_get_fsk_hz` | `sdrkit_hopper_gen_set_fsk_hz` |
| `gen_fsk_bps` | `sdrkit_hopper_gen_get_fsk_bps` | `sdrkit_hopper_gen_set_fsk_bps` |

## Try it

```sh
sdrkit run hopper --source synth:hopper
sdrkit run hopper --source <driver>:<address> --freq 0
```
<!-- generated: end -->
