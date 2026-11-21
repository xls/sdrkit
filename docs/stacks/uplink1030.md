<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Mode S interrogations 1030 MHz

Radar interrogations on 1030 MHz: Mode A, Mode C, all-calls and Mode S uplink messages with the aircraft they address.

Stack id: `uplink1030`. Group: Aircraft.

Signal types: Mode S interrogations.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.03 GHz |
| Tuning range | 1.025 GHz to 1.035 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 8 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 8 MHz |
| Automatic gain | High fixed gain |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:uplink1030` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `expiry_s` | Expiry time | number (s) | 60 | 5 to 3600 | How long an aircraft's address stays known after its last trusted message. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `interrogations` | Interrogations | whole number |
| `mode_a` | Mode A | whole number |
| `mode_c` | Mode C | whole number |
| `mode_s` | Mode S | whole number |
| `all_call_a` | Mode A/C/S all-call | whole number |
| `all_call_c` | Mode C/S all-call | whole number |
| `ac_only_all_call` | Mode A/C-only all-call | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `addressed` | Addressed aircraft | `icao` (text), `interrogations` (whole number), `formats` (text), `last` (text), `last_heard` (time) |
| `interrogators` | Interrogators | `code` (text), `all_calls` (whole number), `last_heard` (time) |
| `by_uf` | Interrogations by format | `format_number` (whole number), `count` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `uplink.interrogation` | Interrogation | `kind` (choice), `level_dbfs` (number (dBFS)), `p2` (yes or no), `uf` (whole number), `hex` (text), `target` (text), `confirmed` (yes or no), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `adsb`, version 1.1. The C header `sdrkit/adsb.h`, the registry record `registry/adsb.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `UplinkFields`. The result fields of the interrogation decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `interrogations` | whole number | always | 1.0 | Interrogations of all kinds. |
| `mode_a` | whole number | always | 1.0 | Mode A interrogations. |
| `mode_c` | whole number | always | 1.0 | Mode C interrogations. |
| `mode_s` | whole number | always | 1.0 | Mode S interrogations. |
| `all_call_a` | whole number | always | 1.0 | Mode A/C/S all-calls. |
| `all_call_c` | whole number | always | 1.0 | Mode C/S all-calls. |
| `ac_only_all_call` | whole number | always | 1.0 | Mode A/C-only all-calls. |

### Table `addressed`

Struct `AddressedRow`. An aircraft that the interrogations address. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `icao` | text | always | 1.0 | The interrogated address. |
| `interrogations` | whole number | optional | 1.0 | Interrogations. |
| `formats` | text | always | 1.0 | Uplink formats with their counts ("UF4x3 UF5x1"). |
| `last` | text | always | 1.0 | The fields of the last interrogation. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. |

### Table `interrogators`

Struct `InterrogatorRow`. An interrogator heard in all-calls. The row key is its code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | text | always | 1.0 | The interrogator code of an all-call. |
| `all_calls` | whole number | optional | 1.0 | All-calls. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. |

### Table `by_uf`

Struct `CountRow`. A count of messages of one format. The row key is the format number in two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `format_number` | whole number | always | 1.0 | A downlink or uplink format number. |
| `count` | whole number | always | 1.0 | How many. |

### Event `uplink.interrogation`

Struct `InterrogationEvent`. The fields of the event of an interrogation. The interrogated address is the target once it is confirmed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | What the interrogation is. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `p2` | yes or no | always | 1.0 | The P2 (side-lobe suppression) pulse was seen. |
| `uf` | whole number | optional | 1.0 | The uplink format of a Mode S interrogation. |
| `hex` | text | optional | 1.0 | The Mode S message in hex. |
| `target` | text | optional | 1.0 | The interrogated address, once it is confirmed (see `confirmed`). |
| `confirmed` | yes or no | optional | 1.1 | Mode S to one aircraft: the address from the parity is in the table of addressed aircraft, so it is the target. False when it is not yet: a damaged message also gives an address, so the target has no value and the address is only in the message. No value for Mode A/C, all-calls and broadcasts. |
| `text` | text | optional | 1.0 | The fields of a Mode S interrogation. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_uplink1030_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `expiry_s` | `sdrkit_uplink1030_get_expiry_s` | `sdrkit_uplink1030_set_expiry_s` |

## Try it

```sh
sdrkit run uplink1030 --source synth:uplink1030
sdrkit run uplink1030 --source iiod:192.168.2.1 --freq 1030
```
<!-- generated: end -->
