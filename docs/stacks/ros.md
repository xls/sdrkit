<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ROS (HF)

Identifies ROS transmissions by their publicly reported tones and rate. The text itself is not public and is not read.

Stack id: `ros`. Group: HF and utility.

Signal types: ROS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `ros16` | ROS 16 baud | 0 Hz |
| `ros8` | ROS 8 baud | 0 Hz |
| `ros4` | ROS 4 baud | 0 Hz |
| `ros1` | ROS 1 baud (older versions) | 0 Hz |
| `rosmf` | ROS MF (MF1 / MF2 / MF7) | 0 Hz |
| `roseme` | ROS EME (VHF) | 0 Hz |
| `ros500` | ROS HF 500 Hz | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ros` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB) | How the sideband audio is made. USB is usual on HF. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_signal` | Test signal | choice | `grid` | `grid` (Tone grid), `noise` (Noise), `fsk` (FSK), `carrier` (Carrier), `sweep` (Sweep) |  |
| `gen_baud` | Symbol rate | number (Bd) | 16 | 0.5 to 64 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `sub_mode` | Sub-mode | text |
| `name` | Found | text |
| `why` | Why | text |
| `symbol_rate_hz` | Symbol rate | frequency |
| `spacing_hz` | Grid spacing | frequency |
| `fit_pct` | Grid fit | number (%) |
| `lowest_hz` | Lowest tone | frequency |
| `highest_hz` | Highest tone | frequency |
| `purity_pct` | Purity | number (%) |
| `distinct` | Positions used | whole number |
| `score_pct` | Score | number (%) |
| `detected` | Identified | yes or no |
| `weak` | Few symbols | yes or no |
| `offset_hz` | Offset | frequency |
| `detections` | Identifications | whole number |
| `misfits` | Did not fit | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `detections` | Identifications | `time_s` (number (s)), `sub_mode` (text), `name` (text), `symbol_rate_hz` (frequency), `why` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `waterfall` | Newest spectrum | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ros.detection` | ROS identified | `sub_mode` (text), `name` (text), `symbol_rate_hz` (frequency) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `hfkbd`, version 2.7. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `RosFields`. The result fields of ROS. The fields of the detection have no value without one.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sub_mode` | text | optional | 1.0 | The sub-mode found, if the rate gives one. |
| `name` | text | optional | 1.0 | What was found. |
| `why` | text | optional | 1.0 | Why. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate. |
| `spacing_hz` | frequency | optional | 1.0 | The spacing of the tone grid. |
| `fit_pct` | number (%) | optional | 1.0 | How well the tones fit the grid. |
| `lowest_hz` | frequency | optional | 1.0 | The lowest tone. |
| `highest_hz` | frequency | optional | 1.0 | The highest tone. |
| `purity_pct` | number (%) | optional | 1.0 | How pure the tones are. |
| `distinct` | whole number | optional | 1.0 | The tone positions used. |
| `score_pct` | number (%) | optional | 1.0 | The score. |
| `detected` | yes or no | always | 1.0 | The signal is identified. |
| `weak` | yes or no | always | 1.0 | Few symbols were seen. |
| `offset_hz` | frequency | always | 1.0 | The tuning error. |
| `detections` | whole number | always | 1.0 | Identifications. |
| `misfits` | whole number | always | 1.0 | Signals that did not fit. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `detections`

Struct `RosDetectionRow`. A row of the ROS identifications. The row key is the place of the identification in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `sub_mode` | text | optional | 1.0 | The sub-mode, if the rate gives one. |
| `name` | text | always | 1.0 | What was found. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `why` | text | always | 1.0 | Why. |

### Event `ros.detection`

Struct `RosDetectionEvent`. The fields of a ROS identification.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sub_mode` | text | optional | 1.0 | The sub-mode, if the rate gives one. |
| `name` | text | always | 1.0 | What was found. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ros_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `demod` | `sdrkit_ros_get_demod` | `sdrkit_ros_set_demod` |
| `listen` | `sdrkit_ros_get_listen` | `sdrkit_ros_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_signal` | `sdrkit_ros_gen_get_signal` | `sdrkit_ros_gen_set_signal` |
| `gen_baud` | `sdrkit_ros_gen_get_baud` | `sdrkit_ros_gen_set_baud` |

## Try it

```sh
sdrkit run ros --source synth:ros
sdrkit run ros --source synth:ros --submode ros16
sdrkit run ros --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/hfkbd/hfkbd_ros16_12k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
