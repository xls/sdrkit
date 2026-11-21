<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# QAM video link

A single-carrier 16-QAM or 64-QAM link at 700 to 1400 ksymbols a second with LDPC and erasure-coded groups: video, audio, text and data packets, and the station's callsign. Receive and transmit.

Stack id: `qam_video`. Group: Custom links.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.29 GHz |
| Tuning range | 100 MHz to 6 GHz |
| Sample rate | 2.4 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:qam_video` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen_profile` | Listen profile | choice | `a` | `a` (A: 700 ksym/s), `b` (B: 1050 ksym/s), `cd` (C and D: 1400 ksym/s) |  |
| `callsign` | Station callsign | text | empty |  | 1 to 7 characters of A-Z, 0-9 and /. Empty sends the demonstration callsign TEST1. |
| `station_tag` | Station tag | text | empty |  | Hex bytes that start the station identification text, before " de " and the callsign. The bytes that other stations of this signal send are a row of data/custom/custom_tags.csv. |
| `profile` | Profile | choice | `a` | `a` (A: 700 ksym/s, 16-QAM), `b` (B: 1050 ksym/s, 16-QAM), `c` (C: 1400 ksym/s, 16-QAM), `d` (D: 1400 ksym/s, 64-QAM) |  |
| `fec` | FEC profile | whole number | 1 | 0 to 7 | 0: 4 data cells; 1: 14 + 2; 2: 12 + 4; 3: 28 + 4; 4: 24 + 8; 5: 6 + 2; 6: 56 + 8; 7: 48 + 16. |
| `file` | File | text | empty |  | A file sent as Data packets of stream 1. Empty sends the synthetic test stream. |
| `media_s` | Synthetic stream | number (s) | 5 | 1 to 600 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `locked` | Locked | yes or no |
| `profile` | Profile | text |
| `esn0_db` | Es/N0 | number (dB) |
| `offset_hz` | Offset | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `blocks_bad` | Blocks bad | whole number |
| `groups_restored` | Groups restored | whole number |
| `groups_lost` | Groups lost | whole number |
| `packets` | Packets | whole number |
| `callsign` | Callsign | text |
| `symbol_rate_hz` | Symbol rate | frequency |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `streams` | Streams | `stream` (text), `type` (text), `packets` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `video.tx` | Video link transmission | `state` (text), `reason` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send |  |  | Sends the file (or the synthetic test stream) once with the station's settings. Transmit must be on. |
| `stop` | Stop |  |  | Ends the transmission at once. |

## Typed interface

The stack is in the family `custom`, version 1.2. The C header `sdrkit/custom.h`, the registry record `registry/custom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `QamVideoFields`. The result fields of `qam_video`. A value of the last frame has no value before the first frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | always | 1.0 | A frame header passed in the last 2 s. |
| `profile` | text | optional | 1.0 | The profile of the last frame. |
| `esn0_db` | number (dB) | optional | 1.0 | The Es/N0 of the last frame. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame. |
| `frames_ok` | whole number | always | 1.0 | Frames with all four blocks good. |
| `frames_bad` | whole number | always | 1.0 | Frames with a failed block. |
| `blocks_bad` | whole number | always | 1.0 | Blocks that failed. |
| `groups_restored` | whole number | always | 1.0 | Groups restored by the erasure code. |
| `groups_lost` | whole number | always | 1.0 | Groups lost. |
| `packets` | whole number | always | 1.0 | Packets received. |
| `callsign` | text | optional | 1.0 | The callsign of the station identification. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate of the listen profile. |
| `note` | text | always | 1.0 | The note on the source of the stack. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `streams`

Struct `QamVideoStreamRow`. A row of the streams heard. The row key is the packet type and the stream id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `stream` | text | always | 1.0 | The stream id. |
| `type` | text | always | 1.0 | The packet type: video, audio, text, picture, data or other. |
| `packets` | whole number | always | 1.0 | Packets of the stream. |

### Event `video.tx`

Struct `TxEvent`. The fields of a transmission that started, finished or was refused.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | What the transmission did: `started`, `finished` or `refused`. |
| `reason` | text | always | 1.0 | Why the transmission was refused or ended early; empty for none. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_qam_video_clear` | none | none |
| `send` | `sdrkit_qam_video_send` | none | none |
| `stop` | `sdrkit_qam_video_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen_profile` | `sdrkit_qam_video_get_listen_profile` | `sdrkit_qam_video_set_listen_profile` |
| `callsign` | `sdrkit_qam_video_get_callsign` | `sdrkit_qam_video_set_callsign` |
| `station_tag` | `sdrkit_qam_video_get_station_tag` | `sdrkit_qam_video_set_station_tag` |
| `profile` | `sdrkit_qam_video_get_profile` | `sdrkit_qam_video_set_profile` |
| `fec` | `sdrkit_qam_video_get_fec` | `sdrkit_qam_video_set_fec` |
| `file` | `sdrkit_qam_video_get_file` | `sdrkit_qam_video_set_file` |
| `media_s` | `sdrkit_qam_video_get_media_s` | `sdrkit_qam_video_set_media_s` |

## Try it

```sh
sdrkit run qam_video --source synth:qam_video
sdrkit run qam_video --source iiod:192.168.2.1 --freq 1290
```
<!-- generated: end -->

## Description

The stack follows a private description of the signal; no public document defines it. It was verified only against the test
vectors of that description and against its own transmitter.

A frame has a 32-symbol Zadoff-Chu preamble, a 140-symbol BPSK header (64 bits with a CRC-16, rate 1/2 convolutional code) and
a data region of 7680 coded bits with a real pilot before every 15 data symbols. Profiles A, B, C and D run at 700, 1050, 1400
and 1400 ksymbols a second with 16-QAM (64-QAM for D) and LDPC rate 3/4 (n = 1920); robust form, chosen in the header, is QPSK
with rate 1/2. Each frame carries four blocks, each a cell of 178 bytes (118 robust) with a CRC-16; one permutation and one
scrambler spread the 7680 bits. A unit-amplitude tone whose frequency moves in a V from +0.45 to -0.45 of the symbol rate over
0.5 s rides 10 dB below the data as a signature.

Above the frames, typed packets (video, audio, text, picture, data, each with a CRC-32) fill FEC groups of G data cells and P
parity cells of a Cauchy erasure code over GF(256); any G cells restore a group. Codec units are cut into fragments with a
presentation time. A text packet of stream 0 identifies the station: the bytes of the station tag, " de " and the callsign.

The receiver mixes the input by a coarse offset that steps through 0, ±0.04, ±0.08 and ±0.12 of the symbol rate until a header
passes, resamples it to 4 samples a symbol of the listen profile, finds the preamble with a two-half correlation, reads the
header with a search of the residual frequency and a Viterbi decoder, refines the timing, fits a phase slope on the preamble,
the header and the pilots, removes the signature, and decodes the blocks. When a block fails, recovery passes estimate the
channel (lags -2 to 3 symbols) by least squares on the known symbols, then also on the decoded data, equalise the frame with a
decision-feedback equaliser, and keep the result when more blocks pass. The demapping, the decoding and the recovery passes run
on a helper thread behind a bounded queue. Good cells go into the groups, which the erasure code restores, and the packets come
out on the `packets` byte stream, whole with their headers; the packets of the groups still open at the end of the input come
out too.

The transmitter sends a file as Data packets of stream 1, or the synthetic test stream, once on `send`, in normal form, at 4
samples a symbol resampled to the radio rate and scaled by 0.25. The engine's transmit switch must be on.

## Tuning: notes

The stack holds no band. The usual dial is a row of `data/custom/custom_dials.csv`; the station tag that other stations of this
signal send is a row of `data/custom/custom_tags.csv`.
