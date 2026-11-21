<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FY-3 AHRPT

Fengyun-3 AHRPT: VIRR, MERSI and MWRI pictures from the polar FY-3 satellites (L-band, and X-band with a down-converter).

Stack id: `fy3_ahrpt`. Group: Weather satellites.

Signal types: HRPT / AHRPT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.7014 GHz |
| Tuning range | 1.69 GHz to 1.71 GHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 4.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fy3_ahrpt` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `preset` | Satellite | choice | `auto` | `auto` (Automatic), `fy3c` (1701.400 FY-3C AHRPT (2.6 Msym/s)), `fy3ab` (1704.500 FY-3A / FY-3B AHRPT (2.8 Msym/s)), `fy3g` (7790 FY-3G (6 Msym/s, X-band, needs a down-converter)), `fy3d` (7820 FY-3D (30 Msym/s, X-band, needs a down-converter and 40 Msps)), `fy3f` (7790 FY-3F (38.4 Msym/s, X-band)), `fy3eh` (7860 FY-3E / FY-3H (38.4 Msym/s, X-band)) | Retunes to the preset's frequency; restarts. |
| `symbol_rate` | Symbol rate | number (Msym/s) | 0 | 0 to 500 | 0 = automatic. Restarts. |
| `roll_off` | Roll-off | number | 0 | 0 to 1 | 0 = automatic. Restarts. |
| `instrument` | Instrument | choice | `VIRR` | `VIRR`, `MERSI`, `MERSI-II`, `MERSI-3`, `MERSI-LL`, `MERSI-RM`, `MWRI`, `MWRI-2` | The instrument whose picture is shown. |
| `view` | View | choice | `true` | `true` (True colour (1, 9, 7)), `false` (False colour (2, 1, 4 inverted)), `ch1` (Channel 1 (0.63 um red)), `ch2` (Channel 2 (0.865 um)), `ch3` (Channel 3 (3.74 um)), `ch4` (Channel 4 (10.8 um)), `ch5` (Channel 5 (12 um)), `ch6` (Channel 6 (1.6 um)), `ch7` (Channel 7 (0.455 um blue)), `ch8` (Channel 8 (0.505 um)), `ch9` (Channel 9 (0.555 um green)), `ch10` (Channel 10 (1.36 um)), `ch11` (Channel 11 (1 km)), `ch12` (Channel 12 (1 km)), `ch13` (Channel 13 (1 km)), `ch14` (Channel 14 (1 km)), `ch15` (Channel 15 (1 km)), `ch16` (Channel 16 (1 km)), `ch17` (Channel 17 (1 km)), `ch18` (Channel 18 (1 km)), `ch19` (Channel 19 (1 km)), `ch20` (Channel 20 (1 km)), `ch21` (Channel 21 (1 km)), `ch22` (Channel 22 (1 km)), and others | The composite, else a channel. A view that the instrument shown lacks is refused. |
| `follow` | Follow | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_satellite` | Test spacecraft id | whole number | 51 | 0 to 255 |  |
| `gen_symbol_rate` | Test symbol rate | number (Msym/s) | 2.6 | 0.1 to 100 |  |
| `gen_qneg` | Test Q negated | yes or no | false |  |  |
| `gen_swap` | Test I and Q exchanged | yes or no | false |  |  |
| `gen_fill_every` | Test fill frame every | whole number | 0 | 0 to 1000 | 0: the instrument's own share of fill frames. |

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
| `product` | Product | choice |
| `preset` | Preset | text |
| `satellite` | Satellite | text |
| `measured_symbol_rate_hz` | Measured symbol rate | frequency |
| `rails_locked` | Rails locked | yes or no |
| `rail_i_quality` | I rail quality | number |
| `rail_q_quality` | Q rail quality | number |
| `differential_hypothesis` | Differential hypothesis | whole number |
| `picture_rows` | Picture rows | whole number |
| `hint` | Hint | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `instruments` | Instruments | `instrument` (text), `frames` (whole number), `rows` (whole number) |
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

Struct `Fy3Fields`. The fields of FY-3 AHRPT and MPT beside the link layers.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `product` | choice | always | 1.0 | The product: AHRPT or MPT. |
| `preset` | text | always | 1.0 | The preset setting (its choice key, "auto" for automatic). |
| `satellite` | text | optional | 1.0 | The satellite heard (from the spacecraft id). |
| `measured_symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate measured from the signal. |
| `rails_locked` | yes or no | optional | 1.0 | Both rails have their phase. |
| `rail_i_quality` | number | optional | 1.0 | The lock quality of the I rail, 0 to 1. |
| `rail_q_quality` | number | optional | 1.0 | The lock quality of the Q rail, 0 to 1. |
| `differential_hypothesis` | whole number | optional | 1.0 | The differential hypothesis locked (0 to 7). |
| `picture_rows` | whole number | always | 1.0 | The rows of the tallest picture. |
| `hint` | text | always | 1.0 | What to look at when nothing decodes. |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `instruments`

Struct `Fy3InstrumentRow`. An instrument of FY-3. The row key names the instrument.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `instrument` | text | always | 1.0 | An instrument. |
| `frames` | whole number | always | 1.0 | The frames of an instrument. |
| `rows` | whole number | always | 1.0 | The lines or scans of an instrument. |

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
| `clear` | `sdrkit_fy3_ahrpt_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `preset` | `sdrkit_fy3_ahrpt_get_preset` | `sdrkit_fy3_ahrpt_set_preset` |
| `symbol_rate` | `sdrkit_fy3_ahrpt_get_symbol_rate` | `sdrkit_fy3_ahrpt_set_symbol_rate` |
| `roll_off` | `sdrkit_fy3_ahrpt_get_roll_off` | `sdrkit_fy3_ahrpt_set_roll_off` |
| `instrument` | `sdrkit_fy3_ahrpt_get_instrument` | `sdrkit_fy3_ahrpt_set_instrument` |
| `view` | `sdrkit_fy3_ahrpt_get_view` | `sdrkit_fy3_ahrpt_set_view` |
| `follow` | `sdrkit_fy3_ahrpt_get_follow` | `sdrkit_fy3_ahrpt_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_satellite` | `sdrkit_fy3_ahrpt_gen_get_satellite` | `sdrkit_fy3_ahrpt_gen_set_satellite` |
| `gen_symbol_rate` | `sdrkit_fy3_ahrpt_gen_get_symbol_rate` | `sdrkit_fy3_ahrpt_gen_set_symbol_rate` |
| `gen_qneg` | `sdrkit_fy3_ahrpt_gen_get_qneg` | `sdrkit_fy3_ahrpt_gen_set_qneg` |
| `gen_swap` | `sdrkit_fy3_ahrpt_gen_get_swap` | `sdrkit_fy3_ahrpt_gen_set_swap` |
| `gen_fill_every` | `sdrkit_fy3_ahrpt_gen_get_fill_every` | `sdrkit_fy3_ahrpt_gen_set_fill_every` |

## Try it

```sh
sdrkit run fy3_ahrpt --source synth:fy3_ahrpt
sdrkit run fy3_ahrpt --source iiod:192.168.2.1 --freq 1701.4
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
