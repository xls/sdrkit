<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PI4

Beacons: 4-FSK, 146 symbols of 1/6 s with a K = 32 code, 8 characters each minute before the CW identification.

Stack id: `pi4`. Group: Weak-signal digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.3 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 3.5 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:pi4` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `k` | Tone spacing factor | choice | `40` | `40` (PI4 (K = 40)), `80` (PI4-80 (K = 80)), `96` (PI4-96 (K = 96)), `120` (PI4-120 (K = 120)) | PI4, PI4-80, PI4-96 or PI4-120. Which one a beacon uses depends on its band and channel spacing. |
| `f_lo_hz` | Lowest tone-0 frequency | frequency | 400 Hz | 200 Hz to 3 kHz |  |
| `f_hi_hz` | Highest tone-0 frequency | frequency | 2.6 kHz | 300 Hz to 3.8 kHz | Clipped so that tone 3 lies below 3800 Hz. |
| `demod` | Demodulator | choice | `usb` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) |  |
| `listen` | Listen | yes or no | false |  | Copies the 8 kHz audio to the speaker. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_text` | Test text | text | empty |  | The message of the test signal. Empty: the stack's own test text. |
| `gen_k` | Test signal tone spacing factor | choice | `40` | `40` (PI4 (K = 40)), `80` (PI4-80 (K = 80)), `96` (PI4-96 (K = 96)), `120` (PI4-120 (K = 120)) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `decodes_total` | Decodes | whole number |
| `note` | Note | text |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `decodes` | Decodes | `utc` (time), `time_s` (number (s)), `snr_db` (number (dB)), `audio_hz` (frequency), `mode` (text), `message` (text), `dt_s` (number (s)), `carrier_hz` (number (Hz)), `sync` (number), `k` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wsjt.decode` | Decode | `mode` (text), `text` (text), `snr_db` (number (dB)), `audio_hz` (frequency), `callsign` (text), `source` (text), `family` (choice), `dt_s` (number (s)), `carrier_hz` (number (Hz)), `sync` (number), `k` (whole number) |
| `wsjt.spot` | Station heard | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `source` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `wsjt`, version 3.0. The C header `sdrkit/wsjt.h`, the registry record `registry/wsjt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `ScatterFields`. The result fields that each of `fsk441`, `jt6m`, `iscat` and `pi4` shows.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `decodes_total` | whole number | always | 1.0 | The decodes since the last clear. |
| `note` | text | always | 1.0 | What no public document confirms. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. These stacks do not measure it: it has no value. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `decodes`

Struct `Pi4DecodeRow`. A row of the decode table of `pi4`. The row key is the number of the decode.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The time of the start of the decode. |
| `time_s` | number (s) | always | 1.0 | The signal time of the start of the decode, from the first sample. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `mode` | text | always | 1.0 | The mode or its variant. |
| `message` | text | always | 1.0 | The decoded text. |
| `dt_s` | number (s) | always | 1.0 | The start of the decode against the minute. |
| `carrier_hz` | number (Hz) | always | 1.0 | The carrier frequency: tone 0 plus half a spacing. |
| `sync` | number | always | 1.0 | The sync value c. |
| `k` | whole number | always | 1.0 | The tone spacing factor of the decode. |

### Event `wsjt.decode`

Struct `Pi4DecodeEvent`. The fields of a decode of `pi4`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | text | always | 1.0 | The mode or its variant. |
| `text` | text | always | 1.0 | The decoded text. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `callsign` | text | optional | 1.0 | The first callsign of the text. |
| `source` | text | optional | 1.0 | The first callsign of the text. |
| `family` | choice | always | 1.0 | The family of the source: a message. |
| `dt_s` | number (s) | always | 1.0 | The start of the decode against the minute. |
| `carrier_hz` | number (Hz) | always | 1.0 | The carrier frequency: tone 0 plus half a spacing. |
| `sync` | number | always | 1.0 | The sync value c. |
| `k` | whole number | always | 1.0 | The tone spacing factor of the decode. |

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
| `clear` | `sdrkit_pi4_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `k` | `sdrkit_pi4_get_k` | `sdrkit_pi4_set_k` |
| `f_lo_hz` | `sdrkit_pi4_get_f_lo_hz` | `sdrkit_pi4_set_f_lo_hz` |
| `f_hi_hz` | `sdrkit_pi4_get_f_hi_hz` | `sdrkit_pi4_set_f_hi_hz` |
| `demod` | `sdrkit_pi4_get_demod` | `sdrkit_pi4_set_demod` |
| `listen` | `sdrkit_pi4_get_listen` | `sdrkit_pi4_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_text` | `sdrkit_pi4_gen_get_text` | `sdrkit_pi4_gen_set_text` |
| `gen_k` | `sdrkit_pi4_gen_get_k` | `sdrkit_pi4_gen_set_k` |

## Try it

```sh
sdrkit run pi4 --source synth:pi4
sdrkit run pi4 --source iiod:192.168.2.1 --freq 1300
```
<!-- generated: end -->

## Description

Beacons: 4-FSK, 146 symbols of 1/6 s (24.333 s) carrying the sync vector and a K = 32 convolutional code of a message of up to 8 characters, sent each minute before a CW identification and a plain carrier. The receiver follows the published PI4 description: a sync search over a 30 s window every 6 s, then the Fano decoder and a check of the re-encoded bits. The tone-spacing factor of the beacon is a setting; which factor a beacon uses by its band and channel spacing is in `crates/stacks/sdrkit-stack-wsjt/data/pi4_variants.csv`. The CW identification and the carrier are not decoded.

## Tuning: notes

The stack holds no calling frequency: the usual dials are in `crates/stacks/sdrkit-stack-wsjt/data/scatter_dials.csv`, for the host to offer.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run pi4 --source synth:pi4 --seconds 62
sdrkit run pi4 --source iiod:192.168.2.1 --freq 1296 k=40
```
