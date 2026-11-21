<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SELCAL (aircraft tone call)

Detects the two-tone calls ground stations send to alert an aircraft's crew, on HF or VHF voice channels.

Stack id: `selcal`. Group: Aviation.

Signal types: SELCAL.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 8.891 MHz |
| Tuning range | 1.5 MHz to 400 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 6 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:selcal` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `demod` | Demodulator | choice | `auto` | `auto` (Auto), `fm` (FM), `usb` (USB), `lsb` (LSB), `am` (AM) | Auto: USB below 30 MHz, AM above. |
| `listen` | Listen | yes or no | false |  | Plays the channel audio. Decoding is the same either way. |
| `audio_input` | Audio input | yes or no | false |  | The samples are sound card audio (8 kHz or more in the real part). |
| `aircraft_table` | Aircraft table | text | empty |  | Lines of a SELCAL code and the aircraft's registration, separated by a tab, a comma or a semicolon. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `calls` | Calls | whole number |
| `codes` | Codes | whole number |
| `tone` | Strongest tone | frequency |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calls` | Calls | `time` (time), `signal_s` (number (s)), `code` (text), `code_row` (text), `tones` (text (Hz)), `dial` (frequency), `level_db` (number (dB)), `aircraft` (text) |
| `codes` | Codes | `code` (text), `aircraft` (text), `heard` (whole number), `last_heard` (time), `dial` (frequency) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Audio spectrum | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `tones.selcal` | SELCAL call | `source` (text), `text` (text), `family` (choice), `code` (text), `classic` (yes or no), `level_db` (number (dB)), `shift_hz` (frequency) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `avdata`, version 1.1. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SelcalFields`. The result fields of SELCAL.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `calls` | whole number | always | 1.0 | Calls in the log. |
| `codes` | whole number | always | 1.0 | Codes in the table. |
| `tone` | frequency | optional | 1.0 | The strongest tone of the audio now. No value without a tone. |

### Table `calls`

Struct `SelcalCallRow`. A row of the calls. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The wall-clock time. No value for a recording. |
| `signal_s` | number (s) | always | 1.0 | The signal time. |
| `code` | text | always | 1.0 | The code, "AM-CP". |
| `code_row` | text | always | 1.0 | The code as its row key in the codes. |
| `tones` | text (Hz) | always | 1.0 | The four tones, "a b / c d". |
| `dial` | frequency | optional | 1.0 | The dial frequency. No value when it is not known. |
| `level_db` | number (dB) | always | 1.0 | The level of the tones, dB relative to a full-scale sine. |
| `aircraft` | text | optional | 1.0 | The aircraft the code belongs to. |

### Table `codes`

Struct `SelcalCodeRow`. A row of the codes heard. The row key is the code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | text | always | 1.0 | The code. |
| `aircraft` | text | optional | 1.0 | The aircraft the code belongs to. |
| `heard` | whole number | optional | 1.0 | How often the code was heard. |
| `last_heard` | time | optional | 1.0 | When the code was last heard. |
| `dial` | frequency | optional | 1.0 | The last dial frequency. No value when it is not known. |

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

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_selcal_clear` | none | none |
| `remove` | `sdrkit_selcal_remove` | `sdrkit_selcal_code_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `demod` | `sdrkit_selcal_get_demod` | `sdrkit_selcal_set_demod` |
| `listen` | `sdrkit_selcal_get_listen` | `sdrkit_selcal_set_listen` |
| `audio_input` | `sdrkit_selcal_get_audio_input` | `sdrkit_selcal_set_audio_input` |
| `aircraft_table` | `sdrkit_selcal_get_aircraft_table` | `sdrkit_selcal_set_aircraft_table` |

## Try it

```sh
sdrkit run selcal --source synth:selcal
sdrkit run selcal --source <driver>:<address> --freq 8.891
```
<!-- generated: end -->
