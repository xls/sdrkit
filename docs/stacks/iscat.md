<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ISCAT

Scatter: 42-FSK in 24-symbol frames with Costas sync and length symbols; the message folded over all frames.

Stack id: `iscat`. Group: Weak-signal digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 144 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 3.5 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `auto` | ISCAT (A or B) | 144 MHz |
| `a` | ISCAT-A | 144 MHz |
| `b` | ISCAT-B | 144 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:iscat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Variant | choice | `auto` | `auto` (Automatic), `a` (ISCAT-A), `b` (ISCAT-B) | Automatic tries ISCAT-A and ISCAT-B on each reading and keeps the stronger sync. |
| `tol_hz` | Frequency tolerance | frequency | 100 Hz | 25 Hz to 300 Hz | How far tone 0 may lie from its nominal frequency. |
| `demod` | Demodulator | choice | `usb` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) |  |
| `listen` | Listen | yes or no | false |  | Copies the 8 kHz audio to the speaker. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_text` | Test text | text | empty |  | The message of the test signal. Empty: the stack's own test text. |
| `gen_variant` | Test signal variant | choice | `a` | `a` (ISCAT-A), `b` (ISCAT-B) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `decodes_total` | Decodes | whole number |
| `note` | Note | text |
| `level_dbfs` | Level | number (dBFS) |
| `variant` | Variant | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `decodes` | Decodes | `utc` (time), `time_s` (number (s)), `snr_db` (number (dB)), `audio_hz` (frequency), `mode` (text), `message` (text), `variant` (text), `sync_ratio` (number), `frames` (whole number), `unclear` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wsjt.decode` | Decode | `mode` (text), `text` (text), `snr_db` (number (dB)), `audio_hz` (frequency), `callsign` (text), `source` (text), `family` (choice), `variant` (text), `sync_ratio` (number), `frames` (whole number), `unclear` (whole number) |
| `wsjt.spot` | Station heard | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `source` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `wsjt`, version 4.0. The C header `sdrkit/wsjt.h`, the registry record `registry/wsjt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `ScatterFields`. The result fields that each of `fsk441`, `jt6m`, `iscat` and `pi4` shows.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `decodes_total` | whole number | always | 1.0 | The decodes since the last clear. |
| `note` | text | always | 1.0 | What no public document confirms. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. These stacks do not measure it: it has no value. |

### Result fields

Struct `IscatFields`. The result fields that `iscat` adds.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `variant` | text | optional | 1.0 | The variant of the newest decode. No value before the first decode. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `decodes`

Struct `IscatDecodeRow`. A row of the decode table of `iscat`. The row key is the number of the decode.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The time of the start of the decode. |
| `time_s` | number (s) | always | 1.0 | The signal time of the start of the decode, from the first sample. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `mode` | text | always | 1.0 | The mode or its variant. |
| `message` | text | always | 1.0 | The decoded text. |
| `variant` | text | always | 1.0 | The variant of the decode. |
| `sync_ratio` | number | always | 1.0 | The sync ratio r. |
| `frames` | whole number | always | 1.0 | The whole frames used. |
| `unclear` | whole number | always | 1.0 | The characters shown as `?`. |

### Event `wsjt.decode`

Struct `IscatDecodeEvent`. The fields of a decode of `iscat`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | text | always | 1.0 | The mode or its variant. |
| `text` | text | always | 1.0 | The decoded text. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `callsign` | text | optional | 1.0 | The first callsign of the text. |
| `source` | text | optional | 1.0 | The first callsign of the text. |
| `family` | choice | always | 1.0 | The family of the source: a message. |
| `variant` | text | always | 1.0 | The variant of the decode. |
| `sync_ratio` | number | always | 1.0 | The sync ratio r. |
| `frames` | whole number | always | 1.0 | The whole frames used. |
| `unclear` | whole number | always | 1.0 | The characters shown as `?`. |

### Event `wsjt.spot`

Struct `SpotEvent`. The fields of a station heard, with the standard spot fields.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `freq_hz` | frequency | always | 1.0 | The radio frequency. |
| `mode` | text | always | 1.0 | The emission as spot networks name it. |
| `source` | text | always | 1.0 | The callsign. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_iscat_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_iscat_get_submode` | `sdrkit_iscat_set_submode` |
| `tol_hz` | `sdrkit_iscat_get_tol_hz` | `sdrkit_iscat_set_tol_hz` |
| `demod` | `sdrkit_iscat_get_demod` | `sdrkit_iscat_set_demod` |
| `listen` | `sdrkit_iscat_get_listen` | `sdrkit_iscat_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_text` | `sdrkit_iscat_gen_get_text` | `sdrkit_iscat_gen_set_text` |
| `gen_variant` | `sdrkit_iscat_gen_get_variant` | `sdrkit_iscat_gen_set_variant` |

## Try it

```sh
sdrkit run iscat --source synth:iscat
sdrkit run iscat --source synth:iscat --submode auto
sdrkit run iscat --source iiod:192.168.2.1 --freq 144
```
<!-- generated: end -->

## Description

Scatter: 42-FSK in frames of 24 symbols, four Costas sync symbols and two length symbols each, the message folded over all frames. ISCAT-A has 21.5 Hz tones from 1012 Hz, ISCAT-B 43.1 Hz tones from 560 Hz; the automatic variant tries both on each reading of up to 24 s, every 4 s. Not confirmed by a public document: the positions of tone 0 (47 and 13 spacings), the rule of the length symbols, the character set and the longest message (41 characters with the @ here).

## Tuning: notes

The stack holds no calling frequency: the usual dials are in `crates/stacks/sdrkit-stack-wsjt/data/scatter_dials.csv`, for the host to offer.

## Submodes: notes

Choose one with `--submode <id>`. A submode sets the `submode` setting; it never follows the frequency.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run iscat --source synth:iscat --seconds 12
sdrkit run iscat --source synth:iscat --submode b gen_variant=b
sdrkit run iscat --source iiod:192.168.2.1 --freq 144.14
```
