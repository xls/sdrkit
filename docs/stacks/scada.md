<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SCADA telemetry

Modbus RTU, Modbus ASCII and DNP3 over narrowband FSK radio links: the modem found by itself, the frames, polls paired with replies, and the devices with their registers and points.

Stack id: `scada`. Group: Data and IoT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 450 MHz |
| Tuning range | 26 MHz to 1 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 40 kHz |
| Offset from the signal | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:scada` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_link` | Test signal link | choice | `bell202` | `bell202` (Bell 202 AFSK 1200 Bd), `v23` (V.23 AFSK 1200 Bd), `fsk2_4800` (2-FSK 4800 bit/s), `fsk2_9600` (2-FSK 9600 bit/s), `fsk2_19200` (2-FSK 19200 bit/s), `fsk4_4800` (4-FSK 4800 bit/s), `fsk4_9600` (4-FSK 9600 bit/s), `fsk4_19200` (4-FSK 19200 bit/s) |  |
| `gen_framing` | Test signal framing | choice | `8e1` | `8e1` (8E1), `8o1` (8O1), `8n1` (8N1), `8n2` (8N2), `hdlc_nrz` (HDLC NRZ), `hdlc_nrzi` (HDLC NRZI) |  |
| `gen_traffic` | Test signal traffic | choice | `modbus` | `modbus` (Modbus RTU), `modbus_ascii` (Modbus ASCII), `dnp3` (DNP3) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `locked` | Locked | yes or no |
| `link` | Link | text |
| `messages` | Messages | whole number |
| `modbus` | Modbus messages | whole number |
| `dnp3` | DNP3 messages | whole number |
| `unknown` | Unknown frames | whole number |
| `crc_failures` | CRC failures | whole number |
| `encrypted` | Encrypted | whole number |
| `not_recognised` | Not recognised | whole number |
| `devices` | Devices | whole number |
| `offset_hz` | Offset | frequency |
| `deviation_hz` | Deviation | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `modem.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `id` (whole number), `time_s` (number (s)), `wall_clock_utc` (time), `link` (text), `protocol` (text), `from` (text), `to` (text), `function` (text), `summary` (text), `crc` (text), `direction` (text), `pair_id` (whole number), `response_ms` (number (ms)), `encrypted` (yes or no), `raw_hex` (text), `quality_db` (number (dB)) |
| `devices` | Devices | `protocol` (text), `address` (whole number), `role` (text), `first_heard` (time), `last_heard` (time), `first_s` (number (s)), `last_s` (number (s)), `last_poll_s` (number (s)), `polls` (whole number), `replies` (whole number), `unanswered` (whole number), `exceptions` (whole number), `unsolicited` (whole number), `crc_failures` (whole number), `mean_response_ms` (number (ms)), `last_iin` (text), `peers` (text), `points` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `scada.message` | SCADA message | `source` (text), `target` (text), `protocol` (text), `function` (text), `text` (text), `crc` (text), `family` (choice) |
| `scada.device` | SCADA device | `protocol` (text), `address` (whole number), `role` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `export_csv` | Export the messages as CSV |  | `csv` (text) | Replies with the messages as CSV text: id, time, wall clock, link, protocol, from, to, function, summary, CRC, direction, pair, response time, encrypted, raw hex. |

## Typed interface

The stack is in the family `trunk`, version 1.1. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `ScadaFields`. The counts, the link and the measurements of the SCADA receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | always | 1.1 | A chain of the modem holds lock. |
| `link` | text | optional | 1.1 | The link of the last good frame: the modem, the framing, the tones or deviation and the carrier offset. |
| `messages` | whole number | always | 1.1 | The messages kept. |
| `modbus` | whole number | always | 1.1 | The Modbus messages (RTU and ASCII). |
| `dnp3` | whole number | always | 1.1 | The DNP3 messages. |
| `unknown` | whole number | always | 1.1 | The HDLC frames that hold neither protocol. |
| `crc_failures` | whole number | always | 1.1 | The frames whose CRC, LRC or block CRC failed. |
| `encrypted` | whole number | always | 1.1 | The DNP3 Secure Authentication messages (shown raw). |
| `not_recognised` | whole number | always | 1.1 | The locked bursts that gave no frame (vendor radio framings), kept as raw bits. |
| `devices` | whole number | always | 1.1 | The devices heard. |
| `offset_hz` | frequency | optional | 1.1 | The carrier offset measured on the link of the last good frame. |
| `deviation_hz` | frequency | optional | 1.1 | The deviation measured on that link (the FM deviation on audio FSK, the outer one on 4-FSK). |
| `level_dbfs` | number (dBFS) | optional | 1.1 | The level of the channel. |
| `note` | text | always | 1.1 | What no public document confirms, and what is shown raw. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `ScadaMessageRow`. A row of the SCADA messages. The row key is the number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.1 | The message number. |
| `time_s` | number (s) | always | 1.1 | The signal time. |
| `wall_clock_utc` | time | optional | 1.1 | The wall-clock time, when the host gives a clock. |
| `link` | text | always | 1.1 | The link: the modem and the framing ("Bell 202 AFSK 1200 Bd, 8E1"). |
| `protocol` | text | always | 1.1 | The protocol: "Modbus RTU", "Modbus ASCII", "DNP3" or "unknown". |
| `from` | text | always | 1.1 | The sender ("master", "slave 17", "outstation 1"). |
| `to` | text | always | 1.1 | The receiver. |
| `function` | text | always | 1.1 | The function. |
| `summary` | text | always | 1.1 | The summary: addresses, quantities, values, objects, indications. |
| `crc` | text | always | 1.1 | The check: "good", "failed", or empty when there is none. |
| `direction` | text | optional | 1.1 | The direction: "request", "reply" or "unsolicited". |
| `pair_id` | whole number | optional | 1.1 | The id of the paired message. |
| `response_ms` | number (ms) | optional | 1.1 | The time from the request to this reply. |
| `encrypted` | yes or no | always | 1.1 | DNP3 Secure Authentication: shown raw. |
| `raw_hex` | text | always | 1.1 | The bytes as hex, or the bits of a burst not recognised. |
| `quality_db` | number (dB) | always | 1.1 | -20 log10 of the eye error of the symbols. |

### Table `devices`

Struct `ScadaDeviceRow`. A row of the SCADA devices. The row key is the protocol and the address ("modbus/17", "dnp3/1").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `protocol` | text | always | 1.1 | "Modbus" or "DNP3". |
| `address` | whole number | always | 1.1 | The address. |
| `role` | text | always | 1.1 | The role: "slave", "master" or "outstation". |
| `first_heard` | time | optional | 1.1 | When it was first heard, wall clock. |
| `last_heard` | time | optional | 1.1 | When it was last heard, wall clock. |
| `first_s` | number (s) | always | 1.1 | When it was first heard, signal time. |
| `last_s` | number (s) | always | 1.1 | When it was last heard, signal time. |
| `last_poll_s` | number (s) | optional | 1.1 | The last poll, signal time. |
| `polls` | whole number | always | 1.1 | The polls sent or received. |
| `replies` | whole number | always | 1.1 | The replies sent or received. |
| `unanswered` | whole number | always | 1.1 | The polls that had no reply. |
| `exceptions` | whole number | always | 1.1 | The exception replies. |
| `unsolicited` | whole number | always | 1.1 | The unsolicited responses. |
| `crc_failures` | whole number | always | 1.1 | The frames whose check failed. |
| `mean_response_ms` | number (ms) | optional | 1.1 | The mean response time. |
| `last_iin` | text | optional | 1.1 | The last internal indications of a DNP3 outstation. |
| `peers` | text | always | 1.1 | The addresses it talked with. |
| `points` | text | always | 1.1 | The registers or points seen so far ("40108 = 555", "AI 0 = 1000"). |

### Event `scada.message`

Struct `ScadaMessageEvent`. A SCADA message: `source` is the sender, `target` the receiver, `text` the summary.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.1 | The sender. |
| `target` | text | always | 1.1 | The receiver. |
| `protocol` | text | always | 1.1 | The protocol. |
| `function` | text | always | 1.1 | The function. |
| `text` | text | always | 1.1 | The summary. |
| `crc` | text | always | 1.1 | The check: "good", "failed", or empty. |
| `family` | choice | always | 1.1 | The kind of thing that the event is about. |

### Event `scada.device`

Struct `ScadaDeviceEvent`. A new SCADA device.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `protocol` | text | always | 1.1 | "Modbus" or "DNP3". |
| `address` | whole number | always | 1.1 | The address. |
| `role` | text | always | 1.1 | The role. |
| `family` | choice | always | 1.1 | The kind of thing that the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_scada_clear` | none | none |
| `export_csv` | `sdrkit_scada_export_csv` | none | `sdrkit_scada_csv_reply` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_link` | `sdrkit_scada_gen_get_link` | `sdrkit_scada_gen_set_link` |
| `gen_framing` | `sdrkit_scada_gen_get_framing` | `sdrkit_scada_gen_set_framing` |
| `gen_traffic` | `sdrkit_scada_gen_get_traffic` | `sdrkit_scada_gen_set_traffic` |

## Try it

```sh
sdrkit run scada --source synth:scada
sdrkit run scada --source iiod:192.168.2.1 --freq 450
```
<!-- generated: end -->

## Description

The stack reads SCADA radio telemetry: Modbus RTU, Modbus ASCII and DNP3 frames over narrowband FSK radio links. It finds the
modem by itself among seven FSK chains, reads the frames in async characters or HDLC, pairs polls with replies, and lists the
devices with their registers and points. Proprietary radio framings are not decoded: a locked burst that gives no frame is kept
as "not recognised" with its raw bits. DNP3 Secure Authentication is shown raw. Receive only: the stack has no path to send or
repeat a frame. Local law may forbid listening to utility traffic.

## Links

| Chain | Input | Symbols a second | Levels |
|---|---|---|---|
| Audio FSK (Bell 202 1200 / 2200 Hz, V.23 1300 / 2100 Hz) | the narrow discriminator at 24 000 samples/s, mixed down by 1700 Hz, low-passed at 1300 Hz, a second discriminator | 1200 | 2, mark (the lower tone) = 1 |
| 2-FSK 4800 bit/s | narrow (channel ±8 kHz), 96 000 samples/s | 4800 | 2, higher frequency = 1 |
| 2-FSK 9600 and 19 200 bit/s | wide (channel ±16 kHz) | 9600, 19 200 | 2 |
| 4-FSK 4800 bit/s | narrow | 2400 | 4: +3 01, +1 00, -1 10, -3 11 |
| 4-FSK 9600 and 19 200 bit/s | wide | 4800, 9600 | 4 |

Each chain has a moving mean (3/4 of a symbol, 1/2 for 4-FSK), a clock pulled at the crossings of the centre (4-FSK only at
crossings between opposite symbols), levels from the extremes of the last symbols while unlocked and decision-directed once
locked, and an eye error that sets the lock. A rise of the channel power starts the levels and the eye of an unlocked chain
again, so the preamble of a burst alone decides the lock.

## Framing

Async 8E1, 8O1 and 8N1, normal and inverted on direct FSK. Modbus RTU without parity has two stop bits (Modbus over Serial Line
V1.02); such a frame shows as 8N2. Modbus ASCII (a colon, hex pairs, the LRC, CR LF) is read from the 8N1 characters and shows
as 7E1, 7O1 or 7N2. HDLC NRZ (both senses) and NRZI, with the FCS CRC-16/X.25. A burst ends after 28 idle bits, at a loss of
lock, or at 1024 bytes.

## Protocols

- **Modbus**: the functions 1 to 8, 11, 12, 15 to 17, 20 to 24 and 43 with their accepted lengths, exceptions, CRC-16/MODBUS;
  register numbers in the conventional form (40001 + index for holding registers); a request pairs with the next reply of the
  same slave within 5 s.
- **DNP3**: the link header and data blocks with CRC-16/DNP, link functions, transport segments joined per source and
  destination, the application header and IIN, object headers with their qualifiers, and the values of binary, double-bit,
  counter, analog input and analog output points; group 43 has the sizes of group 42 (DNP Users Group note AN2013-004b). A
  request pairs with the response of the same sequence within 10 s, a confirm with the response that asked for it.

A message carries the time of the end of its burst, so the response time runs from the end of the request burst to the end
of the reply burst.

The `export_csv` command replies with the messages as CSV text. The library has no JSON export: the tables `messages` and
`devices` carry everything.

## Not confirmed by a public document

The carriage of async bytes on direct 2-FSK and 4-FSK with an idle mark, the 4-FSK dibit order, the Modbus function length
table, the exception codes, the DNP3 qualifier, string and file objects, and the Bell 202 and V.23 tones follow the
specification. The result field `note` says the same.

## Test signal

The default `synth:scada` sends worked frames 1 to 4 (a Modbus poll and its reply, a poll and its exception) on Bell 202 in 8E1.
`gen_link`, `gen_framing` and `gen_traffic` (Modbus RTU, Modbus ASCII, DNP3) change it.

## Measured

Ten Modbus frames at 192 ksps, all received at: Bell 202 56 dB-Hz, 2-FSK 4800 56, 2-FSK 9600 59, 2-FSK 19 200 62, 4-FSK 4800 62,
4-FSK 9600 67, 4-FSK 19 200 70.
