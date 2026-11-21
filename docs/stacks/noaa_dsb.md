<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NOAA DSB

The TIP beacon (DSB) of the NOAA KLM satellites on 137 MHz: minor frames at 8320 bit/s and HIRS sounder lines.

Stack id: `noaa_dsb`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 137.77 MHz |
| Tuning range | 136 MHz to 138 MHz |
| Sample rate | 1.024 MHz |
| Smallest sample rate | 40 kHz |
| Signal bandwidth | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:noaa_dsb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `view` | View | choice | `rgb214` | `rgb214` (Colour (2, 1, 4)), `ch1` (HIRS channel 1), `ch2` (HIRS channel 2), `ch3` (HIRS channel 3), `ch4` (HIRS channel 4), `ch5` (HIRS channel 5), `ch6` (HIRS channel 6), `ch7` (HIRS channel 7), `ch8` (HIRS channel 8), `ch9` (HIRS channel 9), `ch10` (HIRS channel 10), `ch11` (HIRS channel 11), `ch12` (HIRS channel 12), `ch13` (HIRS channel 13), `ch14` (HIRS channel 14), `ch15` (HIRS channel 15), `ch16` (HIRS channel 16), `ch17` (HIRS channel 17), `ch18` (HIRS channel 18), `ch19` (HIRS channel 19), `ch20` (HIRS channel 20) | What the HIRS picture shows. |
| `follow` | Follow | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scid` | Test spacecraft address | whole number | 15 | 0 to 15 |  |
| `gen_pm_index` | Test phase index | number (deg) | 67 | 10 to 85 |  |
| `gen_lines` | Test picture lines | whole number | 400 | 1 to 4000 |  |

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
| `sync_hits` | Sync words | whole number |
| `tip_frames` | TIP minor frames | whole number |
| `lines` | HIRS lines | whole number |
| `minor_frame` | Minor frame | whole number |
| `chip_pairing` | Chip pairing | whole number |
| `inverted` | Inverted | yes or no |
| `hirs_parity_errors` | HIRS parity errors | whole number |
| `note` | Note | text |
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
| `clear` | New picture |  |  | Starts a new picture. |

## Typed interface

The stack is in the family `satimg`, version 1.0. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `NoaaDsbFields`. The fields of NOAA DSB.

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
| `sync_hits` | whole number | always | 1.0 | Sync words found. |
| `tip_frames` | whole number | always | 1.0 | TIP minor frames. |
| `lines` | whole number | always | 1.0 | HIRS lines. |
| `minor_frame` | whole number | optional | 1.0 | The minor frame counter of the last frame, 0 to 319. |
| `chip_pairing` | whole number | optional | 1.0 | Which chip starts a bit: 0 or 1. |
| `inverted` | yes or no | always | 1.0 | The data arrive inverted. |
| `hirs_parity_errors` | whole number | always | 1.0 | HIRS elements without odd parity. |
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
| `clear` | `sdrkit_noaa_dsb_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `view` | `sdrkit_noaa_dsb_get_view` | `sdrkit_noaa_dsb_set_view` |
| `follow` | `sdrkit_noaa_dsb_get_follow` | `sdrkit_noaa_dsb_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scid` | `sdrkit_noaa_dsb_gen_get_scid` | `sdrkit_noaa_dsb_gen_set_scid` |
| `gen_pm_index` | `sdrkit_noaa_dsb_gen_get_pm_index` | `sdrkit_noaa_dsb_gen_set_pm_index` |
| `gen_lines` | `sdrkit_noaa_dsb_gen_get_lines` | `sdrkit_noaa_dsb_gen_set_lines` |

## Try it

```sh
sdrkit run noaa_dsb --source synth:noaa_dsb
sdrkit run noaa_dsb --source iiod:192.168.2.1 --freq 137.77
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
