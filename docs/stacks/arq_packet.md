<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Packet link with selective repeat

Burst packets 100 kHz wide at 80 ksymbols a second, in nine profiles from BPSK rate 1/4 to 1024-QAM rate 5/6, with LDPC blocks; a message goes one way in repeated passes or two way with acknowledgement bursts and selective repeat. Receive and transmit.

Stack id: `arq_packet`. Group: Custom links.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 433 MHz |
| Tuning range | 30 MHz to 6 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 125 kHz |
| Signal bandwidth | 100 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:arq_packet` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `callsign` | Station callsign | text | empty |  | 1 to 7 characters of A-Z, 0-9 and /. Empty sends the demonstration callsign TEST1. |
| `profile` | Profile | choice | `2` | `0` (0: BPSK 1/4), `1` (1: QPSK 1/4), `2` (2: QPSK 1/2), `3` (3: QPSK 3/4), `4` (4: 16-QAM 1/2), `5` (5: 16-QAM 3/4), `6` (6: 64-QAM 2/3), `7` (7: 256-QAM 3/4), `8` (8: 1024-QAM 5/6) |  |
| `passes` | Passes | whole number | 2 | 1 to 9 |  |
| `link` | Link | choice | `one_way` | `one_way` (One way, repeated passes), `two_way` (Two way, selective repeat) |  |
| `message` | Message | text | empty |  | The text to send. Empty sends a test text, unless a file is set. |
| `file` | File | text | empty |  | A file of 1 byte to 8 MiB to send in place of the text. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `blocks_good` | Blocks good | whole number |
| `blocks_bad` | Blocks bad | whole number |
| `profile` | Profile | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `callsign` | Callsign | text |
| `bursts` | Bursts | whole number |
| `messages` | Messages | whole number |
| `repeats` | Repeats | whole number |
| `acks_heard` | Acknowledgements heard | whole number |
| `locked` | Locked | yes or no |
| `note` | Note | text |
| `sequence` | Sequence | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `callsign` (text), `sequence` (whole number), `length` (whole number (bytes)), `text` (text), `time` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `packet.message` | Message received | `callsign` (text), `sequence` (whole number), `length` (whole number (bytes)), `text` (text) |
| `packet.ack_out` | Acknowledgement to send | `callsign` (text), `sequence` (whole number), `blocks` (whole number), `blocks_acked` (whole number), `snr_db` (number (dB)) |
| `packet.ack_in` | Acknowledgement heard | `callsign` (text), `sequence` (whole number), `blocks_acked` (whole number), `snr_db` (number (dB)) |
| `packet.tx` | Packet transmission | `state` (text), `reason` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send |  |  | Sends the message (the text, the file or a test text) once. Transmit must be on. |
| `stop` | Stop |  |  | Ends the transmission after the burst on the air. |

## Typed interface

The stack is in the family `custom`, version 1.2. The C header `sdrkit/custom.h`, the registry record `registry/custom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `ArqPacketFields`. The result fields of `arq_packet`. A value of the last burst has no value before the first burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `blocks_good` | whole number | always | 1.0 | Blocks whose CRC passed. |
| `blocks_bad` | whole number | always | 1.0 | Blocks whose CRC failed. |
| `profile` | whole number | optional | 1.0 | The profile of the last burst. |
| `snr_db` | number (dB) | optional | 1.0 | The Es/N0 of the last burst. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last burst. |
| `callsign` | text | optional | 1.0 | The callsign of the last burst. |
| `bursts` | whole number | always | 1.0 | Bursts whose header was read. |
| `messages` | whole number | always | 1.0 | Messages listed. |
| `repeats` | whole number | always | 1.0 | Messages heard again within 120 s. |
| `acks_heard` | whole number | always | 1.0 | Acknowledgement bursts heard. |
| `locked` | yes or no | always | 1.0 | A burst header was read in the last 5 s. |
| `note` | text | always | 1.0 | The note on the source of the stack. |
| `sequence` | whole number | optional | 1.0 | The sequence number of the last burst. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `ArqPacketMessageRow`. A row of the messages received. The row key is a running number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The sending station. |
| `sequence` | whole number | always | 1.0 | The sequence number of the last burst of the message. |
| `length` | whole number (bytes) | always | 1.0 | The message length. |
| `text` | text | always | 1.0 | The message as text. |
| `time` | time | always | 1.0 | When the message was completed. |

### Event `packet.message`

Struct `ArqPacketMessageEvent`. The fields of a completed message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The sending station. |
| `sequence` | whole number | always | 1.0 | The sequence number of the last burst of the message. |
| `length` | whole number (bytes) | always | 1.0 | The message length. |
| `text` | text | always | 1.0 | The message as text. |

### Event `packet.ack_out`

Struct `ArqPacketAckOutEvent`. The fields of the acknowledgement that the receiver would send for a data burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The sending station of the burst. |
| `sequence` | whole number | always | 1.0 | The sequence number of the burst. |
| `blocks` | whole number | always | 1.0 | Blocks of the burst. |
| `blocks_acked` | whole number | always | 1.0 | Blocks acknowledged. |
| `snr_db` | number (dB) | always | 1.0 | The Es/N0 of the burst. |

### Event `packet.ack_in`

Struct `ArqPacketAckInEvent`. The fields of an acknowledgement burst heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The acknowledging station. |
| `sequence` | whole number | always | 1.0 | The sequence number that it acknowledges. |
| `blocks_acked` | whole number | always | 1.0 | Blocks acknowledged. |
| `snr_db` | number (dB) | always | 1.0 | The Es/N0 that it reports. |

### Event `packet.tx`

Struct `TxEvent`. The fields of a transmission that started, finished or was refused.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | What the transmission did: `started`, `finished` or `refused`. |
| `reason` | text | always | 1.0 | Why the transmission was refused or ended early; empty for none. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_arq_packet_clear` | none | none |
| `send` | `sdrkit_arq_packet_send` | none | none |
| `stop` | `sdrkit_arq_packet_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `callsign` | `sdrkit_arq_packet_get_callsign` | `sdrkit_arq_packet_set_callsign` |
| `profile` | `sdrkit_arq_packet_get_profile` | `sdrkit_arq_packet_set_profile` |
| `passes` | `sdrkit_arq_packet_get_passes` | `sdrkit_arq_packet_set_passes` |
| `link` | `sdrkit_arq_packet_get_link` | `sdrkit_arq_packet_set_link` |
| `message` | `sdrkit_arq_packet_get_message` | `sdrkit_arq_packet_set_message` |
| `file` | `sdrkit_arq_packet_get_file` | `sdrkit_arq_packet_set_file` |

## Try it

```sh
sdrkit run arq_packet --source synth:arq_packet
sdrkit run arq_packet --source iiod:192.168.2.1 --freq 433
```
<!-- generated: end -->

## Description

The stack follows a private description of the signal; no public document defines it. It was verified only against the test
vectors of that description and against its own transmitter.

A burst is a single carrier at 80 ksymbols a second, made at 640 ksps (RRC roll-off 0.25, 12 symbols): 8 ramp symbols, a
preamble of two Zadoff-Chu sequences of length 127 (roots 41 and 86), a header of 80 bits (callsign in 40 bits, profile,
block count, sequence number, flags and a CRC-16) coded at rate 1/2 with K = 7 and sent 4 times in BPSK, then the blocks
with a pilot after every 31 data symbols, and 8 ramp symbols. A block is an LDPC code word of 2304 bits (the payload bytes of
the profile and their CRC-16), scrambled per block. The nine profiles:

| Profile | Modulation | Rate | Payload bytes a block |
|---|---|---|---|
| 0 | BPSK | 1/4 | 70 |
| 1 | QPSK | 1/4 | 70 |
| 2 | QPSK | 1/2 | 142 |
| 3 | QPSK | 3/4 | 214 |
| 4 | 16-QAM | 1/2 | 142 |
| 5 | 16-QAM | 3/4 | 214 |
| 6 | 64-QAM | 2/3 | 190 |
| 7 | 256-QAM | 3/4 | 214 |
| 8 | 1024-QAM | 5/6 | 238 |

Each block of a message carries a byte offset, a last flag and up to 255 message bytes, so the receiver places every good
block on its own. The receiver keeps one message per sending station, drops it after 30 s without a burst from that station,
and lists a completed message once: the same message from the same station within 120 s counts as a repeat.

The receiver converts its input to 320 ksps and filters it with the matched RRC. A detector sums the products of neighbouring
symbols of each preamble half against the tone that the half gives, which a carrier offset does not change; each peak is
checked by the power of 31 coherent blocks of 8 preamble symbols over ±80 symbols. The whole preamble then gives the timing
and the carrier, the header is decoded at carrier steps of 10 Hz, and the 942 known symbols of the preamble and header give the
fine timing, the carrier, the gain and the Es/N0. The pilots give a residual phase slope and a gain per pilot. Carrier offsets
of ±2.5 kHz are tested. A payload of 2048 symbols or more is read along a timing that drifts in a straight line, fitted
to the zeros of the Gardner timing error of windows of 512 symbols, so the receiver follows a sample-clock error (±20 ppm
over a burst of 1 s is tested).

The sender has two links. One way (the default) sends every burst of the message (at most 1 s each) in 1 to 9 passes, with
30 ms of silence round each burst and 0.4 s after each pass. Two way sends one burst at a time with the acknowledgement-request
flag: the lowest bytes not yet acknowledged go first, and the profile steps down on failures and up after two good
acknowledgements with room. A burst counts as lost when no acknowledgement comes 252 ms after its end (60 ms of silence,
150 ms of turnaround and the 42 ms acknowledgement). When the receiver hears a burst that asks for an acknowledgement and the
engine's transmit switch is on, the sender answers with an acknowledgement burst (profile 0, one block: the Es/N0 and a bitmap
of 512 blocks). An acknowledgement names only the burst's sequence number, not the station it answers, so two senders on one
channel can take each other's acknowledgements. The burst level is 0.35.

## Tuning: notes

The stack holds no band. The usual dial is a row of `data/custom/custom_dials.csv`, for the host to offer.

## Result fields: notes

The SNR is the Es/N0 of the last burst. `locked` is on while a header was read in the last 5 s.

## Events: notes

The bytes of each message also leave on the `messages` byte stream.

## Try it: notes

```sh
sdrkit run arq_packet --source synth:arq_packet --seconds 12
sdrkit run arq_packet --source iiod:192.168.2.1 --freq 433
```
