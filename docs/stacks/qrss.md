<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# QRSS / DFCW

Very slow Morse: a narrow spectrogram of the steady tone as picture rows, and the text read from its keying.

Stack id: `qrss`. Group: HF and utility.

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

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `qrss` | QRSS | 14 MHz |
| `dfcw` | DFCW | 14 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:qrss` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Keying | choice | `qrss` | `qrss` (QRSS), `dfcw` (DFCW) | QRSS: on-off keying. DFCW: dots and dashes on two frequencies. |
| `auto` | Automatic centre and element length | yes or no | true |  |  |
| `centre_hz` | Tone | frequency | 1 kHz | 300 Hz to 3.5 kHz | Setting it turns the automatic centre off. |
| `element_s` | Element length | number (s) | 0 | 0 to 120 | 0 finds the length from the keying; else 0.5 to 120 s. |
| `span_hz` | Window width | frequency | 12 Hz | 2 Hz to 50 Hz |  |
| `dash_below` | Dash below the dot frequency | yes or no | false |  |  |
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_element_s` | Test signal element length | number (s) | 0 | 0 to 120 | 0: 3 s for QRSS, 6 s for DFCW. |
| `gen_shift_hz` | Test signal DFCW shift | frequency | 1.5 Hz | -10 Hz to 10 Hz | The dash frequency above the dot frequency (negative: below). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `text` | Text | text |
| `callsign` | Callsign | text |
| `centre_hz` | Tone | frequency |
| `element_s` | Element length | number (s) |
| `snr_db` | SNR | number (dB) |
| `locked` | Tone found | yes or no |
| `picture_rows` | Rows | whole number |
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
| `qrss.text` | QRSS text | `text` (text), `element_s` (number (s)), `centre_hz` (frequency), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfkbd`, version 2.8. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Pictures and fax (`pictures`). Media: sound (monitor), pictures.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `QrssFields`. The result fields of QRSS. The fields of the reading have no value before the first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the newest reading. |
| `callsign` | text | optional | 1.0 | The newest callsign of the text, if it has one. |
| `centre_hz` | frequency | optional | 1.0 | The tone, if one is found. |
| `element_s` | number (s) | optional | 1.0 | The length of an element, while a tone is found. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the reading, while a tone is found. |
| `locked` | yes or no | always | 1.0 | A tone is found. |
| `picture_rows` | whole number | always | 1.0 | The picture rows made. |
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

The column `callsign` names a row.

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

### Event `qrss.text`

Struct `QrssTextEvent`. The fields of a QRSS reading.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text. |
| `element_s` | number (s) | always | 1.0 | The length of an element. |
| `centre_hz` | frequency | always | 1.0 | The tone. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_qrss_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_qrss_get_submode` | `sdrkit_qrss_set_submode` |
| `auto` | `sdrkit_qrss_get_auto` | `sdrkit_qrss_set_auto` |
| `centre_hz` | `sdrkit_qrss_get_centre_hz` | `sdrkit_qrss_set_centre_hz` |
| `element_s` | `sdrkit_qrss_get_element_s` | `sdrkit_qrss_set_element_s` |
| `span_hz` | `sdrkit_qrss_get_span_hz` | `sdrkit_qrss_set_span_hz` |
| `dash_below` | `sdrkit_qrss_get_dash_below` | `sdrkit_qrss_set_dash_below` |
| `demod` | `sdrkit_qrss_get_demod` | `sdrkit_qrss_set_demod` |
| `listen` | `sdrkit_qrss_get_listen` | `sdrkit_qrss_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_qrss_gen_get_message` | `sdrkit_qrss_gen_set_message` |
| `gen_element_s` | `sdrkit_qrss_gen_get_element_s` | `sdrkit_qrss_gen_set_element_s` |
| `gen_shift_hz` | `sdrkit_qrss_gen_get_shift_hz` | `sdrkit_qrss_gen_set_shift_hz` |

## Try it

```sh
sdrkit run qrss --source synth:qrss
sdrkit run qrss --source synth:qrss --submode qrss
sdrkit run qrss --source <driver>:<address> --freq 14
```
<!-- generated: end -->

## Description

Very slow Morse. QRSS keys one carrier on and off; DFCW sends dots on one frequency and dashes on a second, a few tenths of a Hz to a few Hz apart. The receiver finds the steady tone, mixes it down to a stream of 100 samples a second, draws its spectrogram as picture rows (half an element per window), tracks the strongest line, and reads the keying with the element length found from the runs (1 to 120 s). The text is read again from the whole record every 2 s. Not confirmed by a public document: the DFCW gap lengths and the dot on the lower frequency are common practice; the 0.13 element by which the half-element window lengthens marks was measured on generated signals.

## Tuning: notes

The stack holds no calling frequency: the usual dials are in `crates/stacks/sdrkit-stack-hfkbd/data/slow_dials.csv`, for the host to offer.

## Submodes: notes

Choose one with `--submode <id>`. A submode sets the `submode` setting; it never follows the frequency.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run qrss --source synth:qrss --seconds 45
sdrkit run qrss --source synth:qrss --submode dfcw
sdrkit run qrss --source iiod:192.168.2.1 --freq 10.14
```
