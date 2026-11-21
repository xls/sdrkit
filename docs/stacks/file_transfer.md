<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# File transfer in LDPC frames

One file over the air in frames of LDPC blocks, each with a CRC-32, and a hash of the whole file: 1 MHz, 500 kHz or 200 kHz wide, ten profiles from BPSK rate 1/4 to 1024-QAM rate 9/10; one way in repeated passes or a two-way session with acknowledged bursts and resume. Receive and transmit.

Stack id: `file_transfer`. Group: Custom links.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.29 GHz |
| Tuning range | 100 MHz to 6 GHz |
| Sample rate | 2.4 MHz |
| Smallest sample rate | 1.25 MHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:file_transfer` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `callsign` | Station callsign | text | empty |  | 1 to 6 characters of A-Z, 0-9, /, - and . that the sender sends; empty sends TEST1. When set, the stack takes only a file sent to this callsign or to CQ. |
| `widths` | Widths searched | choice | `all` | `all` (All widths), `1m` (1 MHz only), `500k` (500 kHz only), `200k` (200 kHz only) |  |
| `folder` | Folder | text | `file_transfer` |  | The folder in the host's storage where received files are saved. Empty saves at the top. |
| `width` | Width | choice | `200k` | `1m` (1 MHz), `500k` (500 kHz), `200k` (200 kHz) |  |
| `profile` | Profile | choice | `2` | `0` (0: BPSK 1/4), `1` (1: QPSK 1/4), `2` (2: QPSK 1/2), `3` (3: QPSK 3/4), `4` (4: 16-QAM 3/4), `5` (5: 64-QAM 3/4), `6` (6: 256-QAM 3/4), `7` (7: 256-QAM 5/6), `8` (8: 1024-QAM 5/6), `9` (9: 1024-QAM 9/10) |  |
| `passes` | Passes | whole number | 2 | 1 to 9 |  |
| `link` | Link | choice | `one_way` | `one_way` (One way, repeated passes), `two_way` (Two way, acknowledged) |  |
| `file` | File | text | empty |  | A file in the host's storage, 1 byte to 256 MiB. Empty sends a short test text. |
| `recipient` | Recipient | text | `CQ` |  | The callsign of the receiving station of a two-way session; CQ for anyone. |
| `session` | Session id | whole number | 0 | 0 to 65000 | 1 to 65000; 0 takes a number from the clock. |
| `fixed_profile` | Fixed profile | yes or no | false |  | Off: a two-way sender takes the profile that the receiver asks for. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `session` | Session | whole number |
| `blocks_good` | Blocks good | whole number |
| `blocks_bad` | Blocks bad | whole number |
| `frames` | Frames | whole number |
| `progress` | Progress | number (%) |
| `hash_ok` | Hash good | yes or no |
| `profile` | Profile | whole number |
| `width` | Width | text |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `callsign` | Callsign | text |
| `files` | Files | whole number |
| `held_bytes` | Bytes held | whole number (bytes) |
| `acks_heard` | Acknowledgements heard | whole number |
| `locked` | Locked | yes or no |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `files` | Files | `name` (text), `size` (whole number (bytes)), `hash` (text), `state` (text), `callsign` (text), `time` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `file.received` | File received | `name` (text), `size` (whole number (bytes)), `hash` (text), `path` (text), `callsign` (text) |
| `file.open` | Transfer announced | `session` (whole number), `name` (text), `size` (whole number (bytes)), `held_bytes` (whole number (bytes)), `callsign` (text) |
| `file.tx` | File transmission | `state` (text), `reason` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send |  |  | Sends the file (or a test text) one way or in a two-way session. Transmit must be on. |
| `stop` | Stop |  |  | Ends the transmission after the frame or burst on the air. |

## Typed interface

The stack is in the family `custom`, version 1.2. The C header `sdrkit/custom.h`, the registry record `registry/custom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FileTransferFields`. The result fields of `file_transfer`. A value of the last frame has no value before the first frame, and a value of the transfer none while no transfer is announced.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `session` | whole number | optional | 1.0 | The session id of the active transfer. |
| `blocks_good` | whole number | always | 1.0 | Blocks whose CRC passed. |
| `blocks_bad` | whole number | always | 1.0 | Blocks whose CRC failed. |
| `frames` | whole number | always | 1.0 | Frames whose header was read. |
| `progress` | number (%) | optional | 1.0 | Bytes of the active transfer received. |
| `hash_ok` | yes or no | optional | 1.0 | The last file passed its hash. |
| `profile` | whole number | optional | 1.0 | The profile of the last frame. |
| `width` | text | optional | 1.0 | The width of the last frame. |
| `snr_db` | number (dB) | optional | 1.0 | The Es/N0 of the last frame. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame. |
| `callsign` | text | optional | 1.0 | The callsign of the last frame. |
| `files` | whole number | always | 1.0 | Files received. |
| `held_bytes` | whole number (bytes) | always | 1.0 | Bytes held from an earlier session when the last transfer was announced. |
| `acks_heard` | whole number | always | 1.0 | Acknowledgement frames heard. |
| `locked` | yes or no | always | 1.0 | A frame header was read in the last 5 s. |
| `note` | text | always | 1.0 | The note on the source of the stack. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `files`

Struct `FileTransferFileRow`. A row of the files heard. The row key is the session id and the start of the hash.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The file name. |
| `size` | whole number (bytes) | always | 1.0 | The size. |
| `hash` | text | always | 1.0 | The hash, hex. |
| `state` | text | always | 1.0 | The state: receiving, verified, or the hash failed. |
| `callsign` | text | always | 1.0 | The sending station. |
| `time` | time | always | 1.0 | When the state was reached. |

### Event `file.received`

Struct `FileTransferReceivedEvent`. The fields of a file that passed every check and was saved.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The file name. |
| `size` | whole number (bytes) | always | 1.0 | The size. |
| `hash` | text | always | 1.0 | The hash, hex. |
| `path` | text | always | 1.0 | Where the file was saved; empty if it was not saved. |
| `callsign` | text | always | 1.0 | The sending station. |

### Event `file.open`

Struct `FileTransferOpenEvent`. The fields of an announced transfer.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `session` | whole number | always | 1.0 | The session id. |
| `name` | text | always | 1.0 | The file name. |
| `size` | whole number (bytes) | always | 1.0 | The size. |
| `held_bytes` | whole number (bytes) | always | 1.0 | Bytes held from an earlier session. |
| `callsign` | text | always | 1.0 | The sending station. |

### Event `file.tx`

Struct `TxEvent`. The fields of a transmission that started, finished or was refused.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | What the transmission did: `started`, `finished` or `refused`. |
| `reason` | text | always | 1.0 | Why the transmission was refused or ended early; empty for none. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_file_transfer_clear` | none | none |
| `send` | `sdrkit_file_transfer_send` | none | none |
| `stop` | `sdrkit_file_transfer_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `callsign` | `sdrkit_file_transfer_get_callsign` | `sdrkit_file_transfer_set_callsign` |
| `widths` | `sdrkit_file_transfer_get_widths` | `sdrkit_file_transfer_set_widths` |
| `folder` | `sdrkit_file_transfer_get_folder` | `sdrkit_file_transfer_set_folder` |
| `width` | `sdrkit_file_transfer_get_width` | `sdrkit_file_transfer_set_width` |
| `profile` | `sdrkit_file_transfer_get_profile` | `sdrkit_file_transfer_set_profile` |
| `passes` | `sdrkit_file_transfer_get_passes` | `sdrkit_file_transfer_set_passes` |
| `link` | `sdrkit_file_transfer_get_link` | `sdrkit_file_transfer_set_link` |
| `file` | `sdrkit_file_transfer_get_file` | `sdrkit_file_transfer_set_file` |
| `recipient` | `sdrkit_file_transfer_get_recipient` | `sdrkit_file_transfer_set_recipient` |
| `session` | `sdrkit_file_transfer_get_session` | `sdrkit_file_transfer_set_session` |
| `fixed_profile` | `sdrkit_file_transfer_get_fixed_profile` | `sdrkit_file_transfer_set_fixed_profile` |

## Try it

```sh
sdrkit run file_transfer --source synth:file_transfer
sdrkit run file_transfer --source iiod:192.168.2.1 --freq 1290
```
<!-- generated: end -->

## Description

The stack follows a private description of the signal; no public document defines it. It was verified only against the test
vectors of that description and against its own transmitter.

## The signal

A frame is a single carrier at 4 samples a symbol, RRC roll-off 0.25 over 20 symbols: a preamble of 512 symbols (the last 10
symbols of a Zadoff-Chu sequence of length 251, then the sequence twice), a header of 122 bits (width, profile, frame kind,
session id, frame number, block count, frame index in the burst, last-frame flag, a callsign of six characters and a CRC-16)
coded at rate 1/2 with K = 7 and sent in BPSK, then the blocks with a pilot (+1 or -1) before every 31 block symbols. A block is
an LDPC code word of 7680 bits: the payload bytes of the profile and their CRC-32, scrambled per block and frame. The root of the
preamble names the width:

| Width | Symbols a second | Internal rate | Root | Most symbols of a data frame |
|---|---|---|---|---|
| 1 MHz | 800 000 | 3.2 Msps | 13 | 32 000 |
| 500 kHz | 400 000 | 1.6 Msps | 11 | 32 000 |
| 200 kHz | 160 000 | 640 ksps | 7 | 16 000 |

| Profile | Modulation | Rate | File bytes a block | Blocks a frame (1 MHz, 500 kHz / 200 kHz) |
|---|---|---|---|---|
| 0 | BPSK | 1/4 | 230 | 3 / 1 |
| 1 | QPSK | 1/4 | 230 | 7 / 3 |
| 2 | QPSK | 1/2 | 470 | 7 / 3 |
| 3 | QPSK | 3/4 | 710 | 7 / 3 |
| 4 | 16-QAM | 3/4 | 710 | 15 / 7 |
| 5 | 64-QAM | 3/4 | 710 | 23 / 11 |
| 6 | 256-QAM | 3/4 | 710 | 31 / 15 |
| 7 | 256-QAM | 5/6 | 790 | 31 / 15 |
| 8 | 1024-QAM | 5/6 | 790 | 39 / 19 |
| 9 | 1024-QAM | 9/10 | 854 | 39 / 19 |

A data block carries a byte offset, a length and the file bytes, so the receiver places every good block on its own. Session
messages (announcement, start, finish, acknowledgement and the replies) go in control and acknowledgement frames at profile 1, of
1 to 5 blocks. A burst is 1 to 16 data frames sent back to back. The hash of the whole file is BLAKE2s-256 (or SHA-256 when the
announcement says so).

## Sending

`send` reads the file of the `file` setting from the host's storage (1 byte to 256 MiB; empty sends a short test text) and
announces it under its name without folders, cut to its last 90 characters. Transmit must be on; the transmit rate must be at
least 1.25 Msps for 1 MHz, 625 ksps for 500 kHz and 250 ksps for 200 kHz. The level is 0.35 of full scale.

- **One way** (the default) sends each pass as: the announcement, the start, the data bursts in file order (the announcement
  and the start again before every eighth burst), the finish message and 0.5 s of silence. Each frame and burst has 20 ms of
  silence before and after it. A receiver that joins late or loses blocks completes the file on a later pass.
- **Two way** sends the announcement and waits for the receiver's acceptance, which lists the byte ranges it already has and
  the profile it wants. Then it sends one burst at a time and waits for its acknowledgement; blocks not acknowledged go again.
  After the last byte it sends the finish message and waits for the receiver's verdict. With no reply within 0.5 s after its
  transmission it repeats the announcement, asks again for the acknowledgement of the burst, or repeats the finish message; after
  6 of these in a row the transfer fails. Unless `fixed_profile` is on, the sender takes the profile the receiver asks for: the
  receiver steps down after a loss above 20 % or a mean Es/N0 below the need of the profile plus 0.5 dB, and up after 3 bursts
  with a loss below 2 % and room of 2 dB above the next profile.

The session id comes from the `session` setting, or from the clock when it is 0.

## Receiving

The receiver searches the widths of the `widths` setting at the same time. For each it converts the input to 4 samples a symbol
with a rational resampler (a Blackman-windowed sinc of 6 × max(up, down) on each side, zero delay); for an input rate whose
ratio would need more than 20 000 phases it uses the rate rounded to a whole kHz. A detector runs the covariance of the input at
a lag of 251 symbols over 261 symbols. At each candidate the timing comes from 32 blocks of 16 preamble symbols, the carrier from
the phase step between the blocks and then from the two copies of the sequence, and the gain and noise from the whole preamble;
the header symbols then refine the carrier. The pilots give a gain that a straight line fitted over ±12 pilots smooths, and the
Es/N0. The carrier capture is about ±2.2 kHz at 200 kHz, ±8.8 kHz at 500 kHz and ±20 kHz at 1 MHz; offsets are tested to ±2.1 kHz at
200 kHz and ±5 kHz at 1 MHz. The receiver does
not follow a sample-clock error inside a frame.

A file is delivered only when every byte has arrived and the hash of the whole file matches the finish message; a hash that
fails drops every byte received. The file is saved through the host's storage in the `folder` setting, under a safe name: the
name from the air without folders, other characters than letters, digits, `. _ - ( ) +` and space replaced by `_`, at most 100
characters, `received.bin` when nothing is left, `_` before a reserved device name, and ` (n)` before the extension when the name
is taken. The file bytes also leave on the `files` byte stream.

An unfinished transfer is kept: a later announcement of the same file (same name, sizes and hash), also in a new session, goes
on from the bytes held. After a file was delivered, the later passes of the same file are ignored. When the `callsign` setting
is set, the receiver takes only a file sent to that callsign or to CQ. Files above 256 MiB and compressed files are refused.

When the engine's transmit switch is on, the stack answers what it receives: the acceptance, the acknowledgement of each burst
and the verdict. A one-way sender does not listen, so such answers are only on the air.

## Tuning: notes

The stack holds no band. The usual dial is a row of `data/custom/custom_dials.csv`, for the host to offer.

## Result fields: notes

The SNR is the Es/N0 of the last frame from its pilots. `progress` is the share of the active transfer received. `held_bytes`
is what an earlier session left when the last transfer was announced. `locked` is on while a frame was read in the last 5 s.

## Result tables: notes

A row appears when a transfer is announced (`receiving`) and changes when the file is verified and saved, or when its hash fails.

## Try it: notes

```sh
sdrkit run file_transfer --source synth:file_transfer --seconds 6
sdrkit run file_transfer --source iiod:192.168.2.1 --freq 1290
```
