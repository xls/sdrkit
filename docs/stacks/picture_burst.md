<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Picture in a burst

One picture up to 1920 x 1080 in a burst of 10 s at 158 ksymbols a second, in up to 8 layers from robust to fast; the stack shows the picture that the good layers give. Receive and transmit.

Stack id: `picture_burst`. Group: Custom links.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 433 MHz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 400 kHz |
| Signal bandwidth | 200 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:picture_burst` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `callsign` | Station callsign | text | empty |  | Up to 9 characters of A-Z, 0-9, /, - and . Empty sends the demonstration callsign TEST1. |
| `profile` | Profile | choice | `0` | `0` (0: Balanced), `1` (1: Robust), `2` (2: Quality), `3` (3: Direct) |  |
| `picture` | Picture | text | empty |  | A binary PPM (P6, maxval 255) picture. Empty sends a test picture. A picture above 1920 x 1080 is shrunk. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `header_ok` | Header good | yes or no |
| `profile` | Profile | whole number |
| `layers` | Layers decoded | whole number |
| `layers_sent` | Layers sent | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `callsign` | Callsign | text |
| `bursts` | Bursts seen | whole number |
| `bursts_decoded` | Bursts decoded | whole number |
| `locked` | Locked | yes or no |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `pictures` | Pictures | `callsign` (text), `size` (text), `layers` (whole number), `time` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `picture.burst` | Picture received | `callsign` (text), `size` (text), `layers` (whole number) |
| `picture.tx` | Picture transmission | `state` (text), `reason` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send |  |  | Sends the picture (or the test picture) once in a burst of 10 s. Transmit must be on. |
| `stop` | Stop |  |  | Ends the transmission at once. |

## Typed interface

The stack is in the family `custom`, version 1.2. The C header `sdrkit/custom.h`, the registry record `registry/custom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `PictureBurstFields`. The result fields of `picture_burst`. A value of the last burst has no value before the first decoded burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `header_ok` | yes or no | optional | 1.0 | The header of the last burst decoded. |
| `profile` | whole number | optional | 1.0 | The profile of the last burst. |
| `layers` | whole number | optional | 1.0 | Layers of the last burst decoded. |
| `layers_sent` | whole number | optional | 1.0 | Layers of the last burst sent. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last burst. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last burst. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The clock error of the last burst, if measured. |
| `callsign` | text | optional | 1.0 | The callsign of the last burst. |
| `bursts` | whole number | always | 1.0 | Bursts seen. |
| `bursts_decoded` | whole number | always | 1.0 | Bursts decoded. |
| `locked` | yes or no | always | 1.0 | A burst is being received, or one was decoded. |
| `note` | text | always | 1.0 | The note on the source of the stack. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `pictures`

Struct `PictureBurstPictureRow`. A row of the pictures received. The row key is the number of the decoded burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `size` | text | always | 1.0 | The picture size, "width x height". |
| `layers` | whole number | always | 1.0 | Layers decoded. |
| `time` | time | always | 1.0 | When the burst was decoded. |

### Event `picture.burst`

Struct `PictureBurstEvent`. The fields of a decoded burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `size` | text | always | 1.0 | The picture size, "width x height". |
| `layers` | whole number | always | 1.0 | Layers decoded. |

### Event `picture.tx`

Struct `TxEvent`. The fields of a transmission that started, finished or was refused.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | What the transmission did: `started`, `finished` or `refused`. |
| `reason` | text | always | 1.0 | Why the transmission was refused or ended early; empty for none. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_picture_burst_clear` | none | none |
| `send` | `sdrkit_picture_burst_send` | none | none |
| `stop` | `sdrkit_picture_burst_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `callsign` | `sdrkit_picture_burst_get_callsign` | `sdrkit_picture_burst_set_callsign` |
| `profile` | `sdrkit_picture_burst_get_profile` | `sdrkit_picture_burst_set_profile` |
| `picture` | `sdrkit_picture_burst_get_picture` | `sdrkit_picture_burst_set_picture` |

## Try it

```sh
sdrkit run picture_burst --source synth:picture_burst
sdrkit run picture_burst --source iiod:192.168.2.1 --freq 433
```
<!-- generated: end -->

## Description

The stack follows a private description of the signal; no public document defines it. It was verified only against the test
vectors of that description and against its own transmitter.

A burst is 6 315 508 samples at 632 ksps (9.993 s): a pair of linear chirps (up, then down, 40 ms each, -85 to +85 kHz), the
symbol slots at 4 samples a symbol (RRC roll-off 0.2), and the chirp pair again at the end. The slots hold the header (290 bits,
rate 1/4 convolutional code, sent 16 times with a scrambler per copy, BPSK), then the blocks of the layers, with a real pilot
after every 31 data symbols. A block is an LDPC code word of 7680 bits (rates 1/4 to 9/10) of payload bytes and a CRC-16,
scrambled per block and repetition, in BPSK, QPSK, 16-QAM or 64-QAM. The picture is the embedded stream of the wavelet picture
codec: each prefix decodes to the whole picture at a lower quality. Profiles 0 (balanced), 1 (robust), 2 (quality) and 3
(direct) set the layers; the header carries the layer table.

The receiver keeps 16 s of the input at 632 ksps. When a start chirp stands 60 times above the mean, it captures from 0.3 s
before it to 10.45 s after it and decodes the capture: the chirp pair gives the start and the carrier offset, the header is
tried at sample-clock errors of 0, ±30, ±60 and ±90 ppm and timing offsets of 0 to ±0.75 sample, the end chirps give the clock
error, the timing is refined by the pilot Es/N0, and the pilots give the gain and phase. The layers are decoded in order until a
block fails (a block that the 8-bit min-sum decoder does not decode is tried again with sum-product decoding in f32); the picture comes out on the `pictures` stream as RGB pixels. The end of the input decodes a capture in progress.

The transmitter sends a binary PPM picture (or a test picture), shrunk to fit 1920 x 1080, once on `send`, scaled to a peak of
0.9 and converted to the radio rate. The engine's transmit switch must be on.

## Tuning: notes

The stack holds no band. The usual dial is a row of `data/custom/custom_dials.csv`, for the host to offer.

## Result fields: notes

The SNR is in 190 kHz.

## Try it: notes

```sh
sdrkit run picture_burst --source synth:picture_burst --seconds 12
sdrkit run picture_burst --source iiod:192.168.2.1 --freq 433
```
