<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Burst finder

Every transmission shorter than 3 s anywhere in the span: frequency, length, bandwidth and level; sources that repeat at a steady interval.

Stack id: `burst_finder`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:burst_finder` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `hot_db` | Hot bin over the frame median | number (dB) | 14 | 8 to 30 |  |
| `max_burst_s` | Longest burst | number (s) | 3 | 0.1 to 30 |  |
| `repeat_tolerance_pct` | Interval agreement | number (%) | 5 | 1 to 20 |  |
| `fft_size` | Frame length | whole number | 256 | 128 to 1024 | Points of a frame: 128, 256, 512 or 1024 (rounded to a power of two). Frames step by half of it. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bursts` | Bursts | text | `0.1, 0.0032, 120000, 0, 1, 24, 0.25, 0.001; 0.7, 0.0011, -310000, 0, 1; 1.1, 0.0011, -310000, 0, 1; 1.9, 0.0011, -310000, 0, 1; 2.0, 0.0011, -310000, 0, 1; 3.3, 0.0011, -310000, 0, 1; 3.7, 0.0011, -310000, 0, 1; 4.4, 0.0011, -310000, 0, 1; 5.0, 0.0011, -310000, 0, 1; 5.4, 0.0011, -310000, 0, 1; 0.5, 0.001, 500000, 400000, 3, 6, 0.9; 1.3, 0.00035, -700000, 0, 1, 5, 1.1` |  | Bursts separated by ';', each 'start s, length s, offset Hz, width Hz, amplitude' and for a train 'count, every s, jitter' (a share of the interval). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `bursts` | Bursts | whole number |
| `elapsed_s` | Time listened | duration (s) |
| `bursts_per_s` | Bursts per second | number (1/s) |
| `repeating` | Repeating sources | whole number |
| `bin_khz` | Bin width | number (kHz) |
| `frame_us` | Frame step | number (us) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bursts` | Bursts | `time_s` (duration (s)), `frequency` (frequency), `length_ms` (number (ms)), `class` (choice), `width_khz` (number (kHz)), `level_db` (number (dB)) |
| `repeating` | Repeating sources | `frequency` (frequency), `period_s` (duration (s)), `bursts` (whole number), `spread_pct` (number (%)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `burst.found` | Burst | `time_s` (duration (s)), `frequency` (frequency), `length_ms` (number (ms)), `class` (choice), `width_khz` (number (kHz)), `level_db` (number (dB)) |
| `burst.repeating` | Repeating source | `frequency` (frequency), `period_s` (duration (s)), `bursts` (whole number), `spread_pct` (number (%)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the bursts in progress; the tables stay. |

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

Struct `BurstFinderFields`. The result fields of `burst_finder`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts` | whole number | always | 1.0 | The bursts found. |
| `elapsed_s` | duration (s) | always | 1.0 | The time listened. |
| `bursts_per_s` | number (1/s) | always | 1.0 | Bursts per second over the time listened. |
| `repeating` | whole number | always | 1.0 | The sources found repeating now. |
| `bin_khz` | number (kHz) | always | 1.0 | The width of a bin: the frequency resolution. |
| `frame_us` | number (us) | always | 1.0 | The frame step: the time resolution. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bursts`

Struct `BurstFinderBurstRow`. A row of the burst log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The start of the burst, signal time. |
| `frequency` | frequency | always | 1.0 | The median centre of the burst. |
| `length_ms` | number (ms) | always | 1.0 | The length of the burst. |
| `class` | choice | always | 1.0 | The bandwidth class. |
| `width_khz` | number (kHz) | always | 1.0 | The median width, at least one bin. |
| `level_db` | number (dB) | always | 1.0 | The mean peak over the median of the frames. |

### Table `repeating`

Struct `BurstFinderRepeatingRow`. A row of the list of repeating sources.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The frequency of the source: its first burst. |
| `period_s` | duration (s) | always | 1.0 | The median interval between its bursts. |
| `bursts` | whole number | always | 1.0 | Its bursts in the history. |
| `spread_pct` | number (%) | always | 1.0 | The RMS deviation of the agreeing intervals from the period, over the period. |

### Event `burst.found`

Struct `BurstFoundEvent`. A burst was found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The start of the burst, signal time. |
| `frequency` | frequency | always | 1.0 | The median centre of the burst. |
| `length_ms` | number (ms) | always | 1.0 | The length of the burst. |
| `class` | choice | always | 1.0 | The bandwidth class. |
| `width_khz` | number (kHz) | always | 1.0 | The median width, at least one bin. |
| `level_db` | number (dB) | always | 1.0 | The mean peak over the median of the frames. |

### Event `burst.repeating`

Struct `BurstRepeatingEvent`. A source was found repeating.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The frequency of the source: its first burst. |
| `period_s` | duration (s) | always | 1.0 | The median interval between its bursts. |
| `bursts` | whole number | always | 1.0 | Its bursts in the history. |
| `spread_pct` | number (%) | always | 1.0 | The RMS deviation of the agreeing intervals from the period, over the period. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_burst_finder_clear` | none | none |
| `reset` | `sdrkit_burst_finder_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `hot_db` | `sdrkit_burst_finder_get_hot_db` | `sdrkit_burst_finder_set_hot_db` |
| `max_burst_s` | `sdrkit_burst_finder_get_max_burst_s` | `sdrkit_burst_finder_set_max_burst_s` |
| `repeat_tolerance_pct` | `sdrkit_burst_finder_get_repeat_tolerance_pct` | `sdrkit_burst_finder_set_repeat_tolerance_pct` |
| `fft_size` | `sdrkit_burst_finder_get_fft_size` | `sdrkit_burst_finder_set_fft_size` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bursts` | `sdrkit_burst_finder_gen_get_bursts` | `sdrkit_burst_finder_gen_set_bursts` |

## Try it

```sh
sdrkit run burst_finder --source synth:burst_finder
sdrkit run burst_finder --source <driver>:<address> --freq 0
```
<!-- generated: end -->
