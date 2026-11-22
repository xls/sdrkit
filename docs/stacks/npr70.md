<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# New Packet Radio

A detector of New Packet Radio 4GFSK IP modems on 70 cm: sync word, network id, symbol rate and tuning offset of each frame; the frame content is not read.

Stack id: `npr70`. Group: Amateur digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 437 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 4 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:npr70` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `rates` | Symbol rates searched | text (ksym/s) | `all` |  | "all" searches every rate that fits the sample rate; else a list such as "100,500" (50, 100, 180, 300, 500). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `sync_words` | Sync words | whole number |
| `rates_searched` | Rates searched | text (ksym/s) |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `network` (text), `symbol_rate_hz` (frequency), `sync` (text), `tuning_hz` (number (Hz)), `payload` (text) |
| `networks` | Networks | `network` (text), `frames` (whole number), `symbol_rate_hz` (frequency), `first_heard` (time), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `npr70.frame` | New Packet Radio frame | `network` (text), `symbol_rate_hz` (frequency), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `ism`, version 2.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `NprFields`. The result fields of the New Packet Radio stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sync_words` | whole number | always | 1.0 | Sync words found. |
| `rates_searched` | text (ksym/s) | always | 1.0 | The symbol rates searched, in ksym/s. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. It has no value before the first input. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `frames`

Struct `NprFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the frame, when the samples are live. |
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `network` | text | always | 1.0 | The network. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `sync` | text | always | 1.0 | The sync word. |
| `tuning_hz` | number (Hz) | always | 1.0 | The tuning offset of the frame. |
| `payload` | text | always | 1.0 | What the payload is. |

### Table `networks`

Struct `NprNetworkRow`. A row of the networks heard. The row key is the network number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `network` | text | always | 1.0 | The network. |
| `frames` | whole number | always | 1.0 | The frames heard. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

### Event `npr70.frame`

Struct `NprFrameEvent`. The fields of a frame event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `network` | text | always | 1.0 | The network. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_npr70_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `rates` | `sdrkit_npr70_get_rates` | `sdrkit_npr70_set_rates` |

## Try it

```sh
sdrkit run npr70 --source synth:npr70
sdrkit run npr70 --source iiod:192.168.2.1 --freq 437
```
<!-- generated: end -->
