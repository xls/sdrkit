<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Tone decoders (CTCSS, DCS, DTMF, selcall)

CTCSS tones, DCS codes, DTMF keys, sequential calls, SELCAL and two-tone pages, with a gate that opens for a chosen tone.

Stack id: `tones`. Group: Tones and selective calling.

Signal types: Narrow FM voice, Keyed voice.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 145.5 MHz |
| Tuning range | 1.5 MHz to 1.3 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:tones` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `demod` | Demodulation | choice | `auto` | `auto` (auto (USB on HF, FM above 30 MHz)), `fm` (FM), `usb` (USB), `lsb` (LSB), `am` (AM) |  |
| `audio_input` | Audio input | yes or no | false |  | The input is audio (a sound card or an audio recording): real samples, 1.0 is 5 kHz of deviation. The source sets it. |
| `gate` | Gate | choice | `always` | `always`, `any` (on any tone or code), `chosen` (only on the chosen tone or code) |  |
| `gate_kind` | Gate on | choice | `ctcss` | `ctcss` (CTCSS), `dcs` (DCS) |  |
| `gate_tone` | Tone | choice | `100.0` | `67.0` (67.0 Hz), `69.3` (69.3 Hz), `71.9` (71.9 Hz), `74.4` (74.4 Hz), `77.0` (77.0 Hz), `79.7` (79.7 Hz), `82.5` (82.5 Hz), `85.4` (85.4 Hz), `88.5` (88.5 Hz), `91.5` (91.5 Hz), `94.8` (94.8 Hz), `97.4` (97.4 Hz), `100.0` (100.0 Hz), `103.5` (103.5 Hz), `107.2` (107.2 Hz), `110.9` (110.9 Hz), `114.8` (114.8 Hz), `118.8` (118.8 Hz), `123.0` (123.0 Hz), `127.3` (127.3 Hz), `131.8` (131.8 Hz), `136.5` (136.5 Hz), `141.3` (141.3 Hz), `146.2` (146.2 Hz), and others | The CTCSS tone that opens the gate. |
| `gate_code` | Code | choice | `023` | `023` (D023), `025` (D025), `026` (D026), `031` (D031), `032` (D032), `043` (D043), `047` (D047), `051` (D051), `054` (D054), `065` (D065), `071` (D071), `072` (D072), `073` (D073), `074` (D074), `114` (D114), `115` (D115), `116` (D116), `125` (D125), `131` (D131), `132` (D132), `134` (D134), `143` (D143), `152` (D152), `155` (D155), and others | The DCS code that opens the gate. |
| `gate_inverted` | Code inverted | yes or no | false |  |  |
| `listen` | Listen | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `audio_db` | Audio level | number (dB) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `carrier_recent` | Carrier seen in 5 s | yes or no |
| `ctcss_hz` | CTCSS tone | frequency |
| `dcs` | DCS code | text |
| `dcs_alias` | DCS alias | text |
| `strongest_hz` | Strongest tone | frequency |
| `dtmf` | DTMF keys | text |
| `gate_open` | Gate open | yes or no |
| `input_rate_hz` | Input rate | frequency |
| `audio_input` | Audio input | yes or no |
| `demod` | Demodulation | choice |
| `detections` | Detections | whole number |
| `ctcss_count` | CTCSS tones | whole number |
| `dcs_count` | DCS codes | whole number |
| `dtmf_count` | DTMF strings | whole number |
| `selcall_count` | Sequential calls | whole number |
| `selcal_count` | SELCAL calls | whole number |
| `two_tone_count` | Two-tone pages | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Tones heard | `start_s` (number (s)), `kind` (choice), `text` (text), `length` (duration (s)), `detail` (text), `level_db` (number (dB)), `active` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Audio spectrum | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `tones.subaudio` | CTCSS or DCS | `family` (choice), `text` (text), `kind` (choice), `event` (choice), `tone_hz` (frequency), `eia` (yes or no), `motorola` (text), `measured_hz` (frequency), `code` (text), `standard` (yes or no), `alias` (text), `level_db` (number (dB)) |
| `tones.dtmf` | DTMF key | `family` (choice), `text` (text), `key` (text), `down` (yes or no), `row_db` (number (dB)), `column_db` (number (dB)) |
| `tones.selcall` | Selcall call | `source` (text), `text` (text), `digits` (text), `sent` (text), `set` (text), `also_fits` (text), `length_s` (duration (s)), `tone_s` (duration (s)) |
| `tones.selcal` | SELCAL call | `source` (text), `text` (text), `family` (choice), `code` (text), `classic` (yes or no), `level_db` (number (dB)), `shift_hz` (frequency) |
| `tones.two_tone` | Two-tone page | `text` (text), `a_hz` (frequency), `b_hz` (frequency), `a_s` (duration (s)), `b_s` (duration (s)), `ended` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `trunk`, version 1.2. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `TonesFields`. The result fields of the tone decoders.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `audio_db` | number (dB) | always | 1.0 | The audio level. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the carrier. No value for audio input. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the carrier from the tuned frequency. No value for audio input. |
| `carrier_recent` | yes or no | optional | 1.0 | A carrier was seen in the last 5 s. No value for audio input. |
| `ctcss_hz` | frequency | optional | 1.0 | The CTCSS tone that is locked now. |
| `dcs` | text | optional | 1.0 | The DCS code that is locked now. |
| `dcs_alias` | text | optional | 1.0 | The other reading of the waveform of the DCS code. |
| `strongest_hz` | frequency | optional | 1.0 | The strongest tone of the audio spectrum. |
| `dtmf` | text | always | 1.0 | The DTMF keys of the string that goes on now. |
| `gate_open` | yes or no | always | 1.0 | The listening gate is open. |
| `input_rate_hz` | frequency | always | 1.0 | The sample rate of the input. |
| `audio_input` | yes or no | always | 1.0 | The input is audio. |
| `demod` | choice | always | 1.0 | The demodulation in use. |
| `detections` | whole number | always | 1.0 | Every decision of the decoders. |
| `ctcss_count` | whole number | always | 1.0 | The CTCSS tones heard. |
| `dcs_count` | whole number | always | 1.0 | The DCS codes heard. |
| `dtmf_count` | whole number | always | 1.0 | The DTMF strings heard. |
| `selcall_count` | whole number | always | 1.0 | The sequential calls heard. |
| `selcal_count` | whole number | always | 1.0 | The SELCAL calls heard. |
| `two_tone_count` | whole number | always | 1.0 | The two-tone pages heard. |

### Table `events`

Struct `TonesHeardRow`. A row of the list of tones heard. The row key is the sequence number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start_s` | number (s) | always | 1.0 | The signal time of the start. |
| `kind` | choice | always | 1.0 | What the row holds. |
| `text` | text | always | 1.0 | What was heard ("123.0 Hz", "D023N", "159#", "12345", "AM-CP", "682.5 + 1000.0 Hz"). |
| `length` | duration (s) | always | 1.0 | The length so far. |
| `detail` | text | always | 1.0 | The details. |
| `level_db` | number (dB) | always | 1.0 | The level, relative to a full-scale sine. |
| `active` | yes or no | always | 1.0 | It still goes on. |

### Event `tones.subaudio`

Struct `SubAudioFields`. The fields of a `tones.subaudio` event: a change of a CTCSS tone or of a DCS code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `text` | text | always | 1.0 | One line for people. |
| `kind` | choice | always | 1.0 | The signalling. |
| `event` | choice | always | 1.0 | The change. |
| `tone_hz` | frequency | optional | 1.0 | The CTCSS tone. |
| `eia` | yes or no | optional | 1.0 | The CTCSS tone is an EIA tone. |
| `motorola` | text | optional | 1.0 | The Motorola code of the CTCSS tone. |
| `measured_hz` | frequency | optional | 1.0 | The measured CTCSS tone. |
| `code` | text | optional | 1.0 | The DCS code with its polarity ("D023N"). |
| `standard` | yes or no | optional | 1.0 | The DCS code is a standard code. |
| `alias` | text | optional | 1.0 | A standard code that the same word also reads as. |
| `level_db` | number (dB) | always | 1.0 | The level of the tone, dB relative to a full-scale sine. |

### Event `tones.dtmf`

Struct `DtmfFields`. The fields of a `tones.dtmf` event: a DTMF key that went down or up.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `text` | text | always | 1.0 | One line for people: the key. |
| `key` | text | always | 1.0 | The DTMF key. |
| `down` | yes or no | always | 1.0 | The key went down (or up). |
| `row_db` | number (dB) | always | 1.0 | The level of the row tone. |
| `column_db` | number (dB) | always | 1.0 | The level of the column tone. |

### Event `tones.selcall`

Struct `SelcallFields`. The fields of a `tones.selcall` event: a selcall call.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The digits of the call. |
| `text` | text | always | 1.0 | One line for people: the digits. |
| `digits` | text | always | 1.0 | The digits of a call as heard. |
| `sent` | text | always | 1.0 | The digits as sent, with the repeat tones. |
| `set` | text | always | 1.0 | The tone set the call was read as. |
| `also_fits` | text | optional | 1.0 | The other tone sets the call fits, separated by commas. |
| `length_s` | duration (s) | always | 1.0 | The length of the call. |
| `tone_s` | duration (s) | always | 1.0 | The mean length of a tone. |

### Event `tones.selcal`

Struct `SelcalFields`. The fields of a `tones.selcal` event: a SELCAL call.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The code. |
| `text` | text | always | 1.0 | One line for people: the code. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `code` | text | always | 1.0 | The SELCAL code ("AB-CD"). |
| `classic` | yes or no | always | 1.0 | The code uses only the 16 classic tones. |
| `level_db` | number (dB) | always | 1.0 | The level of the tones, dB relative to a full-scale sine. |
| `shift_hz` | frequency | always | 1.0 | The shift of the tones from their frequencies (a sideband receiver off tune). |

### Event `tones.two_tone`

Struct `TwoToneFields`. The fields of a `tones.two_tone` event: a two-tone page.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | One line for people: the two tones. |
| `a_hz` | frequency | always | 1.0 | The frequency of tone A. |
| `b_hz` | frequency | always | 1.0 | The frequency of tone B. |
| `a_s` | duration (s) | always | 1.0 | The length of tone A. |
| `b_s` | duration (s) | always | 1.0 | The length of tone B. |
| `ended` | yes or no | always | 1.0 | Tone B ended (it did not run past the end of the input). |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_tones_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `demod` | `sdrkit_tones_get_demod` | `sdrkit_tones_set_demod` |
| `audio_input` | `sdrkit_tones_get_audio_input` | `sdrkit_tones_set_audio_input` |
| `gate` | `sdrkit_tones_get_gate` | `sdrkit_tones_set_gate` |
| `gate_kind` | `sdrkit_tones_get_gate_kind` | `sdrkit_tones_set_gate_kind` |
| `gate_tone` | `sdrkit_tones_get_gate_tone` | `sdrkit_tones_set_gate_tone` |
| `gate_code` | `sdrkit_tones_get_gate_code` | `sdrkit_tones_set_gate_code` |
| `gate_inverted` | `sdrkit_tones_get_gate_inverted` | `sdrkit_tones_set_gate_inverted` |
| `listen` | `sdrkit_tones_get_listen` | `sdrkit_tones_set_listen` |

## Try it

```sh
sdrkit run tones --source synth:tones
sdrkit run tones --source iiod:192.168.2.1 --freq 145.5
```
<!-- generated: end -->
