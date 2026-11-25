<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# TD-SCDMA cells

TD-SCDMA cells: SYNC-DL code, cell parameter, frame parity, level and carrier offset from the downlink pilot and the midamble; the broadcast channel is not decoded.

Stack id: `tdscdma`. Group: Cellular and railway.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.0174 GHz |
| Tuning range | 1.88 GHz to 2.4 GHz |
| Sample rate | 2.56 MHz |
| Smallest sample rate | 1.6 MHz |
| Signal bandwidth | 1.6 MHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:tdscdma` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `capture_s` | Capture length | number (s) | 0.08 | 0.01 to 10 | Seconds of input searched at a time. |
| `threshold` | SYNC-DL threshold | number | 6 | 3 to 20 | A SYNC-DL code is a candidate when its folded correlation exceeds this many times the median. |
| `max_cells` | Cells per capture | whole number | 6 | 1 to 16 | The most cells measured in one capture, strongest first. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `roll_off` | Pulse roll-off | number | 0.22 | 0.05 to 1 | The roll-off of the root raised cosine matched filter (0.22; not confirmed against the text of TS 25.105). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_multipliers` | Test signal code multipliers | yes or no | false |  | w(16, 1) = -1 and w(16, 2) = -j on the P-CCPCH; not confirmed against the text of TS 25.223. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `captures` | Captures searched | whole number |
| `retunes` | Retunes | whole number |
| `decoded` | Messages decoded | whole number |
| `failed` | Messages failed | whole number |
| `capture_centre` | Capture centre | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `cell_count` | Cells | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `sync_dl` (whole number), `parameter` (text), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `offset_hz` (frequency), `quadruples` (text), `frame_parity` (text), `clarity` (number), `captures_seen` (whole number), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.tdscdma_cell` | TD-SCDMA cell | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `cell`, version 1.3. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `SearchFields`. The counts and the level of the capture search of every cell stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `captures` | whole number | always | 1.0 | Captures searched. |
| `retunes` | whole number | always | 1.0 | Retunes seen (scan steps). |
| `decoded` | whole number | always | 1.0 | Messages decoded. |
| `failed` | whole number | always | 1.0 | Messages that failed their check. |
| `capture_centre` | frequency | optional | 1.0 | The centre frequency of the newest capture. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. |

### Result fields

Struct `TdscdmaFields`. The result fields of `tdscdma`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell_count` | whole number | always | 1.0 | Cells in the table. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `cells`

Struct `TdscdmaCellRow`. A row of the cells of `tdscdma`. The row key is the code and the radio centre.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sync_dl` | whole number | always | 1.0 | The SYNC-DL code. |
| `parameter` | text | optional | 1.0 | The cell parameter, "p / p+1" when only the pair is known. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The DwPTS level. |
| `snr_db` | number (dB) | always | 1.0 | The DwPTS SNR. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `quadruples` | text | always | 1.0 | The quadruples counted. |
| `frame_parity` | text | optional | 1.0 | The SFN parity of the first whole frame of the newest capture. |
| `clarity` | number | always | 1.0 | The midamble clarity. |
| `captures_seen` | whole number | always | 1.0 | Captures in which the cell was seen. |
| `last_heard` | time | optional | 1.0 | When the cell was last heard. |

### Event `cell.tdscdma_cell`

Struct `FoundCellEvent`. A cell found or identified by a search stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The row key of the cell in the `cells` table. |
| `text` | text | always | 1.0 | What was found. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_tdscdma_clear` | none | none |
| `remove` | `sdrkit_tdscdma_remove` | `sdrkit_cell_search_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `capture_s` | `sdrkit_tdscdma_get_capture_s` | `sdrkit_tdscdma_set_capture_s` |
| `threshold` | `sdrkit_tdscdma_get_threshold` | `sdrkit_tdscdma_set_threshold` |
| `max_cells` | `sdrkit_tdscdma_get_max_cells` | `sdrkit_tdscdma_set_max_cells` |
| `roll_off` | `sdrkit_tdscdma_get_roll_off` | `sdrkit_tdscdma_set_roll_off` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_multipliers` | `sdrkit_tdscdma_gen_get_multipliers` | `sdrkit_tdscdma_gen_set_multipliers` |

## Try it

```sh
sdrkit run tdscdma --source synth:tdscdma
sdrkit run tdscdma --source iiod:192.168.2.1 --freq 2017.4
```
<!-- generated: end -->

## Description

A census of the 1.28 Mcps TDD cells of one 1.6 MHz carrier, by steps 1 to 3 of the cell search of TS 25.224 Annex CA. Each capture of 80 ms is correlated with the 32 SYNC-DL codes, folded over the 5 ms sub-frames. For each code found: the four cell parameters of its group are scored on the midamble of time slot 0, which gives the parameter pair in use and the frame boundaries; the phase quadruples of the SYNC-DL (S1 and S2) give the frame with even SFN, and so the initial cell parameter and the frame parity. Each cell kept is taken out of the samples before the next one is measured. The broadcast channel (P-CCPCH) is not decoded.

Standard: 3GPP TS 25.221, TS 25.223, TS 25.224.

Not confirmed against the text of a public document: the root raised cosine pulse with roll-off 0.22 (the setting `roll_off`), the form of the midamble shifts, the traffic burst of 352 + 144 + 352 + 16 chips, and the complex multipliers of the channelisation codes (the test signal sends them only with `gen_multipliers`). A cell on a secondary carrier of a multi-frequency cell shows nothing.

## Result tables: notes

The cell parameter is also the number of the scrambling code and of the basic midamble. "36 / 37" means that only the pair is known.
