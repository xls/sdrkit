<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# JT6M

6 m scatter: 44-FSK at 21.5 baud with a sync tone every third symbol; pings read, a long signal folded into its message.

Stack id: `jt6m`. Group: Weak-signal digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 50 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 3.5 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:jt6m` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tol_hz` | Frequency tolerance | frequency | 200 Hz | 50 Hz to 400 Hz | How far the sync tone may lie from 1076.66 Hz. |
| `demod` | Demodulator | choice | `usb` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) |  |
| `listen` | Listen | yes or no | false |  | Copies the 8 kHz audio to the speaker. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_text` | Test text | text | empty |  | The message of the test signal. Empty: the stack's own test text. |
| `gen_continuous` | Continuous test signal | yes or no | false |  | Off: pings of 0.8, 1.6 and 0.6 s in a loop of 24 s. On: the message without a break. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `decodes_total` | Decodes | whole number |
| `note` | Note | text |
| `level_dbfs` | Level | number (dBFS) |
| `folded` | Folded message | text |
| `fold_period` | Fold period | whole number |
| `tuning_hz` | Tuning | number (Hz) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `decodes` | Pings | `utc` (time), `time_s` (number (s)), `snr_db` (number (dB)), `audio_hz` (frequency), `mode` (text), `message` (text), `duration_s` (number (s)), `clear_frames` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wsjt.decode` | Ping | `mode` (text), `text` (text), `snr_db` (number (dB)), `audio_hz` (frequency), `callsign` (text), `source` (text), `family` (choice), `duration_s` (number (s)), `clear_frames` (whole number) |
| `wsjt.spot` | Station heard | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `source` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `wsjt`, version 2.0. The C header `sdrkit/wsjt.h`, the registry record `registry/wsjt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `ScatterFields`. The result fields that each of `fsk441`, `jt6m`, `iscat` and `pi4` shows.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `decodes_total` | whole number | always | 1.0 | The decodes since the last clear. |
| `note` | text | always | 1.0 | What no public document confirms. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. These stacks do not measure it: it has no value. |

### Result fields

Struct `Jt6mFields`. The result fields that `jt6m` adds. No field has a value before the first ping.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `folded` | text | optional | 1.0 | The folded message of the newest window that gave one. |
| `fold_period` | whole number | optional | 1.0 | The period of the folded message, in characters. |
| `tuning_hz` | number (Hz) | optional | 1.0 | The offset of the sync tone of the newest ping. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `decodes`

Struct `Jt6mDecodeRow`. A row of the ping table of `jt6m`. The row key is the number of the ping.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The time of the start of the decode. |
| `time_s` | number (s) | always | 1.0 | The signal time of the start of the decode, from the first sample. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `mode` | text | always | 1.0 | The mode or its variant. |
| `message` | text | always | 1.0 | The decoded text. |
| `duration_s` | number (s) | always | 1.0 | The length of the ping. |
| `clear_frames` | whole number | always | 1.0 | The clear frames of the ping. |

### Event `wsjt.decode`

Struct `Jt6mDecodeEvent`. The fields of a ping of `jt6m`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | text | always | 1.0 | The mode or its variant. |
| `text` | text | always | 1.0 | The decoded text. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `callsign` | text | optional | 1.0 | The first callsign of the text. |
| `source` | text | optional | 1.0 | The first callsign of the text. |
| `family` | choice | always | 1.0 | The family of the source: a message. |
| `duration_s` | number (s) | always | 1.0 | The length of the ping. |
| `clear_frames` | whole number | always | 1.0 | The clear frames of the ping. |

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
| `clear` | `sdrkit_jt6m_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `tol_hz` | `sdrkit_jt6m_get_tol_hz` | `sdrkit_jt6m_set_tol_hz` |
| `demod` | `sdrkit_jt6m_get_demod` | `sdrkit_jt6m_set_demod` |
| `listen` | `sdrkit_jt6m_get_listen` | `sdrkit_jt6m_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_text` | `sdrkit_jt6m_gen_get_text` | `sdrkit_jt6m_gen_set_text` |
| `gen_continuous` | `sdrkit_jt6m_gen_get_continuous` | `sdrkit_jt6m_gen_set_continuous` |

## Try it

```sh
sdrkit run jt6m --source synth:jt6m
sdrkit run jt6m --source <driver>:<address> --freq 50
```
<!-- generated: end -->

## Description

6 m scatter: 44-FSK at 21.53 baud, tones 21.53 Hz apart from the sync tone at 1076.66 Hz, a sync symbol every third symbol and one character in each other symbol. Windows of 6 s every 3 s are read for pings (runs of clear frames); a long weak signal is folded into its repeating message. Not confirmed by a public document: that each symbol's tone starts at phase 0, and the cut of a message at 58 characters.

## Tuning: notes

The stack holds no calling frequency: the usual dials are in `crates/stacks/sdrkit-stack-wsjt/data/scatter_dials.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run jt6m --source synth:jt6m --seconds 26
sdrkit run jt6m --source iiod:192.168.2.1 --freq 50.23
```
