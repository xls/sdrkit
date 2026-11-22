<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Throb

Throb and ThrobX: one or two tones at a time out of 9 or 11, each pair a character; the variant is found from the signal.

Stack id: `throb`. Group: HF and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 14 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 3 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:throb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `variant` | Variant | choice | `auto` | `auto` (Automatic), `throb1` (Throb1), `throb2` (Throb2), `throb4` (Throb4), `throbx1` (ThrobX1), `throbx2` (ThrobX2), `throbx4` (ThrobX4) | Automatic finds the variant from the signal. Throb1 also reads Throb2, ThrobX1 also ThrobX2. |
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_variant` | Test signal variant | choice | `throb1` | `throb1` (Throb1), `throb2` (Throb2), `throb4` (Throb4), `throbx1` (ThrobX1), `throbx2` (ThrobX2), `throbx4` (ThrobX4) |  |
| `gen_centre_hz` | Audio centre | frequency | 1.5 kHz | 300 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `text` | Text | text |
| `callsign` | Callsign | text |
| `variant` | Variant | text |
| `centre_hz` | Audio centre | frequency |
| `score` | Score | number |
| `snr_db` | SNR | number (dB) |
| `characters` | Characters | whole number |
| `note` | Note | text |
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
| `throb.text` | Throb characters | `text` (text), `variant` (text), `centre_hz` (frequency), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfkbd`, version 2.6. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `ThrobFields`. The result fields of Throb. The fields of the reading have no value before the first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The decoded text. |
| `callsign` | text | optional | 1.0 | The newest callsign of the text, if it has one. |
| `variant` | text | optional | 1.0 | The variant found. |
| `centre_hz` | frequency | optional | 1.0 | The audio centre found. |
| `score` | number | optional | 1.0 | The score of the reading. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the reading. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `note` | text | always | 1.0 | A note on the stack. |

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

### Event `throb.text`

Struct `ThrobTextEvent`. The fields of the characters of one Throb reading.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The characters. |
| `variant` | text | always | 1.0 | The variant. |
| `centre_hz` | frequency | always | 1.0 | The audio centre. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `remove` | `sdrkit_throb_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_throb_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `variant` | `sdrkit_throb_get_variant` | `sdrkit_throb_set_variant` |
| `demod` | `sdrkit_throb_get_demod` | `sdrkit_throb_set_demod` |
| `listen` | `sdrkit_throb_get_listen` | `sdrkit_throb_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_throb_gen_get_message` | `sdrkit_throb_gen_set_message` |
| `gen_variant` | `sdrkit_throb_gen_get_variant` | `sdrkit_throb_gen_set_variant` |
| `gen_centre_hz` | `sdrkit_throb_gen_get_centre_hz` | `sdrkit_throb_gen_set_centre_hz` |

## Try it

```sh
sdrkit run throb --source synth:throb
sdrkit run throb --source <driver>:<address> --freq 14
```
<!-- generated: end -->

## Description

Throb and ThrobX: one or two tones at a time out of 9 (Throb, 8 or 16 Hz apart) or 11 (ThrobX, 7.8125 or 15.625 Hz apart), each pair a character, at 1, 2 or 4 symbols a second. Every 6 s the receiver reads the last 30 s: the tone comb in an averaged spectrum, the variant, the symbol timing and the pairs, and it reports the characters that are new. Not confirmed by a public document: both tables of tone pairs, the pulse shapes and the alternation of the ThrobX idle and space pairs.

## Tuning: notes

The stack holds no calling frequency: the usual dials are in `crates/stacks/sdrkit-stack-hfkbd/data/slow_dials.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run throb --source synth:throb --seconds 40
sdrkit run throb --source iiod:192.168.2.1 --freq 14.075
```
