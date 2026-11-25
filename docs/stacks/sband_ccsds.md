<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# S-band CCSDS

A CCSDS telemetry link on S or L band, set by its link settings (modulation, rate, code, frame): the frame list with spacecraft and virtual channel ids. It needs a tracking dish.

Stack id: `sband_ccsds`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.215 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 4 MHz |
| Smallest sample rate | 500 kHz |
| Signal bandwidth | 1.52 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sband_ccsds` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `link_name` | Link name | text | `S-band link` |  | The name in the summary. |
| `modulation` | Modulation | choice | `bpsk` | `bpsk` (BPSK), `qpsk` (QPSK), `oqpsk` (OQPSK), `pm` (PM (residual carrier)), `fsk` (FSK) |  |
| `symbol_rate` | Symbol rate | frequency | 1 MHz | 1 kHz to 20 MHz |  |
| `roll_off` | Matched filter roll-off | number | 0.5 | 0.1 to 1 |  |
| `conv` | K = 7 code | yes or no | false |  | The rate 1/2 code of CCSDS, second output inverted, over the frames and their sync words. |
| `frame_bits` | Frame length | whole number (bits) | 10232 | 64 to 262144 | The frame with its sync word. |
| `sync_word` | Sync word | text | `1ACFFC1D` |  | 32 bits, 8 hexadecimal digits. |
| `nrzm` | NRZ-M | yes or no | false |  | On a coded link before the code; on uncoded QPSK the four-phase differential coding. |
| `randomiser` | CCSDS randomiser | choice | `frame` | `none` (None), `frame` (On the coded frame), `inside` (Inside the Reed-Solomon code) |  |
| `rs` | Reed-Solomon | choice | `255_223` | `none` (None), `255_223` ((255,223)), `255_239` ((255,239)) |  |
| `rs_interleave` | Interleave | whole number | 5 | 1 to 8 |  |
| `rs_basis` | Basis | choice | `dual` | `dual` (Dual), `conventional` (Conventional) |  |

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
| `link` | Link | text |
| `carrier_locked` | Carrier locked | yes or no |
| `code_locked` | Code locked | yes or no |
| `phase_ambiguity` | Phase ambiguity | whole number |
| `hint` | Hint | text |

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

### Result fields

Struct `SbandCcsdsFields`. The fields of the S-band CCSDS stack beside those of the frame table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | text | always | 1.0 | The name of the link. |
| `carrier_locked` | yes or no | optional | 1.0 | The carrier is locked (also while frames decode). |
| `code_locked` | yes or no | optional | 1.0 | The code synchroniser holds its lock. |
| `phase_ambiguity` | whole number | optional | 1.0 | The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late. |
| `hint` | text | always | 1.0 | What the user can do (empty when all is well). |

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
| `clear` | `sdrkit_sband_ccsds_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `link_name` | `sdrkit_sband_ccsds_get_link_name` | `sdrkit_sband_ccsds_set_link_name` |
| `modulation` | `sdrkit_sband_ccsds_get_modulation` | `sdrkit_sband_ccsds_set_modulation` |
| `symbol_rate` | `sdrkit_sband_ccsds_get_symbol_rate` | `sdrkit_sband_ccsds_set_symbol_rate` |
| `roll_off` | `sdrkit_sband_ccsds_get_roll_off` | `sdrkit_sband_ccsds_set_roll_off` |
| `conv` | `sdrkit_sband_ccsds_get_conv` | `sdrkit_sband_ccsds_set_conv` |
| `frame_bits` | `sdrkit_sband_ccsds_get_frame_bits` | `sdrkit_sband_ccsds_set_frame_bits` |
| `sync_word` | `sdrkit_sband_ccsds_get_sync_word` | `sdrkit_sband_ccsds_set_sync_word` |
| `nrzm` | `sdrkit_sband_ccsds_get_nrzm` | `sdrkit_sband_ccsds_set_nrzm` |
| `randomiser` | `sdrkit_sband_ccsds_get_randomiser` | `sdrkit_sband_ccsds_set_randomiser` |
| `rs` | `sdrkit_sband_ccsds_get_rs` | `sdrkit_sband_ccsds_set_rs` |
| `rs_interleave` | `sdrkit_sband_ccsds_get_rs_interleave` | `sdrkit_sband_ccsds_set_rs_interleave` |
| `rs_basis` | `sdrkit_sband_ccsds_get_rs_basis` | `sdrkit_sband_ccsds_set_rs_basis` |

## Try it

```sh
sdrkit run sband_ccsds --source synth:sband_ccsds
sdrkit run sband_ccsds --source iiod:192.168.2.1 --freq 2215
```
<!-- generated: end -->

## Settings: notes

Every setting but `link_name` takes effect when the stack starts again. The defaults describe the plain CCSDS frame of 1279 bytes (the sync word and 5 × 255 bytes): a test default, not a satellite. The code word length is (frame bytes − 4) / interleave, at most 255; the virtual fill is 255 minus it. The file `data/sband_links.csv` of the crate holds 46 links as rows of these settings; no public document confirms their parameters.
