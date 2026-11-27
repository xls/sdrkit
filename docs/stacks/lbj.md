<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LBJ

Chinese railway train broadcasts: train number, speed, kilometre post, locomotive, route and position.

Stack id: `lbj`. Group: Cellular and railway.

Signal types: LBJ.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 821 MHz to 821.5 MHz |
| Sample rate | 960 kHz |
| Smallest sample rate | 256 kHz |
| Signal bandwidth | 35 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 150 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:lbj` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `locomotives` | Locomotive classes | text | empty |  | The class names of the locomotive type codes, one code and its name a line (the data file data/cell/locomotives.txt). Without it a locomotive shows its type code. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_deviation_hz` | Test signal deviation | frequency | 4.2 kHz | 1 kHz to 8 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bursts` | Bursts | whole number |
| `messages` | Messages | whole number |
| `failed_checks` | Failed checks | whole number |
| `blank_frames` | Empty frames | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `dropped` | Dropped | whole number |
| `modem.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `received` (time), `train` (text), `speed_kmh` (whole number (km/h)), `km_post_km` (number (km)), `direction` (choice), `locomotive` (text), `route` (text), `position` (position), `level_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `lbj.message` | LBJ message | `source` (text), `text` (text), `family` (choice), `position` (position), `speed_kt` (number (kt)), `speed_kmh` (whole number (km/h)), `km_post_km` (number (km)), `direction` (choice), `locomotive` (text), `route` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `cell`, version 1.10. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `LbjFields`. The result fields of `lbj`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts` | whole number | always | 1.0 | Bursts with a sync word. |
| `messages` | whole number | always | 1.0 | Messages shown. |
| `failed_checks` | whole number | always | 1.0 | Messages refused by the checks. |
| `blank_frames` | whole number | always | 1.0 | Empty frames. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the channel. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the channel. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the newest message. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `LbjMessageRow`. A message of `lbj`. The row key is the place of the message in the order received.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `received` | time | optional | 1.0 | When the message came (wall clock), when known. |
| `train` | text | optional | 1.0 | The train (prefix and digits), or the network time of a time message. |
| `speed_kmh` | whole number (km/h) | optional | 1.0 | The speed. |
| `km_post_km` | number (km) | optional | 1.0 | The kilometre post. |
| `direction` | choice | optional | 1.0 | Up or down. |
| `locomotive` | text | optional | 1.0 | The locomotive class and number. |
| `route` | text | optional | 1.0 | The route name. |
| `position` | position | optional | 1.0 | The position. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the burst. |

The rows are a log: they arrive over time. Place of a row: the column `position`. Speed: `speed_kmh`. Time: `received`.

### Event `lbj.message`

Struct `LbjMessageEvent`. A message of `lbj` shown or completed by its pair.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The train, or the network time of a time message. |
| `text` | text | always | 1.0 | The message for people. |
| `family` | choice | always | 1.0 | The family of the source. |
| `position` | position | optional | 1.0 | The position. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed in knots. |
| `speed_kmh` | whole number (km/h) | optional | 1.0 | The speed. |
| `km_post_km` | number (km) | optional | 1.0 | The kilometre post. |
| `direction` | choice | optional | 1.0 | Up or down. |
| `locomotive` | text | optional | 1.0 | The locomotive class and number. |
| `route` | text | optional | 1.0 | The route name. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_lbj_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `locomotives` | `sdrkit_lbj_get_locomotives` | `sdrkit_lbj_set_locomotives` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_deviation_hz` | `sdrkit_lbj_gen_get_deviation_hz` | `sdrkit_lbj_gen_set_deviation_hz` |

## Try it

```sh
sdrkit run lbj --source synth:lbj
sdrkit run lbj --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/cell/lbj_256k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
