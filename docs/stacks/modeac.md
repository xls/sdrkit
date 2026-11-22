<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Mode A/C replies

Mode A and Mode C transponder replies on 1090 MHz: squawk codes and pressure altitudes.

Stack id: `modeac`. Group: Aircraft.

Signal types: Mode A/C replies.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.09 GHz |
| Tuning range | 1.085 GHz to 1.095 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 2 MHz |
| Radio filter | 3.5 MHz |
| Automatic gain | High fixed gain |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:modeac` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `expiry_s` | Expiry time | number (s) | 60 | 5 to 3600 | How long an aircraft's address stays known after its last trusted message. |
| `min_replies` | Replies before a code is shown | whole number | 2 | 1 to 20 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_targets` | Transponders | whole number | 6 | 1 to 50 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `modeac_replies` | Mode A/C replies | whole number |
| `modeac_codes` | Mode A/C codes | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `modeac` | Mode A/C | `code` (text), `squawk` (text), `alt_ft` (whole number (ft)), `replies` (whole number), `spi` (yes or no), `level_dbfs` (number (dBFS)), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `modeac.reply` | Mode A/C reply | `code` (text), `squawk` (text), `altitude_ft` (whole number (ft)), `spi` (yes or no), `level_dbfs` (number (dBFS)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `adsb`, version 1.2. The C header `sdrkit/adsb.h`, the registry record `registry/adsb.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `ModeAcFields`. The result fields of the Mode A/C decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `modeac_replies` | whole number | always | 1.0 | Replies decoded. |
| `modeac_codes` | whole number | always | 1.0 | Codes shown. |

### Table `modeac`

Struct `ModeAcRow`. A Mode A/C code heard at least the set number of times. The row key is the code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | text | always | 1.0 | The 13 code bits in hex. |
| `squawk` | text | always | 1.0 | The code read as a squawk. |
| `alt_ft` | whole number (ft) | optional | 1.0 | The code read as a Mode C altitude. |
| `replies` | whole number | optional | 1.0 | Replies heard. |
| `spi` | yes or no | always | 1.0 | An SPI pulse was heard. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the last reply. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. |

### Event `modeac.reply`

Struct `ModeAcReplyEvent`. The fields of the event of a Mode A/C reply.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | text | always | 1.0 | The 13 code bits in hex. |
| `squawk` | text | always | 1.0 | The code read as a squawk. |
| `altitude_ft` | whole number (ft) | optional | 1.0 | The code read as a Mode C altitude. |
| `spi` | yes or no | always | 1.0 | An SPI pulse was heard. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_modeac_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `expiry_s` | `sdrkit_modeac_get_expiry_s` | `sdrkit_modeac_set_expiry_s` |
| `min_replies` | `sdrkit_modeac_get_min_replies` | `sdrkit_modeac_set_min_replies` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_targets` | `sdrkit_modeac_gen_get_targets` | `sdrkit_modeac_gen_set_targets` |

## Try it

```sh
sdrkit run modeac --source synth:modeac
sdrkit run modeac --source iiod:192.168.2.1 --freq 1090
```
<!-- generated: end -->
