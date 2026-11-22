<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NOAA APT

Analogue APT pictures of the NOAA KLM satellites on 137 MHz: FM with a 2400 Hz subcarrier, two picture channels a line.

Stack id: `noaa_apt`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 137.1 MHz |
| Tuning range | 136 MHz to 138 MHz |
| Sample rate | 1.024 MHz |
| Smallest sample rate | 60 kHz |
| Signal bandwidth | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:noaa_apt` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `picture` | Picture | choice | `APT` | `APT`, `APT channel A`, `APT channel B` | The full line, or one of the two channels of the line. |
| `follow` | Follow | yes or no | true |  |  |
| `line_filter` | Line filter | yes or no | true |  | Removes clicks and smooths noise along each line of both pictures, with the noise measured in space A. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_lines` | Test picture lines | whole number | 400 | 1 to 4000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `signal_time` | Signal time | duration (s) |
| `input_rate_hz` | Input rate | frequency |
| `locked` | Line lock | yes or no |
| `lines` | Lines | whole number |
| `lines_missed` | Lines without sync | whole number |
| `subcarrier_level` | Subcarrier level | number |
| `sync_quality` | Sync correlation | number |
| `word_rate_hz` | Word rate | frequency |
| `note` | Note | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `images` | Images | `product` (text), `channel` (text), `time` (duration (s)), `width` (whole number (px)), `height` (whole number (px)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | New picture |  |  | Starts a new picture. |

## Typed interface

The stack is in the family `satimg`, version 1.1. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `NoaaAptFields`. The fields of NOAA APT.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, dBFS. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset: the energy location, the search and the loop. |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the symbol clock, ppm. |
| `signal_time` | duration (s) | always | 1.0 | The seconds of signal decoded. |
| `input_rate_hz` | frequency | optional | 1.0 | The input rate. |
| `locked` | yes or no | always | 1.0 | The line lock holds. |
| `lines` | whole number | always | 1.0 | Lines decoded. |
| `lines_missed` | whole number | always | 1.0 | Lines taken at the expected place because their sync was not found. |
| `subcarrier_level` | number | always | 1.0 | The mean subcarrier amplitude (full deviation is 1). |
| `sync_quality` | number | always | 1.0 | The correlation of the last line sync with its pattern. |
| `word_rate_hz` | frequency | always | 1.0 | Words a second. |
| `note` | text | always | 1.0 | A note ("picture not decoded: ..."). |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `images`

Struct `ImageRow`. A finished image. The row key names the image. The picture itself leaves on the picture stream.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `product` | text | always | 1.0 | The product, or the instrument, that an image shows. |
| `channel` | text | optional | 1.0 | The channel that an image shows, when the product names one. |
| `time` | duration (s) | optional | 1.0 | The signal time at which the image started. |
| `width` | whole number (px) | always | 1.0 | The image width in pixels. |
| `height` | whole number (px) | optional | 1.0 | The image height in pixels (rows received for a picture that grows). |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_noaa_apt_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `picture` | `sdrkit_noaa_apt_get_picture` | `sdrkit_noaa_apt_set_picture` |
| `follow` | `sdrkit_noaa_apt_get_follow` | `sdrkit_noaa_apt_set_follow` |
| `line_filter` | `sdrkit_noaa_apt_get_line_filter` | `sdrkit_noaa_apt_set_line_filter` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_lines` | `sdrkit_noaa_apt_gen_get_lines` | `sdrkit_noaa_apt_gen_set_lines` |

## Try it

```sh
sdrkit run noaa_apt --source synth:noaa_apt
sdrkit run noaa_apt --source iiod:192.168.2.1 --freq 137.1
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
