<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LDACS

Structure only: LDACS OFDM found by its cyclic prefix (symbol 120 us, prefix 17.6 us), with the carrier offset and the frame duty; nothing is decoded.

Stack id: `ldacs`. Group: Aviation.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 625 kHz |
| Signal bandwidth | 500 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ldacs` prints this table with every option.

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `fft_len` | FFT length | whole number (samples) | 64 | 8 to 4096 | The FFT length of the numerology at 625 ksps. |
| `prefix` | Cyclic prefix | whole number (samples) | 11 | 1 to 4096 | The cyclic prefix of the numerology at 625 ksps. |
| `frame_ms` | Frame period | number (ms) | 240 | 0.1 to 10000 | The period over which the power is folded to tell bursts from a continuous signal. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_duty` | Test signal duty | number (%) | 100 | 1 to 100 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `detected` | Detected | yes or no |
| `symbol_us` | Symbol | number (us) |
| `prefix_us` | Prefix | number (us) |
| `timing` | Symbol timing | whole number (samples) |
| `correlation` | Prefix correlation | number |
| `offset_hz` | Offset | frequency |
| `frame` | Frame structure | text |
| `content` | Content | text |
| `text` | Text | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `detections` | Detections | `time_s` (number (s)), `symbol_us` (number (us)), `prefix_us` (number (us)), `offset_hz` (frequency), `correlation` (number), `frame` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avdata`, version 1.2. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: none.

### Result fields

Struct `OfdmShapeFields`. The result fields of an OFDM structure stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `detected` | yes or no | always | 1.0 | The OFDM of the numerology is there. |
| `symbol_us` | number (us) | optional | 1.0 | The symbol duration with its prefix. No value while the numerology of the settings is not valid. |
| `prefix_us` | number (us) | optional | 1.0 | The prefix duration. No value while the numerology of the settings is not valid. |
| `timing` | whole number (samples) | optional | 1.0 | The sample of the symbol period where the prefix correlation peaks, while detected. |
| `correlation` | number | optional | 1.0 | The prefix correlation, while detected. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset, while detected. |
| `frame` | text | optional | 1.0 | The frame structure in words. No value while it is not known. |
| `content` | text | always | 1.0 | What the stack decodes. |
| `text` | text | always | 1.0 | The summary line. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `detections`

Struct `OfdmShapeDetectionRow`. A row of the detection starts. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the start. |
| `symbol_us` | number (us) | always | 1.0 | The symbol duration with its prefix. |
| `prefix_us` | number (us) | always | 1.0 | The prefix duration. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `correlation` | number | always | 1.0 | The prefix correlation. |
| `frame` | text | optional | 1.0 | The frame structure in words. No value while it was not known. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ldacs_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `fft_len` | `sdrkit_ldacs_get_fft_len` | `sdrkit_ldacs_set_fft_len` |
| `prefix` | `sdrkit_ldacs_get_prefix` | `sdrkit_ldacs_set_prefix` |
| `frame_ms` | `sdrkit_ldacs_get_frame_ms` | `sdrkit_ldacs_set_frame_ms` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_duty` | `sdrkit_ldacs_gen_get_duty` | `sdrkit_ldacs_gen_set_duty` |

## Try it

```sh
sdrkit run ldacs --source synth:ldacs
sdrkit run ldacs --source iiod:192.168.2.1 --freq 1000
```
<!-- generated: end -->

## Description

Structure only. The input is resampled to 625 ksps; the correlation of each sample with the one 64 samples earlier, folded over the 75-sample symbol period, finds the cyclic prefix (symbol 120 us, prefix 17.6 us), the symbol timing and the carrier offset. The power folded over the 240 ms super-frame says whether it comes in bursts. The FFT length, the prefix and the frame period are settings. Nothing is decoded.

Standard: EUROCONTROL / SESAR LDACS A/G specification (numerology).
