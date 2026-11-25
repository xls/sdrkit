<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DECT cordless phones

DECT base stations: their identities (RFPI), slots and broadcast system information; no call content is decoded.

Stack id: `dect`. Group: Digital voice.

DECT base stations : the A-fields of the beacons with their R-CRC, the base station identity (RFPI) and the broadcast system information (Qt). The B-field is never decoded, paging and MAC control tails are logged by type only, and nothing is sent.

Standard: ETSI EN 300 175-2 (physical layer), EN 300 175-3 clauses 6.2 and 7.2 (A-field, R-CRC, Qt), EN 300 175-6 (RFPI).

Signal types: DECT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.897344 GHz |
| Tuning range | 900 MHz to 1.93 GHz |
| Sample rate | 5 MHz |
| Smallest sample rate | 4.608 MHz |
| Signal bandwidth | 1.728 MHz |
| Offset from the signal | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dect` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `band` | Band | choice | `eu` | `eu` (Europe), `us` (US / Canada (DECT 6.0)), `br` (Brazil), `la` (Latin America), `ism900` (US ISM 900) |  |
| `scan` | Scan the band's carriers | yes or no | false |  | Asks the host to visit each carrier of the band in turn. |
| `dwell` | Scan dwell | duration (s) | 5 s | 1 to 60 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_option` | Test signal bases | choice | `0` | `0` (Two bases with handsets), `1` (One base), `2` (No base) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bases` | Base stations | whole number |
| `base_bursts` | Base bursts | whole number |
| `handset_bursts` | Handset bursts | whole number |
| `carrier` | Carrier | frequency |
| `band` | Band | text |
| `carrier_number` | Carrier number | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `demod.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bases` | Base stations | `rfpi` (text), `arc` (text), `emc` (whole number), `fpn` (whole number), `rpn` (whole number), `slots` (text), `level_dbfs` (number (dBFS)), `frequency` (frequency), `last_qt` (text), `system_info` (text), `heard` (whole number), `last_heard` (time) |
| `bursts` | Bursts | `time_s` (number (s)), `from` (text), `tail_type` (text), `tail` (text), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `dect.base` | New DECT base station | `source` (text), `text` (text), `family` (choice), `rfpi` (text), `emc` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `ism`, version 4.2. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `bases`.

### Result fields

Struct `DectFields`. The result fields of the base station table of the DECT stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bases` | whole number | always | 1.0 | The base stations in the table. |
| `base_bursts` | whole number | always | 1.0 | Bursts from base stations. |
| `handset_bursts` | whole number | always | 1.0 | Bursts from handsets. |
| `carrier` | frequency | always | 1.0 | The carrier the receiver is tuned to. |
| `band` | text | optional | 1.0 | The band of the carrier, if the carrier is one of a band. |
| `carrier_number` | whole number | optional | 1.0 | The number of the carrier in its band, if the carrier is one of a band. |

### Result fields

Struct `DectDemodFields`. The counts of the DECT demodulator.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | A-fields with a good R-CRC. |
| `frames_bad` | whole number | always | 1.0 | Bursts whose S-field was found and whose A-field failed. |

### Table `bases`

Struct `DectBaseRow`. A row of the base stations. The row key is the RFPI.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rfpi` | text | always | 1.0 | The RFPI as 10 hex digits. |
| `arc` | text | always | 1.0 | The access rights class. |
| `emc` | whole number | optional | 1.0 | The equipment manufacturer code (class A). |
| `fpn` | whole number | optional | 1.0 | The fixed part number (class A). |
| `rpn` | whole number | optional | 1.0 | The radio fixed part number (class A). |
| `slots` | text | always | 1.0 | The slots in use, as 24 digits. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last beacon. |
| `frequency` | frequency | always | 1.0 | The carrier frequency. |
| `last_qt` | text | optional | 1.0 | The last Qt tail as hex. |
| `system_info` | text | optional | 1.0 | The decoded system information of the last Qt tail. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last beacon. |

The rows are the stations or nodes of the stack. The column `rfpi` names a row. Default order: the column `level_dbfs`, highest first. Time: `last_heard`.

### Table `bursts`

Struct `DectBurstRow`. A row of the burst log. The row key is the number of the burst in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the burst. |
| `from` | text | always | 1.0 | "base" or "handset". |
| `tail_type` | text | always | 1.0 | The name of the tail type. |
| `tail` | text | always | 1.0 | The tail as hex. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the burst. |

The column `from` names a row.

### Event `dect.base`

Struct `DectBaseEvent`. The fields of a new base station event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The RFPI as 10 hex digits. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `rfpi` | text | always | 1.0 | The RFPI as 10 hex digits. |
| `emc` | whole number | optional | 1.0 | The equipment manufacturer code (class A). |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dect_clear` | none | none |
| `remove` | `sdrkit_dect_remove` | `sdrkit_dect_base_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `band` | `sdrkit_dect_get_band` | `sdrkit_dect_set_band` |
| `scan` | `sdrkit_dect_get_scan` | `sdrkit_dect_set_scan` |
| `dwell` | `sdrkit_dect_get_dwell` | `sdrkit_dect_set_dwell` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_option` | `sdrkit_dect_gen_get_option` | `sdrkit_dect_gen_set_option` |

## Try it

```sh
sdrkit run dect --source synth:dect
sdrkit run dect --source iiod:192.168.2.1 --freq 1897.344
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
