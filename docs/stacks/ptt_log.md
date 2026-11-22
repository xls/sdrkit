<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PTT and duty-cycle logger

The key-ups of one channel: start, length, level and gap; kerchunks, a stuck key and the duty cycle.

Stack id: `ptt_log`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 16 kHz |
| Signal bandwidth | 12 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ptt_log` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel_khz` | Channel width | number (kHz) | 12 | 3 to 30 | Twice the pass edge of the channel filter. |
| `on_db` | Keyed above (over the floor) | number (dB) | 6 | 2 to 30 |  |
| `off_db` | Released below (over the floor) | number (dB) | 3 | 2 to 30 |  |
| `hang_s` | Release after | number (s) | 0.3 | 0.05 to 5 |  |
| `kerchunk_s` | Kerchunk shorter than | number (s) | 0.5 | 0.1 to 5 |  |
| `stuck_s` | Stuck key after | number (s) | 180 | 30 to 3600 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_keyed` | Transmissions | text | `10, 14.2; 20, 22.5; 30, 31.0; 40.3, 40.6; 50, 60.5; 75, 76.2; 90, 98; 100, 101.5; 110, 111; 111.7, 113; 130, 135; 150, 150.35; 170, 171.3; 200, 215; 300, 303` |  | Transmissions separated by ';', each 'from s, to s'. |
| `gen_fades` | Fades | text | `12.0, 12.12; 55, 55.12; 94, 94.15; 132, 132.2` |  | Fades separated by ';', each 'from s, to s': the carrier is off. |
| `gen_tone_hz` | Carrier offset | number (Hz) | 0 | -1000000 to 1000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `keyed` | Keyed | yes or no |
| `keyed_s` | Keyed for | duration (s) |
| `floor_db` | Floor | number (dB) |
| `transmissions` | Transmissions | whole number |
| `kerchunks` | Kerchunks | whole number |
| `duty_pct` | Duty cycle | number (%) |
| `duty_10min_pct` | Duty cycle, 10 min | number (%) |
| `mean_s` | Mean length | duration (s) |
| `median_s` | Median length | duration (s) |
| `longest_s` | Longest | duration (s) |
| `per_hour` | Transmissions per hour | number (1/h) |
| `elapsed_s` | Time measured | duration (s) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `transmissions` | Transmissions | `number` (whole number), `kind` (choice), `start_s` (duration (s)), `length_s` (duration (s)), `over_floor_db` (number (dB)), `gap_s` (duration (s)), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ptt.transmission` | Transmission | `number` (whole number), `kind` (choice), `start_s` (duration (s)), `length_s` (duration (s)), `over_floor_db` (number (dB)), `gap_s` (duration (s)), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Learns the floor again and drops a transmission in progress; the table stays. |

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

Struct `PttLogFields`. The result fields of `ptt_log`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `keyed` | yes or no | always | 1.0 | The channel is keyed now. |
| `keyed_s` | duration (s) | optional | 1.0 | The length of the present transmission; no value while idle. |
| `floor_db` | number (dB) | optional | 1.0 | The floor of the channel, relative; no value before the first reading. |
| `transmissions` | whole number | always | 1.0 | The transmissions that ended. |
| `kerchunks` | whole number | always | 1.0 | Those shorter than the kerchunk length. |
| `duty_pct` | number (%) | optional | 1.0 | The keyed share of the time since the end of the warm-up, the present transmission included. |
| `duty_10min_pct` | number (%) | optional | 1.0 | The keyed share of the last 10 minutes (or of the time measured, when shorter). |
| `mean_s` | duration (s) | optional | 1.0 | The mean length of the transmissions that ended. |
| `median_s` | duration (s) | optional | 1.0 | The median length. |
| `longest_s` | duration (s) | optional | 1.0 | The longest. |
| `per_hour` | number (1/h) | optional | 1.0 | Transmissions per hour of the time measured. |
| `elapsed_s` | duration (s) | always | 1.0 | The time measured since the end of the warm-up. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `transmissions`

Struct `PttLogTransmissionRow`. A row of the transmission log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `number` | whole number | always | 1.0 | The number of the transmission, from 1. |
| `kind` | choice | always | 1.0 | The kind. |
| `start_s` | duration (s) | always | 1.0 | The start, signal time. |
| `length_s` | duration (s) | always | 1.0 | The length (so far, for a stuck key). |
| `over_floor_db` | number (dB) | always | 1.0 | The mean level over the floor of its keyed blocks. |
| `gap_s` | duration (s) | optional | 1.0 | The time from the end of the transmission before; no value for the first. |
| `level_db` | number (dB) | always | 1.0 | The mean level of its keyed blocks, relative to full scale. |

### Event `ptt.transmission`

Struct `PttTransmissionEvent`. A transmission ended, or a stuck key was found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `number` | whole number | always | 1.0 | The number of the transmission, from 1. |
| `kind` | choice | always | 1.0 | The kind. |
| `start_s` | duration (s) | always | 1.0 | The start, signal time. |
| `length_s` | duration (s) | always | 1.0 | The length (so far, for a stuck key). |
| `over_floor_db` | number (dB) | always | 1.0 | The mean level over the floor of its keyed blocks. |
| `gap_s` | duration (s) | optional | 1.0 | The time from the end of the transmission before; no value for the first. |
| `level_db` | number (dB) | always | 1.0 | The mean level of its keyed blocks, relative to full scale. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ptt_log_clear` | none | none |
| `reset` | `sdrkit_ptt_log_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel_khz` | `sdrkit_ptt_log_get_channel_khz` | `sdrkit_ptt_log_set_channel_khz` |
| `on_db` | `sdrkit_ptt_log_get_on_db` | `sdrkit_ptt_log_set_on_db` |
| `off_db` | `sdrkit_ptt_log_get_off_db` | `sdrkit_ptt_log_set_off_db` |
| `hang_s` | `sdrkit_ptt_log_get_hang_s` | `sdrkit_ptt_log_set_hang_s` |
| `kerchunk_s` | `sdrkit_ptt_log_get_kerchunk_s` | `sdrkit_ptt_log_set_kerchunk_s` |
| `stuck_s` | `sdrkit_ptt_log_get_stuck_s` | `sdrkit_ptt_log_set_stuck_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_keyed` | `sdrkit_ptt_log_gen_get_keyed` | `sdrkit_ptt_log_gen_set_keyed` |
| `gen_fades` | `sdrkit_ptt_log_gen_get_fades` | `sdrkit_ptt_log_gen_set_fades` |
| `gen_tone_hz` | `sdrkit_ptt_log_gen_get_tone_hz` | `sdrkit_ptt_log_gen_set_tone_hz` |

## Try it

```sh
sdrkit run ptt_log --source synth:ptt_log
sdrkit run ptt_log --source <driver>:<address> --freq 0
```
<!-- generated: end -->
