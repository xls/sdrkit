<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# MFSK

Multi-tone keyboard mode with error correction (MFSK16 and its family), and the pictures sent with it.

Stack id: `mfsk`. Group: HF and utility.

Signal types: MFSK.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mfsk` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `variant` | Variant | choice | `MFSK16` | `MFSK4`, `MFSK8`, `MFSK11`, `MFSK16`, `MFSK22`, `MFSK31`, `MFSK32`, `MFSK64`, `MFSK128`, `MFSK64L`, `MFSK128L` |  |
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
| `gen_picture_width` | Picture width | whole number (px) | 0 | 0 to 320 | A grey test picture of this width follows 20 characters. 0 sends none. |
| `gen_picture_height` | Picture height | whole number (px) | 30 | 1 to 256 |  |

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
| `picture_id` | Picture | whole number |
| `picture_width` | Width | whole number (px) |
| `picture_height` | Height | whole number (px) |
| `picture_rows` | Rows | whole number |
| `picture_progress_pct` | Progress | number (%) |
| `picture_complete` | Complete | yes or no |
| `picture_colour` | Colour | yes or no |
| `picture_mode` | Picture mode | text |
| `pictures` | Pictures | whole number |
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
| `mfsk.picture` | Picture received | `picture_id` (whole number), `picture_width` (whole number (px)), `picture_height` (whole number (px)), `picture_rows` (whole number), `picture_complete` (yes or no), `picture_mode` (text) |
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

Struct `PictureFields`. The picture being received, or else the newest one. Each field of a picture has no value before the first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `picture_id` | whole number | optional | 1.0 | The number of the picture. |
| `picture_width` | whole number (px) | optional | 1.0 | Its width. |
| `picture_height` | whole number (px) | optional | 1.0 | Its height. |
| `picture_rows` | whole number | optional | 1.0 | The rows received. |
| `picture_progress_pct` | number (%) | optional | 1.0 | How much of it is in. |
| `picture_complete` | yes or no | optional | 1.0 | It is complete. |
| `picture_colour` | yes or no | optional | 1.0 | It is in colour. |
| `picture_mode` | text | optional | 1.0 | Its mode. |
| `pictures` | whole number | always | 1.0 | The pictures finished. |

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

### Event `mfsk.picture`

Struct `PictureEvent`. The fields of a picture received or cut off.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `picture_id` | whole number | always | 1.0 | The number of the picture. |
| `picture_width` | whole number (px) | always | 1.0 | Its width. |
| `picture_height` | whole number (px) | always | 1.0 | Its height. |
| `picture_rows` | whole number | always | 1.0 | The rows received. |
| `picture_complete` | yes or no | always | 1.0 | It is complete. |
| `picture_mode` | text | always | 1.0 | Its mode. |

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
| `remove` | `sdrkit_mfsk_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_mfsk_clear` | none | none |
| `send` | `sdrkit_mfsk_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_mfsk_stop` | none | none |
| `tune` | `sdrkit_mfsk_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `variant` | `sdrkit_mfsk_get_variant` | `sdrkit_mfsk_set_variant` |
| `centre` | `sdrkit_mfsk_get_centre` | `sdrkit_mfsk_set_centre` |
| `find_signal` | `sdrkit_mfsk_get_find_signal` | `sdrkit_mfsk_set_find_signal` |
| `demod` | `sdrkit_mfsk_get_demod` | `sdrkit_mfsk_set_demod` |
| `listen` | `sdrkit_mfsk_get_listen` | `sdrkit_mfsk_set_listen` |
| `tx_level` | `sdrkit_mfsk_get_tx_level` | `sdrkit_mfsk_set_tx_level` |
| `tx_tune_limit` | `sdrkit_mfsk_get_tx_tune_limit` | `sdrkit_mfsk_set_tx_tune_limit` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_mfsk_gen_get_message` | `sdrkit_mfsk_gen_set_message` |
| `gen_picture_width` | `sdrkit_mfsk_gen_get_picture_width` | `sdrkit_mfsk_gen_set_picture_width` |
| `gen_picture_height` | `sdrkit_mfsk_gen_get_picture_height` | `sdrkit_mfsk_gen_set_picture_height` |

## Try it

```sh
sdrkit run mfsk --source synth:mfsk
sdrkit run mfsk --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text in each of the eleven variants. The engine setting `transmit_allowed` is the only switch: with it
off, `send` and `tune` are refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the whole text and queues it. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the settings chosen refuses `send` and `tune`.

The sender shares `variant`, `centre` and `demod` with the receiver, so both sides use one value. Its own settings are
`tx_level` (the peak, 0.05 to 0.9 of full scale, default 0.5) and `tx_tune_limit` (default 10 s). With `demod` on LSB the
signal is the mirror image; NFM and AM refuse `send`.

Characters: every character, as its UTF-8 bytes; each byte is its code of the MFSK varicode (all 256 bytes).

### Results and events

`tx_sending` (a burst or the tune carrier is on air), `tx_sent` (characters of the burst coded so far) and `tx_waiting`. The
event `hfkbd.tx` says when text started a burst, was queued, finished (with the reason: stopped, the transmitter is off, the
format changed) or was refused (with the reason).

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate from 12 kHz
up. On a live radio whose host gives a transmit lead, the first sample states its time on air: the time of the pass that writes
it plus the lead and two passes. The last sample carries the end mark. A burst is framed by 0.15 s of silence before and 0.1 s
after, with a 10 ms raised-cosine fade at each end of the signal. The envelope is constant: the peak is `tx_level`, the peak-to-average power ratio 0 dB.

### The signal

- One tone per symbol with a continuous phase; tone k at centre - (tones - 1) spacing / 2 + k spacing. MFSK11 and MFSK22 keep
  their symbols of 1024 and 512 samples at 11025 S/s.
- Each data bit goes through the K = 7, rate 1/2 code (0x6D, then 0x4F; the register starts at zero). S code bits (S the bits
  per symbol) make a word, the first collected most significant; bit k of a word is delayed by k x depth words (the delay lines
  start full of zeros); the tone is the Gray code of the word.
- A burst: MFSK16 starts with 8 symbols of the lowest tone; the other variants with (2 P) mod S zero code bits and P / 3 zero
  data bits (P the preamble count of the variant; none for MFSK64L and MFSK128L). Then CR, STX, CR, the text, a 1, (S - 1) x
  depth x S / 2 + 70 zero bits (130 for MFSK16), and zero bits until the last word is complete.

"CQ" at MFSK16: after the 8 symbols of tone 0 the tones are 12, 0, 0, 0, 12, 0, 12, 12, 0, 12, 10, 12, 12, 10, 6, 0, 10, 6, 6,
0, 6, 3; at a centre of 1500 Hz tone k is at 1382.8125 + 15.625 k Hz.

### Checks

- The tones of the worked examples (MFSK16 and MFSK31 with "CQ") and the code: a single 1 gives the pairs 3, 2, 3, 3, 0, 1, 3.
- The stack's own receiver reads every variant whole without noise (signal finder off) and at 30 dB in 2.5 kHz. Over the seeds
  1 to 10: MFSK16 at -4 dB 10 of 10, MFSK64 at 6 dB 10 of 10.
- The 99 % power bandwidth: MFSK16 252.0 Hz, MFSK32 505.4 Hz (the specification's figures 252 and 504 Hz); every variant within
  the span of its tones and 1.5 baud each side. Every phase step is one of the tones. With LSB on both sides the text goes
  through.
- Also covered: the frame of a burst, `stop`, `tune` and the gate.

### Settled and open points

- The start of MFSK16 (8 symbols of the lowest tone) follows the MFSK16 description. No public document gives the start of the
  other variants, the length of the flush, the place of the tones from the centre or the 99 % power figures; those follow the
  specification.
- Gray code: the two public documents point to opposite directions and do not define the bit order in a word. The sender uses
  the receiver's convention (the tone is the Gray code of the word, the first code bit most significant). One recording of
  another station's MFSK16 settles it.
- After digital silence the AGC of the front end starts its level at zero, so the first tens of ms of a burst come out many
  times too loud. The receiver's tone alignment and the signal finder now count every symbol (every spectrum) as a share of
  its own energy, so such a burst no longer moves the alignment or the centre; before, most variants lost characters of a
  noise-free burst. The overshoot itself stays (it is in the shared AGC).
- `stop` ends the signal at once with the 10 ms fade, as in every keyboard sender, not at the end of a symbol.
