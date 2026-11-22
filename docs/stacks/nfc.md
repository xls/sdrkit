<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NFC / RFID 13.56 MHz

Contactless cards and tags at 13.56 MHz: the tag type and the UID broadcast during anticollision and inventory, and the name of every frame.

Stack id: `nfc`. Group: Contactless, 13.56 MHz.

NFC and RFID at 13.56 MHz : the tag type and the UID broadcast during anticollision and inventory, and the name of every frame. Memory contents, NDEF records and APDU payloads are never shown, and a MIFARE Classic session after its authentication request is "encrypted, not decoded". Nothing is transmitted, cloned or emulated.

Standard: ISO/IEC 14443-2 and -3, ISO/IEC 15693-2 and -3.

Signal types: NFC.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 13.56 MHz |
| Tuning range | 13.553 MHz to 13.567 MHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 2.4 MHz |
| Automatic gain | Radio AGC |

## Settings

The stack has no settings.

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames` | Frames | whole number |
| `tags` | Tags | whole number |
| `failures` | Failures | whole number |
| `tag_frames` | Tag frames | whole number |
| `field_dbfs` | Field level | number (dBFS) |
| `subcarrier_dbfs` | Subcarrier level | number (dBFS) |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `tags` | Tags | `uid` (text), `type` (text), `hint` (text), `atqa` (text), `sak` (text), `historical` (text), `app_data` (text), `system` (text), `heard` (whole number), `last_heard` (time) |
| `frames` | Frames | `time_s` (number (s)), `direction` (text), `link` (text), `hex` (text), `name` (text), `crc` (text), `parity_ok` (yes or no), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `nfc.tag` | NFC tag | `source` (text), `text` (text), `family` (choice), `uid` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `ism`, version 1.1. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `NfcFields`. The result fields of the tag table of the NFC stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames` | whole number | always | 1.0 | Frames heard. |
| `tags` | whole number | always | 1.0 | Tags identified. |
| `failures` | whole number | always | 1.0 | CRC or parity failures outside encrypted sessions. |
| `tag_frames` | whole number | always | 1.0 | Frames from tags. |

### Result fields

Struct `NfcDetectFields`. The levels of the NFC detector. They have no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `field_dbfs` | number (dBFS) | optional | 1.0 | The field level of the input. |
| `subcarrier_dbfs` | number (dBFS) | optional | 1.0 | The level of the stronger subcarrier detector. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `tags`

Struct `NfcTagRow`. A row of the tags. The row key is the UID as hex.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `uid` | text | always | 1.0 | The UID as hex, most significant first. |
| `type` | text | always | 1.0 | The type of the tag. |
| `hint` | text | optional | 1.0 | The card-type hint. |
| `atqa` | text | optional | 1.0 | The ATQA. |
| `sak` | text | optional | 1.0 | The SAK. |
| `historical` | text | optional | 1.0 | The ATS historical bytes. |
| `app_data` | text | optional | 1.0 | The PUPI application data. |
| `system` | text | optional | 1.0 | The DSFID, AFI, block count and size and IC version byte of an ISO 15693 tag. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

### Table `frames`

Struct `NfcFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `direction` | text | always | 1.0 | "reader" or "tag". |
| `link` | text | always | 1.0 | The link. |
| `hex` | text | always | 1.0 | The bytes in hex, or nothing for content not shown. |
| `name` | text | always | 1.0 | The name of the frame. |
| `crc` | text | always | 1.0 | The CRC result. |
| `parity_ok` | yes or no | always | 1.0 | The parity result. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |

### Event `nfc.tag`

Struct `NfcTagEvent`. The fields of a tag event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The UID as hex. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `uid` | text | always | 1.0 | The UID as hex. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_nfc_clear` | none | none |
| `remove` | `sdrkit_nfc_remove` | `sdrkit_nfc_tag_args` | none |

## Try it

```sh
sdrkit run nfc --source synth:nfc
sdrkit run nfc --source <driver>:<address> --freq 13.56
```
<!-- generated: end -->
