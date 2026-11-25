<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GOES CDA

The telemetry downlinks of GOES-R (BPSK, 40 or 4 ksps) and GOES-N (PM, 4 kbit/s) to their ground station: the frame list. It needs a fixed dish.

Stack id: `goes_cda`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.693 GHz |
| Tuning range | 1.67 GHz to 1.71 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 80 kHz |
| Offset from the signal | 100 kHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `r40` | GOES-R 40 ksps | 1.693 GHz |
| `r4` | GOES-R 4 ksps | 1.693 GHz |
| `n` | GOES-N | 1.694 GHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:goes_cda` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_pm_index` | Test phase index (GOES-N) | number (°) | 60 | 10 to 85 |  |

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

Content kind: Satellite links (`satellite_links`). Media: none.

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
| `clear` | `sdrkit_goes_cda_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_pm_index` | `sdrkit_goes_cda_gen_get_pm_index` | `sdrkit_goes_cda_gen_set_pm_index` |

## Try it

```sh
sdrkit run goes_cda --source synth:goes_cda
sdrkit run goes_cda --source synth:goes_cda --submode r40
sdrkit run goes_cda --source iiod:192.168.2.1 --freq 1693
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
