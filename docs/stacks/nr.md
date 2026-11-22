<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# 5G NR cells

5G NR cells: physical cell identity, SS/PBCH beams and the MIB of every SSB on the sync raster.

Stack id: `nr`. Group: Cellular and railway.

Signal types: 5G NR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 600 MHz to 4.2 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 3.84 MHz |
| Signal bandwidth | 20 MHz |
| Radio filter | 15 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:nr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `capture_s` | Capture length | number (s) | 0.08 | 0.01 to 10 | Seconds of input searched at a time. |
| `max_cells` | Cells per candidate | whole number | 6 | 1 to 12 |  |
| `min_pss_db` | PSS threshold | number (dB) | 10 | 6 to 20 |  |
| `hide_bands` | Hide bands | text | empty |  |  |
| `only_bands` | Only bands | text | empty |  |  |
| `min_rsrp` | Minimum RSRP | number (dBFS) | -120 | -120 to -20 |  |
| `mib_only` | Only cells with a MIB | yes or no | false |  |  |
| `not_barred` | Leave out barred cells | yes or no | false |  |  |
| `band_order` | Band order | text | empty |  | Band numbers taken first where bands overlap ("1 3 7"); the other bands follow in band number order. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `0` | `0` (Two n78 cells), `1` (n28 cell), `2` (Barred n78 cell) |  |

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
| `with_mib` | With MIB | whole number |
| `gscn_lo` | Lowest GSCN searched | whole number |
| `gscn_hi` | Highest GSCN searched | whole number |
| `candidates` | Candidates | whole number |
| `banner` | Notice | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `band` (whole number), `gscn` (whole number), `frequency` (frequency), `pci` (whole number), `ssb_scs` (frequency), `beams` (whole number), `sfn` (whole number), `sib1` (yes or no), `rsrp_dbfs` (number (dBFS)), `rsrq_db` (number (dB)), `sinr_db` (number (dB)), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.nr_cell` | NR cell | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `cell`, version 1.1. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `NrFields`. The result fields of `nr`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell_count` | whole number | always | 1.0 | Cells in the table. |
| `with_mib` | whole number | always | 1.0 | Cells with a MIB. |
| `gscn_lo` | whole number | optional | 1.0 | The lowest GSCN the newest capture searched. |
| `gscn_hi` | whole number | optional | 1.0 | The highest GSCN the newest capture searched. |
| `candidates` | whole number | optional | 1.0 | The candidates the newest capture searched. |
| `banner` | text | always | 1.0 | The notice. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `cells`

Struct `NrCellRow`. A row of the cells of `nr`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | whole number | optional | 1.0 | The band. |
| `gscn` | whole number | optional | 1.0 | The GSCN. |
| `frequency` | frequency | always | 1.0 | The frequency of the SSB. |
| `pci` | whole number | always | 1.0 | The physical cell identity. |
| `ssb_scs` | frequency | always | 1.0 | The subcarrier spacing of the SSB. |
| `beams` | whole number | always | 1.0 | The beams seen. |
| `sfn` | whole number | optional | 1.0 | The SFN of the MIB. |
| `sib1` | yes or no | always | 1.0 | Whether the MIB schedules SIB1. |
| `rsrp_dbfs` | number (dBFS) | always | 1.0 | The RSRP. |
| `rsrq_db` | number (dB) | always | 1.0 | The RSRQ. |
| `sinr_db` | number (dB) | always | 1.0 | The SINR. |
| `last_heard` | time | optional | 1.0 | When the cell was last heard. |

### Event `cell.nr_cell`

Struct `FoundCellEvent`. A cell found or identified by a search stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The row key of the cell in the `cells` table. |
| `text` | text | always | 1.0 | What was found. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_nr_clear` | none | none |
| `remove` | `sdrkit_nr_remove` | `sdrkit_cell_search_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `capture_s` | `sdrkit_nr_get_capture_s` | `sdrkit_nr_set_capture_s` |
| `max_cells` | `sdrkit_nr_get_max_cells` | `sdrkit_nr_set_max_cells` |
| `min_pss_db` | `sdrkit_nr_get_min_pss_db` | `sdrkit_nr_set_min_pss_db` |
| `hide_bands` | `sdrkit_nr_get_hide_bands` | `sdrkit_nr_set_hide_bands` |
| `only_bands` | `sdrkit_nr_get_only_bands` | `sdrkit_nr_set_only_bands` |
| `min_rsrp` | `sdrkit_nr_get_min_rsrp` | `sdrkit_nr_set_min_rsrp` |
| `mib_only` | `sdrkit_nr_get_mib_only` | `sdrkit_nr_set_mib_only` |
| `not_barred` | `sdrkit_nr_get_not_barred` | `sdrkit_nr_set_not_barred` |
| `band_order` | `sdrkit_nr_get_band_order` | `sdrkit_nr_set_band_order` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_nr_gen_get_scene` | `sdrkit_nr_gen_set_scene` |

## Try it

```sh
sdrkit run nr --source synth:nr
sdrkit run nr --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/cell/nr_n28_15khz_3840k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
