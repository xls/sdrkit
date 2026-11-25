<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CROWD36

CROWD36 34-tone MFSK at 40 Bd: the tones found anywhere in the channel, the sync tones, the characters of the letters and figures cases. Not deciphered.

Stack id: `crowd36`. Group: HF utility.

CROWD36 (also called CIS-36): MFSK of one tone at a time out of 34 tones 40 Hz apart from 1410 Hz above an upper-sideband dial, at 40 Bd (10 and 20 Bd are also described). Two sync tones alternate; then characters follow in a letters case and a figures case with Cyrillic letters. The traffic is believed to be enciphered; nothing is deciphered.

Standard: no issuing body publishes the signal. Public descriptions (the Wavecom decoder help on CIS-36, the Shoc signal

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

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:crowd36` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `sync_high_tone` | Sync high tone | whole number (tone) | 24 | 0 to 33 | The tone number the higher sync tone is taken as; it sets the numbering of every tone. It must match the station. |
| `sideband` | Sideband | choice | `upper` | `upper` (Upper), `lower` (Lower) | lower mirrors the numbering; the sync cannot tell the sidebands apart. |
| `speed` | Speed | choice | `40` | `40` (40 Bd), `20` (20 Bd), `10` (10 Bd) | The stack and the test signal use this speed. |
| `search_hz` | Search width | frequency | 4.5 kHz | 500 Hz to 4.5 kHz | The tones are searched up to this far from their nominal place (and within the channel). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_groups` | Groups | whole number | 40 | 1 to 1000 | Random five-letter groups from the seed. |
| `gen_sync_high` | Sync high tone | whole number (tone) | 24 | 0 to 33 |  |
| `gen_sync_low` | Sync low tone | whole number (tone) | 10 | 0 to 33 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `text` | Text | text |
| `state` | State | whole number |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `sync_tones` | Sync tones | text |
| `characters` | Characters | whole number |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `refused` | Refused | whole number |
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

The stack is in the family `hfdata`, version 2.0. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

Key readings: `snr_db`, `offset_hz`.

### Result fields

Struct `Crowd36Fields`. The result fields of the CROWD36 stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `text` | text | always | 1.0 | The text read, the last characters. |
| `state` | whole number | always | 1.0 | The search state: 0 searching, 1 tones found, 2 decoding. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the tones from their nominal place. Absent while the tones are not found. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR in 3 kHz of the last fit. Absent while the tones are not found. |
| `sync_tones` | text | optional | 1.0 | The sync tones of the last sync, after the correction. Absent before a sync. |
| `characters` | whole number | always | 1.0 | Characters read. |
| `good` | whole number | always | 1.0 | Text lines read. |
| `bad` | whole number | always | 1.0 | Tones read outside the character tables. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel, dBFS. |
| `refused` | whole number | always | 1.0 | Text lines refused by the check of their form. |

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

The rows are a log: they arrive over time. The column `id` names a row. Time: `time`.

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
| `clear` | `sdrkit_crowd36_clear` | none | none |
| `reset` | `sdrkit_crowd36_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `sync_high_tone` | `sdrkit_crowd36_get_sync_high_tone` | `sdrkit_crowd36_set_sync_high_tone` |
| `sideband` | `sdrkit_crowd36_get_sideband` | `sdrkit_crowd36_set_sideband` |
| `speed` | `sdrkit_crowd36_get_speed` | `sdrkit_crowd36_set_speed` |
| `search_hz` | `sdrkit_crowd36_get_search_hz` | `sdrkit_crowd36_set_search_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_groups` | `sdrkit_crowd36_gen_get_groups` | `sdrkit_crowd36_gen_set_groups` |
| `gen_sync_high` | `sdrkit_crowd36_gen_get_sync_high` | `sdrkit_crowd36_gen_set_sync_high` |
| `gen_sync_low` | `sdrkit_crowd36_gen_get_sync_low` | `sdrkit_crowd36_gen_set_sync_low` |

## Try it

```sh
sdrkit run crowd36 --source synth:crowd36
sdrkit run crowd36 --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->
