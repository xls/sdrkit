<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Fading analyser

Of one carrier: level percentiles, fade depth, S4, Rician K, fade rate and length, Doppler spread and C/N0.

Stack id: `fading`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 6 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fading` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `window_s` | Statistics window | number (s) | 60 | 10 to 600 |  |
| `first_s` | First reading after | number (s) | 30 | 5 to 600 |  |
| `fade_db` | Fade threshold under the mean | number (dB) | 10 | 3 to 30 |  |
| `log_depth_db` | Fade logged deeper than | number (dB) | 20 | 3 to 40 |  |
| `log_length_s` | Fade logged longer than | number (s) | 0.5 | 0.05 to 10 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_fd_hz` | Largest Doppler | number (Hz) | 5 | 0.01 to 200 |  |
| `gen_k` | Rice factor | number | 0 | 0 to 1000000000 | Direct over scattered power as a ratio; 0 is Rayleigh fading. |
| `gen_off` | No carrier | text | empty |  | Spans separated by ';', each 'from s, to s'. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `k_db` | Rician K-factor | number (dB) |
| `k` | Rician K-factor ratio | number |
| `rayleigh` | Rayleigh | yes or no |
| `p1_db` | Level at 1 % | number (dB) |
| `p10_db` | Level at 10 % | number (dB) |
| `p50_db` | Level at 50 % | number (dB) |
| `p90_db` | Level at 90 % | number (dB) |
| `p99_db` | Level at 99 % | number (dB) |
| `fade_depth_db` | Fade depth | number (dB) |
| `s4` | Scintillation index | number |
| `fade_rate_hz` | Fade rate | number (1/s) |
| `fade_length_ms` | Average fade length | number (ms) |
| `doppler_hz` | Doppler spread | number (Hz) |
| `cn0_dbhz` | C/N0 | number (dB-Hz) |
| `carrier_offset_hz` | Carrier | frequency |
| `window_s` | Envelope kept | duration (s) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Log | `time_s` (duration (s)), `kind` (choice), `depth_db` (number (dB)), `length_ms` (number (ms)), `k_db` (number (dB)), `p10_db` (number (dB)), `fade_rate_hz` (number (1/s)), `fade_length_ms` (number (ms)), `doppler_hz` (number (Hz)), `cn0_dbhz` (number (dB-Hz)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

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

Struct `FadingFields`. The result fields of `fading`: the statistics of the envelope of one carrier over the window, and the carrier now.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `k_db` | number (dB) | optional | 1.0 | The Rician K-factor; 40 dB for no fading, no value for K = 0 (Rayleigh). |
| `k` | number | optional | 1.0 | The K-factor as a ratio. |
| `rayleigh` | yes or no | optional | 1.0 | No steady component: K = 0. |
| `p1_db` | number (dB) | optional | 1.0 | The level exceeded by 99 % of the time, against the mean power. |
| `p10_db` | number (dB) | optional | 1.0 | The level at 10 %. |
| `p50_db` | number (dB) | optional | 1.0 | The median level. |
| `p90_db` | number (dB) | optional | 1.0 | The level at 90 %. |
| `p99_db` | number (dB) | optional | 1.0 | The level at 99 %. |
| `fade_depth_db` | number (dB) | optional | 1.0 | The median over the 1 % level. |
| `s4` | number | optional | 1.0 | The scintillation index S4. |
| `fade_rate_hz` | number (1/s) | optional | 1.0 | Downward crossings of the fade threshold per second. |
| `fade_length_ms` | number (ms) | optional | 1.0 | The average time under the fade threshold. |
| `doppler_hz` | number (Hz) | optional | 1.0 | The RMS Doppler spread. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | The carrier to noise density of the last block. |
| `carrier_offset_hz` | frequency | optional | 1.0 | The carrier from the tuned frequency. |
| `window_s` | duration (s) | optional | 1.0 | The envelope kept. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `FadingEventRow`. A row of the `fading` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | What happened. |
| `depth_db` | number (dB) | optional | 1.0 | The deepest level of a fade against the mean power. |
| `length_ms` | number (ms) | optional | 1.0 | The length of a fade. |
| `k_db` | number (dB) | optional | 1.0 | The K-factor. |
| `p10_db` | number (dB) | optional | 1.0 | The level at 10 %. |
| `fade_rate_hz` | number (1/s) | optional | 1.0 | The fade rate. |
| `fade_length_ms` | number (ms) | optional | 1.0 | The average fade length. |
| `doppler_hz` | number (Hz) | optional | 1.0 | The Doppler spread. |
| `cn0_dbhz` | number (dB-Hz) | optional | 1.0 | The carrier to noise density. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fading_clear` | none | none |
| `reset` | `sdrkit_fading_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `window_s` | `sdrkit_fading_get_window_s` | `sdrkit_fading_set_window_s` |
| `first_s` | `sdrkit_fading_get_first_s` | `sdrkit_fading_set_first_s` |
| `fade_db` | `sdrkit_fading_get_fade_db` | `sdrkit_fading_set_fade_db` |
| `log_depth_db` | `sdrkit_fading_get_log_depth_db` | `sdrkit_fading_set_log_depth_db` |
| `log_length_s` | `sdrkit_fading_get_log_length_s` | `sdrkit_fading_set_log_length_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_fd_hz` | `sdrkit_fading_gen_get_fd_hz` | `sdrkit_fading_gen_set_fd_hz` |
| `gen_k` | `sdrkit_fading_gen_get_k` | `sdrkit_fading_gen_set_k` |
| `gen_off` | `sdrkit_fading_gen_get_off` | `sdrkit_fading_gen_set_off` |

## Try it

```sh
sdrkit run fading --source synth:fading
sdrkit run fading --source <driver>:<address> --freq 0
```
<!-- generated: end -->
