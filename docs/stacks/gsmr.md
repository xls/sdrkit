<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GSM / GSM-R

GSM 900 and GSM-R base stations: frequency, BSIC, operator, location area and cell identity from the broadcast channel.

Stack id: `gsmr`. Group: Cellular and railway.

Signal types: GSM.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 860 MHz to 960 MHz |
| Sample rate | 5 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 4.8 MHz |
| Radio filter | 5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gsmr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `capture_s` | Capture length | number (s) | 2.2 | 0.01 to 10 | Seconds of input searched at a time. |
| `max_carriers` | Carriers per capture | whole number | 12 | 1 to 24 | Humps searched, strongest first. |
| `gsm_band` | Band raster | choice | `900` | `900` (900 MHz), `all` (900 + 1800 + 850) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `0` | `0` (Two cells), `1` (Band edge), `2` (Three cells) |  |
| `gen_centre_hz` | Test signal radio centre | frequency | 922.8 MHz | 860 MHz to 960 MHz | The radio centre the test cells are placed against. |

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
| `railway_cells` | In the railway band | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `cells` | Cells | `plmn` (text), `operator` (text), `arfcn` (whole number), `frequency` (frequency), `bsic` (whole number), `ncc` (whole number), `bcc` (whole number), `lac` (whole number), `cell_id` (whole number), `band` (choice), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `last_heard` (time) |
| `carriers` | Carriers | `frequency` (frequency), `level_db` (number (dB)), `synchronised` (yes or no) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cell.gsm_cell` | GSM cell | `source` (text), `text` (text), `family` (choice) |

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

Struct `GsmrFields`. The result fields of `gsmr`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cell_count` | whole number | always | 1.0 | Cells in the table. |
| `identified` | whole number | always | 1.0 | Cells with a location area. |
| `railway_cells` | whole number | always | 1.0 | Cells in the railway band. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `cells`

Struct `GsmrCellRow`. A row of the cells of `gsmr`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `plmn` | text | optional | 1.0 | The network. |
| `operator` | text | optional | 1.0 | The operator of the network. |
| `arfcn` | whole number | optional | 1.0 | The ARFCN. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `bsic` | whole number | always | 1.0 | The base station identity code. |
| `ncc` | whole number | always | 1.0 | The network colour code. |
| `bcc` | whole number | always | 1.0 | The base station colour code. |
| `lac` | whole number | optional | 1.0 | The location area code. |
| `cell_id` | whole number | optional | 1.0 | The cell identity. |
| `band` | choice | optional | 1.0 | The band of the ARFCN. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `last_heard` | time | optional | 1.0 | When the cell was last heard. |

### Table `carriers`

Struct `GsmrCarrierRow`. A carrier of the newest capture of `gsmr`. The row key is its place, strongest first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The frequency. |
| `level_db` | number (dB) | always | 1.0 | The level over the floor of the spectrum. |
| `synchronised` | yes or no | always | 1.0 | Whether an SCH decoded on the carrier. |

Default order: the column `level_db`, highest first.

### Event `cell.gsm_cell`

Struct `FoundCellEvent`. A cell found or identified by a search stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The row key of the cell in the `cells` table. |
| `text` | text | always | 1.0 | What was found. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_gsmr_clear` | none | none |
| `remove` | `sdrkit_gsmr_remove` | `sdrkit_cell_search_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `capture_s` | `sdrkit_gsmr_get_capture_s` | `sdrkit_gsmr_set_capture_s` |
| `max_carriers` | `sdrkit_gsmr_get_max_carriers` | `sdrkit_gsmr_set_max_carriers` |
| `gsm_band` | `sdrkit_gsmr_get_gsm_band` | `sdrkit_gsmr_set_gsm_band` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_gsmr_gen_get_scene` | `sdrkit_gsmr_gen_set_scene` |
| `gen_centre_hz` | `sdrkit_gsmr_gen_get_centre_hz` | `sdrkit_gsmr_gen_set_centre_hz` |

## Try it

```sh
sdrkit run gsmr --source synth:gsmr
sdrkit run gsmr --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/cell/gsmr_1cell_1000k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
