<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HF weak-signal chat

Short messages, beacons and voice in 2.4 kHz of upper-sideband audio, in UTC time slots: five chirp profiles for weak signals and three OFDM profiles, each block coded by its own LDPC code; one way or acknowledged. Receive and transmit.

Stack id: `hf_chat`. Group: Custom links.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 14.1 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 2.4 kHz |
| Offset from the signal | 20 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hf_chat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen` | Profiles searched | choice | `all` | `all` (All profiles), `0` (Profile 0), `1` (Profile 1), `2` (Profile 2), `3` (Profile 3), `4` (Profile 4), `5` (Profile 5), `6` (Profile 6), `7` (Profile 7) |  |
| `callsign` | Station callsign | text | empty |  | 1 to 9 characters of A-Z, 0-9, '-', '/' and '.'. Empty sends the demonstration callsign TEST1. |
| `profile` | Profile | choice | `3` | `0` (0: Abyss, chirp SF 10, rate 1/4 x 3), `1` (1: Deep, chirp SF 10, rate 1/4), `2` (2: Weak, chirp SF 9, rate 1/3), `3` (3: Chat, chirp SF 8, rate 1/2), `4` (4: Mid, chirp SF 6, rate 1/2), `5` (5: Fast data, OFDM BPSK, rate 1/4), `6` (6: Voice-700, OFDM QPSK), `7` (7: Voice-1200, OFDM QPSK) |  |
| `kind` | Message | choice | `chat` | `chat` (Chat), `short_beacon` (Short beacon), `full_beacon` (Full beacon), `data` (Data) |  |
| `to` | Recipient | text | `CQ` |  | A callsign; empty sends to CQ. |
| `text` | Text | text | empty |  | Up to 100 characters for a chat, 10 for a full beacon. Empty sends a test text. |
| `locator` | Locator | text | empty |  | Six characters, AA00AA. |
| `lat` | Latitude | number (°) | 0 | -90 to 90 |  |
| `lon` | Longitude | number (°) | 0 | -180 to 180 |  |
| `power_step` | Power step | whole number | 0 | 0 to 255 | EIRP dBm = -40 + 0.5 x step; a short beacon takes 0 to 63. |
| `file` | File | text | empty |  | The bytes of a data message. |
| `passes` | Passes | whole number | 2 | 1 to 9 |  |
| `link` | Link | choice | `one_way` | `one_way` (One way, repeated passes), `two_way` (Two way, acknowledged) |  |

The setting `to` takes the column `callsign` of a row of the table `beacons`.

## Result fields

| Key | Label | Kind |
|---|---|---|
| `profile` | Profile | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `clock_error_s` | Clock error | number (s) |
| `callsign` | Callsign | text |
| `bursts` | Bursts | whole number |
| `blocks_good` | Blocks good | whole number |
| `blocks_bad` | Blocks bad | whole number |
| `messages` | Messages | whole number |
| `beacons` | Beacons | whole number |
| `acks_heard` | Acknowledgements heard | whole number |
| `voice_blocks` | Voice blocks | whole number |
| `locked` | Locked | yes or no |
| `note` | Note | text |
| `message_id` | Message id | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `callsign` (text), `to` (text), `text` (text), `time` (time) |
| `beacons` | Beacons | `callsign` (text), `locator` (text), `position` (position), `power_dbm` (number (dBm)), `text` (text), `time` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `chat.message` | Message received | `callsign` (text), `to` (text), `text` (text) |
| `chat.beacon` | Beacon received | `callsign` (text), `locator` (text), `position` (position), `power_dbm` (number (dBm)), `text` (text) |
| `chat.ack` | Acknowledgement heard | `callsign` (text), `message_id` (whole number), `blocks_acked` (whole number), `snr_db` (number (dB)), `profile` (whole number) |
| `chat.voice` | Voice heard | `callsign` (text), `call` (whole number), `profile` (whole number) |
| `chat.tx` | Chat transmission | `state` (text), `reason` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send |  |  | Sends the message in the next caller slots. Transmit must be on. |
| `stop` | Stop |  |  | Ends the transmission after the burst on the air. |

## Typed interface

The stack is in the family `custom`, version 2.0. The C header `sdrkit/custom.h`, the registry record `registry/custom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Keyboard text (`text_chat`). Media: text.

### Result fields

Struct `HfChatFields`. The result fields of `hf_chat`. A value of the last burst has no value before the first burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `profile` | whole number | optional | 1.0 | The profile of the last burst. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last burst. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last burst. |
| `clock_error_s` | number (s) | optional | 1.0 | The clock error of the last burst against the slot grid. |
| `callsign` | text | optional | 1.0 | The sending station of the last burst; empty if the burst names none. |
| `bursts` | whole number | always | 1.0 | Bursts decoded. |
| `blocks_good` | whole number | always | 1.0 | Blocks whose code word and CRC held. |
| `blocks_bad` | whole number | always | 1.0 | Blocks that did not decode. |
| `messages` | whole number | always | 1.0 | Messages completed. |
| `beacons` | whole number | always | 1.0 | Beacons heard. |
| `acks_heard` | whole number | always | 1.0 | Acknowledgements heard. |
| `voice_blocks` | whole number | always | 1.0 | Voice blocks heard. |
| `locked` | yes or no | always | 1.0 | A burst was decoded in the last 30 s. |
| `note` | text | always | 1.0 | The note on the source of the stack. |
| `message_id` | whole number | optional | 1.0 | The message id of the last burst; 0 if it carries no message part. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `HfChatMessageRow`. A row of the messages received. The row key is a running number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The sending station; empty for a data message. |
| `to` | text | always | 1.0 | The recipient; empty for a data message. |
| `text` | text | always | 1.0 | The text. |
| `time` | time | always | 1.0 | When the message was completed, UTC. |

The rows are the messages of the stack. The column `callsign` names a row. Sender: `callsign`. Addressee: `to`. Text: `text`. Time: `time`.

### Table `beacons`

Struct `HfChatBeaconRow`. A row of the beacons received. The row key is a running number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The station. |
| `locator` | text | always | 1.0 | The locator. |
| `position` | position | always | 1.0 | The position. |
| `power_dbm` | number (dBm) | always | 1.0 | The power. |
| `text` | text | always | 1.0 | The text of a full beacon; empty for a short one. |
| `time` | time | always | 1.0 | When the beacon was completed, UTC. |

The rows are the stations or nodes of the stack. The column `callsign` names a row. Place of a row: the column `position`. Time: `time`.

### Event `chat.message`

Struct `HfChatMessageEvent`. The fields of a completed chat or data message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The sending station; empty for a data message. |
| `to` | text | always | 1.0 | The recipient; empty for a data message. |
| `text` | text | always | 1.0 | The text. |

### Event `chat.beacon`

Struct `HfChatBeaconEvent`. The fields of a completed beacon.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The station. |
| `locator` | text | always | 1.0 | The locator. |
| `position` | position | always | 1.0 | The position. |
| `power_dbm` | number (dBm) | always | 1.0 | The power. |
| `text` | text | always | 1.0 | The text of a full beacon; empty for a short one. |

### Event `chat.ack`

Struct `HfChatAckEvent`. The fields of an acknowledgement heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The acknowledging station. |
| `message_id` | whole number | always | 1.0 | The message id that it acknowledges. |
| `blocks_acked` | whole number | always | 1.0 | Blocks acknowledged. |
| `snr_db` | number (dB) | always | 1.0 | The SNR that it reports. |
| `profile` | whole number | always | 1.0 | The profile that it asks for. |

### Event `chat.voice`

Struct `HfChatVoiceEvent`. The fields of a voice block heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The calling station. |
| `call` | whole number | always | 1.0 | The call number. |
| `profile` | whole number | always | 1.0 | The profile of the burst. |

### Event `chat.tx`

Struct `TxEvent`. The fields of a transmission that started, finished or was refused.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | What the transmission did: `started`, `finished` or `refused`. |
| `reason` | text | always | 1.0 | Why the transmission was refused or ended early; empty for none. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hf_chat_clear` | none | none |
| `send` | `sdrkit_hf_chat_send` | none | none |
| `stop` | `sdrkit_hf_chat_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen` | `sdrkit_hf_chat_get_listen` | `sdrkit_hf_chat_set_listen` |
| `callsign` | `sdrkit_hf_chat_get_callsign` | `sdrkit_hf_chat_set_callsign` |
| `profile` | `sdrkit_hf_chat_get_profile` | `sdrkit_hf_chat_set_profile` |
| `kind` | `sdrkit_hf_chat_get_kind` | `sdrkit_hf_chat_set_kind` |
| `to` | `sdrkit_hf_chat_get_to` | `sdrkit_hf_chat_set_to` |
| `text` | `sdrkit_hf_chat_get_text` | `sdrkit_hf_chat_set_text` |
| `locator` | `sdrkit_hf_chat_get_locator` | `sdrkit_hf_chat_set_locator` |
| `lat` | `sdrkit_hf_chat_get_lat` | `sdrkit_hf_chat_set_lat` |
| `lon` | `sdrkit_hf_chat_get_lon` | `sdrkit_hf_chat_set_lon` |
| `power_step` | `sdrkit_hf_chat_get_power_step` | `sdrkit_hf_chat_set_power_step` |
| `file` | `sdrkit_hf_chat_get_file` | `sdrkit_hf_chat_set_file` |
| `passes` | `sdrkit_hf_chat_get_passes` | `sdrkit_hf_chat_set_passes` |
| `link` | `sdrkit_hf_chat_get_link` | `sdrkit_hf_chat_set_link` |

## Try it

```sh
sdrkit run hf_chat --source synth:hf_chat
sdrkit run hf_chat --source <driver>:<address> --freq 14.1
```
<!-- generated: end -->

## Description

The stack follows a private description of the signal; no public document defines it. It was verified only against the test
vectors of that description and against its own transmitter. The signal has not been tested on a real HF path.

The signal is complex baseband at 2400 samples a second in the audio of an upper-sideband transceiver: audio frequency =
baseband frequency + 1500 Hz, so it fills 300 Hz to 2700 Hz. A burst is one or more lattice units (3.5 s each: four rising
and four falling sweeps of 1600 Hz/s and seven comb tones), the data, and a closing weave of four sweeps (1 s). A block is
`K` bits (a message id, a block index, a slice of the message and a CRC-16) coded by the profile's own quasi-cyclic LDPC code
and interleaved. The eight profiles:

| Profile | Name | Modulation | K | N | Seconds of a block | Slot |
|---|---|---|---|---|---|---|
| 0 | Abyss | chirps SF 10, three copies, 10 lattice units | 128 | 512 | 66.56 | 120 s |
| 1 | Deep | chirps SF 10, 2 lattice units | 256 | 1024 | 43.95 | 60 s |
| 2 | Weak | chirps SF 9 | 336 | 1008 | 23.89 | 60 s, 2 blocks |
| 3 | Chat | chirps SF 8 | 512 | 1024 | 13.65 | 30 s |
| 4 | Mid | chirps SF 6, guard of 16 chips | 510 | 1020 | 5.67 | 15 s |
| 5 | Fast data | OFDM BPSK, 40 symbols | 360 | 1440 | 1.00 | 15 s, 6 blocks |
| 6 | Voice-700 | OFDM QPSK, 24 symbols | 600 | 1728 | 0.60 | push to talk |
| 7 | Voice-1200 | OFDM QPSK, 24 symbols | 900 | 1728 | 0.60 | push to talk |

A chirp symbol carries SF bits (Gray coded) as the start of a chirp of `2^SF` chips; a 16-bit LFSR, restarted for every
block, decides whether each chirp rises or falls. The OFDM profiles have 48 carriers 50 Hz apart, a pilot on every fourth.
The messages are a chat (to a callsign or a 22-bit hash of one, up to 100 characters of a 64-character alphabet or raw
bytes), a short beacon (callsign, locator, power), a full beacon (callsign, position, power, locator, up to 10 characters)
and a data frame (bytes with a CRC-24); a message takes 1 to 16 blocks. An acknowledgement fills one block on its own. A voice
block is 15 frames of 40 ms, each the speech codec bits and 12 bits of a side channel (callsign and call number with a
CRC-16).

The receiver needs no clock. It takes the upper sideband of its input to 2400 samples a second and correlates it with the
eight sweeps of a lattice unit, each alone; the sums of the rising and of the falling correlations at their offsets give the
start of a unit and the carrier offset together (±107 Hz). Profiles 0 and 1 are found by the sum of their units at their
offsets, after the statistic at every offset where their units line up with themselves is known. A unit found starts a try
of every profile listened to; the closing weave gives the number of blocks of a burst that can hold more than one, else the
header of block 0 gives the length of the message. The carrier follows a straight line between the offsets of the units and
of the closing weave. Paths up to 20 samples from the strongest show in the sweep sums; with more than one, the start and
the offset come from the power centroids, and the bin energies of each chirp symbol are added over the paths. A search over half a chip on the first
symbols refines the start of a chirp burst. Chirp blocks
are decoded from the bin energies by iterative demapping: the decoder's extrinsic LLRs go back as a priori LLRs, up to 6
rounds of 12 iterations. OFDM blocks take a channel of 12 delay taps fitted to the pilots and
smoothed over 7 symbols, then, after a failed decode, to every carrier weighted by its decision, up to 8 rounds. A block counts
when its CRC holds (or the CRC of a voice side channel). Blocks join by message id; a message delivered in the last 20
minutes is a repeat. The SNR is measured on the symbols of the blocks decoded, the clock error against the UTC slot grid (from
the host's clock on live input, else from the start of the stream).

The sender keys in UTC slots: a burst starts 1.5 s after the start of a slot, the caller sends in even slots, the answering
station in odd slots. One way (the default) sends the message in 1 to 9 passes, each caller slot with up to its profile's
blocks. Two way is an acknowledged session: each caller slot carries the lowest blocks not yet acknowledged, an
acknowledgement may move the session to the profile it suggests (the message is cut again), and a block sent 8 times without
an acknowledgement ends it. With two way set, the stack also answers the blocks it hears with an acknowledgement in the next
odd slot, in the profile of the burst. Profiles 6 and 7 carry speech: sdrkit has no speech codec of their frame sizes (28 and
48 bits a 40 ms frame), so the sender refuses them and the receiver reports the side channel of each voice block, not the
speech.

## Tuning: notes

The frequency is the upper-sideband dial. The stack holds no band. The usual dial is a row of `data/custom/custom_dials.csv`,
for the host to offer.

## Result fields: notes

The SNR is that of the last burst in 2.4 kHz. The clock error is the start of the last burst less the nearest burst start of
its profile's slot grid. `locked` is on while a burst was decoded in the last 30 s.

## Result tables: notes

A short beacon has no position of its own: the table shows the centre of its locator square.

## Events: notes

The text of each chat and the bytes of each data message also leave on the `messages` byte stream.

## Try it: notes

```sh
sdrkit run hf_chat --source synth:hf_chat --seconds 25
sdrkit run hf_chat --source <driver>:<address> --freq 14.1
```
