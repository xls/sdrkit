<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# AWS L-band

The direct broadcast of the Arctic Weather Satellite (QPSK, 1.785 Msym/s): scans of the microwave radiometer in 19 channels. It needs a tracking dish.

Stack id: `aws`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.707 GHz |
| Tuning range | 1.69 GHz to 1.71 GHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 2.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:aws` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `view` | View | choice | `ch1` | `ch1` (Channel 1 (54 GHz receiver)), `ch2` (Channel 2 (54 GHz receiver)), `ch3` (Channel 3 (54 GHz receiver)), `ch4` (Channel 4 (54 GHz receiver)), `ch5` (Channel 5 (54 GHz receiver)), `ch6` (Channel 6 (54 GHz receiver)), `ch7` (Channel 7 (54 GHz receiver)), `ch8` (Channel 8 (54 GHz receiver)), `ch9` (Channel 9 (89 GHz receiver)), `ch10` (Channel 10 (183 GHz receiver)), `ch11` (Channel 11 (183 GHz receiver)), `ch12` (Channel 12 (183 GHz receiver)), `ch13` (Channel 13 (183 GHz receiver)), `ch14` (Channel 14 (183 GHz receiver)), `ch15` (Channel 15 (183 GHz receiver)), `ch16` (Channel 16 (325 GHz receiver)), `ch17` (Channel 17 (325 GHz receiver)), `ch18` (Channel 18 (325 GHz receiver)), `ch19` (Channel 19 (325 GHz receiver)) | The radiometer channel shown. |
| `follow` | Follow | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_lines` | Test picture scans | whole number | 400 | 1 to 4000 |  |

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
| `lines` | Scans | whole number |
| `science_packets` | Science packets | whole number |
| `packet_crc_errors` | Packet CRC errors | whole number |
| `odd_records` | Records of another length | whole number |
| `idle_frames` | Idle frames | whole number |
| `note` | Note | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `virtual_channels` | Virtual channels | `vcid` (whole number), `frames` (whole number) |
| `apids` | Application processes | `apid` (whole number), `packets` (whole number) |
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

The stack is in the family `satimg`, version 1.1. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `AwsFields`. The fields of AWS beside the link layers.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `lines` | whole number | always | 1.0 | Scans of the radiometer. |
| `science_packets` | whole number | always | 1.0 | Science packets. |
| `packet_crc_errors` | whole number | always | 1.0 | Packets whose error control field was wrong. |
| `odd_records` | whole number | always | 1.0 | Scan records that were not 7599 bytes long. |
| `idle_frames` | whole number | always | 1.0 | Idle frames. |
| `note` | text | always | 1.0 | A note ("picture not decoded: ..."). |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `virtual_channels`

Struct `AwsVirtualChannelRow`. The frames of one virtual channel of AWS. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | A virtual channel. |
| `frames` | whole number | always | 1.0 | The frames of a virtual channel. |

### Table `apids`

Struct `AwsApidRow`. The packets of one application process of AWS. The row key is the APID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `apid` | whole number | always | 1.0 | An application process. |
| `packets` | whole number | always | 1.0 | The packets of an application process. |

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
| `clear` | `sdrkit_aws_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `view` | `sdrkit_aws_get_view` | `sdrkit_aws_set_view` |
| `follow` | `sdrkit_aws_get_follow` | `sdrkit_aws_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_lines` | `sdrkit_aws_gen_get_lines` | `sdrkit_aws_gen_set_lines` |

## Try it

```sh
sdrkit run aws --source synth:aws
sdrkit run aws --source iiod:192.168.2.1 --freq 1707
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
