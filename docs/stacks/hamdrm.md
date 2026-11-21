<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HamDRM

Digital picture transfer between amateur stations on a DRM-style modem in an SSB channel.

Stack id: `hamdrm`. Group: Amateur digital.

Signal types: HamDRM.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 48 kHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 3 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hamdrm` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `hamdrm_format` | HamDRM format | text | empty |  | A JSON file with the parts of the HamDRM format that no document fixes. Without it nothing is decoded. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mode` | Mode | choice | `B` | `A`, `B`, `E` |  |
| `gen_occupancy` | Occupancy | choice | `2.5` | `2.3` (2.3 kHz), `2.5` (2.5 kHz) |  |
| `gen_qam` | QAM | choice | `16` | `4` (4-QAM), `16` (16-QAM), `64` (64-QAM) |  |
| `gen_protection` | Protection | choice | `0` | `0`, `1` | 4-QAM has protection 1 only. |
| `gen_interleave` | Interleaver | choice | `short` | `short` (400 ms), `long` (2 s) |  |
| `gen_skip_every` | Leave out every n-th body segment | whole number | 0 | 0 to 100 |  |
| `gen_transmissions` | Transmissions per round | whole number | 2 | 1 to 10 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `format` | HamDRM format | text |
| `mode` | Mode | choice |
| `occupancy` | Occupancy | frequency |
| `qam` | QAM | choice |
| `protection` | Protection | whole number |
| `interleave` | Interleaver | choice |
| `label` | Label | text |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `files_completed` | Files completed | whole number |
| `file_id` | File id | whole number |
| `file_name` | File name | text |
| `file_call` | Call sign | text |
| `file_size` | File size | whole number |
| `segments_total` | Segments | whole number |
| `segments_received` | Segments received | whole number |
| `transmissions` | Transmissions | whole number |
| `done` | Done | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `history` | Files | `id` (whole number), `name` (text), `call` (text), `mime` (text), `size` (whole number), `transmissions` (whole number), `completed` (duration (s)), `crc32` (whole number) |
| `missing` | Missing segments | `segment` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the signal and starts again. |

## Typed interface

The stack is in the family `broadcast`, version 1.0. The C header `sdrkit/broadcast.h`, the registry record `registry/broadcast.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `HamdrmFields`. The results of the HamDRM stack. No mode, occupancy, constellation, protection, interleaver, label, SNR or offset before the first frame, and no file before its first segment.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `format` | text | always | 1.0 | Whether the format is loaded, and from what (or why not). |
| `mode` | choice | optional | 1.0 | The robustness mode, A, B or E. |
| `occupancy` | frequency | optional | 1.0 | The occupancy. |
| `qam` | choice | optional | 1.0 | The constellation. |
| `protection` | whole number | optional | 1.0 | The protection level, 0 or 1. |
| `interleave` | choice | optional | 1.0 | The interleaver. |
| `label` | text | optional | 1.0 | The label of the FAC (the station's call sign). |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last frame. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame. |
| `frames_ok` | whole number | always | 1.0 | Frames good. |
| `frames_bad` | whole number | always | 1.0 | Frames bad. |
| `files_completed` | whole number | always | 1.0 | Files completed. |
| `file_id` | whole number | optional | 1.0 | The transport id of the file being received. |
| `file_name` | text | optional | 1.0 | Its name. |
| `file_call` | text | optional | 1.0 | Its call sign. |
| `file_size` | whole number | optional | 1.0 | Its body size. |
| `segments_total` | whole number | optional | 1.0 | Its segments in total, once the last segment number is known. |
| `segments_received` | whole number | optional | 1.0 | Its segments received. |
| `transmissions` | whole number | optional | 1.0 | The transmissions heard. |
| `done` | yes or no | optional | 1.0 | Whether it is complete. |

### Table `history`

Struct `HamdrmFileRow`. A finished file. The row key is its transport id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The transport id. |
| `name` | text | always | 1.0 | The name. |
| `call` | text | always | 1.0 | The call sign of the sender. |
| `mime` | text | always | 1.0 | The content type. |
| `size` | whole number | always | 1.0 | The body size. |
| `transmissions` | whole number | always | 1.0 | The transmissions heard. |
| `completed` | duration (s) | always | 1.0 | When it was complete, in signal time. |
| `crc32` | whole number | always | 1.0 | The CRC-32 of the body. |

### Table `missing`

Struct `HamdrmMissingRow`. A missing body segment of the file being received. The row key is the segment number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `segment` | whole number | always | 1.0 | The number of the segment. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hamdrm_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `hamdrm_format` | `sdrkit_hamdrm_get_hamdrm_format` | `sdrkit_hamdrm_set_hamdrm_format` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mode` | `sdrkit_hamdrm_gen_get_mode` | `sdrkit_hamdrm_gen_set_mode` |
| `gen_occupancy` | `sdrkit_hamdrm_gen_get_occupancy` | `sdrkit_hamdrm_gen_set_occupancy` |
| `gen_qam` | `sdrkit_hamdrm_gen_get_qam` | `sdrkit_hamdrm_gen_set_qam` |
| `gen_protection` | `sdrkit_hamdrm_gen_get_protection` | `sdrkit_hamdrm_gen_set_protection` |
| `gen_interleave` | `sdrkit_hamdrm_gen_get_interleave` | `sdrkit_hamdrm_gen_set_interleave` |
| `gen_skip_every` | `sdrkit_hamdrm_gen_get_skip_every` | `sdrkit_hamdrm_gen_set_skip_every` |
| `gen_transmissions` | `sdrkit_hamdrm_gen_get_transmissions` | `sdrkit_hamdrm_gen_set_transmissions` |

## Try it

```sh
sdrkit run hamdrm --source synth:hamdrm
sdrkit run hamdrm --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
