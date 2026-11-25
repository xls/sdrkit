<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Sub-GHz chat (Flipper Zero)

Text chat with Flipper Zero devices: plain lines, and AES-256-GCM lines with the password or key of a chat you take part in.

Stack id: `subghzchat`. Group: Chat, 433.92 MHz.

The `subghzchat` stack: text chat with Flipper Zero devices on CC1101 GFSK packets, joined into lines. Encrypted lines open only with the key the user types for a chat the user takes part in.

Standard: TI CC1101 data sheet (packet engine, CRC); NIST SP 800-38D (AES-GCM); FIPS 180-4 (SHA-256).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 433.92 MHz |
| Tuning range | 300 MHz to 928 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 200 kHz |
| Signal bandwidth | 200 kHz |
| Offset from the signal | 300 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:subghzchat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `name` | Your chat name | text | `sdrkit` |  |  |
| `encryption` | Encryption | choice | `off` | `off` (Off), `password` (Password), `hex` (Hex key) |  |
| `password` | Password (kept in memory only) | text | empty |  |  |
| `key` | Hex key (kept in memory only) | text | empty |  | 64 hex digits. Spaces, ':' and '-' are ignored. |
| `frequency` | Chat frequency | choice | `433.92` | `433.92` (433.92 MHz), `315.00` (315.00 MHz), `390.00` (390.00 MHz), `868.35` (868.35 MHz), `915.00` (915.00 MHz) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `messages` | Messages | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `bursts` | Bursts | whole number |
| `decrypted` | Decrypted | whole number |
| `unreadable` | Not readable | whole number |
| `repeats` | Repeats dropped | whole number |
| `sent` | Sent | whole number |
| `key_set` | Key set | yes or no |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `chat` | Chat | `time_s` (number (s)), `own` (yes or no), `kind` (choice), `text` (text), `packets` (whole number), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `offset_hz` (frequency), `run` (text) |
| `packets` | Packets | `time_s` (number (s)), `length` (whole number (B)), `crc_ok` (yes or no), `inverted` (yes or no), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `offset_hz` (frequency), `hex` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `subghzchat.line` | Chat line | `text` (text), `family` (choice), `kind` (choice), `snr_db` (number (dB)) |
| `subghzchat.state` | Chat packets coming in | `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `ism`, version 4.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Keyboard text (`text_chat`). Media: text.

### Result fields

Struct `SubghzchatFields`. The result fields of the Sub-GHz chat stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `messages` | whole number | always | 1.0 | Lines heard. |
| `frames_ok` | whole number | always | 1.0 | Packets with a good CRC. |
| `frames_bad` | whole number | always | 1.0 | Packets whose CRC failed. |
| `bursts` | whole number | always | 1.0 | Bursts seen. |
| `decrypted` | whole number | always | 1.0 | Lines decrypted. |
| `unreadable` | whole number | always | 1.0 | Encrypted lines that the key does not open. |
| `repeats` | whole number | always | 1.0 | Encrypted repeats dropped. |
| `sent` | whole number | always | 1.0 | Lines the user sent. |
| `key_set` | yes or no | always | 1.0 | A key is set. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the packets. It has no value before the first good packet. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the last good packet. It has no value before the first good packet. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `chat`

Struct `SubghzchatLineRow`. A row of the chat. The row key is the number of the line in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the line. |
| `own` | yes or no | always | 1.0 | True for a line that this station sent. |
| `kind` | choice | always | 1.0 | How the line was read. |
| `text` | text | always | 1.0 | The text. |
| `packets` | whole number | always | 1.0 | The packets joined into the line. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the first packet of a line heard. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the first packet of a line heard. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the first packet of a line heard. |
| `run` | text | optional | 1.0 | The run id and counter of an encrypted line. |

The rows are the messages of the stack. Text: `text`. Time: `time_s`. Own message: `own`.

### Table `packets`

Struct `SubghzchatPacketRow`. A row of the packet log. The row key is the number of the packet in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the packet. |
| `length` | whole number (B) | always | 1.0 | The payload length. |
| `crc_ok` | yes or no | always | 1.0 | The CRC is right. |
| `inverted` | yes or no | always | 1.0 | The packet was found inverted. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the packet. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the packet. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset of the packet. |
| `hex` | text | always | 1.0 | The payload in hex. |

The column `hex` names a row.

### Event `subghzchat.line`

Struct `SubghzchatLineEvent`. The fields of a chat line event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `kind` | choice | always | 1.0 | How the line was read. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the first packet. |

### Event `subghzchat.state`

Struct `SubghzchatStateEvent`. The fields of the event that packets come in after a quiet time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_subghzchat_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `name` | `sdrkit_subghzchat_get_name` | `sdrkit_subghzchat_set_name` |
| `encryption` | `sdrkit_subghzchat_get_encryption` | `sdrkit_subghzchat_set_encryption` |
| `password` | `sdrkit_subghzchat_get_password` | `sdrkit_subghzchat_set_password` |
| `key` | `sdrkit_subghzchat_get_key` | `sdrkit_subghzchat_set_key` |
| `frequency` | `sdrkit_subghzchat_get_frequency` | `sdrkit_subghzchat_set_frequency` |

## Try it

```sh
sdrkit run subghzchat --source synth:subghzchat
sdrkit run subghzchat --source iiod:192.168.2.1 --freq 433.92
```
<!-- generated: end -->
