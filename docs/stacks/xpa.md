<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# XPA / XPA2 number senders

XPA (20 positions, 10 or 20 Bd) and XPA2 (16 tones, 7.8125 Bd) MFSK number senders: the tones found anywhere in the channel, the speed, sideband and numbering found, the five-figure groups.

Stack id: `xpa`. Group: HF utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 6.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `xpa` | XPA | 6.5 MHz |
| `xpa2` | XPA2 | 6.5 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:xpa` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Signal | choice | `xpa` | `xpa` (XPA), `xpa2` (XPA2) |  |
| `speed` | Speed | choice | `auto` | `auto` (Find it), `10` (10 Bd), `20` (20 Bd) | XPA only: Find it reads 10 and 20 Bd side by side; a value reads one. |
| `sideband` | Sideband | choice | `auto` | `auto` (Find it), `upper` (Upper), `lower` (Lower) | Find it reads both sidebands side by side; a value halves the readings. |
| `numbering` | Tone numbering correction | choice | `auto` | `auto` (Find it), `minus3` (-3 tones), `minus2` (-2 tones), `minus1` (-1 tone), `none` (As found), `plus1` (+1 tone), `plus2` (+2 tones), `plus3` (+3 tones) | Find it reads the corrections -3 to +3 tones side by side; a value reads one. |
| `search_hz` | Search width | frequency | 4.5 kHz | 500 Hz to 4.5 kHz | The tones are searched up to this far from their nominal place (and within the channel). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_groups` | Groups | whole number | 0 | 0 to 1000 | Random five-figure groups from the seed; 0 takes 30 for XPA and 24 for XPA2. |
| `gen_baud` | XPA speed | choice | `10` | `10` (10 Bd), `20` (20 Bd) |  |
| `gen_lower` | Lower sideband | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `state` | State | whole number |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `speed` | Speed | number (Bd) |
| `sideband` | Sideband | text |
| `numbering` | Tone numbering | text |
| `groups` | Groups | whole number |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rows` | Rows | `time` (time), `kind` (text), `id` (text), `text` (text), `snr` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `text` | Text | `text` (text) |
| `tones.found` | Tones found | `text` (text), `offset` (frequency), `snr` (number (dB)) |
| `tones.lost` | Tones lost | `text` (text), `offset` (frequency), `read` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `XpaFields`. The result fields of the XPA stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `text` | text | always | 1.0 | The groups read, the last characters. |
| `state` | whole number | always | 1.0 | The search state: 0 searching, 1 tones found, 2 decoding. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the tones from their nominal place. Absent while the tones are not found. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR in 3 kHz of the last fit. Absent while the tones are not found. |
| `speed` | number (Bd) | optional | 1.0 | The speed of the reading that won. Absent before a reading wins. |
| `sideband` | text | optional | 1.0 | The sideband of the reading that won: "upper" or "lower (tones reversed)". |
| `numbering` | text | optional | 1.0 | The numbering of the reading that won: "as found" or "moved by +N tones". |
| `groups` | whole number | always | 1.0 | Groups read. |
| `good` | whole number | always | 1.0 | Lines of groups read. |
| `bad` | whole number | always | 1.0 | Tones read without a meaning. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel, dBFS. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `rows`

Struct `SenderRow`. A row of a number-sender stack: one thing read. The row key is the count of rows before it, as six digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | always | 1.0 | The time it was read. |
| `kind` | text | always | 1.0 | What it is: start, message start, groups, block sync, break, end, sync, text. |
| `id` | text | always | 1.0 | The signal and speed it was read as. |
| `text` | text | always | 1.0 | What was read. |
| `snr` | number (dB) | optional | 1.0 | The SNR in 3 kHz of the last fit of the tones. Absent without a fit. |

### Event `text`

Struct `TextEvent`. New text: a line, 64 characters, or what came before a pause.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text. |

### Event `tones.found`

Struct `TonesFoundEvent`. The tones of a sender were found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `offset` | frequency | always | 1.0 | The offset of the tones from their nominal place. |
| `snr` | number (dB) | always | 1.0 | The SNR in 3 kHz. |

### Event `tones.lost`

Struct `TonesLostEvent`. The tones of a sender are gone.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `offset` | frequency | always | 1.0 | The offset of the tones when they were lost. |
| `read` | whole number | always | 1.0 | Characters or groups read in the lock. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_xpa_clear` | none | none |
| `reset` | `sdrkit_xpa_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_xpa_get_submode` | `sdrkit_xpa_set_submode` |
| `speed` | `sdrkit_xpa_get_speed` | `sdrkit_xpa_set_speed` |
| `sideband` | `sdrkit_xpa_get_sideband` | `sdrkit_xpa_set_sideband` |
| `numbering` | `sdrkit_xpa_get_numbering` | `sdrkit_xpa_set_numbering` |
| `search_hz` | `sdrkit_xpa_get_search_hz` | `sdrkit_xpa_set_search_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_groups` | `sdrkit_xpa_gen_get_groups` | `sdrkit_xpa_gen_set_groups` |
| `gen_baud` | `sdrkit_xpa_gen_get_baud` | `sdrkit_xpa_gen_set_baud` |
| `gen_lower` | `sdrkit_xpa_gen_get_lower` | `sdrkit_xpa_gen_set_lower` |

## Try it

```sh
sdrkit run xpa --source synth:xpa
sdrkit run xpa --source synth:xpa --submode xpa
sdrkit run xpa --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->
