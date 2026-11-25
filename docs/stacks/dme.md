<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DME

Pulse pairs of a DME beacon or of interrogating aircraft: the channel, X or Y, the pair rate and the identification.

Stack id: `dme`. Group: Navigation.

Signal types: DME.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 978 MHz |
| Tuning range | 960 MHz to 1.215 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1 MHz |
| Radio filter | 1.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dme` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `mode` | Channel mode | choice | `auto` | `auto` (Automatic), `x` (X), `y` (Y) |  |
| `threshold` | Pulse threshold | number (dB) | 12 | 3 to 40 |  |
| `confirm` | Hearings to confirm | whole number | 2 | 1 to 10 | How many times an identification must be heard before it is shown. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mode` | Test signal: mode | choice | `x` | `x` (X), `y` (Y) |  |
| `gen_kind` | Test signal: transmitter | choice | `beacon` | `beacon` (Ground beacon), `aircraft` (Interrogating aircraft) |  |
| `gen_ppps` | Test signal: pairs a second | number | 2700 | 100 to 5000 |  |
| `gen_ident` | Test signal: identification | text | `SY` |  |  |
| `gen_wpm` | Test signal: Morse speed | number (wpm) | 15 | 3 to 25 |  |
| `gen_pause` | Test signal: pause | duration (s) | 1.5 s | 0.5 to 30 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `heard` | Heard | choice |
| `channel_mode` | Channel mode | choice |
| `channel` | Channel | text |
| `paired_vhf_hz` | Paired VHF | frequency |
| `pairs_per_s` | Pairs a second | number (1/s) |
| `spacing_us` | Pair spacing | number (us) |
| `width_us` | Pulse width | number (us) |
| `pairs` | Pairs | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `tuned_hz` | Tuned | frequency |
| `ident` | Identification | text |
| `last_ident` | Last heard | text |
| `ident_wpm` | Morse speed | number (wpm) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `idents` | Identifications heard | `ident` (text), `heard` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `morse.word` | Identification word | `source` (text), `text` (text), `family` (choice), `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `wpm` (number (wpm)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avnav`, version 2.0. The C header `sdrkit/avnav.h`, the registry record `registry/avnav.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Navigation aids (`navigation`). Media: none.

Key readings: `ident`, `pairs_per_s`, `channel`, `level_dbfs`.

### Result fields

Struct `DmeFields`. The result fields of DME, over the last second of pairs.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `heard` | choice | always | 1.0 | What is heard. |
| `channel_mode` | choice | optional | 1.0 | X or Y, from the pulse spacing. No value with too few pairs. |
| `channel` | text | optional | 1.0 | The channel from the tuned frequency and the mode ("17X"). No value with too few pairs. |
| `paired_vhf_hz` | frequency | optional | 1.0 | The paired VOR or localizer frequency. No value without a channel that has one. |
| `pairs_per_s` | number (1/s) | always | 1.0 | Pairs a second. |
| `spacing_us` | number (us) | optional | 1.0 | The mean pair spacing. No value with too few pairs. |
| `width_us` | number (us) | optional | 1.0 | The mean pulse width at half amplitude. No value with too few pairs. |
| `pairs` | whole number | always | 1.0 | Pairs seen in all. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The mean level of the pulses. No value with too few pairs. |

### Result fields

Struct `TunedFields`. The tuned frequency.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tuned_hz` | frequency | always | 1.0 | The tuned frequency. |

### Result fields

Struct `IdentFields`. The identification of the station.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ident` | text | optional | 1.0 | The confirmed identification. No value before one is confirmed. |
| `last_ident` | text | optional | 1.0 | The last identification heard. No value before a word is heard. |
| `ident_wpm` | number (wpm) | optional | 1.0 | The Morse speed of the last identification. No value before a word is heard. |

### Table `idents`

Struct `IdentRow`. A row of the identifications heard. The row key is the word.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `ident` | text | always | 1.0 | The word. |
| `heard` | whole number | always | 1.0 | How often it was heard. |

The column `ident` names a row.

### Event `morse.word`

Struct `IdentWordEvent`. The fields of a decoded identification word: a spot of the station that sends it. The source, the text and the callsign hold the word.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The word. |
| `text` | text | always | 1.0 | The word. |
| `family` | choice | always | 1.0 | The family of the source: other. |
| `callsign` | text | always | 1.0 | The word. |
| `locator` | text | optional | 1.0 | No value: a beacon sends no locator. |
| `snr_db` | number (dB) | optional | 1.0 | No value: the decoder does not measure it. |
| `freq_hz` | frequency | optional | 1.0 | No value: the event carries the frequency. |
| `mode` | text | always | 1.0 | The emission: "CW". |
| `wpm` | number (wpm) | always | 1.0 | The Morse speed. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dme_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `mode` | `sdrkit_dme_get_mode` | `sdrkit_dme_set_mode` |
| `threshold` | `sdrkit_dme_get_threshold` | `sdrkit_dme_set_threshold` |
| `confirm` | `sdrkit_dme_get_confirm` | `sdrkit_dme_set_confirm` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mode` | `sdrkit_dme_gen_get_mode` | `sdrkit_dme_gen_set_mode` |
| `gen_kind` | `sdrkit_dme_gen_get_kind` | `sdrkit_dme_gen_set_kind` |
| `gen_ppps` | `sdrkit_dme_gen_get_ppps` | `sdrkit_dme_gen_set_ppps` |
| `gen_ident` | `sdrkit_dme_gen_get_ident` | `sdrkit_dme_gen_set_ident` |
| `gen_wpm` | `sdrkit_dme_gen_get_wpm` | `sdrkit_dme_gen_set_wpm` |
| `gen_pause` | `sdrkit_dme_gen_get_pause` | `sdrkit_dme_gen_set_pause` |

## Try it

```sh
sdrkit run dme --source synth:dme
sdrkit run dme --source iiod:192.168.2.1 --freq 978
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
