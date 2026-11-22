<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CubeSat telemetry

Amateur and small-satellite packet telemetry: AX.25 (AFSK, G3RUH), AX100 Mode 5, CCSDS, and Geoscan, Lucky-7, Spino, Erminaz, USP, NGHam, U482C, Mobitex and CCSDS concatenated frames. Sends AX.25 UI frames in AFSK 1200 and G3RUH 9600 for an amateur satellite uplink.

Stack id: `cubesat`. Group: Satellite.

Signal types: Packet (1200 bit/s AFSK).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 145.825 MHz |
| Tuning range | 130 MHz to 1 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 30 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `geoscan` | Geoscan | 145.825 MHz |
| `lucky7` | Lucky-7 | 145.825 MHz |
| `spino` | Spino | 145.825 MHz |
| `erminaz` | Erminaz | 145.825 MHz |
| `usp` | USP | 145.825 MHz |
| `ngham` | NGHam | 145.825 MHz |
| `u482c` | U482C | 145.825 MHz |
| `mobitex` | Mobitex | 145.825 MHz |
| `ccsds_concat` | CCSDS concatenated | 145.825 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cubesat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `search` | Search | choice | `table` | `table` (Table (the dial)), `smart` (Smart (blind search)) |  |
| `ccsds_length` | CCSDS code length | whole number (bytes) | 255 | 33 to 255 |  |
| `smart.max_channels` | Carriers followed | whole number | 4 | 1 to 16 |  |
| `smart.min_snr_db` | Carrier SNR | number (dB) | 8 | -10 to 60 |  |
| `extra_bitrates` | Extra bit rates | text (bit/s) | empty |  | Up to four bit rates of 300 to 20000 bit/s, separated by commas: one more FSK slicer each, for every framing. |
| `extra_ax25` | AX.25 on the extra slicers | choice | `off` | `off` (Off), `g3ruh` (G3RUH), `nrzi` (NRZI) |  |
| `afsk_carrier_hz` | U482C audio carrier | number (Hz) | 3600 | 0 to 8000 | 0 turns the audio FSK path off; else 500 to 8000 Hz. |
| `afsk_bitrate` | U482C audio bit rate | choice | `4800` | `1200` (1200 bit/s), `2400` (2400 bit/s), `4800` (4800 bit/s) |  |
| `geoscan_length` | Geoscan frame bytes | choice | `66` | `66` (66 bytes), `74` (74 bytes) |  |
| `ccsds_basis` | CCSDS RS basis | choice | `dual` | `dual` (Dual), `conventional` (Conventional) |  |
| `concat.frame_bytes` | CCSDS concatenated frame size | whole number (bytes) | 223 | 1 to 1784 | Data bytes, check bytes not included: a multiple of the interleave, at most 223 for each code word. |
| `concat.interleave` | Interleave | whole number | 1 | 1 to 8 |  |
| `concat.basis` | Basis | choice | `dual` | `dual` (Dual), `conventional` (Conventional) |  |
| `concat.code` | Code | choice | `ccsds` | `ccsds` (CCSDS), `nasa_dsn` (NASA-DSN), `ccsds_uninverted` (CCSDS uninverted), `nasa_dsn_uninverted` (NASA-DSN uninverted) |  |
| `concat.nrzm` | NRZ-M | yes or no | false |  |  |
| `concat.randomiser` | Randomiser | yes or no | true |  |  |
| `mobitex_variant` | Mobitex variant | choice | `mobitex_nx` | `mobitex_nx` (Mobitex / Mobitex-NX), `beesat1` (BEESAT-1), `beesat9` (BEESAT-9) |  |
| `satellites` | Satellite table | text | `ISS (ARISS packet)\|25544\|145.825\|AX.25 AFSK\|1200\|RS0ISS;NO-44 (PCSat)\|26931\|145.825\|AX.25 AFSK\|1200\|` |  | name\|norad\|MHz\|framing\|baud\|call;... |
| `tracked` | Satellite followed | text | empty |  | name\|norad of the satellite the pass planner follows (from the host). |
| `kiss` | KISS output | yes or no | true |  |  |
| `framing_filter` | Framing | choice | `all` | `all` (Every framing), `ax25_afsk` (AX.25 AFSK), `ax25_g3ruh` (AX.25 G3RUH), `ax25_nrzi` (AX.25 NRZI), `ax100_mode5` (AX100 Mode 5), `ax100_mode6` (AX100 Mode 6), `ccsds_rs` (CCSDS RS), `geoscan` (Geoscan), `lucky7` (Lucky-7), `spino` (Spino), `erminaz` (Erminaz), `usp` (USP), `ngham` (NGHam), `u482c` (U482C), `mobitex` (Mobitex), `ccsds_concat` (CCSDS concatenated) |  |
| `tx_rate` | Speed | choice | `1200` | `1200` (1200 bit/s AFSK), `9600` (9600 bit/s G3RUH) | 1200 bit/s Bell 202 AFSK or 9600 bit/s G3RUH FSK, both on FM with 3 kHz deviation. |
| `tx_delay` | Flags before | number (ms) | 300 | 10 to 1000 | The time of HDLC flags before the first frame, for the receiver to lock. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_framing` | Framing | choice | `0` | `0` (Every framing in turn), `1` (AX.25 AFSK 1200), `2` (AX.25 G3RUH 9600), `3` (AX100 Mode 5, 9600), `4` (AX100 Mode 5, 4800), `5` (AX100 Mode 6, 9600), `6` (CCSDS RS, 9600), `7` (Geoscan, 9600), `8` (Lucky-7, 4800), `9` (Spino, 2400), `10` (Erminaz, 9600), `11` (USP, 9600), `12` (NGHam, 4800), `13` (U482C, AFSK 4800), `14` (Mobitex-NX, 4800), `15` (CCSDS concatenated, 9600) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `rs_repaired` | Repaired by RS | whole number |
| `kiss_sent` | Sent to KISS | whole number |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Frames sent | whole number |
| `tx_waiting` | Frames waiting | whole number |
| `receiver.dropped` | Dropped | whole number |
| `naming.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `heard_at` (time), `framing` (choice), `baud` (whole number), `rs_corrected` (whole number), `satellite` (text), `norad` (whole number), `guess` (yes or no), `freq_hz` (frequency), `from` (text), `to` (text), `path` (text), `frame_type` (text), `pid` (whole number), `csp` (text), `data` (bytes), `payload` (bytes), `text` (text), `snr_db` (number (dB)), `validated` (text), `kind` (text), `inverted` (yes or no), `spacecraft` (whole number), `vc` (whole number), `flags` (whole number) |
| `frames_per_framing` | Frames by framing | `framing` (choice), `frames` (whole number), `sync_hits` (whole number), `decode_failed` (whole number) |
| `carriers` | Carriers | `offset_hz` (frequency), `snr_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `cubesat.frame` | CubeSat frame | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `framing` (choice), `norad` (whole number), `data` (bytes) |
| `cubesat.kiss` | KISS frame | `source` (text), `family` (choice), `kiss_frame` (bytes) |
| `cubesat.tx` | AX.25 transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a frame | `destination` (text, default `CQ`), `path` (text, default empty), `text` (text, default empty), `hex` (text, default empty), `pid` (whole number, default 240) |  | Builds a UI frame from the station callsign to the destination and queues it. A transmission starts when none is on air; frames sent during a transmission go in the next one. Transmit must be on and the station callsign set. |
| `halt` | Halt |  |  | Drops the frames waiting and ends the transmission after the current bit with a 5 ms fall. |

## Typed interface

The stack is in the family `satcom`, version 1.2. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `CubesatFields`. The result fields of the CubeSat telemetry stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `rs_repaired` | whole number | always | 1.0 | Frames the Reed-Solomon code repaired. |
| `kiss_sent` | whole number | always | 1.0 | Frames sent to the KISS output. |
| `note` | text | always | 1.0 | Which parts of the framings received are not confirmed by a public document. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Result fields

Struct `CubesatTxFields`. The result fields of the AX.25 sender of the CubeSat stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.1 | Whether a transmission is on air. |
| `tx_sent` | whole number | always | 1.1 | The frames of the transmission on air. |
| `tx_waiting` | whole number | always | 1.1 | The frames waiting for the next transmission. |

### Table `frames`

Struct `CubesatFrameRow`. A frame of the log. The row key is the sequence number of the frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `heard_at` | time | optional | 1.0 | The wall-clock time a frame was heard (live radios only). |
| `framing` | choice | always | 1.0 | The framing of a frame. |
| `baud` | whole number | always | 1.0 | The bit rate of a frame. |
| `rs_corrected` | whole number | optional | 1.0 | Reed-Solomon symbols corrected (none for a framing without Reed-Solomon). |
| `satellite` | text | optional | 1.0 | The satellite of a frame. |
| `norad` | whole number | always | 1.0 | Its NORAD number. |
| `guess` | yes or no | always | 1.0 | The name comes from the dial frequency. |
| `freq_hz` | frequency | always | 1.0 | The frequency of the frame. |
| `from` | text | optional | 1.0 | AX.25 source. |
| `to` | text | optional | 1.0 | AX.25 destination. |
| `path` | text | optional | 1.0 | AX.25 repeaters, separated by commas. |
| `frame_type` | text | optional | 1.0 | AX.25 frame type. |
| `pid` | whole number | optional | 1.0 | AX.25 PID. |
| `csp` | text | optional | 1.0 | The CSP header in words. |
| `data` | bytes | always | 1.0 | The frame's data. |
| `payload` | bytes | always | 1.0 | The payload. |
| `text` | text | optional | 1.0 | The text beacon. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `validated` | text | optional | 1.0 | What passed the checks of a frame. |
| `kind` | text | optional | 1.0 | The Lucky-7 kind of a frame. |
| `inverted` | yes or no | always | 1.0 | The sync word was found complemented. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft id of a CCSDS header. |
| `vc` | whole number | optional | 1.0 | The virtual channel of a CCSDS header. |
| `flags` | whole number | optional | 1.0 | The NGHam flags. |

### Table `frames_per_framing`

Struct `CubesatFramingRow`. The frames of one framing. The row key is the key of the framing.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `framing` | choice | always | 1.0 | The framing of a frame. |
| `frames` | whole number | always | 1.0 | Frames by framing. |
| `sync_hits` | whole number | optional | 1.0 | Sync word matches of a framing. |
| `decode_failed` | whole number | optional | 1.0 | Candidates of a framing that failed their checks. |

### Table `carriers`

Struct `CubesatCarrierRow`. A carrier found now. The row key is its place ("000").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | always | 1.0 | A carrier's offset from the dial. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |

### Event `cubesat.frame`

Struct `CubesatFrameEvent`. A frame. The source is the satellite, the text the text beacon or nothing.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The satellite. |
| `text` | text | optional | 1.0 | The text beacon. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `framing` | choice | always | 1.0 | The framing of a frame. |
| `norad` | whole number | always | 1.0 | Its NORAD number. |
| `data` | bytes | always | 1.0 | The frame's data. |

### Event `cubesat.kiss`

Struct `CubesatKissEvent`. A frame in KISS form for a packet program. The source is the satellite.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The satellite. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `kiss_frame` | bytes | always | 1.0 | The KISS frame. |

### Event `cubesat.tx`

Struct `CubesatTxEvent`. The fields of a change of an AX.25 transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.1 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.1 | Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused. |
| `target` | text | always | 1.1 | The destination and the digipeaters of the frames, separated by commas. |
| `text` | text | always | 1.1 | The text or the hexadecimal bytes of the frame for started, queued and refused; empty for finished. |
| `family` | choice | always | 1.1 | The family of the source: a satellite. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_cubesat_clear` | none | none |
| `send` | `sdrkit_cubesat_send` | `sdrkit_cubesat_send_args` | none |
| `halt` | `sdrkit_cubesat_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `search` | `sdrkit_cubesat_get_search` | `sdrkit_cubesat_set_search` |
| `ccsds_length` | `sdrkit_cubesat_get_ccsds_length` | `sdrkit_cubesat_set_ccsds_length` |
| `smart.max_channels` | `sdrkit_cubesat_get_smart_max_channels` | `sdrkit_cubesat_set_smart_max_channels` |
| `smart.min_snr_db` | `sdrkit_cubesat_get_smart_min_snr_db` | `sdrkit_cubesat_set_smart_min_snr_db` |
| `extra_bitrates` | `sdrkit_cubesat_get_extra_bitrates` | `sdrkit_cubesat_set_extra_bitrates` |
| `extra_ax25` | `sdrkit_cubesat_get_extra_ax25` | `sdrkit_cubesat_set_extra_ax25` |
| `afsk_carrier_hz` | `sdrkit_cubesat_get_afsk_carrier_hz` | `sdrkit_cubesat_set_afsk_carrier_hz` |
| `afsk_bitrate` | `sdrkit_cubesat_get_afsk_bitrate` | `sdrkit_cubesat_set_afsk_bitrate` |
| `geoscan_length` | `sdrkit_cubesat_get_geoscan_length` | `sdrkit_cubesat_set_geoscan_length` |
| `ccsds_basis` | `sdrkit_cubesat_get_ccsds_basis` | `sdrkit_cubesat_set_ccsds_basis` |
| `concat.frame_bytes` | `sdrkit_cubesat_get_concat_frame_bytes` | `sdrkit_cubesat_set_concat_frame_bytes` |
| `concat.interleave` | `sdrkit_cubesat_get_concat_interleave` | `sdrkit_cubesat_set_concat_interleave` |
| `concat.basis` | `sdrkit_cubesat_get_concat_basis` | `sdrkit_cubesat_set_concat_basis` |
| `concat.code` | `sdrkit_cubesat_get_concat_code` | `sdrkit_cubesat_set_concat_code` |
| `concat.nrzm` | `sdrkit_cubesat_get_concat_nrzm` | `sdrkit_cubesat_set_concat_nrzm` |
| `concat.randomiser` | `sdrkit_cubesat_get_concat_randomiser` | `sdrkit_cubesat_set_concat_randomiser` |
| `mobitex_variant` | `sdrkit_cubesat_get_mobitex_variant` | `sdrkit_cubesat_set_mobitex_variant` |
| `satellites` | `sdrkit_cubesat_get_satellites` | `sdrkit_cubesat_set_satellites` |
| `tracked` | `sdrkit_cubesat_get_tracked` | `sdrkit_cubesat_set_tracked` |
| `kiss` | `sdrkit_cubesat_get_kiss` | `sdrkit_cubesat_set_kiss` |
| `framing_filter` | `sdrkit_cubesat_get_framing_filter` | `sdrkit_cubesat_set_framing_filter` |
| `tx_rate` | `sdrkit_cubesat_get_tx_rate` | `sdrkit_cubesat_set_tx_rate` |
| `tx_delay` | `sdrkit_cubesat_get_tx_delay` | `sdrkit_cubesat_set_tx_delay` |
| `tx_level` | `sdrkit_cubesat_get_tx_level` | `sdrkit_cubesat_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_framing` | `sdrkit_cubesat_gen_get_framing` | `sdrkit_cubesat_gen_set_framing` |

## Try it

```sh
sdrkit run cubesat --source synth:cubesat
sdrkit run cubesat --source synth:cubesat --submode geoscan
sdrkit run cubesat --source iiod:192.168.2.1 --freq 145.825
```
<!-- generated: end -->

## Submodes: notes

The default submode runs every framing at once (Spino only with 2400 in `extra_bitrates`; an Erminaz frame is reported as CCSDS RS). A named submode runs its framing alone, takes 130 ksps or more, and adds the bit rate of its framing (2400 bit/s for Spino). The default frequency of every submode is the dial of the test signal, not a satellite downlink: the host gives the frequency. The data files `cubesat_transmitters.csv`, `cubesat_usp.csv` and `cubesat_dials.csv` of the stack's crate hold the transmitters of these framings, for a host to copy into the settings.

The field `note` says which parts of the framings in use are not confirmed by a public document.

## Settings: notes

Advanced settings. The owner works without any of them set.

## Try it: notes

```sh
sdrkit run cubesat --source synth:cubesat
sdrkit run cubesat --source synth:cubesat/geoscan --submode geoscan
sdrkit run cubesat --source iiod:192.168.2.1 --freq 145.825
```

## Transmit

The stack sends AX.25 UI frames (AX.25 2.2) for an uplink to an amateur satellite, for example a satellite digipeater, in
1200 bit/s Bell 202 AFSK or 9600 bit/s G3RUH FSK on FM. The engine setting `transmit_allowed` is the only switch: with it off,
`send` is refused and a transmission on air ends at once. The source address is the station callsign: without one, or with
one that is not an AX.25 address (1 to 6 letters or digits and an optional SSID 0 to 15, for example `VK2ABC-7`), `send` is
refused.

You need an amateur licence to transmit. An uplink to a satellite must respect the satellite's published band plan: send
only on its uplink frequency and in the modes and at the times its operators allow (the amateur-satellite bands are
145.8 to 146.0 MHz and 435 to 438 MHz). Without a licence, use the sender only into a dummy load, on a cable or in a shielded
enclosure.

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Builds a UI frame from the station callsign to the destination and queues it. A transmission starts when none is on air; frames sent during a transmission go together in the next one. At most 16 frames wait. |
| `halt` | Drops the frames waiting and ends the transmission after the bit on air with a 5 ms fall and the end mark. |

The arguments of `send`: `destination` (an AX.25 address, default `CQ`; empty sends to `CQ`), `path` (up to 8 digipeaters
separated by commas, default empty), `text` (the information field as its UTF-8 bytes), `hex` (the information field as pairs
of hexadecimal digits, spaces allowed) and `pid` (0 to 254, default 240, `F0`: no layer 3). Give either `text` or `hex`; the
information field holds 1 to 256 octets. Addresses are sent in upper case. `send` is refused for an address that is not an
AX.25 address, a digipeater marked as repeated (`*`), more than 8 digipeaters, both or neither of `text` and `hex`, an odd
number of hexadecimal digits, more than 256 octets and the PID 255.

The settings, in the section Transmit: `tx_rate` (1200 bit/s AFSK or 9600 bit/s G3RUH, default 1200), `tx_delay` (the HDLC
flags before the first frame, 10 to 1000 ms, default 300) and `tx_level` (the magnitude, 0.05 to 0.9 of full scale, default
0.5). A change applies to the next transmission.

### Results and events

`tx_sending` (a transmission is on air), `tx_sent` (the frames of the transmission on air) and `tx_waiting`. The event
`cubesat.tx` says when a frame started a transmission, was queued, was refused (with the reason), and when a transmission
finished (with the reason: halted, the transmitter is off, the format changed, the input ended); `target` holds the
destination and the digipeaters, `text` the text or the hexadecimal bytes.

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at the session rate. On a live
radio whose host gives a transmit lead, the first sample states its time on air. The last sample carries the end mark. The
carrier is at the tuned frequency; it rises and falls with a 5 ms raised cosine, and between them the envelope is constant at
`tx_level`. The host corrects the uplink for Doppler: the stack sends at the frequency it is tuned to.

### The signal

- The frame: the destination with the `C` bit set, the source with it clear, the digipeaters with the `H` bit clear, the
  extension bit on the last address; control `03` (UI), the PID and the information. Each address is 6 characters shifted
  left by one bit, padded with spaces, then the SSID octet `CRRSSID0` (reserved bits `11`).
- HDLC: flags `7E` for `tx_delay`, the frame and its FCS (CRC-16/X.25, low octet first) with a zero after every five ones,
  one flag between frames, then closing flags.
- 1200 bit/s: NRZI (a 0 changes the tone), Bell 202 AFSK with 1200 Hz mark and 2200 Hz space, phase continuous, on FM with
  3 kHz deviation.
- 9600 bit/s: NRZI, then the G3RUH scrambler x^17 + x^12 + 1, as levels with a raised-cosine pulse (roll-off 0.5) on FM with
  3 kHz deviation.

Example: from `N0CALL-7` to `CQ` with the text `A`: `86 A2 40 40 40 40 E0 9C 60 86 82 98 98 6F 03 F0 41`.

### Checks

- The stack's own receive side reads each frame with the same bytes at 12 dB in 25 kHz: at 1200 bit/s with a digipeater and
  text, at 9600 bit/s with bytes that hold flag and escape values and a PID other than `F0`, and three frames of which two were
  queued during a transmission.
- The address octets of the example and of a digipeater, and the bits on the line checked by hand: the FCS `906E` of
  `123456789` sent as `6E 90`, and the zero after five ones in the frames `FF` and `7E` and their FCS.
- The level is constant at `tx_level` and no sample passes 0.9 of full scale. The 99 % power bandwidth is 9.3 kHz at
  1200 bit/s and 14.2 kHz at 9600 bit/s, inside the 25 kHz channel of the receiver.
- Also covered: the refusals (the switch off, the station callsign missing or not an AX.25 address, the arguments), `halt` and
  the end mark.
