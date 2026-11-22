<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GK-2A UHRIT

GEO-KOMPSAT-2A ultra high rate xRIT on X band (DVB-S2, 15.6 Msym/s): full-disk pictures, keys and texts.

Stack id: `gk2a_uhrit`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 8.07 GHz |
| Tuning range | 950 MHz to 9 GHz |
| Sample rate | 30 MHz |
| Smallest sample rate | 26.9 MHz |
| Signal bandwidth | 23.45366 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gk2a_uhrit` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 15.62244 MHz | 0 Hz to 60 MHz |  |
| `roll_off` | Roll-off | choice | `0.25` | `auto` (Auto), `0.35`, `0.25`, `0.20`, `0.15`, `0.10`, `0.05` |  |
| `carrier_offset` | Carrier | text (Hz) | empty |  | Offset in Hz from the centre. Empty: the carrier nearest the centre. |
| `filter` | Show | choice | `all` | `all` (All products), `images` (Images), `text` (Text) |  |
| `follow_newest` | Newest | yes or no | true |  |  |
| `pl_scrambling` | PL scrambling code | whole number | 0 | 0 to 262141 |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `uncounted_secondary` | Secondary header outside the length | whole number (bytes) | 8 | 0 to 64 | A packet with its secondary header flag set is this much longer than its length field says. |
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
| `frames_crc_bad` | Frames with a bad CRC | whole number |
| `packets_crc_bad` | Packets with a bad CRC | whole number |
| `files` | Files | whole number |
| `last_file` | Last file | text |
| `products` | Products | whole number |
| `hint` | Hint | text |
| `note` | Note | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carriers | `offset` (frequency), `symbol_rate` (frequency), `roll_off` (number), `snr_db` (number (dB)), `level_db` (number (dB)), `decoded` (yes or no) |
| `streams` | Input streams | `isi` (whole number), `carries` (choice), `packet_bytes` (whole number (bytes)), `sync_byte` (whole number), `ccm` (yes or no), `frames` (whole number), `bits` (whole number) |
| `stream_modcods` | MODCODs of the input streams | `stream` (text), `modcod` (whole number), `modulation` (choice), `code_rate` (choice), `frames` (whole number), `share_pct` (number (%)) |
| `packets_by_apid` | Packets by APID | `apid` (whole number), `packets` (whole number) |
| `frames` | Frames | `vcid` (whole number), `frames` (whole number) |
| `products` | Products | `product` (text), `kind` (choice), `done` (text) |
| `images` | Images | `product` (text), `channel` (text), `time` (duration (s)), `width` (whole number (px)), `height` (whole number (px)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `spectrum` | Spectrum | bin | dB |
| `mer_history` | MER history | frame | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `satimg.product` | Product | `source` (text), `text` (text), `family` (choice), `id` (whole number), `kind` (choice), `name` (text), `vcid` (whole number), `apid` (whole number), `rows` (whole number), `width` (whole number), `height` (whole number), `segments` (whole number), `segments_total` (whole number), `complete` (yes or no), `errors` (yes or no), `version` (whole number), `note` (text) |
| `satimg.text` | Text product | `source` (text), `text` (text), `family` (choice), `name` (text), `inner_name` (text), `id` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `import_keys` | Import keys | `path` (text, required) |  | Reads a KMA key file. |
| `add_keys` | Add keys | `keys` (text, required) |  | Adds keys typed as text. |
| `clear` | Clear |  |  | Forgets every product. |

## Typed interface

The stack is in the family `satimg`, version 1.3. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `UhritFields`. The result fields of the file layer of `gk2a_uhrit`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_crc_bad` | whole number | always | 1.0 | Frames with a bad CRC-16. |
| `packets_crc_bad` | whole number | always | 1.0 | Packets with a bad CRC-32. |
| `files` | whole number | always | 1.0 | Files. |
| `last_file` | text | always | 1.0 | The last file name. |
| `products` | whole number | optional | 1.0 | Products kept by the file layer. No value before the file layer reports. |
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

### Table `packets_by_apid`

Struct `UhritApidRow`. A row of the packets of each APID of `gk2a_uhrit`. The row key is the APID.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `apid` | whole number | always | 1.0 | The APID. |
| `packets` | whole number | always | 1.0 | The packets of the APID. |

### Table `frames`

Struct `UhritFrameRow`. A row of the frames of each virtual channel of `gk2a_uhrit`. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel. |
| `frames` | whole number | always | 1.0 | The frames of the virtual channel. |

### Table `products`

Struct `ProductRow`. A product of an xRIT stack. The row key is the product id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `product` | text | always | 1.0 | The name of a product in the `products` table. |
| `kind` | choice | always | 1.0 | The kind of a product in the `products` table. |
| `done` | text | always | 1.0 | How much of a product arrived: rows or segments for images, a size for others, "..." while it arrives, "errors". |

### Table `images`

Struct `ImageRow`. A finished image. The row key names the image. The picture itself leaves on the picture stream.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `product` | text | always | 1.0 | The product, or the instrument, that an image shows. |
| `channel` | text | optional | 1.0 | The channel that an image shows, when the product names one. |
| `time` | duration (s) | optional | 1.0 | The signal time at which the image started. |
| `width` | whole number (px) | always | 1.0 | The image width in pixels. |
| `height` | whole number (px) | optional | 1.0 | The image height in pixels (rows received for a picture that grows). |

### Event `satimg.product`

Struct `ProductEvent`. A product that changed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | Where the product comes from: its name. |
| `text` | text | always | 1.0 | The text: the name of a product, the text of a text product. |
| `family` | choice | always | 1.0 | The family of the source: a satellite. |
| `id` | whole number | always | 1.0 | The product id. |
| `kind` | choice | always | 1.0 | The kind of a product. |
| `name` | text | always | 1.0 | The name of a product. |
| `vcid` | whole number | always | 1.0 | The virtual channel of a product. |
| `apid` | whole number | always | 1.0 | The APID of a product. |
| `rows` | whole number | always | 1.0 | Rows received. |
| `width` | whole number | always | 1.0 | The picture width. |
| `height` | whole number | always | 1.0 | The picture height. |
| `segments` | whole number | always | 1.0 | Segments received. |
| `segments_total` | whole number | always | 1.0 | Segments in all. |
| `complete` | yes or no | always | 1.0 | Every segment arrived. |
| `errors` | yes or no | always | 1.0 | A file of the product had errors. |
| `version` | whole number | always | 1.0 | Grows with every change of the product. |
| `note` | text | optional | 1.0 | A note ("picture not decoded: ..."). |

### Event `satimg.text`

Struct `TextEvent`. A text product.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | Where the product comes from: its name. |
| `text` | text | always | 1.0 | The text: the name of a product, the text of a text product. |
| `family` | choice | always | 1.0 | The family of the source: a satellite. |
| `name` | text | always | 1.0 | The name of a product. |
| `inner_name` | text | optional | 1.0 | The file name inside a ZIP archive. |
| `id` | whole number | always | 1.0 | The product id. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `import_keys` | `sdrkit_gk2a_uhrit_import_keys` | `sdrkit_satimg_file_args` | none |
| `add_keys` | `sdrkit_gk2a_uhrit_add_keys` | `sdrkit_satimg_keys_args` | none |
| `clear` | `sdrkit_gk2a_uhrit_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_gk2a_uhrit_get_symbol_rate` | `sdrkit_gk2a_uhrit_set_symbol_rate` |
| `roll_off` | `sdrkit_gk2a_uhrit_get_roll_off` | `sdrkit_gk2a_uhrit_set_roll_off` |
| `carrier_offset` | `sdrkit_gk2a_uhrit_get_carrier_offset` | `sdrkit_gk2a_uhrit_set_carrier_offset` |
| `uncounted_secondary` | `sdrkit_gk2a_uhrit_get_uncounted_secondary` | `sdrkit_gk2a_uhrit_set_uncounted_secondary` |
| `filter` | `sdrkit_gk2a_uhrit_get_filter` | `sdrkit_gk2a_uhrit_set_filter` |
| `follow_newest` | `sdrkit_gk2a_uhrit_get_follow_newest` | `sdrkit_gk2a_uhrit_set_follow_newest` |
| `modcod_rule` | `sdrkit_gk2a_uhrit_get_modcod_rule` | `sdrkit_gk2a_uhrit_set_modcod_rule` |
| `modcod` | `sdrkit_gk2a_uhrit_get_modcod` | `sdrkit_gk2a_uhrit_set_modcod` |
| `modcods` | `sdrkit_gk2a_uhrit_get_modcods` | `sdrkit_gk2a_uhrit_set_modcods` |
| `frame_length` | `sdrkit_gk2a_uhrit_get_frame_length` | `sdrkit_gk2a_uhrit_set_frame_length` |
| `pilots` | `sdrkit_gk2a_uhrit_get_pilots` | `sdrkit_gk2a_uhrit_set_pilots` |
| `pl_scrambling` | `sdrkit_gk2a_uhrit_get_pl_scrambling` | `sdrkit_gk2a_uhrit_set_pl_scrambling` |

## Try it

```sh
sdrkit run gk2a_uhrit --source synth:gk2a_uhrit
sdrkit run gk2a_uhrit --source <driver>:<address> --freq 8070
```
<!-- generated: end -->

## Description

The stack reads the data fields of a DVB-S2 generic stream: 2048-byte CADUs that start at byte boundaries, AOS frames that
end with a CRC-16, and space packets whose 8-byte secondary header the length field does not count, each with a CRC-32. The
packets of each virtual channel and APID join into xRIT files (a 10-byte header in front of each is removed), and the files
go to the GK-2A file layer of `gk2a_hrit`: pictures, key import and texts work as there. Virtual channel 63 is fill.

The DVB-S2 receiver is the one of `dvbs`. The symbol rate and the roll-off are settings with the UHRIT values as defaults.

Standard: ETSI EN 302 307-1 (DVB-S2); CCSDS 131.0-B, 732.0-B, 133.0-B; KMA LRIT/HRIT mission specification (headers).

## Not confirmed by a public document

No public document was read for this link. These values are not confirmed by a public document: the symbol rate 15 622 440
sym/s and the roll-off 0.25, the CADUs of 2048 bytes, the CRC-16 of each frame, the 8-byte secondary header outside the
packet length, the CRC-32 of each packet and the 10-byte header before each file. The stack was not run on a real carrier.

## Try it: notes

The test signal is DVB-S2 8PSK 2/3, normal frames, pilots, roll-off 0.25, a generic continuous stream of CADUs on virtual
channel 7 with one xRIT file "IMG_TEST" of 6000 data bytes a step on APID 020. It needs 30 Msps.
