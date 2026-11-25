<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# AFSK tone tuner

Per packet burst of Bell 202 AFSK on FM: mark and space tones, twist, peak deviation, length and gap.

Stack id: `afsk_tuner`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 26 MHz to 6 GHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 16 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:afsk_tuner` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `mark_hz` | Mark tone | number (Hz) | 1200 | 300 to 3000 | 1200 Hz in Bell 202. |
| `space_hz` | Space tone | number (Hz) | 2200 | 300 to 3000 | 2200 Hz in Bell 202. |
| `on_ratio` | Burst on over the floor | number | 6 | 1.5 to 20 |  |
| `off_ratio` | Burst off over the floor | number | 3 | 1.5 to 20 |  |
| `min_burst_s` | Shortest burst analysed | number (s) | 0.1 | 0.05 to 2 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_deviation_hz` | Deviation | number (Hz) | 3000 | 0 to 10000 |  |
| `gen_twist_db` | Twist, space over mark | number (dB) | 0 | -20 to 20 |  |
| `gen_baud` | Bit rate | number (Bd) | 1200 | 50 to 9600 |  |
| `gen_period_s` | Burst period | number (s) | 2 | 0 to 600 |  |
| `gen_burst_s` | Burst length | number (s) | 0.5 | 0.01 to 600 |  |
| `gen_first_s` | First burst at | number (s) | 0.5 | 0 to 600 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `mark_hz` | Mark tone | frequency |
| `space_hz` | Space tone | frequency |
| `mark_error_hz` | Mark error | frequency |
| `space_error_hz` | Space error | frequency |
| `twist_db` | Twist, space over mark | number (dB) |
| `peak_deviation_khz` | Peak deviation | number (kHz) |
| `length_s` | Burst length | duration (s) |
| `gap_s` | Time since the burst before | duration (s) |
| `bursts` | Bursts analysed | whole number |
| `deviation_min_khz` | Least peak deviation | number (kHz) |
| `deviation_max_khz` | Largest peak deviation | number (kHz) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bursts` | Bursts | `time_s` (duration (s)), `length_s` (duration (s)), `mark_hz` (frequency), `space_hz` (frequency), `twist_db` (number (dB)), `peak_deviation_khz` (number (kHz)), `gap_s` (duration (s)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `afsk_tuner.burst` | Burst | `time_s` (duration (s)), `length_s` (duration (s)), `mark_hz` (frequency), `space_hz` (frequency), `twist_db` (number (dB)), `peak_deviation_khz` (number (kHz)), `gap_s` (duration (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `mark_hz`, `space_hz`, `twist_db`, `peak_deviation_khz`.

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

Struct `AfskTunerFields`. The result fields of `afsk_tuner`: the last analysed burst and the totals.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mark_hz` | frequency | optional | 1.0 | The mark tone of the last burst. |
| `space_hz` | frequency | optional | 1.0 | The space tone of the last burst. |
| `mark_error_hz` | frequency | optional | 1.0 | The mark tone minus the set mark tone. |
| `space_error_hz` | frequency | optional | 1.0 | The space tone minus the set space tone. |
| `twist_db` | number (dB) | optional | 1.0 | The twist of the last burst. |
| `peak_deviation_khz` | number (kHz) | optional | 1.0 | The peak deviation of the last burst. |
| `length_s` | duration (s) | optional | 1.0 | The length of the last burst. |
| `gap_s` | duration (s) | optional | 1.0 | The gap before the last burst. |
| `bursts` | whole number | always | 1.0 | The bursts analysed. |
| `deviation_min_khz` | number (kHz) | optional | 1.0 | The least peak deviation over the bursts. |
| `deviation_max_khz` | number (kHz) | optional | 1.0 | The largest peak deviation over the bursts. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bursts`

Struct `AfskTunerBurstRow`. A row of the burst table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The start of the burst, signal time. |
| `length_s` | duration (s) | always | 1.0 | The length of the burst. |
| `mark_hz` | frequency | always | 1.0 | The mark tone. |
| `space_hz` | frequency | always | 1.0 | The space tone. |
| `twist_db` | number (dB) | always | 1.0 | The space-band power over the mark-band power. |
| `peak_deviation_khz` | number (kHz) | always | 1.0 | The 99.5th percentile of the band-limited audio. |
| `gap_s` | duration (s) | optional | 1.0 | The time since the end of the burst before; no value for the first. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `afsk_tuner.burst`

Struct `AfskTunerBurstEvent`. A burst was analysed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The start of the burst, signal time. |
| `length_s` | duration (s) | always | 1.0 | The length of the burst. |
| `mark_hz` | frequency | always | 1.0 | The mark tone. |
| `space_hz` | frequency | always | 1.0 | The space tone. |
| `twist_db` | number (dB) | always | 1.0 | The space-band power over the mark-band power. |
| `peak_deviation_khz` | number (kHz) | always | 1.0 | The 99.5th percentile of the band-limited audio. |
| `gap_s` | duration (s) | optional | 1.0 | The time since the end of the burst before; no value for the first. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_afsk_tuner_clear` | none | none |
| `reset` | `sdrkit_afsk_tuner_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `mark_hz` | `sdrkit_afsk_tuner_get_mark_hz` | `sdrkit_afsk_tuner_set_mark_hz` |
| `space_hz` | `sdrkit_afsk_tuner_get_space_hz` | `sdrkit_afsk_tuner_set_space_hz` |
| `on_ratio` | `sdrkit_afsk_tuner_get_on_ratio` | `sdrkit_afsk_tuner_set_on_ratio` |
| `off_ratio` | `sdrkit_afsk_tuner_get_off_ratio` | `sdrkit_afsk_tuner_set_off_ratio` |
| `min_burst_s` | `sdrkit_afsk_tuner_get_min_burst_s` | `sdrkit_afsk_tuner_set_min_burst_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_deviation_hz` | `sdrkit_afsk_tuner_gen_get_deviation_hz` | `sdrkit_afsk_tuner_gen_set_deviation_hz` |
| `gen_twist_db` | `sdrkit_afsk_tuner_gen_get_twist_db` | `sdrkit_afsk_tuner_gen_set_twist_db` |
| `gen_baud` | `sdrkit_afsk_tuner_gen_get_baud` | `sdrkit_afsk_tuner_gen_set_baud` |
| `gen_period_s` | `sdrkit_afsk_tuner_gen_get_period_s` | `sdrkit_afsk_tuner_gen_set_period_s` |
| `gen_burst_s` | `sdrkit_afsk_tuner_gen_get_burst_s` | `sdrkit_afsk_tuner_gen_set_burst_s` |
| `gen_first_s` | `sdrkit_afsk_tuner_gen_get_first_s` | `sdrkit_afsk_tuner_gen_set_first_s` |

## Try it

```sh
sdrkit run afsk_tuner --source synth:afsk_tuner
sdrkit run afsk_tuner --source <driver>:<address> --freq 0
```
<!-- generated: end -->
