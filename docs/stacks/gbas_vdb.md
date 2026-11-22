<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GBAS VDB

A detector of the GBAS VHF data broadcast: bursts, the slots busy of the 500 ms frame and the 10 500 symbol/s line. The message content is not decoded.

Stack id: `gbas_vdb`. Group: Aviation.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 113 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 250 kHz |
| Smallest sample rate | 62.5 kHz |
| Signal bandwidth | 25 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gbas_vdb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_slots` | Test signal slots | text | `1 3 5` |  | The slots (0 to 7) of the 500 ms frame that hold a burst, separated by spaces. |
| `gen_frames` | Test signal frames | whole number | 0 | 0 to 10000 | Frames with bursts, then silence; 0 sends without end. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `slots_busy` | Slots busy (of 8) | whole number |
| `symbol_rate_seen` | Symbol rate 10 500 /s seen | yes or no |
| `bursts` | Bursts | whole number |
| `length_ms` | Burst length | number (ms) |
| `snr_db` | SNR | number (dB) |
| `text` | Text | text |
| `content` | Content | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bursts` | Burst log | `time_s` (number (s)), `length_ms` (number (ms)), `slots_busy` (whole number), `snr_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avdata`, version 1.2. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Navigation aids (`navigation`). Media: none.

### Result fields

Struct `GbasVdbFields`. The result fields of GBAS VDB.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `slots_busy` | whole number | always | 1.0 | Slots of 8 that held a burst in the last frame. |
| `symbol_rate_seen` | yes or no | always | 1.0 | The line of the symbol rate was seen. |
| `bursts` | whole number | always | 1.0 | Bursts counted. |
| `length_ms` | number (ms) | optional | 1.0 | The length of the last burst. No value before a burst. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last burst. No value before a burst. |
| `text` | text | always | 1.0 | The summary line. |
| `content` | text | always | 1.0 | What the stack decodes. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bursts`

Struct `GbasVdbBurstRow`. A row of the burst log: every 16th burst. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the burst. |
| `length_ms` | number (ms) | always | 1.0 | The length of the burst. |
| `slots_busy` | whole number | always | 1.0 | Slots busy then. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the burst. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_gbas_vdb_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_slots` | `sdrkit_gbas_vdb_gen_get_slots` | `sdrkit_gbas_vdb_gen_set_slots` |
| `gen_frames` | `sdrkit_gbas_vdb_gen_get_frames` | `sdrkit_gbas_vdb_gen_set_frames` |

## Try it

```sh
sdrkit run gbas_vdb --source synth:gbas_vdb
sdrkit run gbas_vdb --source iiod:192.168.2.1 --freq 113
```
<!-- generated: end -->

## Description

A detector. It finds the bursts of a ground station on a 25 kHz channel by their power (a 1 ms power average over the noise floor), counts the slots of the 500 ms frame that they occupy and looks for the line of the 10 500 symbol/s rate in their power. The slots count from the first burst kept, not from the UTC frame, so they are shown as slots busy. The message content is not decoded.

Standard: ICAO Annex 10 Volume I, Appendix B 3.6; US patent 11 988 752 B2 (burst timing and fields).

## Settings: notes

Advanced settings. The owner works without any of them set.
