<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Reticulum (RNode)

Reticulum packets on RNode LoRa radios: packet type, destination and hops, and announces with the signature checked. Data stays encrypted.

Stack id: `reticulum`. Group: IoT and mesh.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 137 MHz to 2.5 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 250 kHz |
| Offset from the signal | 250 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:reticulum` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `auto_plan` | Search bandwidth and spreading factor | yes or no | true |  |  |
| `bw` | Bandwidth | choice | `125000` | `62500` (62.5 kHz), `125000` (125 kHz), `250000` (250 kHz), `500000` (500 kHz) |  |
| `sf` | Spreading factor | whole number | 8 | 5 to 12 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_split` | Test signal: a split packet | yes or no | true |  |  |
| `gen_tampered` | Test signal: an announce with a wrong signature | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `preambles` | Preambles | whole number |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `searching` | Searching | text |
| `destinations_heard` | Destinations | whole number |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `packets` | Packets | `utc` (time), `time_s` (number (s)), `destination` (text), `type` (choice), `content` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `encrypted` (yes or no), `hex` (bytes), `setting` (text) |
| `destinations` | Destinations | `destination` (text), `type` (choice), `content` (text), `packets` (whole number), `identity` (text), `app_text` (text), `first_heard` (time), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `reticulum.destination` | Reticulum destination | `source` (text), `text` (text), `family` (choice) |
| `reticulum.announce` | Reticulum announce | `source` (text), `identity` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `lora`, version 5.0. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Mesh messaging (`mesh`). Media: text.

### Result fields

Struct `RnsFields`. The result fields of the Reticulum stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that gave a packet. |
| `frames_bad` | whole number | always | 1.0 | Frames that gave no packet. |
| `preambles` | whole number | always | 1.0 | Preambles whose sync word followed. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last frame, while frames come. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the bank reports. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame, while frames come. |
| `searching` | text | always | 1.0 | The LoRa settings that the plan searches. |
| `destinations_heard` | whole number | always | 1.0 | The destinations in the table. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `packets`

Struct `RnsPacketRow`. A row of the Reticulum packet log. The row key is the place of the packet in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the packet, when known. |
| `time_s` | number (s) | always | 1.0 | The signal time of the packet. |
| `destination` | text | always | 1.0 | The destination hash as hex. |
| `type` | choice | always | 1.0 | The type of the packet. |
| `content` | text | always | 1.0 | The packet in one line. |
| `freq_hz` | frequency | always | 1.0 | The channel plus the carrier offset. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |
| `hex` | bytes | always | 1.0 | The first bytes of the packet. |
| `setting` | text | always | 1.0 | The LoRa setting of the frame. |

The rows are the messages of the stack. The column `destination` names a row. Addressee: `destination`. Text: `content`. Time: `utc`.

### Table `destinations`

Struct `RnsDestinationRow`. A row of the Reticulum destinations. The row key is the destination hash as hex.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `destination` | text | always | 1.0 | The destination hash as hex. |
| `type` | choice | always | 1.0 | The type of the last packet. |
| `content` | text | always | 1.0 | The last packet in one line. |
| `packets` | whole number | optional | 1.0 | The packets to the destination. |
| `identity` | text | always | 1.0 | The identity that an announce gave, as hex. |
| `app_text` | text | always | 1.0 | The text that an announce gave. |
| `first_heard` | time | optional | 1.0 | The UTC of the first packet. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet. |

The rows are the stations or nodes of the stack. The column `destination` names a row. Time: `first_heard`.

### Event `reticulum.destination`

Struct `RnsDestinationEvent`. The fields of a destination heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The destination hash as hex. |
| `text` | text | always | 1.0 | The line of the event. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `reticulum.announce`

Struct `RnsAnnounceEvent`. The fields of an announce.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The destination hash as hex. |
| `identity` | text | always | 1.0 | The identity as hex. |
| `text` | text | always | 1.0 | The announced text. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_reticulum_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `auto_plan` | `sdrkit_reticulum_get_auto_plan` | `sdrkit_reticulum_set_auto_plan` |
| `bw` | `sdrkit_reticulum_get_bw` | `sdrkit_reticulum_set_bw` |
| `sf` | `sdrkit_reticulum_get_sf` | `sdrkit_reticulum_set_sf` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_split` | `sdrkit_reticulum_gen_get_split` | `sdrkit_reticulum_gen_set_split` |
| `gen_tampered` | `sdrkit_reticulum_gen_get_tampered` | `sdrkit_reticulum_gen_set_tampered` |

## Try it

```sh
sdrkit run reticulum --source synth:reticulum
sdrkit run reticulum --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
