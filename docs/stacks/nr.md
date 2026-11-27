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
| Automatic gain | Found once and held |

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
| `band_search` | Band search | choice | `auto` | `auto` (When nothing is found), `survey` (Always (survey)), `off` (Never) | When the stack steps the radio over the raster of its bands: when the tuned frequency shows no cell whose operator can be read at this rate (until it finds one, else one pass and then the strongest cell found), always (a survey), or never. |
| `search_bands` | Bands searched | text | empty |  | Band numbers to search, in order ("28 5 3"). Empty: the bands of the band order (the bands of the country), else the band of the tuned frequency, else the nearest band. |
| `search_first` | Searched first | text (MHz) | empty |  | Frequencies of known carriers, MHz, that a band search visits first, each at its own centre ("884.45 763"); a frequency outside the bands searched is left out. In auto the search stays on the first cell it finds whose operator can be read at this rate. Empty: none. |
| `search_ranges` | Ranges searched | text (MHz) | empty |  | Frequency ranges, MHz, that a band search keeps to in the bands they overlap ("3400-3800 758-803"); a band that no range overlaps is searched whole. Empty: every band whole. |
| `search_dwell_s` | Longest time per step | number (s) | 2 | 0.1 to 10 | The longest time on one radio centre of a band search. A centre whose first look shows a sync signal is searched until its cells are read, or this time has passed. |
| `search_look_s` | First look | number (s) | 0.08 | 0.02 to 1 | Seconds of signal searched on each radio centre of a band search. A centre that shows a sync signal is searched longer, up to search_dwell_s. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_phase` | Symbol phase of up-conversion | yes or no | true |  | On for cells on the air. Off for a test signal whose symbols all start at phase zero at baseband. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `0` | `0` (Two n78 cells), `1` (n28 cell), `2` (Barred n78 cell), `3` (n5 cell, 30 kHz blocks) |  |

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
| `search_band` | Band searched | text |
| `search_step` | Search step | whole number |
| `search_steps` | Search steps | whole number |
| `search_passes` | Search passes | whole number |
| `quiet_reason` | Why nothing shows | text |
| `skipped` | Not examined between looks | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `decoded` (choice), `plmn` (text), `operator` (text), `tac` (whole number), `nci` (whole number), `band` (whole number), `gscn` (whole number), `frequency` (frequency), `pci` (whole number), `ssb_scs` (frequency), `beams` (whole number), `sfn` (whole number), `sib1` (yes or no), `rsrp_dbfs` (number (dBFS)), `rsrq_db` (number (dB)), `sinr_db` (number (dB)), `last_heard` (time), `generation` (choice), `name` (text) |
| `nr_details` | Details | `cell` (text), `sib1` (choice), `sib1_parts` (whole number), `coreset0_width` (frequency), `rate_needed` (frequency), `scs_common` (frequency), `k_ssb` (whole number), `barred` (yes or no), `intra_freq_reselection` (yes or no), `reserved_other_use` (yes or no), `q_rx_lev_min_dbm` (number (dBm)), `bands` (text), `ssb_period` (duration (s)), `carrier_scs` (frequency), `carrier_rbs` (whole number (RB)), `offset_to_point_a` (whole number (RB)), `bwp_start` (whole number (RB)), `bwp_rbs` (whole number (RB)), `si_window` (whole number (slots)), `tdd` (yes or no), `sib1_count` (whole number), `sib1_occasions` (whole number), `sib1_pdcch` (whole number) |
| `nr_networks` | Networks | `cell` (text), `plmn` (text), `operator` (text), `tac` (whole number), `nci` (whole number), `gnb` (whole number), `cell_id` (whole number), `reserved_operator` (yes or no) |
| `nr_sibs` | SIBs | `cell` (text), `sib` (whole number), `period` (duration (s)), `broadcasting` (yes or no), `value_tag` (whole number) |
| `survey` | Survey | `name` (text), `generation` (choice), `operator` (text), `plmn` (text), `band` (text), `channel` (whole number), `frequency` (frequency), `pci` (whole number), `cell_identity` (whole number), `area_code` (whole number), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.nr_cell` | NR cell | `source` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `search` | Search the bands |  |  | Steps the radio over the raster of the bands of the setting search_bands (empty: the bands of the band order, else the band of the tuned frequency) and lists every cell found. With band_search on auto the search stays on the strongest cell after one pass. |
| `stay` | Stay on a cell | `row` (text, required) |  | Ends the band search and tunes to the carrier of a cell of the cells table, or of the strongest cell found. |

## Typed interface

The stack is in the family `cell`, version 1.10. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

### Result fields

Struct `BandSearchFields`. Where the band search of `cell` and `nr` is, and why nothing shows.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `search_band` | text | optional | 1.7 | The band searched now ("band 28", "n78"). None when no search runs. |
| `search_step` | whole number | optional | 1.7 | The step of the pass, counted from 1: the steps searched in this pass and the one that waits. |
| `search_steps` | whole number | optional | 1.7 | The radio centres of the search. |
| `search_passes` | whole number | always | 1.7 | The passes over every step done since the search started. |
| `quiet_reason` | text | optional | 1.7 | Why no cell shows: what the newest capture held, or what the radio gives. None while cells show. |

### Result fields

Struct `LookFields`. The result set of a decoder that takes its input in looks ([`Decoder::looks`]). The decoder declares the field in [`Decoder::results`], and the description of its stack lists the set; the component writes it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `skipped` | whole number | optional | 1.10 | Items left out between looks, on purpose. It holds a value while the decoder runs on a helper thread. |

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
| `generation` | choice | always | 1.5 | The generation of the radio system. |
| `name` | text | always | 1.5 | Operator, generation, band and physical identity. |

The rows are the stations or nodes of the stack. The column `name` names a row. Time: `last_heard`.

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
| `sib1_occasions` | whole number | optional | 1.5 | The slots of Type0 PDCCH occasions that the newest try searched. |
| `sib1_pdcch` | whole number | optional | 1.5 | The control channels with the SI-RNTI that the newest try decoded. |

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

### Table `survey`

Struct `SurveyRow`. A cell of a coverage survey: one row per cell found, in the same columns for every generation. The row key is the key of the cell in the cells table of the stack. The survey table has no role: the cells table is the nodes table of the stack, and a host finds the survey by its key `survey`.

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

The column `name` names a row.

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
| `search` | `sdrkit_nr_search` | none | none |
| `stay` | `sdrkit_nr_stay` | `sdrkit_cell_search_stay_args` | none |

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
| `band_search` | `sdrkit_nr_get_band_search` | `sdrkit_nr_set_band_search` |
| `search_bands` | `sdrkit_nr_get_search_bands` | `sdrkit_nr_set_search_bands` |
| `search_first` | `sdrkit_nr_get_search_first` | `sdrkit_nr_set_search_first` |
| `search_ranges` | `sdrkit_nr_get_search_ranges` | `sdrkit_nr_set_search_ranges` |
| `search_dwell_s` | `sdrkit_nr_get_search_dwell_s` | `sdrkit_nr_set_search_dwell_s` |
| `search_look_s` | `sdrkit_nr_get_search_look_s` | `sdrkit_nr_set_search_look_s` |

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
