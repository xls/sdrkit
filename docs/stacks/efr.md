<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# EFR teleswitch

Long-wave load-control telegrams: 2-FSK at 200 Bd, 11-bit characters, FT1.2 frames with a checksum; the message number, two addresses and the payload as hex.

Stack id: `efr`. Group: HF utility.

EFR teleswitch: the long-wave load-control telegrams of the EFR transmitters. 2-FSK at 200 Bd with a 340 Hz shift; 11-bit characters (start 0, eight data bits least significant first, even parity, stop 1) carry FT1.2 frames of variable length (`68 L L 68`, the user bytes, the checksum, `16`). The first three user bytes are read as the message number and two addresses; the payload is shown as hex.

Standard: the FT1.2 frame of IEC 60870-5-1 and IEC 60870-5-2 (DIN 19244); those documents are sold, so no part is confirmed

## Tuning

| Property | Value |
|---|---|
| Default frequency | 6.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:efr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `shift_hz` | Shift | frequency | 0 Hz | 0 Hz to 600 Hz | 0 takes 340 Hz. |
| `polarity` | Polarity | choice | `auto` | `auto` (Automatic), `normal` (Normal), `inverted` (Inverted) |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `baud` | Speed | number (Bd) | 200 | 50 to 600 | Not confirmed by a public document. |
| `max_len` | Longest frame | whole number (bytes) | 40 | 3 to 255 | Not confirmed by a public document. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre_hz` | Centre | frequency | 1.5 kHz | 400 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `state` | State | whole number |
| `baud` | Speed | number (Bd) |
| `centre_hz` | Centre | number (Hz) |
| `shift_hz` | Shift | number (Hz) |
| `detail` | Detail | text |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `telegrams` | Telegrams | `time` (time), `checksum` (text), `message` (whole number), `address1` (whole number), `address2` (whole number), `payload_bytes` (whole number), `payload` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fsk.lock` | Sync | `text` (text), `baud` (number (Bd)), `shift_hz` (number (Hz)), `centre_hz` (number (Hz)), `detail` (text) |
| `fsk.lost` | Sync lost | `text` (text), `reason` (text) |
| `text` | Text | `text` (text) |
| `efr.telegram` | Telegram | `text` (text), `checksum` (text), `message` (whole number), `address1` (whole number), `address2` (whole number), `payload_bytes` (whole number), `payload` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `TeleprinterFields`. The result fields of a teleprinter stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `text` | text | always | 1.0 | The text received, the last characters. |
| `state` | whole number | always | 1.0 | The search state: 0 searching, 1 signal found, 2 decoding. |
| `baud` | number (Bd) | optional | 1.0 | The speed of the signal. Absent while the front has no lock. |
| `centre_hz` | number (Hz) | optional | 1.0 | The centre of the signal above the dial. Absent while the front has no lock. |
| `shift_hz` | number (Hz) | optional | 1.0 | The shift between the tones. Absent while the front has no lock or the signal has no shift. |
| `detail` | text | always | 1.0 | What was found beside the speed (cycle, depth, tone length). Empty while not decoding. |
| `good` | whole number | always | 1.0 | Words, characters or frames read. |
| `bad` | whole number | always | 1.0 | Words, characters or frames that failed their check. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel, dBFS. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `telegrams`

Struct `EfrTelegramRow`. A telegram of EFR. The row key is the number of the telegram.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | always | 1.0 | The time a telegram ended. |
| `checksum` | text | always | 1.0 | The checksum: "good" or "bad". |
| `message` | whole number | always | 1.0 | The message number (user byte 1). |
| `address1` | whole number | always | 1.0 | Address 1 (user byte 2). |
| `address2` | whole number | always | 1.0 | Address 2 (user byte 3). |
| `payload_bytes` | whole number | always | 1.0 | The number of payload bytes. |
| `payload` | text | always | 1.0 | The payload as hex. |

### Event `fsk.lock`

Struct `FskLockEvent`. A lock of the words, characters or frames.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `baud` | number (Bd) | always | 1.0 | The speed of the signal. |
| `shift_hz` | number (Hz) | always | 1.0 | The shift between the tones. |
| `centre_hz` | number (Hz) | always | 1.0 | The centre of the signal above the dial. |
| `detail` | text | always | 1.0 | What was found beside the speed. |

### Event `fsk.lost`

Struct `FskLostEvent`. The sync of the words, characters or frames was lost.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `reason` | text | always | 1.0 | Why sync was lost. |

### Event `text`

Struct `TextEvent`. New text: a line, 64 characters, or what came before a pause.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text. |

### Event `efr.telegram`

Struct `EfrTelegramEvent`. A telegram of EFR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `checksum` | text | always | 1.0 | The checksum: "good" or "bad". |
| `message` | whole number | always | 1.0 | The message number (user byte 1). |
| `address1` | whole number | always | 1.0 | Address 1 (user byte 2). |
| `address2` | whole number | always | 1.0 | Address 2 (user byte 3). |
| `payload_bytes` | whole number | always | 1.0 | The number of payload bytes. |
| `payload` | text | always | 1.0 | The payload as hex. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_efr_clear` | none | none |
| `reset` | `sdrkit_efr_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `shift_hz` | `sdrkit_efr_get_shift_hz` | `sdrkit_efr_set_shift_hz` |
| `polarity` | `sdrkit_efr_get_polarity` | `sdrkit_efr_set_polarity` |
| `baud` | `sdrkit_efr_get_baud` | `sdrkit_efr_set_baud` |
| `max_len` | `sdrkit_efr_get_max_len` | `sdrkit_efr_set_max_len` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre_hz` | `sdrkit_efr_gen_get_centre_hz` | `sdrkit_efr_gen_set_centre_hz` |

## Try it

```sh
sdrkit run efr --source synth:efr
sdrkit run efr --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The receiver takes the 24 kHz channel of the family front end, sums each three samples to 8000 samples/s and runs a 2-FSK front: the tone pair from a 1.024 s Hann spectrum, the speed from the eye opening at each candidate speed (else from the line of the transition times), the clock phase, a refinement of both tones in 6 Hz steps, and an integrate-and-dump slicer whose clock follows the signal slowly. The front searches again after 12 s without a speed, or after 3000 symbols without sync. The speed is 200 Bd and the shift 340 Hz (both settings). The character lock tries both polarities and the 11 offsets over nine characters and needs nine of ten with start 0 and stop 1; a wrong start or stop bit raises an error integrator (lost above 3), a parity error clears the byte buffer. Good bytes go to the FT1.2 reader (68 L L 68, the user bytes, the checksum, 16). The first three user bytes are shown as the message number and two addresses, the rest as hex.

Standard: The FT1.2 frame of IEC 60870-5-1 and IEC 60870-5-2 (DIN 19244), which are sold: nothing is confirmed by a public document. The payloads of the load-control systems are not read.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv` and the transmitters are in `crates/stacks/sdrkit-stack-hfdata/data/efr_transmitters.csv`, for the host to offer.

## Try it: notes

```sh
sdrkit run efr --source synth:efr
sdrkit run efr --source <driver>:<address> --freq 0.1276
```
