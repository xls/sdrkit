<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Elektro-L / Arktika-M RDAS

The raw MSU-GS downlink of Elektro-L and Arktika-M on X band (BPSK, 30.72 Msym/s): infrared and visible pictures. It needs 40 Msps or more and an X-band dish with a down-converter; a recording is the usual input.

Stack id: `rdas`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 950 MHz to 9 GHz |
| Sample rate | 45 MHz |
| Smallest sample rate | 40 MHz |
| Signal bandwidth | 45 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `elektro_l` | Elektro-L | 0 Hz |
| `arktika_m` | Arktika-M | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:rdas` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 30.72 MHz | 1 MHz to 60 MHz | Not confirmed by a public document. |
| `roll_off` | Matched filter roll-off | number | 0.5 | 0.1 to 1 | Not confirmed by a public document. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `pass_gap` | Pass end | number (s) | 60 | 1 to 3600 | After this many seconds of signal without a picture line the pass ends: its picture is finished, and the next line starts a new picture. A shorter dropout stays in the picture. |

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
| `frame_lock` | Frame lock | yes or no |
| `sync_hits` | Sync words | whole number |
| `bytes_corrected` | Bytes corrected | whole number |
| `spacecraft` | Spacecraft | whole number |
| `vcid` | Virtual channel | whole number |
| `stream` | Bit stream | whole number |
| `inverted` | Inverted | yes or no |
| `note` | Note | text |
| `carrier_locked` | Carrier locked | yes or no |
| `packets` | MSU-GS frames | whole number |
| `lines_ir` | Infrared lines | whole number |
| `lines_visible` | Visible lines (reduced) | whole number |
| `gate_skipped` | Samples let go by | whole number |
| `gate_skipped_pct` | Input let go by | number (%) |
| `gate_line` | Gate line over mean | number |
| `hint` | Hint | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time` (duration (s)), `spacecraft` (whole number), `vcid` (whole number), `bytes` (whole number), `check` (text), `hex` (text) |
| `virtual_channels` | Virtual channels | `vcid` (whole number), `frames` (whole number) |

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
| `clear` | Clear |  |  | Empties the frame table and the counters; the synchronisation stays. |

## Typed interface

The stack is in the family `satimg`, version 2.0. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite imagery (`satellite_imagery`). Media: pictures.

Key readings: `frame_lock`, `snr_db`, `frames_ok`, `frames_bad`.

### Result fields

Struct `FrameTableFields`. The fields of every stack that lists frames: the link, the frame lock and the note.

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
| `frame_lock` | yes or no | always | 1.0 | A confirmed frame lock is held. |
| `sync_hits` | whole number | always | 1.0 | Sync words found. |
| `bytes_corrected` | whole number | always | 1.0 | Reed-Solomon symbols corrected. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft of a frame header. |
| `vcid` | whole number | optional | 1.0 | The virtual channel of a frame header. |
| `stream` | whole number | optional | 1.0 | The QPSK stream in use, 0 to 3. |
| `inverted` | yes or no | always | 1.0 | The data arrive inverted. |
| `note` | text | always | 1.0 | A note ("picture not decoded: ..."). |

### Result fields

Struct `RdasFields`. The fields of RDAS beside those of the frame table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carrier_locked` | yes or no | always | 1.0 | The carrier is locked (also while frames decode). |
| `packets` | whole number | always | 1.0 | MSU-GS frames found. |
| `lines_ir` | whole number | always | 1.0 | Infrared lines. |
| `lines_visible` | whole number | always | 1.0 | Reduced visible lines. |
| `gate_skipped` | whole number | optional | 1.0 | Input samples let go by the gate. |
| `gate_skipped_pct` | number (%) | optional | 1.0 | The share of the input let go by the gate, percent. |
| `gate_line` | number | optional | 1.0 | The height of the last gate line over the mean. |
| `hint` | text | always | 1.0 | What the user can do (empty when all is well). |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `frames`

Struct `FrameRow`. A frame of a stack that lists frames. The row key is the place of the frame in the stream.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | duration (s) | always | 1.0 | Signal time of a frame. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft of a frame header. |
| `vcid` | whole number | optional | 1.0 | The virtual channel of a frame header. |
| `bytes` | whole number | always | 1.0 | Bytes of a frame with its sync word. |
| `check` | text | always | 1.0 | The check of a frame: good, check failed, or none. |
| `hex` | text | always | 1.0 | The first 24 bytes of a frame after decoding. |

The rows are a log: they arrive over time. The column `hex` names a row. Time: `time`.

### Table `virtual_channels`

Struct `VirtualChannelRow`. The frames of one virtual channel. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel of a frame header. |
| `frames` | whole number | always | 1.0 | The good frames of a virtual channel. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_rdas_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_rdas_get_symbol_rate` | `sdrkit_rdas_set_symbol_rate` |
| `roll_off` | `sdrkit_rdas_get_roll_off` | `sdrkit_rdas_set_roll_off` |
| `pass_gap` | `sdrkit_rdas_get_pass_gap` | `sdrkit_rdas_set_pass_gap` |

## Try it

```sh
sdrkit run rdas --source synth:rdas
sdrkit run rdas --source synth:rdas --submode elektro_l
sdrkit run rdas --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Description

The link is BPSK at 30.72 Msym/s with root raised cosine pulses (roll-off 0.5) and NRZ-M. The frames are CADUs of 1024
bytes behind the marker 1ACFFC1D; the CCSDS randomiser covers the 1020 bytes after the marker and no Reed-Solomon code is
decoded, so every frame found counts as good. The virtual channel comes from bits of bytes 5 and 11. Bytes 24 to 1023 of
each CADU are a continuous bit stream for each virtual channel, which carries the frames of the MSU-GS imager behind a
64-bit sync word: infrared frames of 1820 bytes on virtual channel 4 (one segment of 183 pixels of each of 7 channels) and
visible frames of 15 210 bytes on virtual channels 2, 5 and 3 (one line of 6004 pixels of the visible channels 1, 2 and 3).
Pixels are 10-bit words; the pictures show the top 8 bits.

At 45 Msps the link has 1.46 samples a symbol: the matched filter of the demodulator reads the symbols from these
samples as they are. While no carrier is
found, a gate looks at one piece of 32 768 samples (squared, one transform) and lets the next 7 pieces go by unread; a
line 40 times over the mean opens it and sets the carrier. After 1 s without a good frame the carrier is searched again.

The submodes differ only in the satellite name. Arktika-M is in a highly elliptical orbit:
the carrier loop follows its Doppler shift, and a host can also follow a prediction by retuning.

Standard: CCSDS 131.0-B-5 (marker, randomiser).

## Submodes: notes

The stack holds no frequency (0 Hz): the host sets it from the satellites' downlinks of the data file
`data/links/xband_downlinks.csv`.

## Settings: notes

Give a value as `key=value` after the stack name.

## Items

The pictures "MSU-GS IR" (7 channels, 3294 columns, 3400 lines) and "MSU-GS visible (reduced)" (3 channels, 1501 columns,
4300 lines: every fourth visible line and pixel) leave as `picture.strip` items and the drawn picture. Every good CADU
leaves as a `satimg.cadu` item, so a host can record a frame file.

## Not confirmed by a public document

No public document of this link or of the MSU-GS frames was found. These values are not confirmed by a public document:
the symbol rate 30.72 Msym/s and the roll-off 0.5 (both are settings), NRZ-M, the CADU without Reed-Solomon code, the
virtual channel rule of bytes 5 and 11, the payload from byte 24, the MSU-GS sync word 0218A7A392DD9ABF, the infrared and
visible frame layouts, the channels of each virtual channel, and the second visible sample of each pixel (not used). The
stack was not run on a real carrier.

## Try it: notes

```sh
sdrkit run rdas --source synth:rdas/elektro_l --rate 45M --seconds 1 --audio none
sdrkit run rdas --source <driver>:<address> --submode arktika_m --freq 7865
```

The test signal is BPSK at 30.72 Msym/s with roll-off 0.5, NRZ-M coded CADUs: every third on virtual channel 2 (visible
frames of channel 1), the others on virtual channel 4 (infrared frames, 18 segments a line). The pixels are a test picture
of gradients and waves.
