<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Power-line and impulse noise locator

Impulse rate, level and repetition rate of the noise in a band, with a class: twice the mains, the mains, another steady rate, or none.

Stack id: `noise_locator`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 1 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 20 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:noise_locator` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `mains_hz` | Mains frequencies | text (Hz) | `50, 60` |  | The mains frequencies tested, separated by commas; a rate near one or twice one names a power-line class. |
| `sigmas` | Impulse threshold over the noise | number (sigma) | 6.5 | 3 to 20 |  |
| `window_s` | Impulse window | number (s) | 10 | 2 to 60 |  |
| `analysis_khz` | Analysis band | number (kHz) | 20 | 2 to 100 | Twice the pass edge of the filter before the impulse detector. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_rate_hz` | Impulse rate | number (Hz) | 100 | 0 to 10000 |  |
| `gen_jitter_us` | Timing jitter | number (us) | 60 | 0 to 10000 |  |
| `gen_width_us` | Impulse width | number (us) | 70 | 1 to 100000 |  |
| `gen_peak_db` | Impulse peak | number (dB) | 20 | -40 to 80 |  |
| `gen_spread_db` | Peak spread | number (dB) | 3 | 0 to 40 |  |
| `gen_drop` | Missing share | number | 0.1 | 0 to 1 |  |
| `gen_random_rate` | Random impulse rate | number (Hz) | 0 | 0 to 10000 |  |
| `gen_random_db` | Random impulse peak | number (dB) | 12 | -40 to 80 |  |
| `gen_rate_drift_hz_s` | Rate drift | number (Hz/s) | 0 | -100 to 100 |  |
| `gen_noise_sigma` | Reference noise per quadrature | number | 1 | 0.000001 to 1000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `impulse_rate` | Impulse rate | number (1/s) |
| `impulse_level_db` | Impulse level over the noise | number (dB) |
| `impulse_spread_db` | Impulse level spread | number (dB) |
| `repetition_hz` | Repetition rate | number (Hz) |
| `class` | Class | choice |
| `mains_found_hz` | Mains frequency found | number (Hz) |
| `strength_pct` | Strength | number (%) |
| `impulses` | Impulses | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `patterns` | Patterns | `time_s` (duration (s)), `kind` (choice), `repetition_hz` (number (Hz)), `class` (choice), `impulse_rate` (number (1/s)), `level_db` (number (dB)), `strength_pct` (number (%)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `noise_locator.pattern` | Pattern | `time_s` (duration (s)), `kind` (choice), `repetition_hz` (number (Hz)), `class` (choice), `impulse_rate` (number (1/s)), `level_db` (number (dB)), `strength_pct` (number (%)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Learns the noise again and drops the impulses and the pattern; the table stays. |

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

Struct `NoiseLocatorFields`. The result fields of `noise_locator`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `impulse_rate` | number (1/s) | optional | 1.0 | The impulses per second of the impulse window. |
| `impulse_level_db` | number (dB) | optional | 1.0 | The median impulse level over the noise of the analysis band. |
| `impulse_spread_db` | number (dB) | optional | 1.0 | The 75th minus the 25th percentile of the impulse levels. |
| `repetition_hz` | number (Hz) | optional | 1.0 | The repetition rate of the present pattern; no value without one. |
| `class` | choice | always | 1.0 | The class of the present pattern. |
| `mains_found_hz` | number (Hz) | optional | 1.0 | The mains frequency of the class, when it names one. |
| `strength_pct` | number (%) | optional | 1.0 | The share of the impulses on the rate. |
| `impulses` | whole number | always | 1.0 | The impulses since the start. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `patterns`

Struct `NoiseLocatorPatternRow`. A row of the pattern log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the report. |
| `kind` | choice | always | 1.0 | Found or gone. |
| `repetition_hz` | number (Hz) | always | 1.0 | The repetition rate. |
| `class` | choice | always | 1.0 | The class of the rate. |
| `impulse_rate` | number (1/s) | always | 1.0 | The impulses per second of the window. |
| `level_db` | number (dB) | always | 1.0 | The median impulse level over the noise of the analysis band. |
| `strength_pct` | number (%) | always | 1.0 | The share of the impulses on the rate. |

### Event `noise_locator.pattern`

Struct `NoiseLocatorPatternEvent`. A pattern was found or went.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the report. |
| `kind` | choice | always | 1.0 | Found or gone. |
| `repetition_hz` | number (Hz) | always | 1.0 | The repetition rate. |
| `class` | choice | always | 1.0 | The class of the rate. |
| `impulse_rate` | number (1/s) | always | 1.0 | The impulses per second of the window. |
| `level_db` | number (dB) | always | 1.0 | The median impulse level over the noise of the analysis band. |
| `strength_pct` | number (%) | always | 1.0 | The share of the impulses on the rate. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_noise_locator_clear` | none | none |
| `reset` | `sdrkit_noise_locator_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `mains_hz` | `sdrkit_noise_locator_get_mains_hz` | `sdrkit_noise_locator_set_mains_hz` |
| `sigmas` | `sdrkit_noise_locator_get_sigmas` | `sdrkit_noise_locator_set_sigmas` |
| `window_s` | `sdrkit_noise_locator_get_window_s` | `sdrkit_noise_locator_set_window_s` |
| `analysis_khz` | `sdrkit_noise_locator_get_analysis_khz` | `sdrkit_noise_locator_set_analysis_khz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_rate_hz` | `sdrkit_noise_locator_gen_get_rate_hz` | `sdrkit_noise_locator_gen_set_rate_hz` |
| `gen_jitter_us` | `sdrkit_noise_locator_gen_get_jitter_us` | `sdrkit_noise_locator_gen_set_jitter_us` |
| `gen_width_us` | `sdrkit_noise_locator_gen_get_width_us` | `sdrkit_noise_locator_gen_set_width_us` |
| `gen_peak_db` | `sdrkit_noise_locator_gen_get_peak_db` | `sdrkit_noise_locator_gen_set_peak_db` |
| `gen_spread_db` | `sdrkit_noise_locator_gen_get_spread_db` | `sdrkit_noise_locator_gen_set_spread_db` |
| `gen_drop` | `sdrkit_noise_locator_gen_get_drop` | `sdrkit_noise_locator_gen_set_drop` |
| `gen_random_rate` | `sdrkit_noise_locator_gen_get_random_rate` | `sdrkit_noise_locator_gen_set_random_rate` |
| `gen_random_db` | `sdrkit_noise_locator_gen_get_random_db` | `sdrkit_noise_locator_gen_set_random_db` |
| `gen_rate_drift_hz_s` | `sdrkit_noise_locator_gen_get_rate_drift_hz_s` | `sdrkit_noise_locator_gen_set_rate_drift_hz_s` |
| `gen_noise_sigma` | `sdrkit_noise_locator_gen_get_noise_sigma` | `sdrkit_noise_locator_gen_set_noise_sigma` |

## Try it

```sh
sdrkit run noise_locator --source synth:noise_locator
sdrkit run noise_locator --source <driver>:<address> --freq 0
```
<!-- generated: end -->
