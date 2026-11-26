<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NB-IoT cells

NB-IoT cells: cell identity, operator, band and level from the synchronisation signals, MIB-NB and SIB1-NB.

Stack id: `nbiot`. Group: Cellular and railway.

Signal types: NB-IoT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 450 MHz to 2.2 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1.75 MHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:nbiot` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `capture_s` | Capture length | number (s) | 1 | 0.01 to 10 | Seconds of input searched at a time. |
| `min_npss` | NPSS threshold | number | 3.6 | 2 to 10 |  |
| `read_sib1` | Read SIB1-NB | yes or no | true |  |  |
| `freq_correction_ppm` | Radio frequency correction | number (ppm) | 0 | -100 to 100 | The radio's known frequency error. It splits a carrier offset into raster offset and tuning error. |
| `band_order` | Band order | text | empty |  | Band numbers taken first where bands overlap ("1 3 7"); the other bands follow in band number order. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `0` | `0` (Standalone), `1` (Guard band), `2` (In-band, same PCI), `3` (In-band, other PCI) |  |
| `gen_load` | Test signal load | yes or no | true |  |  |
| `gen_npbch_rotation` | Test signal NPBCH rotation | yes or no | false |  |  |

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
| `identified` | Identified | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `plmn` (text), `operator` (text), `band` (whole number), `earfcn` (whole number), `frequency` (frequency), `pci` (whole number), `operation_mode` (choice), `enb` (whole number), `sector` (whole number), `tac` (whole number), `barred` (yes or no), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `last_heard` (time), `generation` (choice), `name` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.nbiot_cell` | NB-IoT cell | `source` (text), `text` (text), `family` (choice) |

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

Struct `NbiotFields`. The result fields of `nbiot`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell_count` | whole number | always | 1.0 | Cells in the table. |
| `identified` | whole number | always | 1.0 | Cells with SIB1-NB. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `cells`

Struct `NbiotCellRow`. A row of the cells of `nbiot`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plmn` | text | optional | 1.0 | The primary network. |
| `operator` | text | optional | 1.0 | The operator of the primary network. |
| `band` | whole number | optional | 1.0 | The band. |
| `earfcn` | whole number | optional | 1.0 | The EARFCN. |
| `frequency` | frequency | always | 1.0 | The nominal frequency (the offset from 0 Hz when the radio centre is not known). |
| `pci` | whole number | always | 1.0 | The physical cell identity. |
| `operation_mode` | choice | optional | 1.0 | The operation mode. |
| `enb` | whole number | optional | 1.0 | The eNB identity. |
| `sector` | whole number | optional | 1.0 | The sector. |
| `tac` | whole number | optional | 1.0 | The tracking area code. |
| `barred` | yes or no | optional | 1.0 | Whether the cell is barred. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `last_heard` | time | optional | 1.0 | When the cell was last heard. |
| `generation` | choice | always | 1.5 | The generation of the radio system. |
| `name` | text | always | 1.5 | Operator, generation, band and physical identity. |

The rows are the stations or nodes of the stack. The column `name` names a row. Time: `last_heard`.

### Event `cell.nbiot_cell`

Struct `FoundCellEvent`. A cell found or identified by a search stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The row key of the cell in the `cells` table. |
| `text` | text | always | 1.0 | What was found. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_nbiot_clear` | none | none |
| `remove` | `sdrkit_nbiot_remove` | `sdrkit_cell_search_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `capture_s` | `sdrkit_nbiot_get_capture_s` | `sdrkit_nbiot_set_capture_s` |
| `min_npss` | `sdrkit_nbiot_get_min_npss` | `sdrkit_nbiot_set_min_npss` |
| `read_sib1` | `sdrkit_nbiot_get_read_sib1` | `sdrkit_nbiot_set_read_sib1` |
| `freq_correction_ppm` | `sdrkit_nbiot_get_freq_correction_ppm` | `sdrkit_nbiot_set_freq_correction_ppm` |
| `band_order` | `sdrkit_nbiot_get_band_order` | `sdrkit_nbiot_set_band_order` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_nbiot_gen_get_scene` | `sdrkit_nbiot_gen_set_scene` |
| `gen_load` | `sdrkit_nbiot_gen_get_load` | `sdrkit_nbiot_gen_set_load` |
| `gen_npbch_rotation` | `sdrkit_nbiot_gen_get_npbch_rotation` | `sdrkit_nbiot_gen_set_npbch_rotation` |

## Try it

```sh
sdrkit run nbiot --source synth:nbiot
sdrkit run nbiot --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/cell/nbiot_standalone_1920k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
