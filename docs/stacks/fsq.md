<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FSQ

Fast Simple QSO: 33-tone incremental keying with addressed messages and automatic replies.

Stack id: `fsq`. Group: HF and utility.

Signal types: FSQ.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fsq` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `speed` | Speed | choice | `auto` | `auto` (Auto), `1.5`, `2`, `3`, `4.5`, `6` |  |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `tx_centre_hz` | Audio centre | frequency | 1.5 kHz | 300 Hz to 3 kHz | The middle of the 33 tones sent. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The peak magnitude of the signal sent, of full scale. |
| `tx_tune_limit` | Tune time | duration (s) | 10 s | 1 to 60 | The tune carrier ends after this time. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_centre_hz` | Audio centre | frequency | 1.5 kHz | 300 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `speed` | Speed | text |
| `band_hz` | Band | frequency |
| `symbol_rate_hz` | Symbol rate | frequency |
| `lowest_hz` | Lowest tone | frequency |
| `highest_hz` | Highest tone | frequency |
| `quality_pct` | Quality | number (%) |
| `snr_db` | SNR | number (dB) |
| `characters` | Characters | whole number |
| `dropped_symbols` | Dropped symbols | whole number |
| `text` | Text | text |
| `callsign` | Callsign | text |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Characters sent | whole number |
| `tx_waiting` | Characters waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `heard` (whole number), `last_heard` (time) |
| `messages` | Messages | `time_s` (number (s)), `from` (text), `crc_ok` (yes or no), `to` (text), `trigger` (choice), `text` (text), `to_me` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `tone_use` | Tone use | position | share |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfkbd.spot` | Callsign heard | `callsign` (text), `source` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `family` (choice) |
| `hfkbd.text` | Text line | `source` (text), `text` (text), `snr_db` (number (dB)), `mode` (text), `family` (choice) |
| `fsq.message` | Message | `source` (text), `target` (text), `text` (text), `family` (choice), `crc_ok` (yes or no), `trigger` (choice), `to_me` (yes or no) |
| `hfkbd.tx` | Keyboard transmission | `state` (text), `reason` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a line | `text` (text, required), `to` (text, default empty), `trigger` (choice, default `text`) |  | Queues one line from the station callsign with its check, to the addressee with the trigger when one is given. Transmit must be on and the station callsign set. |
| `stop` | Stop sending |  |  | Drops the text waiting and ends the signal with a 10 ms fade. |
| `tune` | Tune |  |  | Sends an unmodulated carrier at the audio centre for the tune time. |

## Typed interface

The stack is in the family `hfkbd`, version 2.8. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Keyboard text (`text_chat`). Media: sound (monitor), pictures, text.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `FsqFields`. The result fields of FSQ and IFKP. The tones have no value before a signal is heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `speed` | text | always | 1.0 | The speed in use. |
| `band_hz` | frequency | always | 1.0 | The width of the tones. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `lowest_hz` | frequency | optional | 1.0 | The lowest tone heard. |
| `highest_hz` | frequency | optional | 1.0 | The highest tone heard. |
| `quality_pct` | number (%) | always | 1.0 | How clean the decisions are, 0 to 100. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `dropped_symbols` | whole number | always | 1.0 | Symbols that the receiver refused. |
| `text` | text | always | 1.0 | The decoded text. |
| `callsign` | text | optional | 1.0 | The newest callsign of the text, if it has one. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Result fields

Struct `KeyboardTxFields`. The result fields of a keyboard sender.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 2.4 | Whether a burst is on air: text or the tune carrier. |
| `tx_sent` | whole number | always | 2.4 | The characters of the burst on air coded into the signal so far. |
| `tx_waiting` | whole number | always | 2.4 | The characters waiting to be sent. |

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

### Table `messages`

Struct `FsqMessageRow`. A row of the FSQ messages. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `from` | text | optional | 1.0 | The sender, when the line starts with "CALL:". |
| `crc_ok` | yes or no | always | 1.0 | The two hex digits equal the CRC-8 of the callsign. |
| `to` | text | optional | 1.0 | The addressee. |
| `trigger` | choice | optional | 1.0 | The trigger. |
| `text` | text | always | 1.0 | The text after the trigger. |
| `to_me` | yes or no | always | 1.0 | Addressed to my call, allcall or cqcqcq. |

The column `from` names a row.

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

### Event `fsq.message`

Struct `FsqMessageEvent`. The fields of an FSQ message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender, when the line starts with "CALL:". |
| `target` | text | optional | 1.0 | The addressee. |
| `text` | text | always | 1.0 | The text after the trigger. |
| `family` | choice | always | 1.0 | The family of the source: a message. |
| `crc_ok` | yes or no | always | 1.0 | The two hex digits equal the CRC-8 of the callsign. |
| `trigger` | choice | optional | 1.0 | The trigger. |
| `to_me` | yes or no | always | 1.0 | Addressed to my call, allcall or cqcqcq. |

### Event `hfkbd.tx`

Struct `KeyboardTxEvent`. The fields of a change of a keyboard transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 2.4 | The state: started, queued, finished or refused. |
| `reason` | text | always | 2.4 | Why it changed: empty, stopped, the transmitter is off, the format changed, tune carrier, or why a text was refused. |
| `text` | text | always | 2.4 | The text: as given to `send` for started, queued and refused; as coded for finished. |
| `family` | choice | always | 2.4 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `remove` | `sdrkit_fsq_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_fsq_clear` | none | none |
| `send` | `sdrkit_fsq_send` | `sdrkit_fsq_send_args` | none |
| `stop` | `sdrkit_fsq_stop` | none | none |
| `tune` | `sdrkit_fsq_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `speed` | `sdrkit_fsq_get_speed` | `sdrkit_fsq_set_speed` |
| `listen` | `sdrkit_fsq_get_listen` | `sdrkit_fsq_set_listen` |
| `tx_centre_hz` | `sdrkit_fsq_get_tx_centre_hz` | `sdrkit_fsq_set_tx_centre_hz` |
| `tx_level` | `sdrkit_fsq_get_tx_level` | `sdrkit_fsq_set_tx_level` |
| `tx_tune_limit` | `sdrkit_fsq_get_tx_tune_limit` | `sdrkit_fsq_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_fsq_gen_get_message` | `sdrkit_fsq_gen_set_message` |
| `gen_centre_hz` | `sdrkit_fsq_gen_get_centre_hz` | `sdrkit_fsq_gen_set_centre_hz` |

## Try it

```sh
sdrkit run fsq --source synth:fsq
sdrkit run fsq --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends FSQ lines from the station. The engine setting `transmit_allowed` is the only switch: with it off, `send` and
`tune` are refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Queues one line: `text`, `to` (a callsign, allcall or cqcqcq; empty for a line to no one) and `trigger` (default plain text). A burst starts when none is on air; a line sent during a burst joins it. The station callsign must be set. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the centre chosen refuses `send` and `tune`.

The sender sends at the speed of `speed`, the receiver's setting; auto sends 3 baud. Its own settings:

| Key | Meaning | Default |
|---|---|---|
| `tx_centre_hz` | The middle of the 33 tones, 300 to 3000 Hz | 1500 Hz |
| `tx_level` | The peak of the signal, 0.05 to 0.9 of full scale | 0.5 |
| `tx_tune_limit` | The time of the tune carrier | 10 s |

The signal is always on the upper sideband. Results: `tx_sending`, `tx_sent` (characters of the burst coded so far) and
`tx_waiting`; the event `hfkbd.tx` says when text started a burst, was queued, finished (with the reason) or was refused (with
the reason). The sender writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate
from 12 kHz up; on a live radio whose host gives a transmit lead, the first sample states its time on air (the time of the pass
that writes it plus the lead and two passes); the last sample carries the end mark. A burst is framed by 0.15 s of silence
before and 0.1 s after, with a 10 ms raised-cosine fade at each end of the signal; the envelope is constant, the peak is
`tx_level`.

The line: a space and a line end, the station callsign, `:`, its check as two lower-case hex digits (CRC-8, polynomial 0x07,
initial 0), then for a directed line the addressee, the trigger, a space, the text, two spaces, BS and two spaces; for a line to
no one the text, a line end and a space. The callsigns are sent in lower case, as the published FSQ examples are, and the check
is taken over the call as sent; the receiver accepts the check of the call in either case. The text is sent as typed; CR LF and
a lone CR are one line end; a character without a code in the FSQ alphabet refuses the line, and the refusal names it.

### The signal

- 33 tones with a continuous phase; tone k (0 to 32) at centre + (k - 16) x 8.7890625 Hz.
- A burst starts on tone 5, which carries no data; each symbol s moves the tone to (previous + s + 1) mod 33. Each character is
  one or two symbols of the FSQ alphabet. The end is two idle characters, 28 30 each.
- Symbol lengths at 12 000 samples a second: 8192, 6144, 4096, 3072 and 2048 for 1.5, 2, 3, 4.5 and 6 baud.

### Checks

- The check of "123456789" is f4, of "k0tst" a0, of "K0TST" c9. The symbols and tones of the two worked examples (the line from
  k0tst to n0xyz with "?" and "hi"; the line "hi" to no one with its idle end) are read from the coder; no tone repeats.
- The stack's own receiver, at the addressee's station, parses a directed message at 3 baud and 10 dB in 2.5 kHz: from, a good
  check, to, the trigger, the text and "to me". It reads the line whole at every speed at 10 dB.
- The 99 % power bandwidth at 3 baud is 285.6 Hz (the specification's figure 287 Hz). Every phase step is one of the 33 tones.
- Also covered: the frame of a burst, `stop`, `tune`, the gate, the refused character and the refusal without a station
  callsign.

### Settled points

- The receiver read 2 baud as garbage, with its own test signal too: its transform of 6144 points made the tone spacing 4.5
  bins. It now takes the next multiple of 4096 points (8192), so the spacing is 6 bins.
- The sender answers nothing by itself: a trigger that asks for a reply is the host's to act on.
