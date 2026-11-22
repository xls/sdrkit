<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# UAT 978 MHz

UAT on 978 MHz: aircraft and TIS-B targets, and the FIS-B weather and notices from ground stations.

Stack id: `uat`. Group: Aircraft.

Signal types: UAT.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 978 MHz |
| Tuning range | 960 MHz to 990 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1.9 MHz |
| Signal bandwidth | 1 MHz |
| Radio filter | 1.5 MHz |
| Automatic gain | High fixed gain |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:uat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `expiry_s` | Expiry time | number (s) | 60 | 5 to 3600 | How long an aircraft's address stays known after its last trusted message. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_aircraft` | Aircraft | whole number | 6 | 0 to 100 |  |
| `gen_uplink` | Uplink frames | yes or no | true |  |  |
| `gen_example` | Example frame only | yes or no | false |  | Sends the example frame of the standard in bursts instead of an airspace. |
| `gen_bursts` | Example bursts | whole number | 4 | 1 to 20 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `aircraft_count` | Aircraft | whole number |
| `with_position` | With position | whole number |
| `message_rate` | Messages per second | frequency |
| `fisb_products` | FIS-B products | whole number |
| `tisb_recent` | TIS-B targets | whole number |
| `fisb_pending` | FIS-B files waiting | whole number |
| `frames_ok` | Frames good | whole number |
| `snr_db` | SNR | number (dB) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `report` | Report | whole number |
| `time_s` | Signal time | number (s) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `aircraft` | Aircraft | `icao` (text), `address_qualifier` (whole number), `callsign` (text), `registration` (text), `country` (text), `squawk` (text), `position` (position (°)), `alt_ft` (whole number (ft)), `geo_alt_ft` (whole number (ft)), `speed_kt` (number (kt)), `heading_deg` (number (°)), `vertical_rate_fpm` (whole number (ft/min)), `on_ground` (yes or no), `tisb` (yes or no), `nic` (whole number), `category` (whole number), `emergency` (whole number), `level_dbfs` (number (dBFS)), `distance_m` (number (m)), `bearing_deg` (number (°)), `heard` (whole number), `last_heard` (time) |
| `fisb_products` | FIS-B products | `time_s` (number (s)), `product_id` (whole number), `name` (text), `stamp` (text), `text` (text), `records` (whole number), `blocks` (whole number) |
| `fisb_records` | FIS-B records | `product` (text), `time_s` (number (s)), `product_id` (whole number), `name` (text), `report` (whole number), `kind` (choice), `active` (yes or no), `label` (text), `overlay` (whole number), `start` (text), `end` (text), `geometry` (choice), `position` (position (°)), `vertices` (whole number), `radius_m` (number (m)), `alt_bottom_ft` (whole number (ft)), `alt_top_ft` (whole number (ft)), `text` (text) |
| `fisb_blocks` | FIS-B grid blocks | `product` (text), `time_s` (number (s)), `product_id` (whole number), `name` (text), `block` (whole number), `south` (yes or no), `scale` (whole number), `corner` (position (°)), `lat_size_deg` (number (°)), `lon_size_deg` (number (°)), `max_bin` (whole number), `bins` (text) |
| `fisb_vertices` | FIS-B vertices | `record` (text), `index` (whole number), `position` (position (°)), `alt_ft` (whole number (ft)) |
| `log` | Log | `time_s` (number (s)), `kind` (choice), `address` (text), `level_dbfs` (number (dBFS)), `summary` (text) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `uat.message` | UAT message | `text` (text), `kind` (choice), `source` (text), `level_dbfs` (number (dBFS)), `position` (position), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `adsb`, version 1.3. The C header `sdrkit/adsb.h`, the registry record `registry/adsb.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `UatFields`. The result fields of the UAT tracker.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aircraft_count` | whole number | always | 1.0 | Aircraft in the table. |
| `with_position` | whole number | always | 1.0 | Aircraft with a position. |
| `message_rate` | frequency | always | 1.0 | Good messages per second over the last second. |
| `fisb_products` | whole number | always | 1.0 | FIS-B products held. |
| `tisb_recent` | whole number | always | 1.0 | TIS-B messages among the last 64. |
| `fisb_pending` | whole number | always | 1.0 | FIS-B files waiting for segments. |
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `noise_dbfs` | number (dBFS) | always | 1.0 | The noise floor. |
| `report` | whole number | always | 1.0 | Reports made. |
| `time_s` | number (s) | always | 1.0 | The signal time of the report, seconds from the start of the input. |

### Table `aircraft`

Struct `UatAircraftRow`. A row of the UAT aircraft table. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `icao` | text | always | 1.0 | The 24-bit address, six hex digits. |
| `address_qualifier` | whole number | always | 1.0 | The address qualifier. |
| `callsign` | text | optional | 1.0 | The flight identification. |
| `registration` | text | optional | 1.0 | The registration derived from the address. |
| `country` | text | optional | 1.0 | The State the address is allocated to. |
| `squawk` | text | optional | 1.0 | The identity code, four octal digits. |
| `position` | position (°) | optional | 1.0 | The last position. |
| `alt_ft` | whole number (ft) | optional | 1.0 | The barometric altitude. |
| `geo_alt_ft` | whole number (ft) | optional | 1.0 | The geometric altitude. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed. |
| `heading_deg` | number (°) | optional | 1.0 | The track or heading. |
| `vertical_rate_fpm` | whole number (ft/min) | optional | 1.0 | The vertical rate. |
| `on_ground` | yes or no | always | 1.0 | On the ground. |
| `tisb` | yes or no | always | 1.0 | Heard through TIS-B. |
| `nic` | whole number | always | 1.0 | The navigation integrity category. |
| `category` | whole number | optional | 1.0 | The UAT emitter category. |
| `emergency` | whole number | always | 1.0 | The emergency state (0 none). |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last message. |
| `distance_m` | number (m) | optional | 1.0 | The distance from the station. |
| `bearing_deg` | number (°) | optional | 1.0 | The bearing from the station. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. |

The column `callsign` names a row. Place of a row: the column `position`. Height: `alt_ft`. Course: `heading_deg`. Speed: `speed_kt`.

### Table `fisb_products`

Struct `FisbProductRow`. A FIS-B product held. The row key is the number of the product in six digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time the product was received. |
| `product_id` | whole number | always | 1.0 | The product id. |
| `name` | text | always | 1.0 | The product name. |
| `stamp` | text | always | 1.0 | The product's own time ("MM-DD HH:MM:SS", the parts sent). |
| `text` | text | always | 1.0 | The text. |
| `records` | whole number | always | 1.0 | TWGO records in the product. |
| `blocks` | whole number | always | 1.0 | Grid blocks in the product. |

The column `name` names a row.

### Table `fisb_records`

Struct `FisbRecordRow`. A text or graphic record (TWGO) of a product held: a NOTAM, AIRMET, SIGMET, G-AIRMET, CWA, SUA or TFR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `product` | text | always | 1.0 | The product the record belongs to: the row key of the products table. |
| `time_s` | number (s) | always | 1.0 | Signal time the product was received. |
| `product_id` | whole number | always | 1.0 | The product id. |
| `name` | text | always | 1.0 | The product name. |
| `report` | whole number | always | 1.0 | The report number of the record. |
| `kind` | choice | always | 1.0 | What the record is. |
| `active` | yes or no | optional | 1.0 | The record is active (text records). |
| `label` | text | optional | 1.0 | The label of a graphic record. |
| `overlay` | whole number | optional | 1.0 | The overlay record identifier. |
| `start` | text | optional | 1.0 | The start of the validity ("MM-DD HH:MM", the parts sent). |
| `end` | text | optional | 1.0 | The end of the validity. |
| `geometry` | choice | optional | 1.0 | The geometry of a graphic record. |
| `position` | position (°) | optional | 1.0 | The first vertex, the centre or the point. |
| `vertices` | whole number | optional | 1.0 | The number of vertices of a path. The vertices are rows of the vertices table. |
| `radius_m` | number (m) | optional | 1.0 | The radius of a circle. |
| `alt_bottom_ft` | whole number (ft) | optional | 1.0 | The lowest altitude. |
| `alt_top_ft` | whole number (ft) | optional | 1.0 | The highest altitude. |
| `text` | text | optional | 1.0 | The text of a text record. |

The column `name` names a row. Place of a row: the column `position`.

### Table `fisb_blocks`

Struct `FisbBlockRow`. A block of a gridded product held (NEXRAD, icing, cloud tops, turbulence, lightning).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `product` | text | always | 1.0 | The product the block belongs to: the row key of the products table. |
| `time_s` | number (s) | always | 1.0 | Signal time the product was received. |
| `product_id` | whole number | always | 1.0 | The product id. |
| `name` | text | always | 1.0 | The product name. |
| `block` | whole number | always | 1.0 | The block number. |
| `south` | yes or no | always | 1.0 | The block is in the southern hemisphere. |
| `scale` | whole number | always | 1.0 | The scale factor (0 high, 1 medium, 2 low resolution). |
| `corner` | position (°) | always | 1.0 | The north-west corner of the block. |
| `lat_size_deg` | number (°) | always | 1.0 | The height of the block. |
| `lon_size_deg` | number (°) | always | 1.0 | The width of the block. |
| `max_bin` | whole number | always | 1.0 | The highest bin value. |
| `bins` | text | optional | 1.0 | The 128 bins, one hex digit each, rows from the north. An empty block has none. |

The column `name` names a row. Place of a row: the column `corner`.

### Table `fisb_vertices`

Struct `FisbVertexRow`. A vertex of the path of a graphic record held.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `record` | text | always | 1.0 | The record the vertex belongs to: the row key of the records table. |
| `index` | whole number | always | 1.0 | The place of the vertex in its path, from 0. |
| `position` | position (°) | always | 1.0 | The position. |
| `alt_ft` | whole number (ft) | always | 1.0 | The altitude. |

Place of a row: the column `position`. Height: `alt_ft`.

### Table `log`

Struct `UatLogRow`. A line of the UAT message log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time. |
| `kind` | choice | always | 1.0 | What the message is. |
| `address` | text | optional | 1.0 | The address. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level. |
| `summary` | text | always | 1.0 | What it said. |

The column `address` names a row.

### Event `uat.message`

Struct `UatMessage`. The fields of the event of a decoded UAT message. The address is the source. The urgency is an emergency for squawk 7500, 7600 or 7700 and for an emergency status.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | What the message said. |
| `kind` | choice | always | 1.0 | What the message is. |
| `source` | text | optional | 1.0 | The address. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `position` | position | optional | 1.0 | The position decoded from the message. |
| `urgency` | choice | always | 1.0 | How urgent the event is. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_uat_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `expiry_s` | `sdrkit_uat_get_expiry_s` | `sdrkit_uat_set_expiry_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_aircraft` | `sdrkit_uat_gen_get_aircraft` | `sdrkit_uat_gen_set_aircraft` |
| `gen_uplink` | `sdrkit_uat_gen_get_uplink` | `sdrkit_uat_gen_set_uplink` |
| `gen_example` | `sdrkit_uat_gen_get_example` | `sdrkit_uat_gen_set_example` |
| `gen_bursts` | `sdrkit_uat_gen_get_bursts` | `sdrkit_uat_gen_set_bursts` |

## Try it

```sh
sdrkit run uat --source synth:uat
sdrkit run uat --source iiod:192.168.2.1 --freq 978
```
<!-- generated: end -->
