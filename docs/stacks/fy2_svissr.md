<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FY-2 S-VISSR

FengYun-2 stretched VISSR on 1687.5 MHz: full-disk infrared and visible pictures, line by line.

Stack id: `fy2_svissr`. Group: Weather satellites.

Signal types: S-VISSR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.6875 GHz |
| Tuning range | 1.67 GHz to 1.71 GHz |
| Sample rate | 2.4 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fy2_svissr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | number (ksym/s) | 0 | 0 to 2000 |  |
| `follow` | Newest | yes or no | true |  |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `pass_gap` | Pass end | number (s) | 60 | 1 to 3600 | After this many seconds of signal without a picture line the pass ends: its picture is finished, and the next line starts a new picture. A shorter dropout stays in the picture. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_symbol_rate` | Test symbol rate | frequency | 660 kHz | 100 kHz to 2 MHz |  |
| `gen_first_line` | Test first line | whole number | 0 | 0 to 2499 |  |
| `gen_lines` | Test lines | whole number | 0 | 1 to 2500 | The test signal sends this many lines of the test picture and then ends. Without end: the picture repeats. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `symbol_rate_hz` | Symbol rate | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `ber` | Bit error rate | number |
| `signal_time` | Signal time | duration (s) |
| `input_rate_hz` | Input rate | frequency |
| `lock_metric` | Carrier lock | number |
| `phase_ambiguity` | Phase ambiguity | whole number |
| `code_locked` | Code locked | yes or no |
| `nrzm` | NRZ-M | yes or no |
| `marker_errors` | Marker errors | whole number |
| `bytes_corrected` | Bytes corrected | whole number |
| `markers` | Markers | whole number |
| `spacecraft` | Spacecraft | whole number |
| `vcid` | Virtual channel | whole number |
| `frame_count` | Frame count | whole number |
| `packets` | Packets | whole number |
| `packets_lost` | Packets lost | whole number |
| `lines` | Lines | whole number |
| `syncs` | Syncs | whole number |
| `rejected` | Refused frames | whole number |
| `usable` | Usable frames | whole number |
| `preamble_ber` | Preamble BER | number |
| `data_ber` | Data BER | number |
| `line_counter` | Line counter | whole number |
| `pictures_started` | Pictures started | whole number |
| `imported` | Imported frames | whole number |
| `hint` | Hint | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `images` | Images | `product` (text), `channel` (text), `time` (duration (s)), `width` (whole number (px)), `height` (whole number (px)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `esn0` | Es/N0 | s | dB |
| `byte_errors` | Byte errors | s | % |
| `frames_per_s` | Frames per second | s |  |
| `constellation` | Constellation |  |  |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `open_svissr` | Open .svissr | `path` (text, required) |  | Decodes a file of clear 44 356-byte frames at the next input. |
| `clear` | New picture |  |  | Forgets every picture and starts new ones. |

## Typed interface

The stack is in the family `satimg`, version 2.0. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite imagery (`satellite_imagery`). Media: pictures.

Key readings: `snr_db`, `frames_ok`, `frames_bad`.

### Result fields

Struct `LinkFields`. The fields of the link layers that every coded stack publishes: the front end, the demodulator, the code, the frames and the packets. A field whose report has not come has no value.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `snr_db` | number (dB) | optional | 1.0 | The Es/N0 of the demodulator, dB. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, dBFS. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset: the energy location, the search and the loop. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The error of the symbol clock, ppm. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The nominal symbol rate. |
| `frames_ok` | whole number | optional | 1.0 | Frames decoded. |
| `frames_bad` | whole number | optional | 1.0 | Frames that failed their check. |
| `ber` | number | optional | 1.0 | The bit error rate before the code. |
| `signal_time` | duration (s) | always | 1.0 | The seconds of signal decoded. |
| `input_rate_hz` | frequency | optional | 1.0 | The input rate. |
| `lock_metric` | number | optional | 1.0 | The carrier lock metric, 0 to 1. |
| `phase_ambiguity` | whole number | optional | 1.0 | The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late. |
| `code_locked` | yes or no | optional | 1.0 | The code synchroniser holds its lock. |
| `nrzm` | yes or no | optional | 1.0 | NRZ-M was found before the code. |
| `marker_errors` | whole number | optional | 1.0 | Wrong bits of the last coded marker compared (of 52). |
| `bytes_corrected` | whole number | optional | 1.0 | Reed-Solomon symbols corrected. |
| `markers` | whole number | optional | 1.0 | Frame markers found. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft of the last frame. |
| `vcid` | whole number | optional | 1.0 | The virtual channel of the last frame. |
| `frame_count` | whole number | optional | 1.0 | The virtual channel frame count of the last frame. |
| `packets` | whole number | optional | 1.0 | Space packets given out. |
| `packets_lost` | whole number | optional | 1.0 | Space packets lost. |

### Result fields

Struct `Fy2SvissrFields`. The fields of FY-2 S-VISSR beside the link layers.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `lines` | whole number | always | 1.0 | Lines decoded. |
| `syncs` | whole number | always | 1.0 | Sync windows found. |
| `rejected` | whole number | always | 1.0 | Frames refused. |
| `usable` | whole number | always | 1.0 | Frames with under 2 % preamble errors. |
| `preamble_ber` | number | always | 1.0 | The preamble bit error ratio of the last frame. |
| `data_ber` | number | always | 1.0 | The estimate of the data bit error ratio (the set bits of the spare area). |
| `line_counter` | whole number | optional | 1.0 | The last line counter. |
| `pictures_started` | whole number | always | 1.0 | Pictures started. |
| `imported` | whole number | always | 1.0 | Frames imported. |
| `hint` | text | always | 1.0 | What to look at when nothing decodes. |

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
| `open_svissr` | `sdrkit_fy2_svissr_open_svissr` | `sdrkit_satimg_file_args` | none |
| `clear` | `sdrkit_fy2_svissr_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_fy2_svissr_get_symbol_rate` | `sdrkit_fy2_svissr_set_symbol_rate` |
| `follow` | `sdrkit_fy2_svissr_get_follow` | `sdrkit_fy2_svissr_set_follow` |
| `pass_gap` | `sdrkit_fy2_svissr_get_pass_gap` | `sdrkit_fy2_svissr_set_pass_gap` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_symbol_rate` | `sdrkit_fy2_svissr_gen_get_symbol_rate` | `sdrkit_fy2_svissr_gen_set_symbol_rate` |
| `gen_first_line` | `sdrkit_fy2_svissr_gen_get_first_line` | `sdrkit_fy2_svissr_gen_set_first_line` |
| `gen_lines` | `sdrkit_fy2_svissr_gen_get_lines` | `sdrkit_fy2_svissr_gen_set_lines` |

## Try it

```sh
sdrkit run fy2_svissr --source synth:fy2_svissr
sdrkit run fy2_svissr --source iiod:192.168.2.1 --freq 1687.5
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
