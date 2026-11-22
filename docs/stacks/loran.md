<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Loran-C / eLoran

A Loran chain on 100 kHz: its GRI, the master and secondaries, their time differences, ECD and signal to noise.

Stack id: `loran`. Group: Navigation.

Signal types: Loran-C.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 100 kHz |
| Tuning range | 90 kHz to 110 kHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 20 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:loran` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gri` | GRI | whole number | 0 | 0 to 99990 | The chain's group repetition interval (9960 for 99.6 ms). 0 searches the chains of the setting chains. |
| `average` | Averaging | duration (s) | 2 s | 0.2 to 60 |  |
| `threshold` | Secondary threshold | number (dB) | 12 | 3 to 40 |  |
| `max_secondaries` | Secondaries | whole number | 5 | 0 to 7 |  |
| `chains` | Chains | text | empty |  | One a line (or separated by ;): the GRI in tens of microseconds, a comma, the chain's name. With GRI 0 these GRIs are searched; the chain found takes its name. Fewer rows cost less to search. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_gri` | Test signal: GRI | whole number | 9960 | 4000 to 9999 |  |
| `gen_tds` | Test signal: TDs (µs) | text | `13797.2,26969.93,42221.65` |  | The time differences of the secondaries in µs, separated by commas. |
| `gen_ecd` | Test signal: ECD of the first secondary | number (µs) | 0 | -5 to 5 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `gri` | GRI | whole number |
| `chain` | Chain | text |
| `master` | Master | yes or no |
| `master_snr_db` | Master SNR | number (dB) |
| `fixes` | Measurements | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations | `station` (text), `td_us` (number (µs)), `ecd_us` (number (µs)), `snr_db` (number (dB)), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avnav`, version 1.4. The C header `sdrkit/avnav.h`, the registry record `registry/avnav.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Navigation aids (`navigation`). Media: none.

### Result fields

Struct `LoranFields`. The result fields of Loran. A field of the chain has no value before the first measurement.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `gri` | whole number | optional | 1.0 | The GRI in use, in tens of microseconds. |
| `chain` | text | optional | 1.0 | The chain of that GRI. No value for a GRI that the `chains` setting does not name. |
| `master` | yes or no | always | 1.0 | The master is found: its correlation SNR is at least 10 dB. |
| `master_snr_db` | number (dB) | optional | 1.0 | The correlation SNR of the master. |
| `fixes` | whole number | always | 1.0 | Measurements made. |

### Table `stations`

Struct `LoranStationRow`. A row of the stations of the chain. The row key is the letter of the station.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station` | text | always | 1.0 | `M` for the master, then the letters of the secondaries. |
| `td_us` | number (µs) | always | 1.0 | The time difference from the master. |
| `ecd_us` | number (µs) | always | 1.0 | The envelope-to-cycle difference relative to the master. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the correlation. |
| `level_db` | number (dB) | always | 1.0 | The level relative to the master. |

The column `station` names a row.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_loran_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `gri` | `sdrkit_loran_get_gri` | `sdrkit_loran_set_gri` |
| `average` | `sdrkit_loran_get_average` | `sdrkit_loran_set_average` |
| `threshold` | `sdrkit_loran_get_threshold` | `sdrkit_loran_set_threshold` |
| `max_secondaries` | `sdrkit_loran_get_max_secondaries` | `sdrkit_loran_set_max_secondaries` |
| `chains` | `sdrkit_loran_get_chains` | `sdrkit_loran_set_chains` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_gri` | `sdrkit_loran_gen_get_gri` | `sdrkit_loran_gen_set_gri` |
| `gen_tds` | `sdrkit_loran_gen_get_tds` | `sdrkit_loran_gen_set_tds` |
| `gen_ecd` | `sdrkit_loran_gen_get_ecd` | `sdrkit_loran_gen_set_ecd` |

## Try it

```sh
sdrkit run loran --source synth:loran
sdrkit run loran --source <driver>:<address> --freq 0.1
```
<!-- generated: end -->

## Description

The chains are data: pass the rows of `data/navigation/loran_chains.csv` (Loran-C and Chayka chains) as the setting
`chains`, or a shorter list to search fewer GRIs.

## Settings: notes

Advanced settings. The owner works without any of them set.
