<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# UMTS cells

UMTS (WCDMA) cells: primary scrambling code, Ec/Io, level and carrier offset from the synchronisation channels and the CPICH, and the operator and cell identity from the broadcast channel.

Stack id: `umts`. Group: Cellular and railway.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.1128 GHz |
| Tuning range | 700 MHz to 2.7 GHz |
| Sample rate | 7.68 MHz |
| Smallest sample rate | 3.84 MHz |
| Signal bandwidth | 5 MHz |
| Radio filter | 5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:umts` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `capture_s` | Capture length | number (s) | 0.08 | 0.01 to 10 | Seconds of input searched at a time. |
| `max_offset_hz` | Carrier search range | number (Hz) | 7500 | 2500 to 12500 | Cells are searched within this carrier offset either side, in steps of 2.5 kHz. |
| `max_cells` | Cells per capture | whole number | 8 | 1 to 16 | The most cells measured in one capture, strongest first. |
| `read_bch` | Read the broadcast channel | yes or no | true |  | Decode the BCH of each cell for its PLMN and cell identity. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `roll_off` | Pulse roll-off | number | 0.22 | 0.05 to 1 | The roll-off of the root raised cosine matched filter (0.22 by TS 25.104; not confirmed against the text). |

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
| `cells` | Cells | `code` (whole number), `group` (whole number), `ec_io_db` (number (dB)), `level_dbfs` (number (dBFS)), `offset_hz` (frequency), `frame_offset` (whole number (chips)), `sttd` (yes or no), `captures_seen` (whole number), `bch_blocks` (whole number), `plmn` (text), `operator` (text), `cell_id` (whole number), `cell_id_parts` (text), `uarfcn` (whole number), `last_heard` (time), `frequency` (frequency), `lac` (whole number), `rac` (whole number), `barred` (yes or no), `reserved_operator` (yes or no), `generation` (choice), `name` (text) |
| `survey` | Survey | `name` (text), `generation` (choice), `operator` (text), `plmn` (text), `band` (text), `channel` (whole number), `frequency` (frequency), `pci` (whole number), `cell_identity` (whole number), `area_code` (whole number), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.umts_cell` | UMTS cell | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `cell`, version 1.5. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `cell_count`.

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

Struct `UmtsFields`. The result fields of `umts`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell_count` | whole number | always | 1.0 | Cells in the table. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `cells`

Struct `UmtsCellRow`. A row of the cells of `umts`. The row key is the primary scrambling code and the radio centre.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | whole number | always | 1.0 | The primary scrambling code. |
| `group` | whole number | always | 1.0 | The code group. |
| `ec_io_db` | number (dB) | always | 1.0 | Ec/Io of the CPICH. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The CPICH level (RSCP). |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `frame_offset` | whole number (chips) | always | 1.0 | The frame offset. |
| `sttd` | yes or no | optional | 1.0 | Whether the cell sends with STTD. |
| `captures_seen` | whole number | always | 1.0 | Captures in which the cell was seen. |
| `bch_blocks` | whole number | always | 1.0 | BCH blocks decoded. |
| `plmn` | text | optional | 1.0 | The network. |
| `operator` | text | optional | 1.0 | The operator of the network. |
| `cell_id` | whole number | optional | 1.0 | The cell identity. |
| `cell_id_parts` | text | optional | 1.0 | The RNC and the cell of the identity. |
| `uarfcn` | whole number | optional | 1.0 | The UARFCN. |
| `last_heard` | time | optional | 1.0 | When the cell was last heard. |
| `frequency` | frequency | optional | 1.5 | The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known. |
| `lac` | whole number | optional | 1.5 | The location area code (SIB 1). |
| `rac` | whole number | optional | 1.5 | The routing area code (SIB 1). |
| `barred` | yes or no | optional | 1.5 | The cell is barred (SIB 3). |
| `reserved_operator` | yes or no | optional | 1.5 | The cell is reserved for operator use (SIB 3). |
| `generation` | choice | always | 1.5 | The generation of the radio system. |
| `name` | text | always | 1.5 | Operator, generation, band and physical identity. |

The rows are the stations or nodes of the stack. The column `name` names a row. Time: `last_heard`.

### Table `survey`

Struct `SurveyRow`. A cell of a coverage survey: one row per cell found, in the same columns for every generation. The row key is the key of the cell in the cells table of the stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.5 | Operator, generation, band and physical identity. |
| `generation` | choice | always | 1.5 | The generation. |
| `operator` | text | optional | 1.5 | The operator name, else the network. |
| `plmn` | text | optional | 1.5 | The network. |
| `band` | text | optional | 1.5 | The band. |
| `channel` | whole number | optional | 1.5 | The channel number: ARFCN, UARFCN, EARFCN or NR-ARFCN. |
| `frequency` | frequency | always | 1.5 | The frequency. |
| `pci` | whole number | always | 1.5 | The physical identity: PCI, primary scrambling code or BSIC. |
| `cell_identity` | whole number | optional | 1.5 | The cell identity of the broadcast. |
| `area_code` | whole number | optional | 1.5 | The tracking or location area code. |
| `level_dbfs` | number (dBFS) | optional | 1.5 | RSRP, RSCP or the received level. |

The rows are the stations or nodes of the stack. The column `name` names a row.

### Event `cell.umts_cell`

Struct `FoundCellEvent`. A cell found or identified by a search stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The row key of the cell in the `cells` table. |
| `text` | text | always | 1.0 | What was found. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_umts_clear` | none | none |
| `remove` | `sdrkit_umts_remove` | `sdrkit_cell_search_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `capture_s` | `sdrkit_umts_get_capture_s` | `sdrkit_umts_set_capture_s` |
| `max_offset_hz` | `sdrkit_umts_get_max_offset_hz` | `sdrkit_umts_set_max_offset_hz` |
| `max_cells` | `sdrkit_umts_get_max_cells` | `sdrkit_umts_set_max_cells` |
| `read_bch` | `sdrkit_umts_get_read_bch` | `sdrkit_umts_set_read_bch` |
| `roll_off` | `sdrkit_umts_get_roll_off` | `sdrkit_umts_set_roll_off` |

## Try it

```sh
sdrkit run umts --source synth:umts
sdrkit run umts --source iiod:192.168.2.1 --freq 2112.8
```
<!-- generated: end -->

## Description

A census of the WCDMA FDD cells in one 5 MHz channel. Each capture of 80 ms is searched in three steps: the slot timing from the primary synchronisation code, the code group and the frame timing from the secondary codes, and the primary scrambling code from the CPICH. The broadcast channel of each cell is then decoded for two blocks: the master information block (MCC and MNC) and system information block type 3 (the cell identity). The sample clock of the capture is measured from the synchronisation code over all frames; when it is off, the capture is resampled and searched again. Nothing addressed to a phone is decoded. The decode is verified on generated signals only.

Standard: 3GPP TS 25.211, TS 25.212, TS 25.213, TS 25.331.

Not confirmed against the text of a public document: the BCH transport block of 246 bits, the roll-off 0.22 of the root raised cosine pulse (the setting `roll_off`), the split of the cell identity into 12 bits of RNC and 16 bits of cell, and the channel raster UARFCN = 5 x f (MHz).

## Result tables: notes

The STTD column is the sign of the SCH symbol a (yes: the P-CCPCH is STTD encoded). An STTD-encoded P-CCPCH is not combined.
