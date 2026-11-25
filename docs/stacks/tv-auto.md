<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# TV (auto)

Finds out which TV standard the channel carries (DVB-T, DVB-T2, DVB-C, DVB-S/S2, ISDB-T, ATSC 1.0 / 3.0, DTMB, analogue) and its parameters, then switches the right stack on

Stack id: `tv-auto`. Group: TV.

Signal types: DVB-T, DVB-T2, DVB-C, DVB-S, DVB-S2, ISDB-T, ATSC 1.0, ATSC 3.0, DTMB, Analog TV.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 474 MHz |
| Tuning range | 40 MHz to 2.2 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:tv-auto` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `satellite` | Satellite input | yes or no | false |  | The input comes from a dish (an LNB). |
| `decode_video` | Decode video | yes or no | false |  | The stack that TV (auto) switches on sends MPEG-2 video as frames of raw pixels (I420) in display order. H.264 and HEVC always leave as coded pictures. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `line` | TV (auto) | text |
| `progress_pct` | Capture | number (%) |
| `tries` | Tries | whole number |
| `switch_to` | Stack | text |
| `standard` | Standard | choice |
| `headline` | Found | text |
| `verdict` | Verdict | choice |
| `confidence_pct` | Confidence | number (%) |
| `width` | Channel width | frequency |
| `centre_offset` | Channel centre offset | frequency |
| `try_again_offered` | Try again offered | yes or no |
| `dropped` | Dropped | whole number |

## Events

| Kind | Label | Fields |
|---|---|---|
| `tv-auto.found` | TV standard found | `stack` (text), `standard` (choice), `text` (text), `width` (frequency), `sure` (yes or no), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `try_again` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `tvterr`, version 2.0. The C header `sdrkit/tvterr.h`, the registry record `registry/tvterr.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `standard`, `verdict`, `confidence_pct`, `width`.

### Result fields

Struct `AutoFields`. The result fields of `tv-auto`: the line shown, the capture and the last detection. A value of the detection has no value before the first detection.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `line` | text | always | 1.0 | The line shown ("Listening to the channel..."). |
| `progress_pct` | number (%) | always | 1.0 | The progress of the capture. |
| `tries` | whole number | always | 1.0 | Captures looked at. |
| `switch_to` | text | optional | 1.0 | The id of the stack requested, when one is found. |
| `standard` | choice | optional | 1.0 | The standard of the last detection. |
| `headline` | text | optional | 1.0 | The headline of the last detection. |
| `verdict` | choice | optional | 1.0 | The verdict of the last detection. |
| `confidence_pct` | number (%) | optional | 1.0 | The confidence of the last detection. |
| `width` | frequency | optional | 1.0 | The channel width found. |
| `centre_offset` | frequency | optional | 1.0 | The centre of the channel from the centre of the capture. |
| `try_again_offered` | yes or no | always | 1.0 | The detector stopped: the command `try_again` starts it again. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Event `tv-auto.found`

Struct `AutoFoundEvent`. The fields of a standard found. The text is the headline.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `stack` | text | always | 1.0 | The id of the stack that receives the standard found. |
| `standard` | choice | always | 1.0 | The standard found. |
| `text` | text | always | 1.0 | The headline. |
| `width` | frequency | always | 1.0 | The channel width found. |
| `sure` | yes or no | always | 1.0 | The detector is sure. |
| `family` | choice | always | 1.0 | The family of the source: a receiver. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `try_again` | `sdrkit_tv_auto_try_again` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `satellite` | `sdrkit_tv_auto_get_satellite` | `sdrkit_tv_auto_set_satellite` |
| `decode_video` | `sdrkit_tv_auto_get_decode_video` | `sdrkit_tv_auto_set_decode_video` |

## Try it

```sh
sdrkit run tv-auto --source iiod:192.168.2.1 --freq 474
```

The stack has no test signal. The `synth` source plays silence for it.
<!-- generated: end -->
