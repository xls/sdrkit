<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GOES GVAR

The GVAR imager broadcast of GOES I to P (BPSK, 2.11 Mbit/s): the block list and reduced visible and infrared pictures.

Stack id: `goes_gvar`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.6857 GHz |
| Tuning range | 1.67 GHz to 1.71 GHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 4.5 MHz |
| Signal bandwidth | 3.2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:goes_gvar` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `picture` | Picture | choice | `Imager visible` | `Imager visible`, `Imager infrared` | The visible picture or the infrared picture. |
| `view` | View | choice | `visible` | `visible` (Visible (channel 1)), `ir2` (Channel 2 (3.9 um)), `ir3` (Channel 3 (6.5 um)), `ir4` (Channel 4 (10.7 um)), `ir6` (Channel 6 (13.3 um) or 5 (12 um)) | The channel shown. The visible picture has one view. |
| `follow` | Follow | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_lines` | Test scans | whole number | 1300 | 1 to 1974 |  |

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
| `signal_time` | Signal time | duration (s) |
| `input_rate_hz` | Input rate | frequency |
| `lock_metric` | Carrier lock | number |
| `spacecraft` | Spacecraft | whole number |
| `sync_hits` | Sync codes | whole number |
| `idle_blocks` | Idle blocks | whole number |
| `data_crc_errors` | Data CRC errors | whole number |
| `block_id` | Block | whole number |
| `block_count` | Block count | whole number |
| `scan` | Scan | whole number |
| `frame_starts` | Imager frames | whole number |
| `visible_rows` | Visible rows | whole number |
| `infrared_rows` | Infrared rows | whole number |
| `records_refused` | Records refused | whole number |
| `note` | Note | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `blocks` | Blocks | `block_count` (whole number), `block_id` (whole number), `word_size` (whole number (bit)), `word_count` (whole number), `header_ok` (yes or no), `data_ok` (yes or no) |
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
| `clear` | New picture |  |  | Starts a new picture. |

## Typed interface

The stack is in the family `satimg`, version 1.2. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite imagery (`satellite_imagery`). Media: pictures.

### Result fields

Struct `GoesGvarFields`. The fields of GOES GVAR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `snr_db` | number (dB) | optional | 1.0 | The Es/N0 of the demodulator, dB. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, dBFS. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset: the energy location, the search and the loop. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The error of the symbol clock, ppm. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The nominal symbol rate. |
| `frames_ok` | whole number | always | 1.0 | Frames decoded. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `signal_time` | duration (s) | always | 1.0 | The seconds of signal decoded. |
| `input_rate_hz` | frequency | optional | 1.0 | The input rate. |
| `lock_metric` | number | optional | 1.0 | The carrier lock metric, 0 to 1. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft of the last frame. |
| `sync_hits` | whole number | always | 1.0 | Synchronisation codes found. |
| `idle_blocks` | whole number | always | 1.0 | Idle blocks. |
| `data_crc_errors` | whole number | always | 1.0 | Blocks whose data field failed its CRC. |
| `block_id` | whole number | optional | 1.0 | The id of the last block. |
| `block_count` | whole number | optional | 1.0 | The block count of the last block. |
| `scan` | whole number | always | 1.0 | The scan count of the last imager block. |
| `frame_starts` | whole number | always | 1.0 | Imager frames whose start was seen. |
| `visible_rows` | whole number | always | 1.0 | Rows of the visible picture. |
| `infrared_rows` | whole number | always | 1.0 | Rows of the infrared picture. |
| `records_refused` | whole number | always | 1.0 | Detector records left out. |
| `note` | text | always | 1.0 | A note ("picture not decoded: ..."). |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `blocks`

Struct `GoesGvarBlockRow`. A block of GOES GVAR. The row key is the place of the block.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `block_count` | whole number | always | 1.0 | The block count of the last block. |
| `block_id` | whole number | always | 1.0 | The id of the last block. |
| `word_size` | whole number (bit) | always | 1.0 | The word size of a block. |
| `word_count` | whole number | always | 1.0 | The word count of a block. |
| `header_ok` | yes or no | always | 1.0 | A header copy of a block had a good CRC. |
| `data_ok` | yes or no | always | 1.0 | The data field of a block had a good CRC. |

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
| `clear` | `sdrkit_goes_gvar_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `picture` | `sdrkit_goes_gvar_get_picture` | `sdrkit_goes_gvar_set_picture` |
| `view` | `sdrkit_goes_gvar_get_view` | `sdrkit_goes_gvar_set_view` |
| `follow` | `sdrkit_goes_gvar_get_follow` | `sdrkit_goes_gvar_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_lines` | `sdrkit_goes_gvar_gen_get_lines` | `sdrkit_goes_gvar_gen_set_lines` |

## Try it

```sh
sdrkit run goes_gvar --source synth:goes_gvar
sdrkit run goes_gvar --source iiod:192.168.2.1 --freq 1685.7
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
