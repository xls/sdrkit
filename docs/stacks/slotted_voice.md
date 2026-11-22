<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Slotted digital voice

Digital voice in 16-QAM frames with a sweep that spells the station and a beacon of callsign, position and text; version 4 carries four voice or data slots. Receive and transmit.

Stack id: `slotted_voice`. Group: Custom links.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 433 MHz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 240 kHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 24 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:slotted_voice` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen_rate` | Symbol rates | choice | `both` | `both` (8000 and 16000 symbols a second), `8000` (8000 symbols a second), `16000` (16000 symbols a second) |  |
| `callsign` | Station callsign | text | empty |  | Sent in the sweep (up to 6 characters and a suffix such as /P) and in the beacon (up to 9 characters). |
| `grid` | Grid locator | text | empty |  | Six characters such as KO26ab. Empty sends AA00aa. |
| `lat` | Latitude | number (°) | 0 | -90 to 90 |  |
| `lon` | Longitude | number (°) | 0 | -180 to 180 |  |
| `power_dbm` | Beacon power value | number (dBm) | 10 | -40 to 87.5 |  |
| `link` | Link quality value | whole number | 0 | 0 to 255 |  |
| `beacon` | Beacon | yes or no | true |  |  |
| `beacon_text` | Beacon text | text | empty |  | Up to 100 bytes, sent every 10 s in version 3 and in the control channel in version 4. |
| `frame_version` | Frame version | choice | `3` | `3` (Version 3 (one voice stream)), `4` (Version 4 (four slots)) |  |
| `bit_rate` | Bit rate | choice | `16` | `12` (12 kbit/s), `16` (16 kbit/s), `24` (24 kbit/s), `32` (32 kbit/s) |  |
| `voice_slots` | Voice slots | whole number | 1 | 0 to 4 |  |
| `voice_profile` | Voice profile | choice | `v60` | `v80` (8.0 kbit/s), `v60` (6.0 kbit/s), `v48` (4.8 kbit/s), `v32r` (3.2 kbit/s robust) |  |
| `robust_data` | Robust data slots | yes or no | false |  |  |
| `speech_file` | Speech file | text | empty |  | A 16-bit PCM WAV file that `send` speaks. Empty speaks a test vowel. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bit_rate` | Test signal bit rate | choice | `16` | `12` (12 kbit/s), `16` (16 kbit/s), `24` (24 kbit/s), `32` (32 kbit/s) |  |
| `gen_frame_version` | Test signal frame version | choice | `3` | `3` (Version 3 (one voice stream)), `4` (Version 4 (four slots)) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `locked` | Locked | yes or no |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `frame_version` | Frame version | whole number |
| `voice_kbps` | Voice bit rate | number (kbit/s) |
| `symbol_rate_hz` | Symbol rate | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `sweep_id` | Sweep identity | text |
| `sweep_db` | Sweep level | number (dB) |
| `callsign` | Callsign | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `locator` (text), `position` (position), `power_dbm` (number (dBm)), `link` (whole number), `text` (text), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `voice.station` | Station beacon | `callsign` (text), `locator` (text), `position` (position), `power_dbm` (number (dBm)), `text` (text) |
| `voice.tx` | Voice transmission | `state` (text), `reason` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send |  |  | Speaks the speech file (or the test vowel) once with the station's settings. Transmit must be on. |
| `stop` | Stop |  |  | Ends the transmission at the next frame. |

## Typed interface

The stack is in the family `custom`, version 1.4. The C header `sdrkit/custom.h`, the registry record `registry/custom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: sound (voice).

### Result fields

Struct `SlottedVoiceFields`. The result fields of `slotted_voice`. A value of the signal has no value while no receiver is locked.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | always | 1.0 | A receiver is locked. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR. |
| `frame_version` | whole number | optional | 1.0 | The frame version received, 3 or 4. |
| `voice_kbps` | number (kbit/s) | optional | 1.0 | The voice bit rate. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate. |
| `frames_ok` | whole number | optional | 1.0 | Words or slots that decoded, of the last receiver that was locked. |
| `frames_bad` | whole number | optional | 1.0 | Words or slots that did not decode, of the last receiver that was locked. |
| `sweep_id` | text | optional | 1.0 | The station identity of the sweep, while a receiver reads it. |
| `sweep_db` | number (dB) | optional | 1.0 | The sweep level relative to the voice. |
| `callsign` | text | optional | 1.0 | The callsign of the newest beacon. |
| `note` | text | always | 1.0 | The note on the source of the stack. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `stations`

Struct `SlottedVoiceStationRow`. A row of the stations heard in beacons. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | always | 1.0 | The grid locator. |
| `position` | position | always | 1.0 | The position. |
| `power_dbm` | number (dBm) | always | 1.0 | The beacon power value. |
| `link` | whole number | always | 1.0 | The link quality value. |
| `text` | text | always | 1.0 | The beacon text, or the text of the last full beacon of the station. |
| `last_heard` | time | always | 1.0 | When the station was last heard. |

The column `callsign` names a row. Place of a row: the column `position`.

### Event `voice.station`

Struct `SlottedVoiceStationEvent`. The fields of a station beacon.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | always | 1.0 | The grid locator. |
| `position` | position | always | 1.0 | The position. |
| `power_dbm` | number (dBm) | always | 1.0 | The beacon power value. |
| `text` | text | always | 1.0 | The beacon text. |

### Event `voice.tx`

Struct `TxEvent`. The fields of a transmission that started, finished or was refused.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | What the transmission did: `started`, `finished` or `refused`. |
| `reason` | text | always | 1.0 | Why the transmission was refused or ended early; empty for none. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_slotted_voice_clear` | none | none |
| `send` | `sdrkit_slotted_voice_send` | none | none |
| `stop` | `sdrkit_slotted_voice_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen_rate` | `sdrkit_slotted_voice_get_listen_rate` | `sdrkit_slotted_voice_set_listen_rate` |
| `callsign` | `sdrkit_slotted_voice_get_callsign` | `sdrkit_slotted_voice_set_callsign` |
| `grid` | `sdrkit_slotted_voice_get_grid` | `sdrkit_slotted_voice_set_grid` |
| `lat` | `sdrkit_slotted_voice_get_lat` | `sdrkit_slotted_voice_set_lat` |
| `lon` | `sdrkit_slotted_voice_get_lon` | `sdrkit_slotted_voice_set_lon` |
| `power_dbm` | `sdrkit_slotted_voice_get_power_dbm` | `sdrkit_slotted_voice_set_power_dbm` |
| `link` | `sdrkit_slotted_voice_get_link` | `sdrkit_slotted_voice_set_link` |
| `beacon` | `sdrkit_slotted_voice_get_beacon` | `sdrkit_slotted_voice_set_beacon` |
| `beacon_text` | `sdrkit_slotted_voice_get_beacon_text` | `sdrkit_slotted_voice_set_beacon_text` |
| `frame_version` | `sdrkit_slotted_voice_get_frame_version` | `sdrkit_slotted_voice_set_frame_version` |
| `bit_rate` | `sdrkit_slotted_voice_get_bit_rate` | `sdrkit_slotted_voice_set_bit_rate` |
| `voice_slots` | `sdrkit_slotted_voice_get_voice_slots` | `sdrkit_slotted_voice_set_voice_slots` |
| `voice_profile` | `sdrkit_slotted_voice_get_voice_profile` | `sdrkit_slotted_voice_set_voice_profile` |
| `robust_data` | `sdrkit_slotted_voice_get_robust_data` | `sdrkit_slotted_voice_set_robust_data` |
| `speech_file` | `sdrkit_slotted_voice_get_speech_file` | `sdrkit_slotted_voice_set_speech_file` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bit_rate` | `sdrkit_slotted_voice_gen_get_bit_rate` | `sdrkit_slotted_voice_gen_set_bit_rate` |
| `gen_frame_version` | `sdrkit_slotted_voice_gen_get_frame_version` | `sdrkit_slotted_voice_gen_set_frame_version` |

## Try it

```sh
sdrkit run slotted_voice --source synth:slotted_voice
sdrkit run slotted_voice --source iiod:192.168.2.1 --freq 433
```
<!-- generated: end -->

## Description

The stack follows a private description of the signal; no public document defines it. It was verified only against the test
vectors of that description and against its own transmitter.

Frame version 3 sends one voice stream at 8000 symbols a second (12 and 16 kbit/s, about 10 kHz wide) or 16000 symbols a second
(24 and 32 kbit/s, about 20 kHz). A frame has 160 symbols: a 32-symbol preamble, 120 data symbols of 16-QAM and 8 pilots. The
speech is voice codec 3; each codec frame becomes one or two words of a (640, 320) LDPC code, rate matched to 480 bits, and four
words are interleaved over four frames. A chirp of -10 dB rides on the signal: four variants of a 0.505 s sweep spell the
station callsign (6 characters and a suffix such as /P) over a cycle of 24 sweeps. A beacon slice at +4500 Hz (BPSK, 1000
symbols a second, rate 1/4 convolutional code) sends the callsign, position, power, grid locator and, every 10 s, a text.

Frame version 4 sends 40 ms superframes of 640 symbols at 16000 symbols a second: an up and a down chirp, a control channel and
four slots. A slot carries voice codec 4 at 8.0, 6.0, 4.8 or 3.2 kbit/s, or 34 or 13 data bytes; the control word, spread over 10
superframes, carries the slot map, the beacon, the callsign of a slot or a text.

The receiver resamples the input to 48 kHz for the slower profile and to 96 kHz for the faster one (the 20 kHz wide signal
fits a 48 kHz input). Until it decodes, a coarse carrier search on the averaged power spectrum finds the band, its width and
its left edge, and moves the signal to 0 Hz, so carriers of several kHz are taken. It searches 1.25 s of input for the preamble,
a peak counting only when preambles stand beside it, one frame apart, measures the carrier, finds the sweep periods, and then
demodulates the frames already in that buffer, so the speech starts within 1.3 s of the start of the signal. It subtracts the
sweep period by period (a period cut by the start of the buffer on the part it has), follows the frames with a gain and phase
fit on the preamble and the pilots, and decodes the words. It reads the bit rate from the codec frames and plays the speech at
16 kHz; in silence the second word of a 24 or 32 kbit/s frame is all zeros and counts as good. A beacon fills the table of
stations. While it is not locked, a scanner keeps the last 36 s of input and, from 26 s on, every 10 s, correlates it with the
four sweep variants, finds the period start and the carrier shift on the correlation folded over the periods, and reads the
station identity from the strongest variant of each period: it reads signals far too weak for voice (at a per-sample SNR of
-20 dB). The version 4 receiver finds the chirp pair, fits the phase and the gain of each superframe on its known symbols and
tries the slot profiles in turn.

The transmitter speaks a 16-bit PCM WAV file (or a test vowel) once on `send`, with the station fields of the settings. The
framework has no microphone or other audio source component, so the graph has no live speech input. The engine's transmit
switch must be on. In version 4 each voice slot may have its own profile; all of them carry the same speech. The early
transmit format of the description (voice codec 1, frames of 202 symbols, a continuous up sweep) exists as a transmitter
only, for its test vector; no receiver accepts it.

Not built: the multipath equaliser and the soft re-estimation of version 4; the receiver meets the echo and the 1 % loss
limits of the description without them.

## Tuning: notes

The stack holds no band. The usual dial is a row of `data/custom/custom_dials.csv`, for the host to offer.

## Result fields: notes

Frames count the LDPC words of version 3 and the slots of version 4.

## Result tables: notes

A header-only beacon keeps the text of the last full beacon of the station.
