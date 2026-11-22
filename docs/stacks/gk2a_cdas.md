<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GK-2A CDAS

A QPSK data link of GEO-KOMPSAT-2A (3.096 Msym/s): the frame list. Its frequency is not known: tune by hand.

Stack id: `gk2a_cdas`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.69 GHz |
| Tuning range | 1 MHz to 12 GHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 6.5 MHz |
| Signal bandwidth | 4.66 MHz |
| Automatic gain | Radio AGC |

## Settings

The stack has no settings.

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

The stack is in the family `satimg`, version 1.3. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

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

The column `hex` names a row.

### Table `virtual_channels`

Struct `VirtualChannelRow`. The frames of one virtual channel. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel of a frame header. |
| `frames` | whole number | always | 1.0 | The good frames of a virtual channel. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_gk2a_cdas_clear` | none | none |

## Try it

```sh
sdrkit run gk2a_cdas --source synth:gk2a_cdas
sdrkit run gk2a_cdas --source iiod:192.168.2.1 --freq 1690
```
<!-- generated: end -->
