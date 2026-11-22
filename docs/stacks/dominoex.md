<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DominoEX

Incremental frequency keying with 18 tones: tolerant of tuning and drift, no error correction.

Stack id: `dominoex`. Group: HF and utility.

Signal types: DominoEX.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dominoex` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `variant` | Variant | choice | `DominoEX 11` | `DominoEX 4`, `DominoEX 5`, `DominoEX 8`, `DominoEX 11`, `DominoEX 16`, `DominoEX 22` |  |
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

The stack is in the family `hfkbd`, version 2.10. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

The rows are the stations or nodes of the stack. The column `callsign` names a row. Time: `last_heard`.

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
| `remove` | `sdrkit_dominoex_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_dominoex_clear` | none | none |
| `send` | `sdrkit_dominoex_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_dominoex_stop` | none | none |
| `tune` | `sdrkit_dominoex_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `variant` | `sdrkit_dominoex_get_variant` | `sdrkit_dominoex_set_variant` |
| `centre` | `sdrkit_dominoex_get_centre` | `sdrkit_dominoex_set_centre` |
| `find_signal` | `sdrkit_dominoex_get_find_signal` | `sdrkit_dominoex_set_find_signal` |
| `demod` | `sdrkit_dominoex_get_demod` | `sdrkit_dominoex_set_demod` |
| `listen` | `sdrkit_dominoex_get_listen` | `sdrkit_dominoex_set_listen` |
| `tx_level` | `sdrkit_dominoex_get_tx_level` | `sdrkit_dominoex_set_tx_level` |
| `tx_tune_limit` | `sdrkit_dominoex_get_tx_tune_limit` | `sdrkit_dominoex_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_dominoex_gen_get_message` | `sdrkit_dominoex_gen_set_message` |

## Try it

```sh
sdrkit run dominoex --source synth:dominoex
sdrkit run dominoex --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text in each of the six variants. The engine setting `transmit_allowed` is the only switch: with it off,
`send` and `tune` are refused and a burst on air ends at once.

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
- Each byte is one to three nibbles of the DominoEX varicode (the table of the receiver); each nibble is one symbol.
- A burst: NUL, CR, STX, CR, the text, 16 spaces.

"CQ" at DominoEX 11: the nibbles NUL 1 15 9, CR 2 13, STX 1 15 11, CR 2 13, C 3 12, Q 6 15 give the tones 3, 2, 13, 17, 14, 17,
16, 11, 15, 12, 17, 13, 3, 2.

### Checks

- The tones of the worked examples ("CQ"; "e t a" steps by 3, 2, 4, 2 and 6 tones), and `<` as the nibbles 0 10 8.
- The stack's own receiver reads every variant at 30 dB in 2.5 kHz whole, and DominoEX 11 at 0 dB with the burst 100 Hz off
  either way (there the signal finder needs about 6 s, so the burst holds the line three times and the later ones are whole).
  Without noise the text is exact after the two line ends of the start. With LSB on both sides the text goes through.
- The 99 % power bandwidth of DominoEX 11 is 196.3 Hz (the specification's figure 196 Hz). Every phase step is one of the 18
  tones.
- Also covered: the frame of a burst, `stop`, `tune`, the gate and the refused character.

### Settled and open points

- `<` (byte 60) is the nibbles 0 10 8, as the receiver decodes it and the structure of the table needs; the published table
  prints 0 8 10, which is also `:`.
- Sideband: the DominoEX description says to use the sideband of SSB on the band, LSB below 9 MHz and USB above. The stack
  holds no band knowledge: `demod` sets it for both sides, default USB. Set LSB below 9 MHz.
- The receiver now keeps the characters it reads before the lock (up to 32, cleared at an erasure) and shows them when the lock
  comes: the lock needs about a dozen symbols and the start of a burst is only four characters long.
- A symbol is the step from the tone before. A window without a tone (its strongest sub-bin under 2.5 times the root of the
  mean power of the others: silence, the edge of a burst) is an erasure, and so is the step from it: the first tone of a
  burst only gives the reference. Noise alone gives 1.8 to 2.7, a symbol at -3 dB in 2.5 kHz 3.5 or more.
- Open: at the start of a burst the symbol timing moves one hop per symbol, so for some positions of the burst against the
  windows the first character after NUL is read wrong (3 of 48 positions of DominoEX 5, 11 and 16 without noise).
