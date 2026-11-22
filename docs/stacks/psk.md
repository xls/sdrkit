<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PSK31

Keyboard chat by phase shift keying: every PSK31, PSK63, PSK125, PSK250 and QPSK signal in the passband at once.

Stack id: `psk`. Group: HF and utility.

Signal types: PSK31.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:psk` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `auto` | Automatic | yes or no | true |  | Chooses the modulation from the signals. Choosing the modulation by hand turns it off. |
| `modulation` | Modulation | choice | `BPSK31` | `BPSK31`, `BPSK63`, `QPSK31`, `QPSK63`, `BPSK125`, `BPSK250`, `QPSK125` |  |
| `threshold` | Threshold | number (dB) | 5 | 4 to 20 | How far above the noise floor a signal must be to be decoded. |
| `focus_hz` | Focus | frequency | -1 Hz | -1 Hz to 4 kHz | The signal nearest this offset is the focus (-1 for none). |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `tx_centre_hz` | Audio frequency | frequency | 1 kHz | 200 Hz to 3 kHz | The audio frequency of the carrier sent. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The peak magnitude of the signal sent, of full scale. |
| `tx_tune_limit` | Tune time | duration (s) | 10 s | 1 to 60 | The tune carrier ends after this time. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_freq_hz` | Audio frequency | frequency | 1 kHz | 100 Hz to 3.9 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `modulation` | Modulation | choice |
| `floor_db` | Noise floor | number (dB) |
| `focus` | Focus | whole number |
| `characters` | Characters | whole number |
| `errors` | Bad characters | whole number |
| `auto` | Automatic | yes or no |
| `auto_locked` | Automatic lock | yes or no |
| `auto_modulation` | Automatic modulation | text |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Characters sent | whole number |
| `tx_waiting` | Characters waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `heard` (whole number), `last_heard` (time) |
| `signals` | Signals | `id` (whole number), `offset_hz` (frequency), `afc_hz` (frequency), `snr_db` (number (dB)), `quality_pct` (number (%)), `dcd` (yes or no), `manual` (yes or no), `first_s` (number (s)), `last_s` (number (s)), `characters` (whole number), `text` (text), `gone` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Window spectrum | Hz | dB |

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

The stack is in the family `hfkbd`, version 2.8. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Keyboard text (`text_chat`). Media: sound (monitor), pictures, text.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `PskFields`. The result fields of PSK.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `modulation` | choice | always | 1.0 | The modulation decoded. |
| `floor_db` | number (dB) | always | 1.0 | The noise floor of the window. |
| `focus` | whole number | optional | 1.0 | The id of the signal in focus, if one is. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `errors` | whole number | always | 1.0 | Characters that the code table does not have. |
| `auto` | yes or no | always | 1.0 | Auto is on. |
| `auto_locked` | yes or no | always | 1.0 | Auto has applied a guess that still decodes. |
| `auto_modulation` | text | optional | 1.0 | The modulation that Auto found. It has no value before Auto finds one. |

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

### Table `signals`

Struct `PskSignalRow`. A PSK signal heard, or one gone whose text stays. The row key is the signal id. The rows of the gone signals stay over a change of frequency: they are the log of the decoded texts.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The signal id. |
| `offset_hz` | frequency | always | 1.0 | The audio frequency. |
| `afc_hz` | frequency | always | 1.0 | The correction since it was found. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `quality_pct` | number (%) | always | 1.0 | How clean the decisions are, 0 to 100. |
| `dcd` | yes or no | always | 1.0 | Text is coming. |
| `manual` | yes or no | always | 1.0 | The user picked it. |
| `first_s` | number (s) | always | 1.0 | The signal time when it was first heard. |
| `last_s` | number (s) | always | 1.0 | The signal time when it was last heard. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `text` | text | always | 1.0 | The decoded text. |
| `gone` | yes or no | always | 1.0 | It is no longer heard. |

Default order: the column `snr_db`, highest first.

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
| `remove` | `sdrkit_psk_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_psk_clear` | none | none |
| `send` | `sdrkit_psk_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_psk_stop` | none | none |
| `tune` | `sdrkit_psk_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `auto` | `sdrkit_psk_get_auto` | `sdrkit_psk_set_auto` |
| `modulation` | `sdrkit_psk_get_modulation` | `sdrkit_psk_set_modulation` |
| `threshold` | `sdrkit_psk_get_threshold` | `sdrkit_psk_set_threshold` |
| `focus_hz` | `sdrkit_psk_get_focus_hz` | `sdrkit_psk_set_focus_hz` |
| `listen` | `sdrkit_psk_get_listen` | `sdrkit_psk_set_listen` |
| `tx_centre_hz` | `sdrkit_psk_get_tx_centre_hz` | `sdrkit_psk_set_tx_centre_hz` |
| `tx_level` | `sdrkit_psk_get_tx_level` | `sdrkit_psk_set_tx_level` |
| `tx_tune_limit` | `sdrkit_psk_get_tx_tune_limit` | `sdrkit_psk_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_psk_gen_get_message` | `sdrkit_psk_gen_set_message` |
| `gen_freq_hz` | `sdrkit_psk_gen_get_freq_hz` | `sdrkit_psk_gen_set_freq_hz` |

## Try it

```sh
sdrkit run psk --source synth:psk
sdrkit run psk --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Receive: notes

- QPSK sense of rotation: the receiver and the sender use the designer's published phase table. The code window 00001 (the
  newest bit 1) advances the phase by 90 degrees, so on the upper sideband the audio tone's phase advances. QPSK is sideband
  sensitive: a signal received on the lower sideband turns the other way and does not decode. The stack has no `reverse`
  setting for that case.
- Text starts only when the carrier detect has been on for five symbols and two zero bits follow: the noise before a burst
  and the rise of its carrier give no character.
- The carrier is on while its magnitude is at least 1.5 times the noise level measured while no carrier was on: the noise
  after a burst does not move the frequency of a signal far.

## Transmit

The stack sends typed text in any of the seven variants of the receiver. The engine setting `transmit_allowed` is the only
switch: with it off, `send` and `tune` are refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the whole text and queues it. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio frequency for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the centre chosen refuses `send` and `tune`.

The sender sends in the variant of `modulation`, the receiver's setting. Its own settings:

| Key | Meaning | Default |
|---|---|---|
| `tx_centre_hz` | The audio frequency of the carrier, 200 to 3000 Hz | 1000 Hz |
| `tx_level` | The peak of the signal, 0.05 to 0.9 of full scale | 0.5 |
| `tx_tune_limit` | The time of the tune carrier | 10 s |

Characters: bytes 0 to 127 as typed, each with its varicode (ITU-R M.2034); CR LF in the text is one LF. A character above 127
refuses the whole text, and the refusal names it. The signal is always on the upper sideband.

### The signal

- BPSK: bit 0 reverses the phase, bit 1 keeps it. QPSK: each bit enters the K = 5 code (register masks 0x17 and 0x19); the
  code symbol turns the phase. The window 00001 (the newest bit 1) advances the phase by 90 degrees, as the designer's phase
  table states: the audio tone's phase advances. The receiver uses the same table.
- Pulse: during each symbol the envelope goes from the last carrier state to the new one by a raised cosine,
  `z(k-1) (0.5 + 0.5 cos pi u) + z(k) (0.5 - 0.5 cos pi u)`, so a reversal passes through zero at mid symbol and a 1 is a steady
  carrier. Symbol k ends at the sample nearest to k / baud seconds.
- A burst: 0.15 s of silence; the carrier rises from zero over one symbol; a preamble of max(32, baud) zero bits (reversals);
  each character's varicode and two zero bits; a postamble of ones, a steady carrier, as the designer's description ends a
  transmission (max(16, preamble / 2) bits, for QPSK max(48, preamble / 2) and 8 more, which also empty the code); the carrier
  falls to zero over one symbol; 0.1 s of silence. The 10 ms fades at the ends of the signal fall on the rise and the fall.
- Level: the peak of the envelope is `tx_level`. The peak-to-average power ratio is 3 dB on reversals and 0 to 3 dB with text.

### Results and events

`tx_sending` (a burst or the tune carrier is on air), `tx_sent` (characters of the burst coded so far) and `tx_waiting`. The
event `hfkbd.tx` says when text started a burst, was queued, finished (with the reason: stopped, the transmitter is off, the
format changed) or was refused (with the reason).

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate from 12 kHz
up. On a live radio whose host gives a transmit lead, the first sample states its time on air: the time of the pass that writes
it plus the lead and two passes. The last sample carries the end mark.

### Checks

- The bits of the worked example BPSK31 "CQ" (32 reversals, 101011010011101110100, then the steady carrier) and the turns of
  QPSK31 "e" (1, 0, 2 and 1 quarter turns) are read from the coder. The 32 rows of the designer's QPSK phase table are checked
  against the code and the turn. The phase steps of a QPSK31 burst of "CQ", measured on the tone in the samples, are those
  of the table over the five-bit window of the bits sent.
- Pulse: between the boundaries of each symbol of BPSK31 the phase keeps or turns by 180 degrees within 2 degrees, and the
  magnitude at mid symbol of a reversal is under 0.02 of the peak.
- 99 % power bandwidth of a long text, within 10 % of the specification's figures: BPSK31 45.4 Hz (43), QPSK31 42.5 Hz (43),
  BPSK63 92.3 Hz (86), BPSK250 367.7 Hz (352).
- The stack's own receiver at the specification's 12 dB in the baud bandwidth, five noise seeds each: BPSK31, BPSK63,
  BPSK125, BPSK250 and QPSK31 read the whole text of every burst with no character error and nothing before it (145 to 1160
  characters each); QPSK63 and QPSK125 read the text with a character lost or added at the start of a burst in some runs.
  After the burst the signal stays within 0.5 Hz (BPSK) and 4 Hz (QPSK) of its frequency. Without noise the text is exact,
  with nothing before or after it.
- Also covered: the frame of a burst, `stop`, `tune`, the gate and the refused character.

### Open points

- QPSK63 and QPSK125 at 12 dB in the baud bandwidth: a character is lost or added at the start of some bursts. The AFC and
  the phase estimate have not settled at the end of the preamble when the signal starts 3 Hz or more off.
- The noise after a burst can still give a character or a space before the carrier detect ends (about one a burst at 20 dB
  in 2.5 kHz, also with the stack's test signal).
- The synthetic recording `hfkbd_qpsk31_12k` turns the phase the other way for each code symbol: it is read only when
  mirrored about its carrier. It needs to be made again with the published table.
- 8PSK125 and 8PSK250 are not sent: the receiver has no 8PSK.
