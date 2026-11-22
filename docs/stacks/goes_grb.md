<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GOES-R Rebroadcast

GOES-R Rebroadcast on L band (DVB-S2, 8.67 Msym/s): ABI and SUVI pictures, GLM lightning flashes, and files.

Stack id: `goes_grb`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.6866 GHz |
| Tuning range | 950 MHz to 2.2 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 14.92 MHz |
| Signal bandwidth | 13.018907 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:goes_grb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 8.665938 MHz | 0 Hz to 60 MHz |  |
| `roll_off` | Roll-off | choice | `0.25` | `auto` (Auto), `0.35`, `0.25`, `0.20`, `0.15`, `0.10`, `0.05` |  |
| `carrier_offset` | Carrier | text (Hz) | empty |  | Offset in Hz from the centre. Empty: the carrier nearest the centre. |
| `product` | Product shown | whole number | -1 | -1 to 2047 | An APID, or -1 for the picture updated last. |
| `pl_scrambling` | PL scrambling code | whole number | 0 | 0 to 262141 |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Virtual channels | text | `5,6` |  | Virtual channel numbers with commas between them. Empty: every channel but 63. |
| `modcod_rule` | MODCODs taken | choice | `any` | `any` (Any), `fixed` (Fixed), `set` (Set) | Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded. |
| `modcod` | MODCOD | whole number | 0 | 0 to 255 |  |
| `modcods` | MODCOD set | text | empty |  | MODCOD numbers with commas between them, for the rule "set". |
| `frame_length` | Frame length | choice | `any` | `any` (Any), `normal` (Normal), `short` (Short) | Frames of the other length are counted and skipped. |
| `pilots` | Pilots | choice | `any` | `any` (Any), `on` (On), `off` (Off) | A check only: a frame with other pilots is counted and decoded. |

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
| `cadus` | CADUs | whole number |
| `spacecraft` | Spacecraft | whole number |
| `vcid` | Virtual channel | whole number |
| `frames_per_vc` | Frames per virtual channel | text |
| `space_packets` | Packets | whole number |
| `packets_lost` | Packets lost | whole number |
| `field_breaks` | Breaks in the stream | whole number |
| `packets_crc_bad` | Packets with a bad CRC | whole number |
| `payloads` | Payloads | whole number |
| `picture_blocks` | Picture blocks | whole number |
| `j2k_failures` | JPEG 2000 failures | whole number |
| `files` | Files | whole number |
| `picture_lines` | Lines of the picture | whole number |
| `shown_product` | Product shown | text |
| `hint` | Hint | text |
| `note` | Note | text |
| `glm_payloads` | GLM payloads | whole number |
| `glm_dropped` | GLM payloads dropped | whole number |
| `glm_packets_lost` | GLM packets lost | whole number |
| `glm_not_decoded` | GLM payloads not decoded | whole number |
| `glm_bad_length` | GLM units of a wrong length | whole number |
| `glm_group_bad_length` | GLM group units of a wrong length | whole number |
| `flash_count` | Flashes | whole number |
| `group_count` | Groups | whole number |
| `event_count` | Events | whole number |
| `glm_rejected` | GLM records rejected | whole number |
| `newest_flash` | Newest flash | time |
| `flash_rate` | Flash rate | number (1/min) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carriers | `offset` (frequency), `symbol_rate` (frequency), `roll_off` (number), `snr_db` (number (dB)), `level_db` (number (dB)), `decoded` (yes or no) |
| `streams` | Input streams | `isi` (whole number), `carries` (choice), `packet_bytes` (whole number (bytes)), `sync_byte` (whole number), `ccm` (yes or no), `frames` (whole number), `bits` (whole number) |
| `stream_modcods` | MODCODs of the input streams | `stream` (text), `modcod` (whole number), `modulation` (choice), `code_rate` (choice), `frames` (whole number), `share_pct` (number (%)) |
| `grb_products` | Products | `apid` (whole number), `name` (text), `updated` (duration (s)) |
| `packets_by_apid` | Packets by APID | `apid` (whole number), `name` (text), `packets` (whole number) |
| `frames` | Frames | `vcid` (whole number), `frames` (whole number) |
| `flashes` | Lightning flashes | `flash` (whole number), `time` (time), `duration` (duration (s)), `position` (position (°)), `area_km2` (number (km²)), `energy_fj` (number (fJ)), `quality` (choice), `groups` (whole number), `events` (whole number) |
| `groups` | Lightning groups | `group` (whole number), `time` (time), `position` (position (°)), `area_km2` (number (km²)), `energy_fj` (number (fJ)), `flash` (whole number), `quality` (choice), `events` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `spectrum` | Spectrum | bin | dB |
| `mer_history` | MER history | frame | dB |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the pictures, the payloads and the counters. |

## Typed interface

The stack is in the family `satimg`, version 1.2. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite imagery (`satellite_imagery`). Media: pictures.

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

Struct `CaduFields`. The counts of the transfer frames in the data fields of a DVB-S2 carrier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `cadus` | whole number | always | 1.0 | CADUs found. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft id of the frames. No value before a frame is read. |
| `vcid` | whole number | optional | 1.0 | The virtual channel of the last frame. |
| `frames_per_vc` | text | always | 1.0 | The frames of each virtual channel, for people. |
| `space_packets` | whole number | always | 1.0 | Space packets. |
| `packets_lost` | whole number | always | 1.0 | Space packets lost. |
| `field_breaks` | whole number | always | 1.0 | Breaks in the stream of data fields. |

### Result fields

Struct `GrbFields`. The result fields of `goes_grb` beside those of the DVB-S2 receiver and the CADUs.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `packets_crc_bad` | whole number | always | 1.0 | Packets with a bad CRC. |
| `payloads` | whole number | always | 1.0 | Payloads. |
| `picture_blocks` | whole number | always | 1.0 | Picture blocks. |
| `j2k_failures` | whole number | always | 1.0 | JPEG 2000 failures. |
| `files` | whole number | always | 1.0 | Files. |
| `picture_lines` | whole number | always | 1.0 | Rows of the picture shown. |
| `shown_product` | text | always | 1.0 | The product shown. |
| `hint` | text | always | 1.0 | What the user can do. |
| `note` | text | always | 1.0 | A note on the state of the stack. |

### Result fields

Struct `GlmFields`. The GLM lightning results of `goes_grb`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `glm_payloads` | whole number | always | 1.1 | Complete GLM payloads (APIDs 300 to 303). |
| `glm_dropped` | whole number | always | 1.1 | GLM payloads dropped because a packet of them was lost or damaged. |
| `glm_packets_lost` | whole number | always | 1.1 | Packets missing from the sequence counts of the GLM APIDs. |
| `glm_not_decoded` | whole number | always | 1.1 | Compressed GLM payloads, saved as files and not decoded. |
| `glm_bad_length` | whole number | always | 1.1 | Flash and event units whose length does not match their count. |
| `glm_group_bad_length` | whole number | always | 1.1 | Group units whose length does not match their count. |
| `flash_count` | whole number | always | 1.1 | Flashes decoded. |
| `group_count` | whole number | always | 1.1 | Groups decoded. |
| `event_count` | whole number | always | 1.1 | Events counted. |
| `glm_rejected` | whole number | always | 1.1 | Flash and group records with an impossible value. |
| `newest_flash` | time | optional | 1.1 | The time of the first event of the newest flash (UTC). |
| `flash_rate` | number (1/min) | always | 1.1 | Flashes in the 60 s of flash time up to the newest flash. |

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

### Table `grb_products`

Struct `GrbProductRow`. A picture product of `goes_grb`. The row key is the APID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `apid` | whole number | always | 1.0 | The APID. |
| `name` | text | always | 1.0 | The name of the product of the APID. |
| `updated` | duration (s) | always | 1.0 | The signal time of the last update. |

The column `name` names a row.

### Table `packets_by_apid`

Struct `GrbApidRow`. A row of the packets of each APID of `goes_grb`. The row key is the APID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `apid` | whole number | always | 1.0 | The APID. |
| `name` | text | always | 1.0 | The name of the product of the APID. |
| `packets` | whole number | always | 1.0 | The packets of the APID. |

The column `name` names a row.

### Table `frames`

Struct `UhritFrameRow`. A row of the frames of each virtual channel of `gk2a_uhrit`. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel. |
| `frames` | whole number | always | 1.0 | The frames of the virtual channel. |

### Table `flashes`

Struct `GlmFlashRow`. A lightning flash of `goes_grb`. The row key is the product time (microseconds since J2000) and the flash id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `flash` | whole number | always | 1.1 | The product-unique flash id. |
| `time` | time | always | 1.1 | The time of the first event (UTC). |
| `duration` | duration (s) | always | 1.1 | From the first to the last event. |
| `position` | position (°) | always | 1.1 | The energy-weighted centroid, latitude first. |
| `area_km2` | number (km²) | optional | 1.1 | The area covered. |
| `energy_fj` | number (fJ) | optional | 1.1 | The radiant energy. |
| `quality` | choice | optional | 1.1 | The data quality. |
| `groups` | whole number | always | 1.1 | Groups received with this flash as parent. |
| `events` | whole number | always | 1.1 | Events received in those groups. |

Place of a row: the column `position`.

### Table `groups`

Struct `GlmGroupRow`. A lightning group of `goes_grb`. The row key is the product time (microseconds since J2000) and the group id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `group` | whole number | always | 1.1 | The product-unique group id. |
| `time` | time | always | 1.1 | The mean time of the events (UTC). |
| `position` | position (°) | always | 1.1 | The energy-weighted centroid, latitude first. |
| `area_km2` | number (km²) | optional | 1.1 | The area covered. |
| `energy_fj` | number (fJ) | optional | 1.1 | The radiant energy. |
| `flash` | whole number | always | 1.1 | The parent flash id. |
| `quality` | choice | optional | 1.1 | The data quality. |
| `events` | whole number | always | 1.1 | Events received with this group as parent. |

Place of a row: the column `position`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_goes_grb_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_goes_grb_get_symbol_rate` | `sdrkit_goes_grb_set_symbol_rate` |
| `roll_off` | `sdrkit_goes_grb_get_roll_off` | `sdrkit_goes_grb_set_roll_off` |
| `carrier_offset` | `sdrkit_goes_grb_get_carrier_offset` | `sdrkit_goes_grb_set_carrier_offset` |
| `product` | `sdrkit_goes_grb_get_product` | `sdrkit_goes_grb_set_product` |
| `channels` | `sdrkit_goes_grb_get_channels` | `sdrkit_goes_grb_set_channels` |
| `modcod_rule` | `sdrkit_goes_grb_get_modcod_rule` | `sdrkit_goes_grb_set_modcod_rule` |
| `modcod` | `sdrkit_goes_grb_get_modcod` | `sdrkit_goes_grb_set_modcod` |
| `modcods` | `sdrkit_goes_grb_get_modcods` | `sdrkit_goes_grb_set_modcods` |
| `frame_length` | `sdrkit_goes_grb_get_frame_length` | `sdrkit_goes_grb_set_frame_length` |
| `pilots` | `sdrkit_goes_grb_get_pilots` | `sdrkit_goes_grb_set_pilots` |
| `pl_scrambling` | `sdrkit_goes_grb_get_pl_scrambling` | `sdrkit_goes_grb_set_pl_scrambling` |

## Try it

```sh
sdrkit run goes_grb --source synth:goes_grb
sdrkit run goes_grb --source iiod:192.168.2.1 --freq 1686.6
```
<!-- generated: end -->

## Description

The stack reads the data fields of a DVB-S2 generic stream: 2048-byte CADUs that start at byte boundaries, AOS frames of
virtual channels 5 and 6, and space packets with an 8-byte secondary header and a CRC-32. Packets join into payloads: picture
blocks in JPEG 2000 (ABI by channel and zone, SUVI by band), GLM lightning data, and other generic payloads (metadata and
information) that leave as `satimg.file` items. Pictures leave as `picture.strip` items, one picture per APID; ABI pictures
are kept at 4 km (every second, fourth or eighth row and column by the channel).

The DVB-S2 receiver is the one of `dvbs`; its data-field output carries every good BBFRAME, whatever its stream type says.
The symbol rate and the roll-off are settings with the GRB values as defaults.

Standard: ETSI EN 302 307-1 (DVB-S2); CCSDS 131.0-B, 732.0-B, 133.0-B; ITU-T T.800 (JPEG 2000); GOES-R Series Product
Definition and Users' Guide (PUG), Volume 4: GRB, revision H.1.

## GLM lightning data

APIDs (PUG A.2): 300 metadata (an NcML text), 301 events, 302 flashes, 303 groups. Each APID has its own 14-bit sequence
count (PUG 4.5.1). On these APIDs a gap in the count counts the packets lost and drops the payload in progress, or the rest
of a payload whose first packet was lost; a payload without its last packet is dropped when the next one starts. A dropped
payload is counted, never decoded in part.

A payload is the 21-byte generic header (PUG 5.3.1, big endian: compression, product time in seconds since 2000-01-01
12:00:00 UTC and microseconds, data unit count) and a data unit (little endian, PUG 4.5): a count of 8 bytes and that many
records (PUG 5.3.2). A unit whose length is not 8 + count x record size is counted, not read, and saved as a file.

| Record | Size | Fields (PUG 7.2.1.6.1) |
|---|---|---|
| Flash | 24 | id (16 bits), time of the first and last event, frame time of the first and last event, latitude and longitude (32-bit floats), area, energy, quality |
| Group | 28 | id (32 bits), time, frame time, latitude and longitude (floats), area, energy, parent flash id, quality; bytes 24 to 27 are not described |
| Event | 16 | id (32 bits), time, latitude, longitude, energy, parent group id (32 bits) |

Scaling (PUG Table 7.2.1.6.2): a time is raw x 0.0003814756 - 5 s after the product time; an event latitude is raw x
0.00203128 - 66.56 degrees; an event longitude is raw x 0.00203128 plus the `add_offset` of `event_lon` in the metadata
(the west bound of the field of view: -141.56 GOES-East, -203.56 GOES-West, -156.06 GOES-Test); energy is raw x 1.52597e-15
J; area is raw x 0.152601862 km². For area, energy and quality, 65535 means no value. Times are UTC from the product time, with days of 86 400 s.

Checks: a flash or group is rejected and counted when its latitude or longitude is not finite, the latitude is beyond
66.56 degrees or the longitude outside -203.56 to 180 degrees (PUG Table 7.2.1.1-1), its area or energy is 65531 to 65534
(valid range 0 to 65530), its quality is not 0, 1, 3, 5 or 65535, or (a flash) its first event is after its last.
Longitudes below -180 degrees are moved into -180 to 180.

Results: the tables `flashes` and `groups` hold the 64 newest of each (row key: product time in microseconds since J2000,
a dash, the id), with time (UTC), position, area (km²), energy (fJ, 10^-15 J), quality, and the counts of the groups and
events received with each one as parent. Events are counted, not listed. The fields give the counts of GLM payloads,
payloads dropped, packets lost, payloads not decoded, units of a wrong length, flashes, groups, events, records rejected,
the time of the newest flash and the flash rate (flashes in the 60 s of flash time up to the newest flash).

## Not confirmed by a public document

Not checked against a public document: the symbol rate 8 665 938 sym/s and the roll-off 0.25, the CADUs of 2048 bytes without
randomiser or Reed-Solomon, the 34-byte picture block header, the APIDs other than GLM, and the 4 km reduction of ABI
pictures. The secondary header of PUG 4.5.2 is not consistent: its table gives the payload variant 5 bits (bits 101 to
105) and the assembler 2 bits, its text gives the variant bits 101 to 103 and the assembler bits 104 to 107. The stack
reads 5 bits. Variant 0 (generic, GLM) reads the same in both; picture variants 2 and 3 would read as 8 and 12 under the
text's layout. SUVI colour is not composed. The stack was not run on a real carrier.

Not decoded: compressed GLM payloads (SZIP, compression 2, or JPEG 2000) are counted and saved as files; the GLM metadata is
saved as a file and only the `add_offset` of `event_lon` is read from it; event longitudes are unknown until a metadata
payload has come. The guide gives the group record a 28-byte stride but describes 24 bytes; the stack follows the stride and
does not read bytes 24 to 27. The guide gives no leap-second rule for the GLM times (the time units of other GRB products
say "neglecting leap seconds").

## Try it: notes

The test signal is DVB-S2 QPSK 9/10, normal frames, no pilots, roll-off 0.25, a generic continuous stream of CADUs: an ABI
CONUS block of channel 13 (APID 0BC, 64 x 16 pixels of 12 bits, lossless JPEG 2000) a line, and a generic payload on APID
580. Each time the 40 picture lines start again, one GLM product goes first: three flashes (one packet), their six groups
(two packets), twelve events (three packets), a second flash unit whose last packet is not sent, and the metadata, the
product time 20 s later each time. It needs 20 Msps.
