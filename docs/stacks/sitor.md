<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SITOR

Maritime teleprinter: SITOR-B broadcasts (as used by NAVTEX), four-of-seven code with time diversity, and SITOR-A (AMTOR) ARQ links read by a monitor.

Stack id: `sitor`. Group: HF and utility.

Signal types: SITOR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 518 kHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 10 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sitor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_lower_hz` | Lower tone | frequency | 1.615 kHz | 100 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `text` | Text | text |
| `callsign` | Callsign | text |
| `offset_hz` | Offset | frequency |
| `shift_hz` | Shift | frequency |
| `symbol_rate_hz` | Symbol rate | frequency |
| `inverted` | Inverted | yes or no |
| `good_words` | Good words | whole number |
| `bad_words` | Bad words | whole number |
| `mutilated` | Mutilated | whole number |
| `characters` | Characters | whole number |
| `arq_locked` | ARQ locked | yes or no |
| `arq_blocks` | ARQ blocks | whole number |
| `arq_new_blocks` | ARQ new blocks | whole number |
| `arq_repeats` | ARQ repeats | whole number |
| `arq_bad` | ARQ bad slots | whole number |
| `arq_control_signals` | ARQ control signals | whole number |
| `arq_cs_offset_bits` | ARQ control signal offset | whole number (bits) |
| `arq_chars` | ARQ characters | whole number |
| `arq_text` | ARQ text | text |
| `selcall` | Selective call | text |
| `arq_changes` | ARQ changes of direction | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `heard` (whole number), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfkbd.spot` | Callsign heard | `callsign` (text), `source` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `family` (choice) |
| `hfkbd.text` | Text line | `source` (text), `text` (text), `snr_db` (number (dB)), `mode` (text), `family` (choice) |
| `sitor.arq` | SITOR-A link event | `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfkbd`, version 2.4. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `SitorFields`. The result fields of SITOR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The decoded text. |
| `callsign` | text | optional | 1.0 | The newest callsign of the text, if it has one. |
| `offset_hz` | frequency | always | 1.0 | The centre of the tones. |
| `shift_hz` | frequency | optional | 1.0 | The shift. It has no value before a signal is found. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The speed. It has no value before a signal is found. |
| `inverted` | yes or no | always | 1.0 | The tones are the other way round. |
| `good_words` | whole number | always | 1.0 | Words of a good code. |
| `bad_words` | whole number | always | 1.0 | Words of a bad code. |
| `mutilated` | whole number | always | 1.0 | Characters lost in both copies. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `arq_locked` | yes or no | always | 1.0 | The ARQ monitor follows a link. |
| `arq_blocks` | whole number | always | 1.0 | ARQ blocks read. |
| `arq_new_blocks` | whole number | always | 1.0 | ARQ blocks that were new. |
| `arq_repeats` | whole number | always | 1.0 | ARQ blocks sent again. |
| `arq_bad` | whole number | always | 1.0 | ARQ slots that did not read. |
| `arq_control_signals` | whole number | always | 1.0 | ARQ control signals read. |
| `arq_cs_offset_bits` | whole number (bits) | optional | 1.0 | Where the control signals lie. It has no value before one is found. |
| `arq_chars` | whole number | always | 1.0 | ARQ characters decoded. |
| `arq_text` | text | always | 1.0 | The ARQ text. |
| `selcall` | text | optional | 1.0 | The selective call of the link, if one was read. |
| `arq_changes` | whole number | always | 1.0 | The changes of direction of the link. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `stations`

Struct `StationRow`. A row of the stations heard. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `freq_hz` | frequency | always | 1.0 | Where it was last heard. |
| `snr_db` | number (dB) | always | 1.0 | Its SNR when last heard. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last time heard. |

### Event `hfkbd.spot`

Struct `SpotEvent`. The fields of a callsign heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `source` | text | always | 1.0 | The callsign. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `freq_hz` | frequency | always | 1.0 | Where it was heard. |
| `mode` | text | always | 1.0 | The stack that heard it ("RTTY"). |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `hfkbd.text`

Struct `LineEvent`. The fields of a finished line of decoded text.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The callsign after "DE", if the line has one. |
| `text` | text | always | 1.0 | The line. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `mode` | text | always | 1.0 | The stack that heard it. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `sitor.arq`

Struct `SitorArqEvent`. The fields of a SITOR-A link event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | What happened. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `remove` | `sdrkit_sitor_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_sitor_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen` | `sdrkit_sitor_get_listen` | `sdrkit_sitor_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_sitor_gen_get_message` | `sdrkit_sitor_gen_set_message` |
| `gen_lower_hz` | `sdrkit_sitor_gen_get_lower_hz` | `sdrkit_sitor_gen_set_lower_hz` |

## Try it

```sh
sdrkit run sitor --source synth:sitor
sdrkit run sitor --source <driver>:<address> --freq 0.518
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
