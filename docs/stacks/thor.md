<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# THOR

DominoEX's incremental frequency keying with convolutional error correction and interleaving.

Stack id: `thor`. Group: HF and utility.

Signal types: THOR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 3 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:thor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `variant` | Variant | choice | `THOR 16` | `THOR 4`, `THOR 5`, `THOR 8`, `THOR 11`, `THOR 16`, `THOR 22`, `THOR 25x4`, `THOR 50x1`, `THOR 50x2`, `THOR 100` |  |
| `centre` | Audio centre | frequency | 1.5 kHz | 300 Hz to 3.5 kHz |  |
| `find_signal` | Find signal | yes or no | true |  | The centre follows the signal while nothing is decoding. |
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The peak magnitude of the signal sent, of full scale. |
| `tx_tune_limit` | Tune time | duration (s) | 10 s | 1 to 60 | The tune carrier ends after this time. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `variant` | Variant | text |
| `centre_hz` | Audio centre | frequency |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `text` | Text | text |
| `characters` | Characters | whole number |
| `callsign` | Callsign | text |
| `quality_pct` | Quality | number (%) |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Characters sent | whole number |
| `tx_waiting` | Characters waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `heard` (whole number), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfkbd.spot` | Callsign heard | `callsign` (text), `source` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `family` (choice) |
| `hfkbd.text` | Text line | `source` (text), `text` (text), `snr_db` (number (dB)), `mode` (text), `family` (choice) |
| `hfkbd.tx` | Keyboard transmission | `state` (text), `reason` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a text | `text` (text, required) |  | Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. |
| `stop` | Stop sending |  |  | Drops the text waiting and ends the signal with a 10 ms fade. |
| `tune` | Tune |  |  | Sends an unmodulated carrier at the audio centre for the tune time. |

## Typed interface

The stack is in the family `hfkbd`, version 2.9. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Keyboard text (`text_chat`). Media: sound (monitor), text.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `KeyboardFields`. The result fields of the keyboard stacks of many tones: MFSK, Olivia, Contestia, DominoEX and THOR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `variant` | text | always | 1.0 | The variant in use. |
| `centre_hz` | frequency | always | 1.0 | The audio centre now. |
| `offset_hz` | frequency | always | 1.0 | The tuning error found. |
| `snr_db` | number (dB) | always | 1.0 | The decoder's own SNR. |
| `text` | text | always | 1.0 | The decoded text. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `callsign` | text | optional | 1.0 | The newest callsign of the text, if it has one. |
| `quality_pct` | number (%) | always | 1.0 | How clean the decisions are, 0 to 100. |

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
| `remove` | `sdrkit_thor_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_thor_clear` | none | none |
| `send` | `sdrkit_thor_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_thor_stop` | none | none |
| `tune` | `sdrkit_thor_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `variant` | `sdrkit_thor_get_variant` | `sdrkit_thor_set_variant` |
| `centre` | `sdrkit_thor_get_centre` | `sdrkit_thor_set_centre` |
| `find_signal` | `sdrkit_thor_get_find_signal` | `sdrkit_thor_set_find_signal` |
| `demod` | `sdrkit_thor_get_demod` | `sdrkit_thor_set_demod` |
| `listen` | `sdrkit_thor_get_listen` | `sdrkit_thor_set_listen` |
| `tx_level` | `sdrkit_thor_get_tx_level` | `sdrkit_thor_set_tx_level` |
| `tx_tune_limit` | `sdrkit_thor_get_tx_tune_limit` | `sdrkit_thor_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_thor_gen_get_message` | `sdrkit_thor_gen_set_message` |

## Try it

```sh
sdrkit run thor --source synth:thor
sdrkit run thor --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text in THOR 4, 5, 8, 11, 16 and 22, the variants with the K = 7 code. THOR 25x4, 50x1, 50x2 and 100 use a
K = 15 code that no public document describes: `send` refuses them. The engine setting `transmit_allowed` is the only switch:
with it off, `send` and `tune` are refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the whole text and queues it. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the centre chosen refuses `send` and `tune`.

The sender shares `variant`, `centre` and `demod` with the receiver, so both sides use one value. Its own settings are
`tx_level` (the peak, 0.05 to 0.9 of full scale, default 0.5) and `tx_tune_limit` (default 10 s). With `demod` on LSB the
signal is the mirror image; NFM and AM refuse `send`.

Characters: bytes 0 to 255 (ISO 8859-1) as typed; CR LF in the text is one LF. A character above 255 refuses the whole text,
and the refusal names it.

### Results and events

`tx_sending` (a burst or the tune carrier is on air), `tx_sent` (characters of the burst coded so far) and `tx_waiting`. The
event `hfkbd.tx` says when text started a burst, was queued, finished (with the reason: stopped, the transmitter is off, the
format changed) or was refused (with the reason).

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate from 12 kHz
up. On a live radio whose host gives a transmit lead, the first sample states its time on air: the time of the pass that writes
it plus the lead and two passes. The last sample carries the end mark. A burst is framed by 0.15 s of silence before and 0.1 s
after, with a 10 ms raised-cosine fade at each end of the signal. The envelope is constant: the peak is `tx_level`, the
peak-to-average power ratio 0 dB.

### The signal

- 18 tones with a continuous phase; tone k (0 to 17) at centre + (k - 8.5) spacing. IFK+: the tone number is 0 before the first
  symbol, and each symbol s (0 to 15) moves it to (previous + 2 + s) mod 18.
- Each byte is its MFSK varicode; each bit enters the K = 7 code (the code of the MFSK16 description, the receiver's); each
  four code bits pass the interleaver of 4 x 4 x 10 (bit k delayed by 10 k words, the first bit the most significant) and are
  one symbol, with no Gray code.
- A burst: 16 symbols of value 0 outside the code; NUL, CR, STX, CR, the text and 16 + 4 x 10 = 56 spaces through the code,
  enough for the interleaver and the receiver's decision delay; a zero bit when a symbol is half full.

"CQ" at THOR 16: after the 16 zero symbols, the tones 6, 8, 0, 2, 12, 4, 6, 8, 0, 10, 16, 12, 8, 4.

### Checks

- The tones of the worked example "CQ", and the code of "e" (1000) from an empty register: 11 01 11 11.
- The stack's own receiver reads THOR 4, 5, 8, 11, 16 and 22 at 30 dB in 2.5 kHz whole, and THOR 16 at 0 dB. Without noise,
  with the receiver's signal finder off, the text is exact after the two line ends of the start.
- The 99 % power bandwidth of THOR 16 is 282.7 Hz (the specification's figure 281 Hz). Every phase step is one of the 18 tones.
- Also covered: the frame of a burst, `stop`, `tune`, the gate and the refusal of the K = 15 variants.

### Open points

- With the signal finder on, the receiver moves its centre in the middle of a short clean burst of THOR 16 and loses part of the
  line: the finder may move while the text is not yet plausible, and the spectrum of a short text is not centred.
- The K = 15 variants are not sent.
