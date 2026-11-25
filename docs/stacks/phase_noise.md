<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Phase noise meter

L(f) of a carrier at 10 Hz to 100 kHz offsets, its spurs and the integrated phase jitter.

Stack id: `phase_noise`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 200 kHz |
| Signal bandwidth | 100 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:phase_noise` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `average_blocks` | Blocks averaged | whole number | 16 | 4 to 64 |  |
| `spur_ratio_db` | Spur over its neighbourhood | number (dB) | 9 | 3 to 20 |  |
| `jitter_lo_hz` | Jitter band, low edge | number (Hz) | 100 | 10 to 1000000 |  |
| `jitter_hi_hz` | Jitter band, high edge | number (Hz) | 100000 | 10 to 1000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_linewidth_hz` | Linewidth | number (Hz) | 20 | 0 to 10000 |  |
| `gen_floor_dbc` | White phase floor | number (dBc/Hz) | -125 | -200 to -40 |  |
| `gen_spur_hz` | Spur offset | number (Hz) | 0 | 0 to 1000000 |  |
| `gen_spur_dbc` | Spur level | number (dBc) | -62 | -150 to -10 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `l_10hz` | L(10 Hz) | number (dBc/Hz) |
| `l_100hz` | L(100 Hz) | number (dBc/Hz) |
| `l_1khz` | L(1 kHz) | number (dBc/Hz) |
| `l_10khz` | L(10 kHz) | number (dBc/Hz) |
| `l_100khz` | L(100 kHz) | number (dBc/Hz) |
| `jitter_mrad` | Integrated phase jitter | number (mrad) |
| `jitter_ps` | Integrated jitter as time | number (ps) |
| `carrier_offset_hz` | Carrier offset | frequency |
| `cnr_db` | Carrier over the noise in 200 kHz | number (dB) |
| `blocks` | Blocks averaged | whole number |
| `block_s` | Block length | duration (s) |
| `bin_hz` | Bin width | frequency |
| `enbw_bins` | Window ENBW | number (bins) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `spurs` | Spurs | `offset_hz` (frequency), `level_dbc` (number (dBc)) |
| `events` | Events | `time_s` (duration (s)), `kind` (choice), `l_10hz` (number (dBc/Hz)), `l_100hz` (number (dBc/Hz)), `l_1khz` (number (dBc/Hz)), `l_10khz` (number (dBc/Hz)), `l_100khz` (number (dBc/Hz)), `jitter_mrad` (number (mrad)), `spurs` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `l_100hz`, `l_1khz`, `l_10khz`, `l_100khz`, `jitter_mrad`.

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

Struct `PhaseNoiseFields`. The result fields of `phase_noise`, from the average. They have no value before 4 blocks are averaged.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `l_10hz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 10 Hz from the carrier. |
| `l_100hz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 100 Hz from the carrier. |
| `l_1khz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 1 kHz from the carrier. |
| `l_10khz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 10 kHz from the carrier. |
| `l_100khz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 100 kHz from the carrier. |
| `jitter_mrad` | number (mrad) | optional | 1.0 | The RMS phase jitter of the integration band. |
| `jitter_ps` | number (ps) | optional | 1.0 | The jitter as time at the tuned frequency; no value without it. |
| `carrier_offset_hz` | frequency | optional | 1.0 | The carrier offset from the tuned frequency. |
| `cnr_db` | number (dB) | optional | 1.0 | The carrier over the noise in 200 kHz. |
| `blocks` | whole number | always | 1.0 | The blocks in the average. |
| `block_s` | duration (s) | optional | 1.0 | The length of a block. |
| `bin_hz` | frequency | optional | 1.0 | The width of a bin. |
| `enbw_bins` | number (bins) | optional | 1.0 | The equivalent noise bandwidth of the window. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `spurs`

Struct `PhaseNoiseSpurRow`. A row of the spur table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | always | 1.0 | The offset of the spur from the carrier, upper side. |
| `level_dbc` | number (dBc) | always | 1.0 | The level of the spur. |

Default order: the column `level_dbc`, highest first.

### Table `events`

Struct `PhaseNoiseEventRow`. A row of the `phase_noise` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | The kind. |
| `l_10hz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 10 Hz from the carrier. |
| `l_100hz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 100 Hz from the carrier. |
| `l_1khz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 1 kHz from the carrier. |
| `l_10khz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 10 kHz from the carrier. |
| `l_100khz` | number (dBc/Hz) | optional | 1.0 | Single-sideband phase noise at 100 kHz from the carrier. |
| `jitter_mrad` | number (mrad) | optional | 1.0 | The jitter. |
| `spurs` | whole number | always | 1.0 | The spurs listed. |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_phase_noise_clear` | none | none |
| `reset` | `sdrkit_phase_noise_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `average_blocks` | `sdrkit_phase_noise_get_average_blocks` | `sdrkit_phase_noise_set_average_blocks` |
| `spur_ratio_db` | `sdrkit_phase_noise_get_spur_ratio_db` | `sdrkit_phase_noise_set_spur_ratio_db` |
| `jitter_lo_hz` | `sdrkit_phase_noise_get_jitter_lo_hz` | `sdrkit_phase_noise_set_jitter_lo_hz` |
| `jitter_hi_hz` | `sdrkit_phase_noise_get_jitter_hi_hz` | `sdrkit_phase_noise_set_jitter_hi_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_linewidth_hz` | `sdrkit_phase_noise_gen_get_linewidth_hz` | `sdrkit_phase_noise_gen_set_linewidth_hz` |
| `gen_floor_dbc` | `sdrkit_phase_noise_gen_get_floor_dbc` | `sdrkit_phase_noise_gen_set_floor_dbc` |
| `gen_spur_hz` | `sdrkit_phase_noise_gen_get_spur_hz` | `sdrkit_phase_noise_gen_set_spur_hz` |
| `gen_spur_dbc` | `sdrkit_phase_noise_gen_get_spur_dbc` | `sdrkit_phase_noise_gen_set_spur_dbc` |

## Try it

```sh
sdrkit run phase_noise --source synth:phase_noise
sdrkit run phase_noise --source <driver>:<address> --freq 0
```
<!-- generated: end -->
