<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HRPT / AHRPT

High-resolution pictures from MetOp (AHRPT), FY-3 frames and archived NOAA HRPT near 1.7 GHz. It needs a tracking dish.

Stack id: `hrpt`. Group: Weather satellites.

Signal types: HRPT / AHRPT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.7013 GHz |
| Tuning range | 1.69 GHz to 1.71 GHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 4.5 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `meteor` | Meteor HRPT | 1.7 GHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hrpt` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `frequency_preset` | Frequency | choice | `1701.300` | `1701.300` (1701.300 MetOp-B / MetOp-C AHRPT), `1701.400` (1701.400 FY-3 AHRPT (check the satellite)), `1698.000` (1698.000 NOAA-19 HRPT (retired)), `1702.500` (1702.500 NOAA-15 HRPT (retired)), `1707.000` (1707.000 NOAA-18 HRPT (retired)) | Active now: MetOp-B and MetOp-C (AHRPT 1701.3 MHz) and the FY-3 satellites (check the frequency of each one). |
| `variant` | Satellite | choice | `auto` | `auto` (Automatic), `metop` (MetOp AHRPT (QPSK 2.33 Msym/s)), `fy3` (FY-3 AHRPT (QPSK 2.8 Msym/s)), `noaa` (NOAA HRPT (retired)) | Restarts the demodulator. |
| `view` | View | choice | `rgb214` | `rgb214` (Colour (2, 1, 4)), `ch1` (Channel 1 (visible)), `ch2` (Channel 2 (near infrared)), `ch3` (Channel 3 (3A / 3B)), `ch4` (Channel 4 (thermal)), `ch5` (Channel 5 (thermal)) | What the picture shows. |
| `hirs_view` | HIRS view | choice | `rgb214` | `rgb214` (Colour (2, 1, 4)), `ch1` (HIRS channel 1), `ch2` (HIRS channel 2), `ch3` (HIRS channel 3), `ch4` (HIRS channel 4), `ch5` (HIRS channel 5), `ch6` (HIRS channel 6), `ch7` (HIRS channel 7), `ch8` (HIRS channel 8), `ch9` (HIRS channel 9), `ch10` (HIRS channel 10), `ch11` (HIRS channel 11), `ch12` (HIRS channel 12), `ch13` (HIRS channel 13), `ch14` (HIRS channel 14), `ch15` (HIRS channel 15), `ch16` (HIRS channel 16), `ch17` (HIRS channel 17), `ch18` (HIRS channel 18), `ch19` (HIRS channel 19), `ch20` (HIRS channel 20) | What the HIRS picture of NOAA HRPT shows. |
| `follow` | Follow | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_variant` | Test satellite | choice | `metop` | `metop` (MetOp AHRPT (QPSK 2.33 Msym/s)), `fy3` (FY-3 AHRPT (QPSK 2.8 Msym/s)), `noaa` (NOAA HRPT (retired)) |  |
| `gen_3b` | Test channel 3B (APID 104) | yes or no | false |  |  |
| `gen_lines` | Test picture lines | whole number | 60 | 1 to 4000 |  |
| `gen_polarity` | Test inverted NOAA polarity | yes or no | false |  |  |
| `gen_pm_index` | Test phase index (Meteor) | number (deg) | 67.5 | 10 to 85 |  |

The setting `view` chooses the view of the picture.

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
| `variant` | Satellite | choice |
| `variant_setting` | Satellite setting | choice |
| `phase_ambiguity_mirrored` | Second output inverted | yes or no |
| `noaa_rotation` | NOAA rotation | whole number |
| `packets_apid_103` | APID 103 packets | whole number |
| `packets_apid_104` | APID 104 packets | whole number |
| `other_packets` | Other packets | whole number |
| `noaa_frames` | NOAA minor frames | whole number |
| `avhrr_lines` | AVHRR lines | whole number |
| `picture_lines` | Picture lines | whole number |
| `onboard_time` | On board | duration (s) |
| `hirs_lines` | HIRS lines | whole number |
| `hirs_valid` | HIRS elements | whole number |
| `hirs_parity_errors` | HIRS parity errors | whole number |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `virtual_channels` | Virtual channels | `vcid` (whole number), `frames` (whole number) |
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

The stack is in the family `satimg`, version 1.3. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `HrptFields`. The fields of `hrpt` beside the link layers.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `variant` | choice | always | 1.0 | The running variant. |
| `variant_setting` | choice | always | 1.0 | The variant setting: automatic or fixed. |
| `phase_ambiguity_mirrored` | yes or no | optional | 1.0 | The code synchroniser undid a mirrored transform (the second output inverted). No value for NOAA. |
| `noaa_rotation` | whole number | optional | 1.0 | NOAA: the polarity plus 2 times the chip phase of the framer. No value for the other satellites. |
| `packets_apid_103` | whole number | always | 1.0 | AVHRR packets of APID 103. |
| `packets_apid_104` | whole number | always | 1.0 | AVHRR packets of APID 104. |
| `other_packets` | whole number | always | 1.0 | Other packets. |
| `noaa_frames` | whole number | always | 1.0 | NOAA minor frames. |
| `avhrr_lines` | whole number | always | 1.0 | AVHRR lines decoded. |
| `picture_lines` | whole number | always | 1.0 | Lines of 2048 pixels in the picture. |
| `onboard_time` | duration (s) | optional | 1.0 | The on-board time of the last line, seconds of the day. |
| `hirs_lines` | whole number | always | 1.0 | HIRS lines given out (NOAA). |
| `hirs_valid` | whole number | always | 1.0 | HIRS elements put into a line. |
| `hirs_parity_errors` | whole number | always | 1.0 | HIRS elements without odd parity. |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `virtual_channels`

Struct `VirtualChannelRow`. The frames of one virtual channel. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel of a frame header. |
| `frames` | whole number | always | 1.0 | The good frames of a virtual channel. |

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
| `clear` | `sdrkit_hrpt_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `frequency_preset` | `sdrkit_hrpt_get_frequency_preset` | `sdrkit_hrpt_set_frequency_preset` |
| `variant` | `sdrkit_hrpt_get_variant` | `sdrkit_hrpt_set_variant` |
| `view` | `sdrkit_hrpt_get_view` | `sdrkit_hrpt_set_view` |
| `hirs_view` | `sdrkit_hrpt_get_hirs_view` | `sdrkit_hrpt_set_hirs_view` |
| `follow` | `sdrkit_hrpt_get_follow` | `sdrkit_hrpt_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_variant` | `sdrkit_hrpt_gen_get_variant` | `sdrkit_hrpt_gen_set_variant` |
| `gen_3b` | `sdrkit_hrpt_gen_get_3b` | `sdrkit_hrpt_gen_set_3b` |
| `gen_lines` | `sdrkit_hrpt_gen_get_lines` | `sdrkit_hrpt_gen_set_lines` |
| `gen_polarity` | `sdrkit_hrpt_gen_get_polarity` | `sdrkit_hrpt_gen_set_polarity` |
| `gen_pm_index` | `sdrkit_hrpt_gen_get_pm_index` | `sdrkit_hrpt_gen_set_pm_index` |

## Try it

```sh
sdrkit run hrpt --source synth:hrpt
sdrkit run hrpt --source synth:hrpt --submode meteor
sdrkit run hrpt --source iiod:192.168.2.1 --freq 1701.3
```
<!-- generated: end -->

## Description

NOAA HRPT also gives a second picture, HIRS: 20 sounder channels of 56 Earth views a line, from the TIP data of the frames
of number 1 (NOAA KLM User's Guide 4.1.3 and 4.3.4.1).

## Settings: notes

Advanced settings. The owner works without any of them set.
