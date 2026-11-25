<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DSRC road toll

The 5.8 GHz downlink of road toll beacons, structure only: HDLC frames counted as beacon broadcasts or private link frames; no identifier or content is shown.

Stack id: `dsrc_toll`. Group: Road toll, 5.8 GHz.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 5.8025 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 4 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dsrc_toll` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bit_rate` | Bit rate | frequency | 500 kHz | 100 kHz to 1 MHz | The downlink bit rate. 500 kbit/s is the usual value; no public document was read to confirm it. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_depth` | Test signal keying depth | number | 1 | 0.3 to 1 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `broadcast_frames` | Broadcast frames | whole number |
| `private_frames` | Private link frames | whole number |
| `frames_bad` | Frames bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `kind` (text), `bytes` (whole number), `control` (text), `identifier` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `dsrc_toll.frame` | DSRC frame | `kind` (text), `bytes` (whole number), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `ism`, version 4.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `DsrcFields`. The result fields of the DSRC road toll stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames with a good check. |
| `broadcast_frames` | whole number | always | 1.0 | Broadcast frames. |
| `private_frames` | whole number | always | 1.0 | Private link frames. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. It has no value before the first input. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `frames`

Struct `DsrcFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `kind` | text | always | 1.0 | The class of the frame. |
| `bytes` | whole number | always | 1.0 | The bytes of the frame, without the check. |
| `control` | text | optional | 1.0 | The control byte as hex, if the frame has one. |
| `identifier` | text | always | 1.0 | What is not shown. |

The column `identifier` names a row.

### Event `dsrc_toll.frame`

Struct `DsrcFrameEvent`. The fields of a frame event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | text | always | 1.0 | The class of the frame. |
| `bytes` | whole number | always | 1.0 | The bytes of the frame, without the check. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dsrc_toll_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bit_rate` | `sdrkit_dsrc_toll_get_bit_rate` | `sdrkit_dsrc_toll_set_bit_rate` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_depth` | `sdrkit_dsrc_toll_gen_get_depth` | `sdrkit_dsrc_toll_gen_set_depth` |

## Try it

```sh
sdrkit run dsrc_toll --source synth:dsrc_toll
sdrkit run dsrc_toll --source iiod:192.168.2.1 --freq 5802.5
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
