<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# IFKP

Incremental frequency keying with 33 tones, a slower relative of FSQ for keyboard chat.

Stack id: `ifkp`. Group: HF and utility.

Signal types: IFKP.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ifkp` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `speed` | Speed | choice | `auto` | `auto` (Auto), `0.5`, `1.0`, `2.0` |  |
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

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `tone_use` | Tone use | position | share |

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

The stack is in the family `hfkbd`, version 2.6. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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
| `remove` | `sdrkit_ifkp_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_ifkp_clear` | none | none |
| `send` | `sdrkit_ifkp_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_ifkp_stop` | none | none |
| `tune` | `sdrkit_ifkp_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `speed` | `sdrkit_ifkp_get_speed` | `sdrkit_ifkp_set_speed` |
| `listen` | `sdrkit_ifkp_get_listen` | `sdrkit_ifkp_set_listen` |
| `tx_centre_hz` | `sdrkit_ifkp_get_tx_centre_hz` | `sdrkit_ifkp_set_tx_centre_hz` |
| `tx_level` | `sdrkit_ifkp_get_tx_level` | `sdrkit_ifkp_set_tx_level` |
| `tx_tune_limit` | `sdrkit_ifkp_get_tx_tune_limit` | `sdrkit_ifkp_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_ifkp_gen_get_message` | `sdrkit_ifkp_gen_set_message` |
| `gen_centre_hz` | `sdrkit_ifkp_gen_get_centre_hz` | `sdrkit_ifkp_gen_set_centre_hz` |

## Try it

```sh
sdrkit run ifkp --source synth:ifkp
sdrkit run ifkp --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text. The engine setting `transmit_allowed` is the only switch: with it off, `send` and `tune` are refused
and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the whole text and queues it. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the centre chosen refuses `send` and `tune`.

The sender sends at the speed of `speed`, the receiver's setting; auto sends 1.0 baud. Its own settings:

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

The text is sent as typed, without a callsign line or a check; CR LF and a lone CR are one line end. A character without a code
in the IFKP alphabet refuses the whole text, and the refusal names it.

### The signal

- 33 tones with a continuous phase; tone k (0 to 32) at centre + (k - 16) x 11.71875 Hz.
- A burst starts on tone 5, which carries no data; each symbol s moves the tone to (previous + s + 1) mod 33. Each character is
  one or two symbols of the IFKP alphabet (space 28, a line end 28 30). The end is two idle symbols 0.
- Symbol lengths at 16 000 samples a second: 8192, 4096 and 2048 for 0.5, 1.0 and 2.0 baud.

### Checks

- The symbols and tones of the worked example "cq de k0tst" with a line end, and "K0TST" in upper case as ten symbols, are read
  from the coder.
- The stack's own receiver reads the text whole at 1.0 baud and 10 dB in 2.5 kHz, and measures 3.906 baud within 0.01; the FSQ
  stack reads nothing of the burst. Without noise the text is exact at 1.0 and 2.0 baud.
- The 99 % power bandwidth at 1.0 baud is 377.9 Hz (33 spacings are 387 Hz). Every phase step is one of the 33 tones.
- Also covered: the frame of a burst, `stop`, `tune`, the gate and the refused character.

### Open points

- At 0.5 baud the receiver loses the first character of a burst: the burst starts with the text after one tone, and the
  receiver settles on the slowest speed only later.
