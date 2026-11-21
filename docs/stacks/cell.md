<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LTE cells

LTE cells: operator, band, cell identity, area code and signal strength from the information every cell broadcasts.

Stack id: `cell`. Group: Cellular and railway.

Signal types: LTE.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 450 MHz to 3.8 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 1.92 MHz |
| Signal bandwidth | 10 MHz |
| Radio filter | 15 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cell` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `capture_s` | Capture length | number (s) | 0.2 | 0.01 to 10 | Seconds of input searched at a time. |
| `system_information` | Read system information | yes or no | true |  | Decode SIB1 to SIB5 (needs the whole carrier in the sample band). |
| `max_cells` | Cells per carrier | whole number | 12 | 1 to 24 |  |
| `min_pss_db` | PSS threshold | number (dB) | 9 | 6 to 20 |  |
| `max_cfo_khz` | Carrier offset searched | number (kHz) | 25 | 5 to 60 |  |
| `hide_bands` | Hide bands | text | empty |  | Band numbers to leave out ("14 28, 64"). |
| `only_bands` | Only bands | text | empty |  |  |
| `search` | Search | text | empty |  |  |
| `min_rsrp` | Minimum RSRP | number (dBFS) | -120 | -120 to -20 |  |
| `band_known` | Band confirmed only | yes or no | false |  |  |
| `identified` | Identified only | yes or no | false |  |  |
| `not_barred` | Leave out barred cells | yes or no | false |  |  |
| `one_per_cell` | One row per cell | yes or no | false |  |  |
| `band_order` | Band order | text | empty |  | Band numbers taken first where bands overlap ("1 3 7"); the other bands follow in band number order. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `0` | `0` (10 MHz, 2 ports), `1` (5 MHz, 4 ports), `2` (1.4 MHz), `3` (20 MHz) |  |
| `gen_sfn0` | Test signal first SFN | whole number | 0 | 0 to 1023 |  |
| `gen_load` | Test signal load | yes or no | true |  |  |

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
| `operators` | Operators | whole number |
| `banner` | Notice | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `plmn` (text), `operator` (text), `country` (text), `plmn_count` (whole number), `band` (whole number), `band_confirmed` (yes or no), `earfcn` (whole number), `frequency` (frequency), `pci` (whole number), `duplex` (choice), `enb` (whole number), `sector` (whole number), `cell_id` (whole number), `tac` (whole number), `width` (frequency), `ports` (whole number), `rsrp_dbfs` (number (dBFS)), `rsrq_db` (number (dB)), `sinr_db` (number (dB)), `carrier_offset` (frequency), `barred` (yes or no), `etws` (yes or no), `cmas` (yes or no), `last_heard` (time) |
| `details` | Details | `cell` (text), `cyclic_prefix` (choice), `phich_duration` (choice), `phich_ng` (whole number), `sfn` (whole number), `clock_ppm` (number (ppm)), `si_messages` (whole number), `si_failed` (whole number), `q_rx_lev_min_dbm` (number (dBm)), `p_max_dbm` (number (dBm)), `si_window` (duration (s)), `value_tag` (whole number), `ref_signal_power_dbm` (number (dBm)), `p_b` (whole number), `ul_earfcn` (whole number), `ul_rbs` (whole number), `ra_preambles` (whole number), `mod_period_coeff` (whole number), `paging_cycle` (whole number (frames)), `t300` (duration (s)), `t310` (duration (s)), `n310` (whole number), `time_alignment` (duration (s)), `ac_barring` (yes or no), `q_hyst_db` (number (dB)), `priority` (whole number), `s_intra_search_db` (number (dB)), `s_non_intra_search_db` (number (dB)), `thresh_serving_low_db` (number (dB)), `t_reselection` (duration (s)) |
| `cell_plmns` | Networks | `cell` (text), `plmn` (text), `operator` (text) |
| `cell_bands` | Other bands | `cell` (text), `band` (whole number), `mfbi` (yes or no) |
| `cell_sibs` | System information blocks | `cell` (text), `sib` (whole number), `period` (whole number (frames)), `seen` (yes or no) |
| `neighbours` | Neighbours | `cell` (text), `list` (choice), `earfcn` (whole number), `pci` (whole number), `q_offset_db` (number (dB)) |
| `inter_carriers` | Other carriers | `cell` (text), `earfcn` (whole number), `rbs` (whole number), `priority` (whole number), `thresh_high_db` (number (dB)), `thresh_low_db` (number (dB)), `q_rx_lev_min_dbm` (number (dBm)) |
| `sites` | Sites | `plmn` (text), `operator` (text), `enb` (whole number), `tac` (whole number), `cell_count` (whole number) |
| `operators` | Operators | `plmn` (text), `operator` (text), `country` (text), `cell_count` (whole number) |
| `carriers` | Carriers | `frequency` (frequency), `width` (frequency), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.lte_cell` | LTE cell | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `cell`, version 1.0. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `CellFields`. The result fields of `cell`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell_count` | whole number | always | 1.0 | Cells in the table. |
| `identified` | whole number | always | 1.0 | Cells with SIB1. |
| `operators` | whole number | always | 1.0 | Operators of the cells. |
| `banner` | text | always | 1.0 | The notice. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `cells`

Struct `CellRow`. A row of the cells of `cell`. The row key is the cell key.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plmn` | text | optional | 1.0 | The primary network. |
| `operator` | text | optional | 1.0 | The operator of the primary network. |
| `country` | text | optional | 1.0 | The country of the primary network. |
| `plmn_count` | whole number | optional | 1.0 | The networks of the cell. |
| `band` | whole number | optional | 1.0 | The band. |
| `band_confirmed` | yes or no | always | 1.0 | Whether SIB1 confirmed the band. |
| `earfcn` | whole number | optional | 1.0 | The EARFCN. |
| `frequency` | frequency | always | 1.0 | The nominal frequency. |
| `pci` | whole number | always | 1.0 | The physical cell identity. |
| `duplex` | choice | always | 1.0 | The duplex. |
| `enb` | whole number | optional | 1.0 | The eNB identity. |
| `sector` | whole number | optional | 1.0 | The sector. |
| `cell_id` | whole number | optional | 1.0 | The cell identity. |
| `tac` | whole number | optional | 1.0 | The tracking area code. |
| `width` | frequency | optional | 1.0 | The bandwidth. |
| `ports` | whole number | optional | 1.0 | The CRS ports. |
| `rsrp_dbfs` | number (dBFS) | always | 1.0 | The RSRP. |
| `rsrq_db` | number (dB) | always | 1.0 | The RSRQ. |
| `sinr_db` | number (dB) | always | 1.0 | The SINR. |
| `carrier_offset` | frequency | always | 1.0 | The carrier offset. |
| `barred` | yes or no | optional | 1.0 | Whether the cell is barred. |
| `etws` | yes or no | always | 1.0 | Whether SIB1 schedules ETWS. |
| `cmas` | yes or no | always | 1.0 | Whether SIB1 schedules CMAS. |
| `last_heard` | time | optional | 1.0 | When the cell was last heard. |

### Table `details`

Struct `CellDetailRow`. A row of the details of `cell`. The row key is the cell key.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.0 | The row key of the cell. |
| `cyclic_prefix` | choice | always | 1.0 | The cyclic prefix. |
| `phich_duration` | choice | optional | 1.0 | The PHICH duration. |
| `phich_ng` | whole number | optional | 1.0 | The PHICH Ng index. |
| `sfn` | whole number | optional | 1.0 | The SFN of the first whole frame. |
| `clock_ppm` | number (ppm) | always | 1.0 | The clock error. |
| `si_messages` | whole number | always | 1.0 | SI messages decoded. |
| `si_failed` | whole number | always | 1.0 | SI messages that failed their CRC. |
| `q_rx_lev_min_dbm` | number (dBm) | optional | 1.0 | q-RxLevMin of SIB1. |
| `p_max_dbm` | number (dBm) | optional | 1.0 | p-Max of SIB1. |
| `si_window` | duration (s) | optional | 1.0 | The SI window. |
| `value_tag` | whole number | optional | 1.0 | The value tag of SIB1. |
| `ref_signal_power_dbm` | number (dBm) | optional | 1.0 | The CRS power of SIB2. |
| `p_b` | whole number | optional | 1.0 | p-b of SIB2. |
| `ul_earfcn` | whole number | optional | 1.0 | The uplink EARFCN. |
| `ul_rbs` | whole number | optional | 1.0 | The uplink RBs. |
| `ra_preambles` | whole number | optional | 1.0 | The random access preambles. |
| `mod_period_coeff` | whole number | optional | 1.0 | The modification period coefficient. |
| `paging_cycle` | whole number (frames) | optional | 1.0 | The paging cycle. |
| `t300` | duration (s) | optional | 1.0 | t300. |
| `t310` | duration (s) | optional | 1.0 | t310. |
| `n310` | whole number | optional | 1.0 | n310. |
| `time_alignment` | duration (s) | optional | 1.0 | The time alignment timer. |
| `ac_barring` | yes or no | optional | 1.0 | Whether access class barring is on. |
| `q_hyst_db` | number (dB) | optional | 1.0 | q-Hyst of SIB3. |
| `priority` | whole number | optional | 1.0 | The reselection priority of SIB3. |
| `s_intra_search_db` | number (dB) | optional | 1.0 | s-IntraSearch of SIB3. |
| `s_non_intra_search_db` | number (dB) | optional | 1.0 | s-NonIntraSearch of SIB3. |
| `thresh_serving_low_db` | number (dB) | optional | 1.0 | threshServingLow of SIB3. |
| `t_reselection` | duration (s) | optional | 1.0 | t-Reselection of SIB3. |

### Table `cell_plmns`

Struct `CellPlmnRow`. A network of a cell of `cell`. The row key is the cell key and the place of the network.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.0 | The row key of the cell. |
| `plmn` | text | always | 1.0 | The network. |
| `operator` | text | optional | 1.0 | The operator of the network. |

### Table `cell_bands`

Struct `CellBandRow`. Another band of a cell of `cell`. The row key is the cell key and the band.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.0 | The row key of the cell. |
| `band` | whole number | always | 1.0 | The band. |
| `mfbi` | yes or no | always | 1.0 | Whether the band is of the multi-band list. |

### Table `cell_sibs`

Struct `CellSibRow`. A system information block of a cell of `cell`. The row key is the cell key and the SIB number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.0 | The row key of the cell. |
| `sib` | whole number | always | 1.0 | The SIB number. |
| `period` | whole number (frames) | optional | 1.0 | The period of the SIB. |
| `seen` | yes or no | always | 1.0 | Whether the SIB was read. |

### Table `neighbours`

Struct `CellNeighbourRow`. A neighbour of a cell of `cell`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.0 | The row key of the cell. |
| `list` | choice | always | 1.0 | The list that names the neighbour. |
| `earfcn` | whole number | optional | 1.0 | The EARFCN of the neighbour. |
| `pci` | whole number | always | 1.0 | The physical cell identity of the neighbour. |
| `q_offset_db` | number (dB) | optional | 1.0 | q-OffsetCell. |

### Table `inter_carriers`

Struct `CellInterCarrierRow`. Another carrier that a cell of `cell` names in SIB5. The row key is the cell key and the EARFCN.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell` | text | always | 1.0 | The row key of the cell. |
| `earfcn` | whole number | always | 1.0 | The EARFCN. |
| `rbs` | whole number | always | 1.0 | The RBs. |
| `priority` | whole number | optional | 1.0 | The reselection priority. |
| `thresh_high_db` | number (dB) | always | 1.0 | threshX-High. |
| `thresh_low_db` | number (dB) | always | 1.0 | threshX-Low. |
| `q_rx_lev_min_dbm` | number (dBm) | always | 1.0 | q-RxLevMin. |

### Table `sites`

Struct `CellSiteRow`. A site of `cell`: the cells of one eNB. The row key is the network and the eNB.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plmn` | text | optional | 1.0 | The network. |
| `operator` | text | optional | 1.0 | The operator of the network. |
| `enb` | whole number | always | 1.0 | The eNB identity. |
| `tac` | whole number | always | 1.0 | The tracking area code. |
| `cell_count` | whole number | always | 1.0 | The cells of the site. |

### Table `operators`

Struct `CellOperatorRow`. An operator of `cell`. The row key is the network.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plmn` | text | always | 1.0 | The network. |
| `operator` | text | optional | 1.0 | The operator of the network. |
| `country` | text | optional | 1.0 | The country of the network. |
| `cell_count` | whole number | always | 1.0 | The cells of the operator. |

### Table `carriers`

Struct `CellCarrierRow`. A carrier of the newest capture of `cell`. The row key is its place, strongest first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The frequency. |
| `width` | frequency | always | 1.0 | The width. |
| `level_db` | number (dB) | always | 1.0 | The level. |

### Event `cell.lte_cell`

Struct `FoundCellEvent`. A cell found or identified by a search stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The row key of the cell in the `cells` table. |
| `text` | text | always | 1.0 | What was found. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_cell_clear` | none | none |
| `remove` | `sdrkit_cell_remove` | `sdrkit_cell_search_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `capture_s` | `sdrkit_cell_get_capture_s` | `sdrkit_cell_set_capture_s` |
| `system_information` | `sdrkit_cell_get_system_information` | `sdrkit_cell_set_system_information` |
| `max_cells` | `sdrkit_cell_get_max_cells` | `sdrkit_cell_set_max_cells` |
| `min_pss_db` | `sdrkit_cell_get_min_pss_db` | `sdrkit_cell_set_min_pss_db` |
| `max_cfo_khz` | `sdrkit_cell_get_max_cfo_khz` | `sdrkit_cell_set_max_cfo_khz` |
| `hide_bands` | `sdrkit_cell_get_hide_bands` | `sdrkit_cell_set_hide_bands` |
| `only_bands` | `sdrkit_cell_get_only_bands` | `sdrkit_cell_set_only_bands` |
| `search` | `sdrkit_cell_get_search` | `sdrkit_cell_set_search` |
| `min_rsrp` | `sdrkit_cell_get_min_rsrp` | `sdrkit_cell_set_min_rsrp` |
| `band_known` | `sdrkit_cell_get_band_known` | `sdrkit_cell_set_band_known` |
| `identified` | `sdrkit_cell_get_identified` | `sdrkit_cell_set_identified` |
| `not_barred` | `sdrkit_cell_get_not_barred` | `sdrkit_cell_set_not_barred` |
| `one_per_cell` | `sdrkit_cell_get_one_per_cell` | `sdrkit_cell_set_one_per_cell` |
| `band_order` | `sdrkit_cell_get_band_order` | `sdrkit_cell_set_band_order` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_cell_gen_get_scene` | `sdrkit_cell_gen_set_scene` |
| `gen_sfn0` | `sdrkit_cell_gen_get_sfn0` | `sdrkit_cell_gen_set_sfn0` |
| `gen_load` | `sdrkit_cell_gen_get_load` | `sdrkit_cell_gen_set_load` |

## Try it

```sh
sdrkit run cell --source synth:cell
sdrkit run cell --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
