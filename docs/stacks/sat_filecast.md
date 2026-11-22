<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Satellite file broadcasts

HimawariCast and GEONETCAST Americas on DVB-S2 (C band, through the LNB): files and HimawariCast full-disc pictures.

Stack id: `sat_filecast`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 950 MHz to 2.15 GHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 4.45 MHz |
| Signal bandwidth | 3.2 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `himawaricast` | HimawariCast | 0 Hz |
| `geonetcast` | GEONETCAST Americas | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sat_filecast` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 60 MHz |  |
| `roll_off` | Roll-off | choice | `auto` | `auto` (Auto), `0.35`, `0.25`, `0.20`, `0.15`, `0.10`, `0.05` |  |
| `carrier_offset` | Carrier | text (Hz) | empty |  | Offset in Hz from the centre. Empty: the carrier nearest the centre. |
| `pid` | PID of the MPE stream | whole number | 256 | 32 to 8190 | The transport stream PID that carries the datagram sections (256 in the test signal). |
| `part_bytes` | Part size | whole number (bytes) | 1408 | 1 to 8192 | Bytes of each numbered part of a file (1408 in the test signal). Not confirmed by a public document. |
| `max_silence` | Give up on a file after | number (s) | 120 | 10 to 3600 |  |
| `isi` | Input stream | whole number | -1 | -1 to 255 | -1: the first stream seen. |
| `pl_scrambling` | PL scrambling code | whole number | 0 | 0 to 262141 |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `name_tag` | Name tag of GEONETCAST | whole number (characters) | 45 | 0 to 200 | GEONETCAST names end with a tag of this many characters, which the stack removes. |
| `modcod_rule` | MODCODs taken | choice | `any` | `any` (Any), `fixed` (Fixed), `set` (Set) | Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded. |
| `modcod` | MODCOD | whole number | 0 | 0 to 255 |  |
| `modcods` | MODCOD set | text | empty |  | MODCOD numbers with commas between them, for the rule "set". |
| `frame_length` | Frame length | choice | `any` | `any` (Any), `normal` (Normal), `short` (Short) | Frames of the other length are counted and skipped. |
| `pilots` | Pilots | choice | `any` | `any` (Any), `on` (On), `off` (Off) | A check only: a frame with other pilots is counted and decoded. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 60 MHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `standard` | Standard | choice |
| `signal_note` | Note | choice |
| `symbol_rate_hz` | Symbol rate | frequency |
| `spectrum_rate` | Symbol rate from the spectrum | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `manual_rate` | Manual symbol rate | yes or no |
| `spectrum_snr_db` | Carrier over floor | number (dB) |
| `misfit` | Fit misfit | number |
| `edge_low` | Lower carrier edge | frequency |
| `edge_high` | Upper carrier edge | frequency |
| `offset_hz` | Offset | frequency |
| `inverted` | Spectrum inverted | yes or no |
| `roll_off` | Roll-off | number |
| `roll_off_source` | Roll-off from | choice |
| `modulation` | Modulation | choice |
| `code_rate` | Code rate | choice |
| `modcod` | MODCOD | whole number |
| `frame` | FEC frame | choice |
| `pilots` | Pilots | yes or no |
| `vcm` | VCM or ACM | yes or no |
| `single_stream` | Single stream | yes or no |
| `pl_scrambling_code` | PL scrambling code | whole number |
| `plframes` | PLFRAMEs | whole number |
| `dummy_frames` | Dummy frames | whole number |
| `unsupported_frames` | Frames not decoded | whole number |
| `other_modcod_frames` | Frames of another MODCOD | whole number |
| `pilot_mismatch` | Frames with other pilots | whole number |
| `bbheader_bad` | BBHEADER failures | whole number |
| `net_bit_rate` | Net bit rate | number (bit/s) |
| `esn0_db` | Es/N0 | number (dB) |
| `mer_db` | MER | number (dB) |
| `ebn0_db` | Eb/N0 | number (dB) |
| `margin_to_qef_db` | Margin to QEF | number (dB) |
| `ldpc_iterations` | LDPC iterations | number |
| `bch_ok` | BCH good | whole number |
| `bch_bad` | BCH failed | whole number |
| `rs_clean` | RS words clean | whole number |
| `rs_corrected` | RS words corrected | whole number |
| `rs_failed` | RS words failed | whole number |
| `carrier_lock` | Carrier lock | yes or no |
| `timing_lock` | Timing lock | yes or no |
| `frame_lock` | Frame lock | yes or no |
| `fec_lock` | FEC lock | yes or no |
| `ts_lock` | TS lock | yes or no |
| `level_dbfs` | Level | number (dBFS) |
| `lnb_mismatch` | LNB on the other polarisation | yes or no |
| `pid` | PID | whole number |
| `pid_packets` | Packets on the PID | whole number |
| `datagrams` | Datagrams | whole number |
| `not_udp` | Not UDP | whole number |
| `broadcast_packets` | File-broadcast packets | whole number |
| `bad_packets` | Bad packets | whole number |
| `files_complete` | Files complete | whole number |
| `files_incomplete` | Files incomplete | whole number |
| `files_failed` | Files failed | whole number |
| `files_dropped` | Files dropped | whole number |
| `bytes` | Bytes | whole number |
| `open_files` | Open files | whole number |
| `last_error` | Last error | text |
| `pictures_finished` | Pictures finished | whole number |
| `hint` | Hint | text |
| `note` | Note | text |
| `s2_bbframe.frames_ok` | Frames good | whole number |
| `s2_bbframe.frames_bad` | Frames bad | whole number |
| `s2_bbframe.ts_packets` | TS packets | whole number |
| `s2_bbframe.crc_errors` | CRC errors | whole number |
| `s2_bbframe.pdus` | PDUs | whole number |
| `s2_bbframe.non_ip` | Non-IP PDUs | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carriers | `offset` (frequency), `symbol_rate` (frequency), `roll_off` (number), `snr_db` (number (dB)), `level_db` (number (dB)), `decoded` (yes or no) |
| `streams` | Input streams | `isi` (whole number), `carries` (choice), `packet_bytes` (whole number (bytes)), `sync_byte` (whole number), `ccm` (yes or no), `frames` (whole number), `bits` (whole number) |
| `stream_modcods` | MODCODs of the input streams | `stream` (text), `modcod` (whole number), `modulation` (choice), `code_rate` (choice), `frames` (whole number), `share_pct` (number (%)) |
| `recent_files` | Recent files | `name` (text), `size` (whole number (bytes)), `parts` (whole number), `received` (whole number), `complete` (yes or no) |
| `pictures` | Pictures | `channel` (text), `stamp` (text), `width` (whole number (px)), `height` (whole number (px)), `segments` (whole number), `total` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `spectrum` | Spectrum | bin | dB |
| `mer_history` | MER history | frame | dB |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the open files and the counters; keeps the pictures. |

## Typed interface

The stack is in the family `satimg`, version 1.1. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `Dvbs2Fields`. The result fields of the DVB-S/S2 receiver: the carrier search, the frames, the code and the locks. A measurement of the carrier search has no value without a carrier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `standard` | choice | optional | 1.0 | The standard received. |
| `signal_note` | choice | optional | 1.0 | A note on the signal for the user. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate in use. No value before a rate is set or measured. |
| `spectrum_rate` | frequency | optional | 1.0 | The symbol rate from the spectrum (width or cyclic line). |
| `clock_ppm` | number (ppm) | always | 1.0 | The error of the symbol clock. |
| `manual_rate` | yes or no | always | 1.0 | The symbol rate was set by hand. |
| `spectrum_snr_db` | number (dB) | optional | 1.0 | The plateau of the carrier over the noise floor. |
| `misfit` | number | optional | 1.0 | The misfit of the raised cosine fit. |
| `edge_low` | frequency | optional | 1.0 | The lower -3 dB edge of the carrier, from the centre. |
| `edge_high` | frequency | optional | 1.0 | The upper -3 dB edge of the carrier, from the centre. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset from the centre. No value without a carrier. |
| `inverted` | yes or no | always | 1.0 | The spectrum is inverted. |
| `roll_off` | number | always | 1.0 | The roll-off. |
| `roll_off_source` | choice | always | 1.0 | Where the roll-off came from. |
| `modulation` | choice | optional | 1.0 | The modulation. |
| `code_rate` | choice | optional | 1.0 | The code rate. |
| `modcod` | whole number | optional | 1.0 | The MODCOD (DVB-S2). |
| `frame` | choice | optional | 1.0 | The FEC frame size (DVB-S2). |
| `pilots` | yes or no | always | 1.0 | Pilots are on. |
| `vcm` | yes or no | always | 1.0 | The MODCOD changes between frames. |
| `single_stream` | yes or no | always | 1.0 | The carrier has one input stream. |
| `pl_scrambling_code` | whole number | always | 1.0 | The PL scrambling code in use. |
| `plframes` | whole number | always | 1.0 | PLFRAMEs seen. |
| `dummy_frames` | whole number | always | 1.0 | Dummy PLFRAMEs seen. |
| `unsupported_frames` | whole number | always | 1.0 | Frames followed but not decoded. |
| `other_modcod_frames` | whole number | always | 1.0 | Frames skipped by the MODCOD rule or the frame length rule. |
| `pilot_mismatch` | whole number | always | 1.0 | Frames whose pilots differ from the setting. |
| `bbheader_bad` | whole number | always | 1.0 | BBHEADERs refused (CRC-8 or length). |
| `net_bit_rate` | number (bit/s) | always | 1.0 | The net bit rate of the data fields. |
| `esn0_db` | number (dB) | optional | 1.0 | Es/N0. |
| `mer_db` | number (dB) | optional | 1.0 | The modulation error ratio. |
| `ebn0_db` | number (dB) | optional | 1.0 | Eb/N0. |
| `margin_to_qef_db` | number (dB) | optional | 1.0 | Es/N0 above the quasi-error-free level of EN 302 307-1 table 13. |
| `ldpc_iterations` | number | always | 1.0 | LDPC iterations, recent mean. |
| `bch_ok` | whole number | always | 1.0 | BCH words good. |
| `bch_bad` | whole number | always | 1.0 | BCH words that failed. |
| `rs_clean` | whole number | always | 1.0 | Reed-Solomon words with no error (DVB-S). |
| `rs_corrected` | whole number | always | 1.0 | Reed-Solomon words corrected (DVB-S). |
| `rs_failed` | whole number | always | 1.0 | Reed-Solomon words that failed (DVB-S). |
| `carrier_lock` | yes or no | always | 1.0 | A carrier is found. |
| `timing_lock` | yes or no | always | 1.0 | The symbol timing runs. |
| `frame_lock` | yes or no | always | 1.0 | The carrier phase and the framing hold. |
| `fec_lock` | yes or no | always | 1.0 | The FEC decodes words. |
| `ts_lock` | yes or no | always | 1.0 | The transport stream is valid. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `lnb_mismatch` | yes or no | always | 1.0 | The LNB supply selects the other polarisation than the transponder's. |

### Result fields

Struct `FilecastFields`. The result fields of `sat_filecast` beside those of the DVB-S2 receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pid` | whole number | always | 1.0 | The PID read. |
| `pid_packets` | whole number | always | 1.0 | Transport packets on the PID. |
| `datagrams` | whole number | always | 1.0 | IP datagrams. |
| `not_udp` | whole number | always | 1.0 | Datagrams that are not UDP. |
| `broadcast_packets` | whole number | always | 1.0 | File-broadcast packets. |
| `bad_packets` | whole number | always | 1.0 | File-broadcast packets refused. |
| `files_complete` | whole number | always | 1.0 | Complete files. |
| `files_incomplete` | whole number | always | 1.0 | Incomplete files. |
| `files_failed` | whole number | always | 1.0 | Files whose bzip2 data failed. |
| `files_dropped` | whole number | always | 1.0 | Files dropped (the queue was full). |
| `bytes` | whole number | always | 1.0 | Bytes of files. |
| `open_files` | whole number | always | 1.0 | Files open. |
| `last_error` | text | always | 1.0 | The last error. |
| `pictures_finished` | whole number | always | 1.0 | Pictures finished. |
| `hint` | text | always | 1.0 | What the user can do. |
| `note` | text | always | 1.0 | A note on the state of the stack. |

### Table `carriers`

Struct `Dvbs2CarrierRow`. A row of the carriers of the survey. The row key is the offset in kHz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset` | frequency | always | 1.0 | The offset of the carrier from the centre. |
| `symbol_rate` | frequency | always | 1.0 | Its symbol rate. |
| `roll_off` | number | always | 1.0 | Its roll-off. |
| `snr_db` | number (dB) | always | 1.0 | Its plateau over the floor. |
| `level_db` | number (dB) | always | 1.0 | Its level against the strongest. |
| `decoded` | yes or no | always | 1.0 | It is the carrier decoded. |

### Table `streams`

Struct `Dvbs2StreamRow`. A row of the input streams. The row key is the ISI.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `isi` | whole number | always | 1.0 | The input stream identifier. |
| `carries` | choice | always | 1.0 | What the stream carries. |
| `packet_bytes` | whole number (bytes) | optional | 1.0 | The user packet length. No value for a stream without user packets. |
| `sync_byte` | whole number | optional | 1.0 | The sync byte of the user packets. |
| `ccm` | yes or no | always | 1.0 | Constant coding and modulation. |
| `frames` | whole number | always | 1.0 | Frames of the stream. |
| `bits` | whole number | always | 1.0 | Data field bits of the stream. |

### Table `stream_modcods`

Struct `Dvbs2StreamModcodRow`. A row of the MODCODs of the input streams. The row key is `<isi>/<modcod>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `stream` | text | always | 1.0 | The input stream, a row key of the stream table. |
| `modcod` | whole number | always | 1.0 | The MODCOD. |
| `modulation` | choice | always | 1.0 | The modulation of the MODCOD. |
| `code_rate` | choice | optional | 1.0 | The code rate of the MODCOD. |
| `frames` | whole number | always | 1.0 | The frames of the stream with this MODCOD. |
| `share_pct` | number (%) | always | 1.0 | The share of the stream's frames with this MODCOD. |

### Table `recent_files`

Struct `RecentFileRow`. A file of `sat_filecast`. The row key is the number of the file.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the file. |
| `size` | whole number (bytes) | always | 1.0 | The size of the file. |
| `parts` | whole number | always | 1.0 | The parts of the file. |
| `received` | whole number | always | 1.0 | The parts received. |
| `complete` | yes or no | always | 1.0 | Every part arrived. |

### Table `pictures`

Struct `FilecastPictureRow`. A picture of `sat_filecast`. The row key is the channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | text | always | 1.0 | The channel. |
| `stamp` | text | always | 1.0 | The time stamp of the picture. |
| `width` | whole number (px) | always | 1.0 | The width of the picture. |
| `height` | whole number (px) | always | 1.0 | The height of the picture. |
| `segments` | whole number | always | 1.0 | The segments received. |
| `total` | whole number | always | 1.0 | The segments in all. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sat_filecast_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_sat_filecast_get_symbol_rate` | `sdrkit_sat_filecast_set_symbol_rate` |
| `roll_off` | `sdrkit_sat_filecast_get_roll_off` | `sdrkit_sat_filecast_set_roll_off` |
| `carrier_offset` | `sdrkit_sat_filecast_get_carrier_offset` | `sdrkit_sat_filecast_set_carrier_offset` |
| `pid` | `sdrkit_sat_filecast_get_pid` | `sdrkit_sat_filecast_set_pid` |
| `part_bytes` | `sdrkit_sat_filecast_get_part_bytes` | `sdrkit_sat_filecast_set_part_bytes` |
| `max_silence` | `sdrkit_sat_filecast_get_max_silence` | `sdrkit_sat_filecast_set_max_silence` |
| `name_tag` | `sdrkit_sat_filecast_get_name_tag` | `sdrkit_sat_filecast_set_name_tag` |
| `isi` | `sdrkit_sat_filecast_get_isi` | `sdrkit_sat_filecast_set_isi` |
| `modcod_rule` | `sdrkit_sat_filecast_get_modcod_rule` | `sdrkit_sat_filecast_set_modcod_rule` |
| `modcod` | `sdrkit_sat_filecast_get_modcod` | `sdrkit_sat_filecast_set_modcod` |
| `modcods` | `sdrkit_sat_filecast_get_modcods` | `sdrkit_sat_filecast_set_modcods` |
| `frame_length` | `sdrkit_sat_filecast_get_frame_length` | `sdrkit_sat_filecast_set_frame_length` |
| `pilots` | `sdrkit_sat_filecast_get_pilots` | `sdrkit_sat_filecast_set_pilots` |
| `pl_scrambling` | `sdrkit_sat_filecast_get_pl_scrambling` | `sdrkit_sat_filecast_set_pl_scrambling` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_symbol_rate` | `sdrkit_sat_filecast_gen_get_symbol_rate` | `sdrkit_sat_filecast_gen_set_symbol_rate` |

## Try it

```sh
sdrkit run sat_filecast --source synth:sat_filecast
sdrkit run sat_filecast --source synth:sat_filecast --submode himawaricast
sdrkit run sat_filecast --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Description

The stack reads the transport stream of a DVB-S2 carrier: the datagram sections (MPE) of one PID, the IPv4 UDP datagrams in
them, and a carousel file broadcast that cuts each file into numbered parts. It never writes a file: each file leaves as a
`satimg.file` item with its cleaned name and its kind (complete, incomplete, HRIT segment, additional data, bad bzip2), and
the host stores it. HimawariCast files are bzip2 data; the stack decompresses them on a helper thread and joins the HRIT
segments of each channel into a full-disc picture (8-bit grey, from the 10-bit counts). GEONETCAST names lose the 45
characters of their tag.

The DVB-S2 receiver is the one of `dvbs`. The stack does not fix the MODCOD, the frame length or the pilots: every frame is
decoded with its own. Below the least sample rate of the symbol rate (rate x 1.3 x 1.35 / 1.02) the stack shows a hint and
the receiver does not start. The service values (symbol rate, PID, part size) are data: the file
`data/plans/dvbs2_products.csv` lists them, and a host copies a row into the settings.

Standard: ETSI EN 302 307-1 (DVB-S2); ETSI EN 301 192 clause 7 (MPE); RFC 791, RFC 768; the bzip2 file format; CGMS LRIT/HRIT
global specification (headers).

## Not confirmed by a public document

No public document was read for the file broadcast. These values are settings or rules of the stack and are not confirmed
by a public document: the packet layout (type, length, file id, part number, the name and the size in the head), the part
sizes, the PIDs, the symbol rates of the services, and the 45-character tag at the end of GEONETCAST names. The MPE layout is
written from knowledge of EN 301 192. IPv6 datagrams are not read. The stack was not run on a real carrier.

## Try it: notes

```sh
sdrkit run sat_filecast --source synth:sat_filecast --submode himawaricast
sdrkit run sat_filecast --source synth:sat_filecast --submode geonetcast
sdrkit run sat_filecast --source iiod:192.168.2.1 --freq 1002 symbol_rate=2586148 pid=1001 part_bytes=1411
```

The test signal is DVB-S2 (roll-off 0.20, normal frames, pilots, MODCOD 6 for HimawariCast and 15 for GEONETCAST) at the
setting `gen_symbol_rate` (else `symbol_rate`, else 2 586 148 sym/s), carrying a carousel on the PID: ten bzip2 HRIT segments
of a 640 x 640 disc and a notice a round (HimawariCast), or eleven data files with the tag (GEONETCAST).
