<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Modulation quality meter

Of one M-PSK carrier: order, symbol rate, carrier offset, MER and EVM, and the best roll-off.

Stack id: `mer`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 90 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mer` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `block_s` | Block length | number (s) | 2 | 0.5 to 10 |  |
| `min_baud` | Lowest symbol rate | number (Bd) | 300 | 50 to 10000 |  |
| `min_mer_db` | Lowest MER shown | number (dB) | 4 | 0 to 20 |  |
| `rolloffs` | Roll-offs tried | text | `0.2, 0.35, 0.5` |  | Roll-offs separated by ',', each 0.1 to 1.0. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_psk` | Phases | whole number | 4 | 2 to 8 |  |
| `gen_symbol_rate` | Symbol rate | number (Bd) | 9600 | 10 to 1000000 |  |
| `gen_rolloff` | Roll-off | number | 0.35 | 0.05 to 1 |  |
| `gen_rotated` | Points turned by pi / M | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `locked` | PSK carrier found | yes or no |
| `modulation` | Order | choice |
| `mer_db` | MER | number (dB) |
| `evm_pct` | EVM | number (%) |
| `symbol_rate_bd` | Symbol rate | number (Bd) |
| `carrier_offset_hz` | Carrier offset | frequency |
| `rolloff` | Best roll-off | number |
| `symbols` | Symbols | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Readings | `time_s` (duration (s)), `modulation` (choice), `mer_db` (number (dB)), `evm_pct` (number (%)), `symbol_rate_bd` (number (Bd)), `carrier_offset_hz` (frequency), `level_db` (number (dB)) |

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

Struct `MerFields`. The result fields of `mer`, from the last block. The readings have no value without a lock.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | optional | 1.0 | A PSK carrier was found and measured in the last block. |
| `modulation` | choice | optional | 1.0 | The order. |
| `mer_db` | number (dB) | optional | 1.0 | The modulation error ratio after matched filtering, timing and phase recovery. |
| `evm_pct` | number (%) | optional | 1.0 | The RMS error vector, of the RMS amplitude. |
| `symbol_rate_bd` | number (Bd) | optional | 1.0 | The symbol rate. |
| `carrier_offset_hz` | frequency | optional | 1.0 | The carrier offset from the tuned frequency. |
| `rolloff` | number | optional | 1.0 | The roll-off of the best matched filter. |
| `symbols` | whole number | optional | 1.0 | The symbols measured in the last block. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `MerEventRow`. A row of the `mer` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `modulation` | choice | always | 1.0 | The order. |
| `mer_db` | number (dB) | always | 1.0 | MER. |
| `evm_pct` | number (%) | always | 1.0 | EVM. |
| `symbol_rate_bd` | number (Bd) | always | 1.0 | The symbol rate. |
| `carrier_offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `level_db` | number (dB) | always | 1.0 | The mean power of the block, relative to full scale. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_mer_clear` | none | none |
| `reset` | `sdrkit_mer_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `block_s` | `sdrkit_mer_get_block_s` | `sdrkit_mer_set_block_s` |
| `min_baud` | `sdrkit_mer_get_min_baud` | `sdrkit_mer_set_min_baud` |
| `min_mer_db` | `sdrkit_mer_get_min_mer_db` | `sdrkit_mer_set_min_mer_db` |
| `rolloffs` | `sdrkit_mer_get_rolloffs` | `sdrkit_mer_set_rolloffs` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_psk` | `sdrkit_mer_gen_get_psk` | `sdrkit_mer_gen_set_psk` |
| `gen_symbol_rate` | `sdrkit_mer_gen_get_symbol_rate` | `sdrkit_mer_gen_set_symbol_rate` |
| `gen_rolloff` | `sdrkit_mer_gen_get_rolloff` | `sdrkit_mer_gen_set_rolloff` |
| `gen_rotated` | `sdrkit_mer_gen_get_rotated` | `sdrkit_mer_gen_set_rotated` |

## Try it

```sh
sdrkit run mer --source synth:mer
sdrkit run mer --source <driver>:<address> --freq 0
```
<!-- generated: end -->
