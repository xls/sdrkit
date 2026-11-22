<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Meteor-M LRPT

Pictures from the Meteor-M weather satellites as they pass over, on 137 MHz.

Stack id: `meteor_lrpt`. Group: Weather satellites.

Signal types: LRPT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 137.9 MHz |
| Tuning range | 136 MHz to 139 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 150 kHz |
| Offset from the signal | 200 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:meteor_lrpt` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `frequency_preset` | Frequency | choice | `137.900` | `137.900` (137.900 MHz (N2-4, N2-3)), `137.100` (137.100 MHz) | Meteor-M N2-3 and N2-4 send LRPT on 137.900 or 137.100 MHz. Which one each satellite uses changes, so use a current list. |
| `modulation` | Modulation | choice | `auto` | `auto` (Automatic), `oqpsk72` (OQPSK 72k (N2-3, N2-4)), `qpsk72` (QPSK 72k (N2)), `oqpsk80` (OQPSK 80k interleaved) | Automatic tries OQPSK 72k (N2-3, N2-4), then QPSK 72k, then the interleaved 80k link, until frames decode. |
| `view` | View | choice | `auto` | `auto` (Colour (automatic)), `rgb123` (Colour 123), `rgb125` (Colour 125), `ch1` (Channel 1), `ch2` (Channel 2), `ch3` (Channel 3), `ch4` (Channel 4), `ch5` (Channel 5), `ch6` (Channel 6) | What the picture shows. |
| `enhance` | Enhancement | choice | `none` | `none` (As received), `stretch` (Contrast stretch), `brighten` (Stretch and brighten), `thermal` (Thermal colours), `satellite` (Satellite preset) | Thermal colours apply to a single channel view. |
| `follow` | Follow | yes or no | true |  | Keeps the newest lines in view. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_modulation` | Test modulation | choice | `oqpsk72` | `oqpsk72` (OQPSK 72k (N2-3, N2-4)), `qpsk72` (QPSK 72k (N2)), `oqpsk80` (OQPSK 80k interleaved) |  |
| `gen_night` | Test night channels | yes or no | false |  |  |
| `gen_rows` | Test picture rows | whole number | 40 | 1 to 1500 |  |

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
| `modulation` | Modulation | choice |
| `modulation_setting` | Modulation setting | choice |
| `interleaver_locked` | Interleaver markers | yes or no |
| `onboard_time` | On board | duration (s) |
| `quality` | Quality | whole number |
| `picture_rows` | Picture rows | whole number |
| `picture_lines` | Picture lines | whole number |
| `row_time` | Row time | duration (s) |
| `first_decoding` | First frames | duration (s) |
| `colour_channels` | Colour channels | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Channels | `channel` (text), `packets` (whole number) |
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

Struct `MeteorLrptFields`. The fields of Meteor-M LRPT beside the link layers.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `modulation` | choice | always | 1.0 | The running modulation. |
| `modulation_setting` | choice | always | 1.0 | The modulation setting: automatic or fixed. |
| `interleaver_locked` | yes or no | always | 1.0 | The deinterleaver of the 80k link holds the markers. |
| `onboard_time` | duration (s) | optional | 1.0 | The on-board time of the last image packet, seconds of the day (Moscow time). |
| `quality` | whole number | optional | 1.0 | The quality factor of the last image packet. |
| `picture_rows` | whole number | always | 1.0 | Rows of 8 lines in the picture. |
| `picture_lines` | whole number | always | 1.0 | Lines of 1568 pixels in the picture. |
| `row_time` | duration (s) | optional | 1.0 | The row time learnt from the on-board times. |
| `first_decoding` | duration (s) | optional | 1.0 | The signal time at which frames first decoded. |
| `colour_channels` | text | optional | 1.0 | The channels the automatic colour view shows ("1,2,3"). |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `channels`

Struct `MeteorLrptChannelRow`. The packets of one channel of Meteor-M LRPT. The row key is the channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | text | always | 1.0 | The name of an image channel or the telemetry. |
| `packets` | whole number | always | 1.0 | The packets of a channel. |

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
| `clear` | `sdrkit_meteor_lrpt_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `frequency_preset` | `sdrkit_meteor_lrpt_get_frequency_preset` | `sdrkit_meteor_lrpt_set_frequency_preset` |
| `modulation` | `sdrkit_meteor_lrpt_get_modulation` | `sdrkit_meteor_lrpt_set_modulation` |
| `view` | `sdrkit_meteor_lrpt_get_view` | `sdrkit_meteor_lrpt_set_view` |
| `enhance` | `sdrkit_meteor_lrpt_get_enhance` | `sdrkit_meteor_lrpt_set_enhance` |
| `follow` | `sdrkit_meteor_lrpt_get_follow` | `sdrkit_meteor_lrpt_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_modulation` | `sdrkit_meteor_lrpt_gen_get_modulation` | `sdrkit_meteor_lrpt_gen_set_modulation` |
| `gen_night` | `sdrkit_meteor_lrpt_gen_get_night` | `sdrkit_meteor_lrpt_gen_set_night` |
| `gen_rows` | `sdrkit_meteor_lrpt_gen_get_rows` | `sdrkit_meteor_lrpt_gen_set_rows` |

## Try it

```sh
sdrkit run meteor_lrpt --source synth:meteor_lrpt
sdrkit run meteor_lrpt --source iiod:192.168.2.1 --freq 137.9
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
