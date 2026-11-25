<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Contestia

A faster variant of Olivia with 6-bit upper-case characters.

Stack id: `contestia`. Group: HF and utility.

Signal types: Contestia.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:contestia` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tones` | Tones | choice | `32` | `2`, `4`, `8`, `16`, `32`, `64`, `128`, `256` |  |
| `bandwidth` | Bandwidth | choice | `1000` | `125` (125 Hz), `250` (250 Hz), `500` (500 Hz), `1000` (1000 Hz), `2000` (2000 Hz) |  |
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

The stack is in the family `hfkbd`, version 3.0. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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
| `tx_sending` | yes or no | always | 1.0 | Whether a burst is on air: text or the tune carrier. |
| `tx_sent` | whole number | always | 1.0 | The characters of the burst on air coded into the signal so far. |
| `tx_waiting` | whole number | always | 1.0 | The characters waiting to be sent. |

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
| `state` | text | always | 1.0 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.0 | Why it changed: empty, stopped, the transmitter is off, the format changed, tune carrier, or why a text was refused. |
| `text` | text | always | 1.0 | The text: as given to `send` for started, queued and refused; as coded for finished. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `remove` | `sdrkit_contestia_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_contestia_clear` | none | none |
| `send` | `sdrkit_contestia_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_contestia_stop` | none | none |
| `tune` | `sdrkit_contestia_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `tones` | `sdrkit_contestia_get_tones` | `sdrkit_contestia_set_tones` |
| `bandwidth` | `sdrkit_contestia_get_bandwidth` | `sdrkit_contestia_set_bandwidth` |
| `centre` | `sdrkit_contestia_get_centre` | `sdrkit_contestia_set_centre` |
| `find_signal` | `sdrkit_contestia_get_find_signal` | `sdrkit_contestia_set_find_signal` |
| `demod` | `sdrkit_contestia_get_demod` | `sdrkit_contestia_set_demod` |
| `listen` | `sdrkit_contestia_get_listen` | `sdrkit_contestia_set_listen` |
| `tx_level` | `sdrkit_contestia_get_tx_level` | `sdrkit_contestia_set_tx_level` |
| `tx_tune_limit` | `sdrkit_contestia_get_tx_tune_limit` | `sdrkit_contestia_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_contestia_gen_get_message` | `sdrkit_contestia_gen_set_message` |

## Try it

```sh
sdrkit run contestia --source synth:contestia
sdrkit run contestia --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/hfkbd/hfkbd_contestia_16_500_12k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text at every pair of tones (2 to 256) and bandwidth (125 to 2000 Hz). The engine setting
`transmit_allowed` is the only switch: with it off, `send` and `tune` are refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the whole text and queues it. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the settings chosen refuses `send` and `tune`.

The sender shares `tones`, `bandwidth`, `centre` and `demod` with the receiver, so both sides use one value. Its own settings
are `tx_level` (the peak, 0.05 to 0.9 of full scale, default 0.5) and `tx_tune_limit` (default 10 s). With `demod` on LSB the
signal is the mirror image; NFM and AM refuse `send`.

Characters: every character. Lower case is sent as upper case, a character outside the alphabet (space, CR, backspace, `!` to `Z`) as `?`; LF and NUL are the idle value, which the receiver does not show.

### Results and events

`tx_sending` (a burst or the tune carrier is on air), `tx_sent` (characters of the burst coded so far) and `tx_waiting`. The
event `hfkbd.tx` says when text started a burst, was queued, finished (with the reason: stopped, the transmitter is off, the
format changed) or was refused (with the reason).

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate from 12 kHz
up. On a live radio whose host gives a transmit lead, the first sample states its time on air: the time of the pass that writes
it plus the lead and two passes. The last sample carries the end mark. A burst is framed by 0.15 s of silence before and 0.1 s
after, with a 10 ms raised-cosine fade at each end of the signal. The envelope varies (two symbols overlap): the sum of two overlapping symbols peaks at `tx_level` at
most, and the burst's peak is `tx_level` within 0.5 dB. The transmitter chain must be linear.

### The signal

- 8000 S/s inside, n = log2(tones) bits per symbol, Ns = 2^max(5, n + 7 - log2(bandwidth / 125)) samples per symbol, one symbol
  every Ns / 2 samples. The tones lie on the grid of 8000 / Ns Hz: the lowest at bin b0 = (integer part of (centre - D) Ns /
  8000) + 1, D = bandwidth / 2, then one every bandwidth / tones.
- A block is n characters in 32 symbols: each character's Walsh word, scrambled, spread over the bit
  planes of the symbols, each symbol Gray coded.
- Each symbol is its tone over Ns samples, shaped by the published Olivia window (1 + 1.1913785723 cos x - 0.0793018558 cos 2x
  - 0.2171442026 cos 3x - 0.0014526076 cos 4x, x from -pi to pi); its phase starts where the previous tone would be, plus or
  minus 90 degrees at random.
- A burst: 0.1 s of silence, the first block with NUL first, the text, NUL characters to the end of its block and 5 blocks of
  NUL (the receiver decides each block late).

Contestia 4/125, a block holding C and Q: the symbols 1, 0, 2, 3, 1, 2, 2, 2, 2, 2, 0, 2, 3, 0, 0, 1 and the tones 1, 0, 3, 2,
1, 3, 3, 3, 3, 3, 0, 3, 2, 0, 0, 1. "cq de k0tst" has the values 35, 49, 59, 36, 37, 59, 43, 16, 52, 51, 52.

### Checks

- The symbols and tones of the worked example, the values of "cq de k0tst" and the tone grid (16/500 at 1200 Hz from
  953.125 Hz).
- Every pair: the stack's own receiver reads the text whole without noise (signal finder off) and at 30 dB in 2.5 kHz. Over the
  seeds 1 to 10, 16/500 at -4 dB: 10 of 10. Lower case comes back in upper case, `~` as `?`.
- The 99 % power bandwidth of 32/1000 is 991.7 Hz; every pair lies within the span of its tones and 1.5 baud each side.
- Without noise every pair of both stacks prints exactly the text sent. 60 s of white noise alone and 5 s of digital silence
  print no character in any pair.
- Also covered: the frame of a burst, `stop`, `tune` and the gate.

### Settled and open points

- The symbol window is the published one (the public description differs from the specification's Hann window). Its ends are
  not zero, so the keying sidebands are wider than with Hann: above 31.25 baud the opposite sideband is 44 to 60 dB down (60 to
  86 dB at 31.25 baud and below), and at 2 tones 125 Hz the 99 % power bandwidth is 198 Hz (144 Hz with Hann).
- At 2 tones and 2000 Hz the tones are 1000 Hz apart (bandwidth / tones, as the public description and the receiver have it).
- No public document gives the order of the Walsh chips, the direction of the Gray code, the place of the tone grid or the NUL
  at the start and the end of a burst; those follow the specification. The phase steps come from a fixed seed; receivers do not
  depend on them.
- The receiver shows the characters of a block only when that block on its own has a signal / sqrt(noise) above 2 (digital
  silence gives 0, a sliver of the burst at its ends at most 1.7, noise alone about 2.4 to 3). Before, the lock of the
  integration over 4 blocks let it print characters from the silence at the ends of a noise-free burst (19 of the 80 Olivia
  and Contestia pairs). The weak-signal figures are unchanged.
- The lock needs a signal / sqrt(noise) of 2.1 + 2.5 / sqrt(n) for n characters per block, at least 4. On a minute of noise
  alone the best phase and offset reach about 2.1 + 2.1 / sqrt(n): 4.2 at 2 tones, 3.6 at 4 tones. With a lock at 4 for
  every pair, the 2-tone pairs printed characters from noise alone (up to 17 of 40 minutes at Contestia 2/1000); at 4.6 they
  print none, and they lose about 1 dB of weak-signal range.
- `stop` ends the signal at once with the 10 ms fade, as in every keyboard sender, not at the end of a symbol.
