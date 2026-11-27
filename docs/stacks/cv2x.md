<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# C-V2X (structure)

Structure only: LTE sidelink (C-V2X) transmissions found by the SC-FDMA prefix and the 1 ms subframe, with the carrier offset and the busy subframes; nothing is decoded.

Stack id: `cv2x`. Group: Cellular and railway.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 5.915 GHz |
| Tuning range | 5.8 GHz to 6 GHz |
| Sample rate | 15.36 MHz |
| Smallest sample rate | 15.36 MHz |
| Signal bandwidth | 10 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cv2x` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel_mhz` | Channel width | choice | `10` | `10` (10 MHz), `20` (20 MHz) | 10 MHz (15.36 Msps) or 20 MHz (30.72 Msps). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_busy` | Test signal busy share | number (%) | 20 | 1 to 100 |  |
| `gen_guard` | Test signal guard symbol | yes or no | true |  |  |
| `gen_resource_blocks` | Test signal resource blocks | whole number | 50 | 1 to 100 | 50 in a 10 MHz channel; not confirmed by a public document read for this stack. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `detected` | Detected | yes or no |
| `symbol_us` | Symbol | number (us) |
| `prefix_us` | Prefix | number (us) |
| `slot_timing` | Slot timing | whole number (samples) |
| `correlation` | Prefix correlation | number |
| `offset_hz` | Offset | frequency |
| `busy_per_s` | Busy subframes | number (1/s) |
| `busy_share` | Busy share | number (%) |
| `guard_seen` | Guard symbol seen | yes or no |
| `frame` | Frame structure | text |
| `content` | Content | text |
| `text` | Text | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `detections` | Detections | `time_s` (number (s)), `offset_hz` (frequency), `correlation` (number), `slot_timing` (whole number (samples)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `cell`, version 1.9. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `Cv2xFields`. The result fields of `cv2x`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `detected` | yes or no | always | 1.0 | Whether the sidelink structure is there. |
| `symbol_us` | number (us) | always | 1.0 | The SC-FDMA symbol length of the channel width. |
| `prefix_us` | number (us) | always | 1.0 | The cyclic prefix length of the channel width. |
| `slot_timing` | whole number (samples) | optional | 1.0 | The slot timing of the detection. |
| `correlation` | number | optional | 1.0 | The prefix correlation of the detection. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the detection. |
| `busy_per_s` | number (1/s) | optional | 1.0 | Busy subframes a second. |
| `busy_share` | number (%) | optional | 1.0 | The share of busy subframes. |
| `guard_seen` | yes or no | optional | 1.0 | Whether the last symbol of the subframe is a guard. |
| `frame` | text | optional | 1.0 | The frame shape over the 1 ms subframe. |
| `content` | text | always | 1.0 | What the stack shows of the content. |
| `text` | text | always | 1.0 | The summary. |
| `note` | text | always | 1.0 | What no public document read for the stack confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `detections`

Struct `Cv2xDetectionRow`. A detection start of `cv2x`, newest first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `correlation` | number | always | 1.0 | The prefix correlation. |
| `slot_timing` | whole number (samples) | always | 1.0 | The slot timing. |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_cv2x_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel_mhz` | `sdrkit_cv2x_get_channel_mhz` | `sdrkit_cv2x_set_channel_mhz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_busy` | `sdrkit_cv2x_gen_get_busy` | `sdrkit_cv2x_gen_set_busy` |
| `gen_guard` | `sdrkit_cv2x_gen_get_guard` | `sdrkit_cv2x_gen_set_guard` |
| `gen_resource_blocks` | `sdrkit_cv2x_gen_get_resource_blocks` | `sdrkit_cv2x_gen_set_resource_blocks` |

## Try it

```sh
sdrkit run cv2x --source synth:cv2x
sdrkit run cv2x --source iiod:192.168.2.1 --freq 5915
```
<!-- generated: end -->

## Description

Structure only. At 15.36 Msps (10 MHz) the correlation of each sample with the one 1024 samples earlier is folded over the 0.5 ms slot, whose seven SC-FDMA symbols have prefixes of 80 and 72 samples. The slot start comes from the ends of the symbols, the carrier offset from the phase of the fold with the half turn of the half-subcarrier shift taken out. The power of each symbol of each 1 ms subframe gives the busy subframes and shows the empty last symbol (the guard) of a sidelink subframe, which an LTE uplink does not have. Nothing is decoded: not the PSCCH, the PSSCH, the PSBCH or the sidelink synchronisation signals.

Standard: 3GPP TS 36.211 clauses 5.6 and 9.

Not confirmed by a public document read for this stack: 50 resource blocks in a 10 MHz channel (the test signal setting `gen_resource_blocks`), the common timing of all transmitters, and the band of the 5.9 GHz ITS channels; the tuning range is a range check only. The thresholds of the slot fold, the busy subframes and the guard symbol are this stack's own.

## Settings: notes

Advanced settings. The owner works without any of them set.
