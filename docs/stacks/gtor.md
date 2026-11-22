<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# G-TOR monitor

G-TOR frames at 100, 200 or 300 Bd, plain or in the Golay form, checked by their CRC: plain text, and Huffman frames as hex.

Stack id: `gtor`. Group: HF utility.

G-TOR monitor: the frames of the amateur hybrid-ARQ protocol. 2-FSK at 100, 200 or 300 Bd; a frame of `baud x 0.24` bytes (24, 48 or 72) ends in a status byte and a CRC-16 (X.25: reflected 1021, start FFFF, complemented; high byte first). The frame is sent as 12-bit groups, bit-interleaved (bit 11 of every group, then bit 10, down to bit 0), plain or with each group replaced by its Golay (24,12) parity.

Standard: "G-TOR: A Hybrid ARQ Protocol for Narrow Bandwidth HF Data Communication", Kantronics, ARRL/TAPR Digital

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gtor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `baud` | Speed | choice | `auto` | `auto` (Automatic), `100` (100 Bd), `200` (200 Bd), `300` (300 Bd) |  |
| `shift_hz` | Shift | frequency | 0 Hz | 0 Hz to 1.2 kHz | 0 finds it from the signal. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_baud` | Speed | choice | `100` | `auto` (Automatic), `100` (100 Bd), `200` (200 Bd), `300` (300 Bd) | Automatic sends 100 Bd. |
| `gen_golay` | Golay form | yes or no | false |  |  |
| `gen_shift_hz` | Shift | frequency | 170 Hz | 60 Hz to 1.1 kHz |  |
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

## Events

| Kind | Label | Fields |
|---|---|---|
| `fsk.lock` | Sync | `text` (text), `baud` (number (Bd)), `shift_hz` (number (Hz)), `centre_hz` (number (Hz)), `detail` (text) |
| `fsk.lost` | Sync lost | `text` (text), `reason` (text) |
| `text` | Text | `text` (text) |
| `gtor.frame` | Frame | `text` (text), `baud` (number (Bd)), `command` (text), `block` (whole number), `form` (text), `inverted` (yes or no), `compression` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

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

### Event `gtor.frame`

Struct `GtorFrameEvent`. A frame of G-TOR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `baud` | number (Bd) | always | 1.0 | The speed of the frame. |
| `command` | text | always | 1.0 | The command of a frame: data, changeover, disconnect, connect. |
| `block` | whole number | always | 1.0 | The block number, 0 to 3. |
| `form` | text | always | 1.0 | "plain" or "Golay". |
| `inverted` | yes or no | always | 1.0 | The bits were inverted. |
| `compression` | text | always | 1.0 | The compression: none, Huffman, swapped Huffman. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_gtor_clear` | none | none |
| `reset` | `sdrkit_gtor_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `baud` | `sdrkit_gtor_get_baud` | `sdrkit_gtor_set_baud` |
| `shift_hz` | `sdrkit_gtor_get_shift_hz` | `sdrkit_gtor_set_shift_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_baud` | `sdrkit_gtor_gen_get_baud` | `sdrkit_gtor_gen_set_baud` |
| `gen_golay` | `sdrkit_gtor_gen_get_golay` | `sdrkit_gtor_gen_set_golay` |
| `gen_shift_hz` | `sdrkit_gtor_gen_get_shift_hz` | `sdrkit_gtor_gen_set_shift_hz` |
| `gen_centre_hz` | `sdrkit_gtor_gen_get_centre_hz` | `sdrkit_gtor_gen_set_centre_hz` |

## Try it

```sh
sdrkit run gtor --source synth:gtor
sdrkit run gtor --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->

## Description

The receiver takes the 24 kHz channel of the family front end, sums each three samples to 8000 samples/s and runs a 2-FSK front: the tone pair from a 1.024 s Hann spectrum, the speed from the eye opening at each candidate speed (else from the line of the transition times), the clock phase, a refinement of both tones in 6 Hz steps, and an integrate-and-dump slicer whose clock follows the signal slowly. The front searches again after 12 s without a speed, or after 3000 symbols without sync. A window of baud x 1.92 bits is de-interleaved into 12-bit groups (bit 11 of every group first) and tried in four forms: plain, Golay (each group mapped by the Golay (24,12) parity, its own inverse), inverted, inverted Golay. A form whose CRC-16 (X.25) matches is a frame; else the window slides by one bit. Windows with many soft decisions near zero are not tried, so noise gives no frames. Plain data frames give their text up to the first fill byte 1E; connect and disconnect frames give their call field raw.

Standard: "G-TOR: A Hybrid ARQ Protocol for Narrow Bandwidth HF Data Communication", Kantronics, ARRL/TAPR Digital Communications Conference 1994 (a conference paper). The shift, the 1C escape, the call field coding and the Huffman table are not confirmed by a public document.

## Tuning: notes

The stack holds no frequency: a usual starting dial is in `crates/stacks/sdrkit-stack-hfdata/data/hf_teleprinter_dials.csv` and band presets are in `crates/stacks/sdrkit-stack-hfdata/data/hf_band_presets.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run gtor --source synth:gtor
sdrkit run gtor --source <driver>:<address> --freq 14
```
