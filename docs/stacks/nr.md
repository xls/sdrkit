<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# 5G NR cells

5G NR cells: physical cell identity, SS/PBCH beams and the MIB of every SSB on the sync raster, and SIB1 (networks, tracking area, cell identity) where CORESET#0 lies in the capture.

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

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_phase` | Symbol phase of up-conversion | yes or no | true |  | On for cells on the air. Off for a test signal whose symbols all start at phase zero at baseband. |

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
| `with_sib1` | With SIB1 | whole number |
| `gscn_lo` | Lowest GSCN searched | whole number |
| `gscn_hi` | Highest GSCN searched | whole number |
| `candidates` | Candidates | whole number |
| `banner` | Notice | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `decoded` (choice), `plmn` (text), `operator` (text), `tac` (whole number), `nci` (whole number), `band` (whole number), `gscn` (whole number), `frequency` (frequency), `pci` (whole number), `ssb_scs` (frequency), `beams` (whole number), `sfn` (whole number), `sib1` (yes or no), `rsrp_dbfs` (number (dBFS)), `rsrq_db` (number (dB)), `sinr_db` (number (dB)), `last_heard` (time) |
| `nr_details` | Details | `cell` (text), `sib1` (choice), `sib1_parts` (whole number), `coreset0_width` (frequency), `rate_needed` (frequency), `scs_common` (frequency), `k_ssb` (whole number), `barred` (yes or no), `intra_freq_reselection` (yes or no), `reserved_other_use` (yes or no), `q_rx_lev_min_dbm` (number (dBm)), `bands` (text), `ssb_period` (duration (s)), `carrier_scs` (frequency), `carrier_rbs` (whole number (RB)), `offset_to_point_a` (whole number (RB)), `bwp_start` (whole number (RB)), `bwp_rbs` (whole number (RB)), `si_window` (whole number (slots)), `tdd` (yes or no), `sib1_count` (whole number) |
| `nr_networks` | Networks | `cell` (text), `plmn` (text), `operator` (text), `tac` (whole number), `nci` (whole number), `gnb` (whole number), `cell_id` (whole number), `reserved_operator` (yes or no) |
| `nr_sibs` | SIBs | `cell` (text), `sib` (whole number), `period` (duration (s)), `broadcasting` (yes or no), `value_tag` (whole number) |

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

The stack is in the family `cell`, version 1.4. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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
| `with_sib1` | whole number | always | 1.4 | Cells with a SIB1. |
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
| `decoded` | choice | always | 1.4 | How far the cell was decoded. |
| `plmn` | text | optional | 1.4 | The first network of SIB1. |
| `operator` | text | optional | 1.4 | The operator of that network. |
| `tac` | whole number | optional | 1.4 | The tracking area code of SIB1. |
| `nci` | whole number | optional | 1.4 | The NR cell identity of SIB1 (36 bits). |
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

### Table `nr_details`

Struct `NrDetailRow`. The details of a cell of `nr`. The row key is the cell key.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.4 | The row key of the cell. |
| `sib1` | choice | always | 1.4 | How far the reception of SIB1 went. |
| `sib1_parts` | whole number | optional | 1.4 | The parts of SIB1 read: 1 the networks, 2 the scheduling, 3 the downlink carrier, 4 the SSB facts. |
| `coreset0_width` | frequency | optional | 1.4 | The width of CORESET#0. |
| `rate_needed` | frequency | optional | 1.4 | The sample rate that holds CORESET#0 with the radio centre of the newest try. |
| `scs_common` | frequency | optional | 1.4 | subCarrierSpacingCommon of the MIB. |
| `k_ssb` | whole number | optional | 1.4 | k_SSB of the MIB. |
| `barred` | yes or no | optional | 1.4 | cellBarred of the MIB. |
| `intra_freq_reselection` | yes or no | optional | 1.4 | intraFreqReselection of the MIB is allowed. |
| `reserved_other_use` | yes or no | optional | 1.4 | cellReservedForOtherUse of SIB1. |
| `q_rx_lev_min_dbm` | number (dBm) | optional | 1.4 | q-RxLevMin of SIB1. |
| `bands` | text | optional | 1.4 | The bands of SIB1, apart by spaces. |
| `ssb_period` | duration (s) | optional | 1.4 | The SSB period of SIB1. |
| `carrier_scs` | frequency | optional | 1.4 | The spacing of the carrier of SIB1. |
| `carrier_rbs` | whole number (RB) | optional | 1.4 | The width of the carrier of SIB1. |
| `offset_to_point_a` | whole number (RB) | optional | 1.4 | offsetToPointA of SIB1. |
| `bwp_start` | whole number (RB) | optional | 1.4 | The first block of the initial downlink BWP. |
| `bwp_rbs` | whole number (RB) | optional | 1.4 | The width of the initial downlink BWP. |
| `si_window` | whole number (slots) | optional | 1.4 | The SI window, slots. |
| `tdd` | yes or no | optional | 1.4 | Whether SIB1 configures a TDD pattern. |
| `sib1_count` | whole number | always | 1.4 | SIB1 decodes. |

### Table `nr_networks`

Struct `NrNetworkRow`. A network of a cell of `nr`, from SIB1. The row key is the cell key and the place of the network.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.4 | The row key of the cell. |
| `plmn` | text | always | 1.4 | The network. |
| `operator` | text | optional | 1.4 | The operator of the network. |
| `tac` | whole number | optional | 1.4 | The tracking area code. |
| `nci` | whole number | always | 1.4 | The NR cell identity (36 bits). |
| `gnb` | whole number | optional | 1.4 | The gNB identity, when SIB1 gives its length. |
| `cell_id` | whole number | optional | 1.4 | The cell identity within the gNB. |
| `reserved_operator` | yes or no | always | 1.4 | cellReservedForOperatorUse. |

### Table `nr_sibs`

Struct `NrSibRow`. A SIB that the SIB1 of a cell of `nr` schedules. The row key is the cell key and the SIB number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.4 | The row key of the cell. |
| `sib` | whole number | always | 1.4 | The SIB number. |
| `period` | duration (s) | always | 1.4 | The period of its SI message. |
| `broadcasting` | yes or no | always | 1.4 | Whether the message is broadcast (else sent on request). |
| `value_tag` | whole number | optional | 1.4 | The value tag. |

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
| `symbol_phase` | `sdrkit_nr_get_symbol_phase` | `sdrkit_nr_set_symbol_phase` |

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
