<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CW

Morse code: every keyed carrier in a span of 14 to 192 kHz decoded at once, with the speed of each.

Stack id: `cw`. Group: HF and utility.

Signal types: CW (Morse).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 14 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cw` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold` | Threshold | number (dB) | 10 | 6 to 24 | How far above the noise floor a carrier must be to be decoded. |
| `decoder` | Timing decoder | choice | `adaptive` | `adaptive` (Adaptive), `classic` (Clustering) |  |
| `wpm` | Speed | number (WPM) | 0 | 0 to 50 | 0 finds the speed by itself. 5 to 50 fixes it. |
| `filter` | Filter width | choice | `100hz` | `50hz` (50 Hz), `100hz` (100 Hz), `250hz` (250 Hz), `500hz` (500 Hz) | The channel filter of each carrier. 50 Hz is the most sensitive and passes keying up to 20 WPM, 100 Hz up to 40 WPM, 250 and 500 Hz up to 60 WPM. |
| `listen` | Listen | choice | `off` | `off` (Off), `tone` (Rebuilt tone), `signal` (Real signal) | Off is silence. Tone is a clean tone at the pitch keyed as the signal listened to, and silence while there is none, as there is no key to rebuild. Signal is the signal listened to through its filter at the pitch, and the dial through the filter while there is none. The signal listened to is the focus, else the strongest signal in the table. |
| `pitch` | Pitch | frequency | 700 Hz | 400 Hz to 1 kHz | The audio frequency of the signal listened to, and of a carrier at the dial. |
| `focus_hz` | Focus | frequency | 0 Hz | -96 kHz to 96 kHz | The signal nearest this offset is the focus. A focus where no signal is starts one there, marked as picked. |
| `forget_s` | Forget after | number (s) | 600 | 30 to 3600 | A signal not heard for this long leaves the signals table. After 30 s without a key down it is marked gone. |
| `span` | Span | choice | `14k` | `14k` (14 kHz), `48k` (48 kHz), `96k` (96 kHz), `192k` (192 kHz) | The width of the band that is watched, centred on the dial. A wider span asks the radio for a higher rate. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_message` | Test text | text | empty |  | The text that the test signal sends. Empty: the stack's own test text. |
| `gen_carrier_hz` | Carrier | frequency | 400 Hz | -96 kHz to 96 kHz |  |
| `gen_wpm` | Speed | number (WPM) | 20 | 5 to 60 |  |
| `gen_farnsworth_wpm` | Farnsworth speed | number (WPM) | 0 | 0 to 60 |  |
| `gen_jitter` | Jitter | number | 0 | 0 to 0.5 |  |
| `gen_signals` | Carriers | whole number | 0 | 0 to 100 | 0: the default test signal. 1 to 100: carriers spread evenly over the span, each with its own call sign. |
| `gen_wpm_max` | Top speed | number (WPM) | 0 | 0 to 60 | With several carriers, the speeds step from the speed up to this (0: all at the speed). |
| `gen_level_spread_db` | Level spread | number (dB) | 0 | 0 to 40 | With several carriers, the levels step down evenly over this many dB. |
| `gen_noise_only` | Noise only | yes or no | false |  | No carrier: only the noise of the test signal settings, as loud as with the carriers. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `span_hz` | Span | frequency |
| `floor_db` | Noise floor | number (dB) |
| `focus` | Focus | whole number |
| `characters` | Characters | whole number |
| `errors` | Bad characters | whole number |
| `dot_ms` | Dot | number (ms) |
| `dash_ratio` | Dash ratio | number |
| `char_gap` | Character gap | number (dots) |
| `word_gap` | Word gap | number (dots) |
| `steadiness_pct` | Steadiness | number (%) |
| `wpm` | Speed | number (WPM) |
| `signals` | Signals | whole number |
| `now_s` | Now | number (s) |
| `listened` | Listened to | whole number |
| `playing` | Playing | choice |
| `listened_hz` | Frequency played | frequency |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations heard | `callsign` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `heard` (whole number), `last_heard` (time) |
| `signals` | Signals | `id` (whole number), `offset_hz` (frequency), `wpm` (number (WPM)), `snr_db` (number (dB)), `key_down` (yes or no), `locked` (yes or no), `farnsworth` (yes or no), `manual` (yes or no), `first_s` (number (s)), `last_s` (number (s)), `characters` (whole number), `text` (text), `confidence` (text), `alternatives` (text), `gone` (yes or no), `errors` (whole number), `freq_hz` (frequency), `level_dbfs` (number (dBFS)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `marks` | Mark lengths | ln(length/dot) | count |
| `gaps` | Gap lengths | ln(length/dot) | count |
| `spectrum` | Spectrum | Hz | dBFS |
| `key` | Key | s | down |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfkbd.spot` | Callsign heard | `callsign` (text), `source` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `family` (choice) |
| `hfkbd.text` | Text line | `source` (text), `text` (text), `snr_db` (number (dB)), `mode` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |

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

Struct `CwFields`. The result fields of the CW skimmer. The fields of the sender have no value without a signal in focus.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `span_hz` | frequency | always | 2.0 | The width of the band watched, centred on the dial. |
| `floor_db` | number (dB) | always | 1.0 | The noise floor of the window. |
| `focus` | whole number | optional | 1.0 | The id of the signal in focus, if one is. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `errors` | whole number | always | 1.0 | Characters that the code table does not have. |
| `dot_ms` | number (ms) | optional | 1.0 | The dot of the signal in focus. |
| `dash_ratio` | number | optional | 1.0 | Its dash as dots. |
| `char_gap` | number (dots) | optional | 1.0 | Its character gap. |
| `word_gap` | number (dots) | optional | 1.0 | Its word gap. |
| `steadiness_pct` | number (%) | optional | 1.0 | How steady its keying is. |
| `wpm` | number (WPM) | optional | 1.0 | Its speed. |
| `signals` | whole number | always | 2.0 | The signals in the table that are heard now. |
| `now_s` | number (s) | always | 2.0 | The signal time now, for the age of a signal's last-heard time. |
| `listened` | whole number | optional | 2.1 | The id of the signal listened to (the row key in the signals table): the focus, else the strongest signal in the table. Absent while there is none, also while the audio plays the dial. |
| `playing` | choice | always | 2.2 | What the audio plays: nothing, the signal listened to, or the dial while there is none. |
| `listened_hz` | frequency | optional | 2.2 | The radio frequency that the audio plays: that of the signal listened to, or the dial. Absent while it plays nothing. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

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

Struct `CwSignalRow`. A CW signal heard, or one gone whose text stays. The row key is the signal id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The signal id. |
| `offset_hz` | frequency | always | 1.0 | The audio frequency. |
| `wpm` | number (WPM) | always | 1.0 | The speed. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `key_down` | yes or no | always | 1.0 | The key is down. |
| `locked` | yes or no | always | 1.0 | The speed is known. |
| `farnsworth` | yes or no | always | 1.0 | The sender spaces as Farnsworth. |
| `manual` | yes or no | always | 1.0 | The user picked it. |
| `first_s` | number (s) | always | 1.0 | The signal time when it was first heard. |
| `last_s` | number (s) | always | 1.0 | The signal time when it was last heard. |
| `characters` | whole number | always | 1.0 | Characters decoded. |
| `text` | text | always | 1.0 | The decoded text. |
| `confidence` | text | always | 1.0 | The confidence of each character, 0 to 9. |
| `alternatives` | text | always | 1.0 | The other readings of unclear characters. |
| `gone` | yes or no | always | 1.0 | It is no longer heard. |
| `errors` | whole number | always | 1.0 | Characters that the code table does not have. |
| `freq_hz` | frequency | always | 2.0 | The radio frequency of the carrier: the dial plus the offset. |
| `level_dbfs` | number (dBFS) | always | 2.0 | The level of the carrier while the key is down, relative to full scale. |

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

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `remove` | `sdrkit_cw_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_cw_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold` | `sdrkit_cw_get_threshold` | `sdrkit_cw_set_threshold` |
| `decoder` | `sdrkit_cw_get_decoder` | `sdrkit_cw_set_decoder` |
| `wpm` | `sdrkit_cw_get_wpm` | `sdrkit_cw_set_wpm` |
| `filter` | `sdrkit_cw_get_filter` | `sdrkit_cw_set_filter` |
| `listen` | `sdrkit_cw_get_listen` | `sdrkit_cw_set_listen` |
| `pitch` | `sdrkit_cw_get_pitch` | `sdrkit_cw_set_pitch` |
| `focus_hz` | `sdrkit_cw_get_focus_hz` | `sdrkit_cw_set_focus_hz` |
| `forget_s` | `sdrkit_cw_get_forget_s` | `sdrkit_cw_set_forget_s` |
| `span` | `sdrkit_cw_get_span` | `sdrkit_cw_set_span` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_message` | `sdrkit_cw_gen_get_message` | `sdrkit_cw_gen_set_message` |
| `gen_carrier_hz` | `sdrkit_cw_gen_get_carrier_hz` | `sdrkit_cw_gen_set_carrier_hz` |
| `gen_wpm` | `sdrkit_cw_gen_get_wpm` | `sdrkit_cw_gen_set_wpm` |
| `gen_farnsworth_wpm` | `sdrkit_cw_gen_get_farnsworth_wpm` | `sdrkit_cw_gen_set_farnsworth_wpm` |
| `gen_jitter` | `sdrkit_cw_gen_get_jitter` | `sdrkit_cw_gen_set_jitter` |
| `gen_signals` | `sdrkit_cw_gen_get_signals` | `sdrkit_cw_gen_set_signals` |
| `gen_wpm_max` | `sdrkit_cw_gen_get_wpm_max` | `sdrkit_cw_gen_set_wpm_max` |
| `gen_level_spread_db` | `sdrkit_cw_gen_get_level_spread_db` | `sdrkit_cw_gen_set_level_spread_db` |
| `gen_noise_only` | `sdrkit_cw_gen_get_noise_only` | `sdrkit_cw_gen_set_noise_only` |

## Try it

```sh
sdrkit run cw --source synth:cw
sdrkit run cw --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/hfkbd/hfkbd_cw_12k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## How the skimmer works

- **Span.** The stack watches 14, 48, 96 or 192 kHz around the dial. For 48 kHz and more it asks the radio for 128, 192 or
  320 kHz, so the band, which lies 20 kHz below the radio centre, stays inside 0.4 of the rate. For 14 kHz it keeps the rate
  of the tuning table. The carrier search uses bins of about 12 Hz at every span.
- **Channels.** A shared fast-convolution bank cuts a channel of ±300 Hz at 2 or 3 kHz for each carrier, so a carrier costs
  the same at every span. At most 100 carriers are followed. A new carrier gets the last second of its channel, so its first
  elements are not lost.
- **Filter width.** Measured on the test signal: 50 Hz decodes up to 20 WPM, 100 Hz up to 40 WPM, 250 and 500 Hz up to 60 WPM.
  The lowest SNR in 500 Hz at which three of three seeds decode `DE K0TST K0TST` is 5 dB (50 Hz) and 8 dB (100 Hz) at 12 WPM,
  and 6 dB (50 Hz), 9 dB (100 Hz) and 15 dB (500 Hz) at 15 WPM.
- **Rows.** A signal shows in the table after it decoded 4 clear characters (not E, T or I) at a known speed of 5 to 60 WPM,
  or when it is the focus or was picked. A signal within 250 Hz of a stronger one whose key follows it (its keying sidebands)
  is dropped, and so is a signal that does not show within 30 s. A signal with no key down for 30 s is marked gone; its row
  leaves after `forget_s`.
- **Focus and audio.** `focus_hz` 0 means no focus. A focus where no signal is starts one there, marked as picked; it is no
  longer picked once the focus moves. Listen plays the focus, or without a focus the strongest signal in the table. With
  no signal to listen to, `signal` plays the dial through the filter, a carrier at the dial at the pitch, and `tone` plays
  nothing. The field `playing` says which (`nothing`, `signal` or `dial`) and `listened_hz` gives the radio frequency
  played; `listened` has no value on the dial. A change of what plays fades in over 30 ms.
- **Series.** `spectrum` is the averaged spectrum of the span (at most 4096 points, each the greatest of its bins), `key` the
  key changes of the signal listened to in the last 2 s, and `now_s` the signal time for the age of a row.
- **Known limit.** The shared timing decoder misses some word spaces once a pause of 2 s is among the gaps it learned from
  (`TESTDEN0XYZ`); the characters are right.
