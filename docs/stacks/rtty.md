<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# RTTY

Radioteletype: ITA2 text sent by two-tone frequency shift keying.

Stack id: `rtty`. Group: HF and utility.

Signal types: RTTY.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:rtty` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `auto` | Automatic | yes or no | true |  | Finds the speed, the shift and the polarity from the signal. Setting one of them by hand turns it off. |
| `baud` | Speed | choice | `45.45` | `45.45` (45.45 Bd), `50` (50 Bd), `75` (75 Bd), `100` (100 Bd) |  |
| `shift` | Shift | choice | `170` | `170` (170 Hz), `200` (200 Hz), `425` (425 Hz), `450` (450 Hz), `850` (850 Hz) |  |
| `reverse` | Reverse | yes or no | false |  | The mark is the lower audio tone. Off, the mark is the upper tone: the higher radio frequency on USB. |
| `unshift_on_space` | Unshift on space | yes or no | false |  | A space returns the figures case to letters. |
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `tx_centre_hz` | Audio centre | frequency | 2.21 kHz | 300 Hz to 3 kHz | The middle of the two tones sent: 2210 Hz gives 2125 and 2295 Hz at 170 Hz shift. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The peak magnitude of the signal sent, of full scale. |
| `tx_tune_limit` | Tune time | duration (s) | 10 s | 1 to 60 | The tune carrier ends after this time. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_lower_hz` | Lower tone | frequency | 2.125 kHz | 100 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `text` | Text | text |
| `callsign` | Callsign | text |
| `mark_hz` | Mark | frequency |
| `space_hz` | Space | frequency |
| `symbol_rate_hz` | Symbol rate | frequency |
| `shift_hz` | Shift | frequency |
| `reverse` | Reverse | yes or no |
| `characters` | Characters | whole number |
| `framing_errors` | Framing errors | whole number |
| `quality_pct` | Quality | number (%) |
| `auto` | Automatic | yes or no |
| `auto_locked` | Automatic lock | yes or no |
| `auto_state` | Automatic state | whole number |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Characters sent | whole number |
| `tx_waiting` | Characters waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `heard` (whole number), `last_heard` (time) |
| `auto_guesses` | Automatic guesses | `lower_hz` (frequency), `shift_hz` (frequency), `symbol_rate_hz` (frequency), `reverse` (yes or no), `score` (number), `characters` (whole number), `framing_errors` (whole number) |

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
| `auto_lock` | Find the settings |  |  | Runs one analysis on up to 20 s of the kept audio, once 6 s are kept, and applies its best guess even with Auto off. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a text | `text` (text, required) |  | Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. |
| `stop` | Stop sending |  |  | Drops the text waiting and ends the signal with a 10 ms fade. |
| `tune` | Tune |  |  | Sends an unmodulated carrier at the audio centre for the tune time. |

## Typed interface

The stack is in the family `hfkbd`, version 2.7. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `RttyFields`. The result fields of RTTY.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The decoded text. |
| `callsign` | text | optional | 1.0 | The newest callsign of the text, if it has one. |
| `mark_hz` | frequency | optional | 1.0 | The mark tone. It has no value while searching. |
| `space_hz` | frequency | optional | 1.0 | The space tone. It has no value while searching. |
| `symbol_rate_hz` | frequency | always | 1.0 | The speed in use. |
| `shift_hz` | frequency | always | 1.0 | The shift in use. |
| `reverse` | yes or no | always | 1.0 | The mark is the upper tone. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `framing_errors` | whole number | always | 1.0 | Characters that the framer dropped. |
| `quality_pct` | number (%) | always | 1.0 | How clean the decisions are, 0 to 100. |
| `auto` | yes or no | always | 1.0 | Auto is on. |
| `auto_locked` | yes or no | always | 1.0 | Auto has applied a guess that still decodes. |
| `auto_state` | whole number | always | 1.0 | The state of Auto: 0 not asked, 1 waiting for audio or running, 2 applied, 3 nothing found. |

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

### Table `auto_guesses`

Struct `RttyGuessRow`. A guess of the last analysis of Auto. The row key is the rank, from 1.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `lower_hz` | frequency | always | 1.0 | The lower tone. |
| `shift_hz` | frequency | always | 1.0 | The shift. |
| `symbol_rate_hz` | frequency | always | 1.0 | The speed. |
| `reverse` | yes or no | always | 1.0 | The mark is the upper tone. |
| `score` | number | always | 1.0 | The text score less 2.5 for each framing error. |
| `characters` | whole number | always | 1.0 | The characters of the trial. |
| `framing_errors` | whole number | always | 1.0 | The framing errors of the trial. |

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
| `remove` | `sdrkit_rtty_remove` | `sdrkit_hfkbd_station_args` | none |
| `auto_lock` | `sdrkit_rtty_auto_lock` | none | none |
| `clear` | `sdrkit_rtty_clear` | none | none |
| `send` | `sdrkit_rtty_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_rtty_stop` | none | none |
| `tune` | `sdrkit_rtty_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `auto` | `sdrkit_rtty_get_auto` | `sdrkit_rtty_set_auto` |
| `baud` | `sdrkit_rtty_get_baud` | `sdrkit_rtty_set_baud` |
| `shift` | `sdrkit_rtty_get_shift` | `sdrkit_rtty_set_shift` |
| `reverse` | `sdrkit_rtty_get_reverse` | `sdrkit_rtty_set_reverse` |
| `unshift_on_space` | `sdrkit_rtty_get_unshift_on_space` | `sdrkit_rtty_set_unshift_on_space` |
| `demod` | `sdrkit_rtty_get_demod` | `sdrkit_rtty_set_demod` |
| `listen` | `sdrkit_rtty_get_listen` | `sdrkit_rtty_set_listen` |
| `tx_centre_hz` | `sdrkit_rtty_get_tx_centre_hz` | `sdrkit_rtty_set_tx_centre_hz` |
| `tx_level` | `sdrkit_rtty_get_tx_level` | `sdrkit_rtty_set_tx_level` |
| `tx_tune_limit` | `sdrkit_rtty_get_tx_tune_limit` | `sdrkit_rtty_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_rtty_gen_get_message` | `sdrkit_rtty_gen_set_message` |
| `gen_lower_hz` | `sdrkit_rtty_gen_get_lower_hz` | `sdrkit_rtty_gen_set_lower_hz` |

## Try it

```sh
sdrkit run rtty --source synth:rtty
sdrkit run rtty --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/hfkbd/hfkbd_rtty_45_170_12k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text. The engine setting `transmit_allowed` is the only switch: with it off, `send` and `tune` are
refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the whole text and queues it. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the centre chosen refuses `send` and `tune`.

The sender shares `baud`, `shift`, `reverse` and `demod` with the receiver, so both sides use one value. Its own settings:

| Key | Meaning | Default |
|---|---|---|
| `tx_centre_hz` | The middle of the two tones, 300 to 3000 Hz | 2210 Hz (tones 2125 and 2295 Hz at 170 Hz shift) |
| `tx_level` | The peak of the signal, 0.05 to 0.9 of full scale | 0.5 |
| `tx_tune_limit` | The time of the tune carrier | 10 s |

Characters: letters are sent in upper case; a line end (LF, or CR LF) is sent as CR LF. Every other character must be in the
ITA2 table with the US figures case; a character that is not in it refuses the whole text, and the refusal names it.

### The signal

- ITA2 with the figures case of US teleprinters (`$` on D, `'` on J, `!` `&` `#` on F G H, `"` on Z, `;` on V, the bell on S).
  The receiver decodes with the same table. A burst starts in letters; FIGS or LTRS goes before a character that is only in
  the other case. Space, CR and LF need no case.
- One character: a start bit (space), five data bits with the least significant first, 1.5 stop bits (mark): 7.5 bit times.
- A burst: 0.15 s of silence, 20 bit times of mark, LTRS, the characters, LTRS, 10 bit times of mark, 0.1 s of silence. The
  signal has a 10 ms raised-cosine fade at each end.
- Two tones with a continuous phase and no shaping. The mark is the higher radio frequency: on USB it is the upper audio tone
  (2295 Hz at the defaults); `reverse` puts it on the lower tone. With `demod` on LSB the signal is the mirror image; NFM and AM
  refuse `send`.
- Element k ends at the sample nearest to the sum of the lengths of elements 0 to k, so the bit clock builds up no error.
- Level: the peak is `tx_level`; the envelope is constant, so the peak-to-average power ratio is 0 dB.

"CQ" at the defaults, the data bits in the order sent (1 = mark): 20 bits of mark; LTRS 0 11111 1.5; C 0 01110 1.5; Q 0 11101
1.5; LTRS; 10 bits of mark.

### Results and events

`tx_sending` (a burst or the tune carrier is on air), `tx_sent` (characters of the burst coded so far) and `tx_waiting`. The
event `hfkbd.tx` says when text started a burst, was queued, finished (with the reason: stopped, the transmitter is off, the
format changed) or was refused (with the reason).

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate from 12 kHz
up. On a live radio whose host gives a transmit lead, the first sample states its time on air, as every sender of the library
does: the time of the pass that writes it plus the lead and two passes. The last sample carries the end mark.

### Checks

The tests read the bits of the worked examples ("CQ", "73", a line end and the US figures) from the coder and compare them
with bit strings typed from ITU-T S.1. The stack's own receiver reads each burst back with the same settings, at 10 dB in
2.5 kHz, at 45.45 Bd and 170 Hz in both polarities, 50/200, 75/425 reversed, 100/450 and 75/850: the whole text with at most 6
framing errors and both tones within 8 Hz; without noise the text is exact with no framing error. The 99 % power bandwidth at
45.45 Bd and 170 Hz is within 10 % of 252 Hz. Every phase step is one of the two tones. The tests also cover the frame of a
burst (silence, fades, level), `stop`, `tune`, the gate and the refused character.

### Settled points

- Polarity: the mark is the higher radio frequency (RTTY Basics, an amateur society page). The receiver had the mark on the
  lower audio tone when `reverse` was off; it now has it on the upper tone, as the sender does. The generator follows. The
  synthetic recording `hfkbd_rtty_45_170_12k` has the mark on the lower tone and is read with `reverse` on.
- Figures case: the US teleprinter table, for receiver, sender and generator alike (before, the receiver and the generator used
  the ITU-T S.1 table, without `$ ! & # " ;`).
- The receiver now reads again the last 1.5 s when it finds the tone pair, so the first characters of a burst are not lost:
  it finds the pair only once both tones were heard.
