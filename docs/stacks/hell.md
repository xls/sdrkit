<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Hellschreiber

Text sent as pictures of letters, column by column: the screen draws what it hears.

Stack id: `hell`. Group: HF and utility.

Signal types: Hellschreiber.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hell` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `variant` | Variant | choice | `Feld Hell` | `Feld Hell`, `Slow Hell`, `Hell 80`, `HellX5`, `HellX9`, `FSK Hell 245`, `FSK Hell 105` |  |
| `centre` | Audio centre | frequency | 1.5 kHz | 300 Hz to 3.5 kHz |  |
| `find_signal` | Find signal | yes or no | true |  | The centre follows the signal while nothing is decoding. |
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The peak magnitude of the signal sent, of full scale. |
| `tx_tune_limit` | Tune time | duration (s) | 10 s | 1 to 60 | The tune carrier ends after this time. |
| `font` | Font file | text | empty |  | A TrueType font file (or OpenType with TrueType outlines) that the host names. The characters that the Hell glyphs do not have are sent with it, 14 pixels high; ASCII keeps the Hell glyphs. Empty, or a file that cannot be read: the Hell glyphs alone, and ? for the rest. |

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
| `columns` | Columns | whole number |
| `snr_db` | SNR | number (dB) |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Characters sent | whole number |
| `tx_waiting` | Characters waiting | whole number |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfkbd.tx` | Keyboard transmission | `state` (text), `reason` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a text | `text` (text, required) |  | Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on. |
| `stop` | Stop sending |  |  | Drops the text waiting and ends the signal with a 10 ms fade. |
| `tune` | Tune |  |  | Sends an unmodulated carrier at the audio centre for the tune time. |

## Typed interface

The stack is in the family `hfkbd`, version 3.0. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Pictures and fax (`pictures`). Media: sound (monitor), pictures.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `HellFields`. The result fields of Hellschreiber.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `variant` | text | always | 1.0 | The variant in use. |
| `centre_hz` | frequency | always | 1.0 | The audio centre now. |
| `columns` | whole number | always | 1.0 | The pixel columns received. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |

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
| `clear` | `sdrkit_hell_clear` | none | none |
| `send` | `sdrkit_hell_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `stop` | `sdrkit_hell_stop` | none | none |
| `tune` | `sdrkit_hell_tune` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `variant` | `sdrkit_hell_get_variant` | `sdrkit_hell_set_variant` |
| `centre` | `sdrkit_hell_get_centre` | `sdrkit_hell_set_centre` |
| `find_signal` | `sdrkit_hell_get_find_signal` | `sdrkit_hell_set_find_signal` |
| `demod` | `sdrkit_hell_get_demod` | `sdrkit_hell_set_demod` |
| `listen` | `sdrkit_hell_get_listen` | `sdrkit_hell_set_listen` |
| `tx_level` | `sdrkit_hell_get_tx_level` | `sdrkit_hell_set_tx_level` |
| `tx_tune_limit` | `sdrkit_hell_get_tx_tune_limit` | `sdrkit_hell_set_tx_tune_limit` |
| `font` | `sdrkit_hell_get_font` | `sdrkit_hell_set_font` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_hell_gen_get_message` | `sdrkit_hell_gen_set_message` |

## Try it

```sh
sdrkit run hell --source synth:hell
sdrkit run hell --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends typed text as columns of dots in each of the seven variants. The engine setting `transmit_allowed` is the only
switch: with it off, `send` and `tune` are refused and a burst on air ends at once.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Queues the text. A burst starts when none is on air; text sent during a burst joins it. At most 600 characters wait. |
| `stop` | Drops the text waiting and ends the signal with a 10 ms fade and the end mark. |
| `tune` | A steady carrier at the audio centre for `tx_tune_limit` seconds (1 to 60, default 10). |

A signal that does not fit 200 to 3800 Hz above the dial at the centre chosen refuses `send` and `tune`.

The sender shares `variant`, `centre` and `demod` with the receiver. Its own settings are `tx_level` (the peak, 0.05 to 0.9 of
full scale, default 0.5), `tx_tune_limit` (default 10 s) and `font` (a font file, default empty). With `demod` on LSB the signal is the mirror image; NFM and AM
refuse `send`.

Characters: every character is sent; CR and LF are sent as a space. A character that the shared Hell font has no glyph for
is drawn with the font file of `font`, 14 pixels high: a TrueType font (or OpenType with TrueType outlines) that the host
names. ASCII always keeps the Hell glyphs. Without a font file, with a file that cannot be read, or for a character that
neither has, the character is sent as `?`.

### The signal

- A column is 14 pixels, the bottom pixel first, at 14 times the column rate (245 pixels a second in Feld Hell). Pixel k ends
  at the sample nearest to k pixel times.
- Each character is a blank column, the glyph's columns from the first column of its cell to its last inked column, and a blank
  column, so two blank columns part two characters. A space is five blank columns. The glyphs are those of the shared Hell
  font, each font row two pixels high.
- A burst: 0.15 s of silence, three full stops, the text, one space, 0.1 s of silence, with a 10 ms fade at each end.
- On-off variants: an inked pixel is the tone, a blank one silence, the phase running on. In Feld Hell and Slow Hell a run of
  ink rises and falls by a raised cosine of min(4 ms, one pixel) inside its first and last pixel; HellX5 and HellX9 are keyed
  square. The peak is `tx_level`; the envelope varies, so the chain must be linear.
- FSK variants (Hell 80, FSK Hell 245, FSK Hell 105): the tone is always on; an inked pixel is at centre + shift / 2 and a
  blank one at centre - shift / 2 (shifts 300, 122.5 and 55 Hz), with a continuous phase and a constant envelope.

### Results and events

`tx_sending`, `tx_sent` (characters of the burst coded so far) and `tx_waiting`. The event `hfkbd.tx` says when text started a
burst, was queued, finished (with the reason) or was refused (with the reason). On a live radio whose host gives a transmit
lead, the first sample states its time on air (the time of the pass that writes it plus the lead and two passes); the last
sample carries the end mark.

### Checks

- The columns of the worked examples ("C", the three full stops and "T") and the five blank columns of a space; "C" lasts
  0.343 s in Feld Hell.
- Feld Hell rises from 10 % to 90 % in 2 to 3 ms; HellX5 within one sample. In FSK Hell 245 and Hell 80 an inked pixel is on
  the higher tone and every phase step is one of the two tones.
- The stack's own receiver draws the burst at 20 dB in 2.5 kHz; its last 60 columns match the sender's columns in 95.2 % of the
  pixels in Feld Hell, 95.8 % in Slow Hell (a short text), 98.9 % in Hell 80, 100 % in HellX5 and HellX9, 99.9 % in FSK Hell 245
  and 99.5 % in FSK Hell 105 (the limit is 92 %).
- The 99 % power bandwidth of a Feld Hell text is 304.7 Hz, under the 492 Hz that the specification gives for random pixels.
- With a font file (Arial on the test machine), Ł and Ş are sent with the font's glyphs between their blank columns and as `?`
  without it; ASCII keeps the Hell glyphs. A Feld Hell text of Polish and Turkish letters at 20 dB: the last 60 columns
  match the sender's columns in 93.0 % of the pixels (the limit for non-ASCII text is 90 %). A file that cannot be read
  sends `?`.
- Also covered: the frame of a burst, `stop`, `tune` and the gate.

### Settled points

- FSK polarity: the black dot is on the higher tone, as the public description of Hell 80 gives (1925 Hz black, 1625 Hz white).
  The receiver and the test signal already had it so.
- Layout: the sender parts characters by two blank columns and sends a space as five; the test signal keeps one and three. The
  receiver needs no font and draws either.
- AM gate: the floor and the peak follow slowly, so after a step of the level (the start of the stream, noise after silence,
  the gain control settling) they differ more than noise does. The gate opens after 3 columns in a row whose highest pixel
  is 6 times their lowest, once a pixel near the floor comes after one near the peak (blank rows come back after ink, the
  low level before a step up does not). Within 16 columns of an open gate it opens again at once. The columns since the
  contrast began (up to 16) are drawn when it opens. Noise alone, with and without silence before it or in it, draws no
  column in Feld Hell, Hell 80, HellX5 and FSK Hell 245 (48 runs each). At 0 dB in 2.5 kHz Feld Hell draws 97 % of the
  columns of the text; HellX5 at 5 dB about 55 %.
