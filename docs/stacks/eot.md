<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# End of train (EOT / HOT)

End-of-train devices: unit, brake pipe pressure, motion, battery and marker light from 1200 bit/s AFSK on FM, or the head-of-train commands.

Stack id: `eot`. Group: Cellular and railway.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 450 MHz |
| Tuning range | 25 MHz to 1 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 12.5 kHz |
| Offset from the signal | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:eot` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `direction` | Link | choice | `eot` | `eot` (End of train to head of train), `hot` (Head of train to end of train) | End of train to head of train (telemetry) or head of train to end of train (commands). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_unit` | Test signal unit | whole number | 12345 | 0 to 131071 |  |
| `gen_psi` | Test signal brake pipe | whole number (psi) | 90 | 0 to 127 |  |
| `gen_type` | Test signal message type | whole number | 0 | 0 to 7 |  |
| `gen_motion` | Test signal moving | yes or no | true |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `frames_repaired` | Frames repaired | whole number |
| `syncs` | Syncs | whole number |
| `last_unit` | Last unit | text |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `modem.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `link` (text), `unit` (text), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `eot.frame` | End-of-train frame | `link` (text), `source` (text), `message_type` (whole number), `chaining` (whole number), `battery` (whole number), `pressure_psi` (whole number (psi)), `charge_pct` (whole number (%)), `spare` (yes or no), `valve_open` (yes or no), `confirmation` (yes or no), `turbine` (yes or no), `moving` (yes or no), `marker_battery` (yes or no), `marker_light` (yes or no), `bits_corrected` (whole number), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `cell`, version 1.9. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `EotFields`. The result fields of `eot`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames shown. |
| `frames_bad` | whole number | always | 1.0 | Frames the code refused. |
| `frames_repaired` | whole number | always | 1.0 | Frames with bits repaired. |
| `syncs` | whole number | always | 1.0 | Frame syncs. |
| `last_unit` | text | optional | 1.0 | The unit of the newest frame. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `frames`

Struct `EotFrameRow`. A frame of `eot`, newest first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `link` | text | always | 1.0 | The link: end of train or head of train. |
| `unit` | text | always | 1.0 | The unit. |
| `text` | text | always | 1.0 | The frame for people. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `eot.frame`

Struct `EotFrameEvent`. One frame with every field. The fields of the end-of-train message are absent in a head-of-train frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | text | always | 1.0 | The link. |
| `source` | text | always | 1.0 | The unit. |
| `message_type` | whole number | always | 1.0 | The message type. |
| `chaining` | whole number | optional | 1.0 | The chaining bits. |
| `battery` | whole number | optional | 1.0 | The battery condition. |
| `pressure_psi` | whole number (psi) | optional | 1.0 | The brake pipe pressure. |
| `charge_pct` | whole number (%) | optional | 1.0 | The battery charge. |
| `spare` | yes or no | optional | 1.0 | The spare bit. |
| `valve_open` | yes or no | optional | 1.0 | The valve is open. |
| `confirmation` | yes or no | optional | 1.0 | The confirmation bit. |
| `turbine` | yes or no | optional | 1.0 | The turbine runs. |
| `moving` | yes or no | optional | 1.0 | The train moves. |
| `marker_battery` | yes or no | optional | 1.0 | The marker battery is good. |
| `marker_light` | yes or no | optional | 1.0 | The marker light is on. |
| `bits_corrected` | whole number | always | 1.0 | The bits repaired. |
| `text` | text | always | 1.0 | The frame for people. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_eot_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `direction` | `sdrkit_eot_get_direction` | `sdrkit_eot_set_direction` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_unit` | `sdrkit_eot_gen_get_unit` | `sdrkit_eot_gen_set_unit` |
| `gen_psi` | `sdrkit_eot_gen_get_psi` | `sdrkit_eot_gen_set_psi` |
| `gen_type` | `sdrkit_eot_gen_get_type` | `sdrkit_eot_gen_set_type` |
| `gen_motion` | `sdrkit_eot_gen_get_motion` | `sdrkit_eot_gen_set_motion` |

## Try it

```sh
sdrkit run eot --source synth:eot
sdrkit run eot --source iiod:192.168.2.1 --freq 450
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
