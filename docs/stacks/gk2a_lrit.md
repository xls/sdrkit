<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GK-2A LRIT

GEO-KOMPSAT-2A LRIT on 1692.14 MHz: full-disk channel pictures and text.

Stack id: `gk2a_lrit`. Group: Weather satellites.

Signal types: LRIT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.69214 GHz |
| Tuning range | 1.67 GHz to 1.71 GHz |
| Sample rate | 2.4 MHz |
| Smallest sample rate | 300 kHz |
| Signal bandwidth | 300 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gk2a_lrit` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `filter` | Show | choice | `all` | `all` (All products), `images` (Images), `text` (Text) |  |
| `follow_newest` | Newest | yes or no | true |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_invert_g2` | Test G2 inverted | yes or no | true |  |  |
| `gen_nrzm` | Test NRZ-M | yes or no | false |  |  |
| `gen_basis` | Test Reed-Solomon basis | choice | `dual` | `dual` (Dual), `conventional` (Conventional) |  |
| `gen_fill` | Test fill frames | whole number | 20 | 0 to 100000 |  |
| `gen_once` | Test files once | yes or no | false |  |  |
| `gen_counter` | Test frame count start | whole number | 0 | 0 to 16777215 |  |
| `gen_encrypt` | Test encrypted VI006 | yes or no | false |  |  |

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
| `link` | Link | text |
| `carrier_locked` | Carrier locked | yes or no |
| `symbol_rate_measured_hz` | Symbol rate measured | frequency |
| `g2_inverted` | Second output inverted | yes or no |
| `rs_basis` | Reed-Solomon basis | choice |
| `byte_errors_pct` | Byte errors | number (%) |
| `transfer_frames` | Transfer frames | whole number |
| `fill_frames` | Fill frames | whole number |
| `frame_gaps` | Frame count gaps | whole number |
| `crc_errors` | CRC errors | whole number |
| `files` | Files | whole number |
| `file_errors` | Files with errors | whole number |
| `length_errors` | Files of a wrong length | whole number |
| `products` | Products | whole number |
| `keys` | Keys loaded | whole number |
| `segments_locked` | Segments locked | whole number |
| `hint` | Hint | text |
| `first_decoding` | First frames | duration (s) |
| `cadus_imported` | CADUs imported | whole number |
| `ggak_frames` | GGAK frames | whole number |
| `ggak_good` | GGAK good | whole number |
| `ggak_bad` | GGAK bad | whole number |
| `ggak_marker` | GGAK marker | whole number |
| `ggak_field` | GGAK field | number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `virtual_channels` | Virtual channels | `vcid` (whole number), `frames` (whole number) |
| `products` | Products | `product` (text), `kind` (choice), `done` (text) |
| `images` | Images | `product` (text), `channel` (text), `time` (duration (s)), `width` (whole number (px)), `height` (whole number (px)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `esn0` | Es/N0 | s | dB |
| `byte_errors` | Byte errors | s | % |
| `frames_per_s` | Frames per second | s |  |
| `constellation` | Constellation |  |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `satimg.product` | Product | `source` (text), `text` (text), `family` (choice), `id` (whole number), `kind` (choice), `name` (text), `vcid` (whole number), `apid` (whole number), `rows` (whole number), `width` (whole number), `height` (whole number), `segments` (whole number), `segments_total` (whole number), `complete` (yes or no), `errors` (yes or no), `version` (whole number), `note` (text) |
| `satimg.text` | Text product | `source` (text), `text` (text), `family` (choice), `name` (text), `inner_name` (text), `id` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `open_cadu` | Open CADU | `path` (text, required) |  | Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input. |
| `clear` | Clear |  |  | Forgets the newest frame. |
| `import_keys` | Import keys | `path` (text, required) |  | Reads a KMA key file. |
| `add_keys` | Add keys | `keys` (text, required) |  | Adds keys typed as text. |

## Typed interface

The stack is in the family `satimg`, version 1.3. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite imagery (`satellite_imagery`). Media: pictures.

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

Struct `XritFields`. The fields of an xRIT stack (GOES HRIT, GK-2A LRIT and HRIT, Elektro-L) beside the link layers.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | text | always | 1.0 | The name of the running link ("HRIT", "LRIT", "HRIT 665.4 ksym/s", "GGAK 5 ksym/s"). |
| `carrier_locked` | yes or no | optional | 1.0 | The carrier is locked (also while frames decode). |
| `symbol_rate_measured_hz` | frequency | optional | 1.0 | The symbol rate the timing loop measures. |
| `g2_inverted` | yes or no | optional | 1.0 | The second code output arrives inverted (as CCSDS sends it). |
| `rs_basis` | choice | optional | 1.0 | The basis of the Reed-Solomon symbols. |
| `byte_errors_pct` | number (%) | optional | 1.0 | Bytes corrected over the last second, percent. |
| `transfer_frames` | whole number | optional | 1.0 | Transfer frames (fill frames not counted). |
| `fill_frames` | whole number | optional | 1.0 | Fill frames. |
| `frame_gaps` | whole number | optional | 1.0 | Gaps of the frame counts. |
| `crc_errors` | whole number | optional | 1.0 | Packets with a wrong CRC. |
| `files` | whole number | optional | 1.0 | Files. |
| `file_errors` | whole number | optional | 1.0 | Files with errors. |
| `length_errors` | whole number | optional | 1.0 | Files whose length differs from their transport header. |
| `products` | whole number | optional | 1.0 | Products kept. |
| `keys` | whole number | optional | 1.0 | GK-2A: keys loaded. |
| `segments_locked` | whole number | optional | 1.0 | GK-2A: segments waiting for a key. |
| `hint` | text | always | 1.0 | What the user can do (empty when all is well). |
| `first_decoding` | duration (s) | optional | 1.0 | The signal time at which frames first decoded. |
| `cadus_imported` | whole number | optional | 1.0 | CADUs imported from a file. |
| `ggak_frames` | whole number | optional | 1.0 | GGAK frames. |
| `ggak_good` | whole number | optional | 1.0 | GGAK frames with a good CRC. |
| `ggak_bad` | whole number | optional | 1.0 | GGAK frames with a bad CRC. |
| `ggak_marker` | whole number | optional | 1.0 | The marker of the last GGAK frame. |
| `ggak_field` | number | optional | 1.0 | \|B\| of the last plausible GGAK field. |

### Table `virtual_channels`

Struct `VirtualChannelRow`. The frames of one virtual channel. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel of a frame header. |
| `frames` | whole number | always | 1.0 | The good frames of a virtual channel. |

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
| `open_cadu` | `sdrkit_gk2a_lrit_open_cadu` | `sdrkit_satimg_file_args` | none |
| `clear` | `sdrkit_gk2a_lrit_clear` | none | none |
| `import_keys` | `sdrkit_gk2a_lrit_import_keys` | `sdrkit_satimg_file_args` | none |
| `add_keys` | `sdrkit_gk2a_lrit_add_keys` | `sdrkit_satimg_keys_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `filter` | `sdrkit_gk2a_lrit_get_filter` | `sdrkit_gk2a_lrit_set_filter` |
| `follow_newest` | `sdrkit_gk2a_lrit_get_follow_newest` | `sdrkit_gk2a_lrit_set_follow_newest` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_invert_g2` | `sdrkit_gk2a_lrit_gen_get_invert_g2` | `sdrkit_gk2a_lrit_gen_set_invert_g2` |
| `gen_nrzm` | `sdrkit_gk2a_lrit_gen_get_nrzm` | `sdrkit_gk2a_lrit_gen_set_nrzm` |
| `gen_basis` | `sdrkit_gk2a_lrit_gen_get_basis` | `sdrkit_gk2a_lrit_gen_set_basis` |
| `gen_fill` | `sdrkit_gk2a_lrit_gen_get_fill` | `sdrkit_gk2a_lrit_gen_set_fill` |
| `gen_once` | `sdrkit_gk2a_lrit_gen_get_once` | `sdrkit_gk2a_lrit_gen_set_once` |
| `gen_counter` | `sdrkit_gk2a_lrit_gen_get_counter` | `sdrkit_gk2a_lrit_gen_set_counter` |
| `gen_encrypt` | `sdrkit_gk2a_lrit_gen_get_encrypt` | `sdrkit_gk2a_lrit_gen_set_encrypt` |

## Try it

```sh
sdrkit run gk2a_lrit --source synth:gk2a_lrit
sdrkit run gk2a_lrit --source iiod:192.168.2.1 --freq 1692.14
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
