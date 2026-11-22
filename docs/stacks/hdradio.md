<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HD Radio

HD Radio (NRSC-5) on FM and AM: station information, programs HD1 to HD8, song titles and album art.

Stack id: `hdradio`. Group: Digital radio.

Signal types: HD Radio.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 520 kHz to 108 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 200 kHz |
| Signal bandwidth | 400 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hdradio` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_waveform` | Waveform | choice | `mp1` | `mp1` (FM hybrid MP1), `ma1` (AM hybrid MA1), `mp3` (FM extended hybrid MP3), `ma3` (AM all-digital MA3) |  |
| `gen_analog_db` | Analogue level | number (dB) | 0 | -20 to 20 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `band` | Band | choice |
| `service_mode` | Service mode | choice |
| `offset_hz` | Offset | frequency |
| `mer_lower_db` | MER lower | number (dB) |
| `mer_upper_db` | MER upper | number (dB) |
| `ber` | Bit error rate | number |
| `blocks` | Blocks | whole number |
| `block_syncs` | Block syncs | whole number |
| `p1_ok` | P1 good | whole number |
| `p1_bad` | P1 bad | whole number |
| `p3_ok` | P3 good | whole number |
| `p3_bad` | P3 bad | whole number |
| `pids_ok` | PIDS good | whole number |
| `pids_bad` | PIDS bad | whole number |
| `pdus` | Audio PDUs | whole number |
| `aas_ok` | AAS good | whole number |
| `aas_bad` | AAS bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `signal_time` | Signal time | duration (s) |
| `codec_note` | Codec | text |
| `p1_on` | P1 | yes or no |
| `pids_on` | PIDS | yes or no |
| `psd_on` | PSD | yes or no |
| `data_on` | Data | yes or no |
| `program` | Program | text |
| `call_sign` | Call sign | text |
| `universal_name` | Universal name | text |
| `long_name` | Long name | text |
| `slogan` | Slogan | text |
| `message` | Message | text |
| `country` | Country | text |
| `facility_id` | Facility id | whole number |
| `location` | Location | position (deg) |
| `altitude_m` | Altitude | number (m) |
| `text` | Text | text |
| `item_running` | Item running | yes or no |
| `slide_name` | Slide | text |
| `slide_category` | Slide category | text |
| `slide_link` | Slide link | text |
| `slide_alert` | Emergency slide | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `programs` | Programs | `number` (whole number), `label` (text), `name` (text), `program_type` (whole number), `type_name` (text), `restricted` (yes or no), `codec_mode` (whole number), `blend` (whole number), `sound_experience` (whole number), `bit_rate_bps` (number (bit/s)), `packets_ok` (whole number), `packets_bad` (whole number), `in_sis` (yes or no), `in_sig` (yes or no), `broadcasting` (yes or no), `psd_count` (whole number), `title` (text), `artist` (text), `album` (text), `genre` (text), `comment` (text), `ufid` (text), `commercial` (text), `art_lot` (whole number), `art_mime` (text), `logo_port` (whole number), `art_port` (whole number) |
| `data_services` | Data services | `name` (text), `service` (whole number), `port` (whole number), `kind` (whole number), `mime` (text), `packets` (whole number), `bytes` (whole number) |
| `files` | Files | `port` (whole number), `lot` (whole number), `name` (text), `mime` (text), `size` (whole number), `received` (whole number), `complete` (yes or no), `version` (whole number), `expiry_mjd` (whole number (MJD)), `updated` (duration (s)) |
| `label_tags` | Text tags | `code` (whole number), `type` (text), `text` (text) |
| `guide_services` | Guide services | `name` (text), `short_name` (text), `description` (text), `sid` (whole number), `eid` (whole number), `genres` (text), `logo_url` (text) |
| `guide_programmes` | Programmes | `crid` (text), `name` (text), `start` (time (s)), `duration` (duration (s)), `description` (text), `genres` (text) |
| `guide_groups` | Programme groups | `name` (text) |
| `slide_categories` | Slide categories | `category` (whole number), `title` (text), `slide` (whole number), `name` (text) |
| `logos` | Logos | `png` (bytes), `jpeg` (bytes) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hdr.psd` | Song | `channel` (text), `title` (text), `artist` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |
| `clear` | Clear |  |  | Forgets the signal and starts again. |

## Typed interface

The stack is in the family `broadcast`, version 1.1. The C header `sdrkit/broadcast.h`, the registry record `registry/broadcast.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Broadcast radio (`broadcast`). Media: pictures.

### Result fields

Struct `HdradioFields`. The results of the HD Radio stack. No band or mode before the first frame, no measurement while the receiver searches and no station data before the PIDS carry it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | choice | optional | 1.0 | The kind of signal. |
| `service_mode` | choice | optional | 1.0 | The service mode. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. |
| `mer_lower_db` | number (dB) | optional | 1.0 | The MER of the lower sideband (AM: the primary). |
| `mer_upper_db` | number (dB) | optional | 1.0 | The MER of the upper sideband (AM: the secondary). |
| `ber` | number | optional | 1.0 | The bit error rate of the P1 channel. |
| `blocks` | whole number | always | 1.0 | Blocks. |
| `block_syncs` | whole number | always | 1.0 | Block syncs. |
| `p1_ok` | whole number | always | 1.0 | P1 frames good. |
| `p1_bad` | whole number | always | 1.0 | P1 frames bad. |
| `p3_ok` | whole number | always | 1.0 | P3 frames good. |
| `p3_bad` | whole number | always | 1.0 | P3 frames bad. |
| `pids_ok` | whole number | always | 1.0 | PIDS frames good. |
| `pids_bad` | whole number | always | 1.0 | PIDS frames bad. |
| `pdus` | whole number | always | 1.0 | Audio PDUs. |
| `aas_ok` | whole number | always | 1.0 | AAS packets good. |
| `aas_bad` | whole number | always | 1.0 | AAS packets bad. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. |
| `signal_time` | duration (s) | always | 1.0 | Signal time. |
| `codec_note` | text | optional | 1.0 | What the audio codec needs, or why it cannot play. |
| `p1_on` | yes or no | always | 1.0 | P1 frames arrive. |
| `pids_on` | yes or no | always | 1.0 | PIDS frames arrive. |
| `psd_on` | yes or no | always | 1.0 | Song data seen. |
| `data_on` | yes or no | always | 1.0 | Data seen. |
| `program` | text | always | 1.0 | The row key of the program to hear in [`PROGRAMS`]. |
| `call_sign` | text | optional | 1.0 | The call sign. |
| `universal_name` | text | optional | 1.0 | The universal name. |
| `long_name` | text | optional | 1.0 | The long name. |
| `slogan` | text | optional | 1.0 | The slogan. |
| `message` | text | optional | 1.0 | The station message. |
| `country` | text | optional | 1.0 | The country. |
| `facility_id` | whole number | optional | 1.0 | The facility id. |
| `location` | position (deg) | optional | 1.0 | The location of the station. |
| `altitude_m` | number (m) | optional | 1.0 | The altitude of the station. |

### Result fields

Struct `LabelFields`. The radio text and whether its item is running.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | optional | 1.0 | The radio text. |
| `item_running` | yes or no | optional | 1.0 | The tagged item (a song) is playing. |

### Result fields

Struct `SlideFields`. The slide shown.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `slide_name` | text | optional | 1.0 | The name of the slide (its MOT ContentName). |
| `slide_category` | text | optional | 1.0 | The category title of the slide. |
| `slide_link` | text | optional | 1.0 | The click-through URL of the slide. |
| `slide_alert` | yes or no | always | 1.0 | The slide is an emergency alert. |

### Table `programs`

Struct `HdradioProgramRow`. An audio program. The row key is "HD1" to "HD8".

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `number` | whole number | always | 1.0 | The program number, 0 for HD1. |
| `label` | text | always | 1.0 | "HD1" to "HD8". |
| `name` | text | optional | 1.0 | The name from the SIG. |
| `program_type` | whole number | always | 1.0 | The program type code. |
| `type_name` | text | optional | 1.0 | The name of the program type. |
| `restricted` | yes or no | always | 1.0 | Restricted access. |
| `codec_mode` | whole number | optional | 1.0 | The codec mode. |
| `blend` | whole number | optional | 1.0 | The blend control. |
| `sound_experience` | whole number | always | 1.0 | The sound experience code. |
| `bit_rate_bps` | number (bit/s) | always | 1.0 | The bit rate. |
| `packets_ok` | whole number | always | 1.0 | Packets good. |
| `packets_bad` | whole number | always | 1.0 | Packets bad. |
| `in_sis` | yes or no | always | 1.0 | Listed in the SIS. |
| `in_sig` | yes or no | always | 1.0 | Listed in the SIG. |
| `broadcasting` | yes or no | always | 1.0 | On air. |
| `psd_count` | whole number | always | 1.0 | Song data messages. |
| `title` | text | optional | 1.0 | The song title. |
| `artist` | text | optional | 1.0 | The artist. |
| `album` | text | optional | 1.0 | The album. |
| `genre` | text | optional | 1.0 | The genre. |
| `comment` | text | optional | 1.0 | The comment. |
| `ufid` | text | optional | 1.0 | The unique file identifier. |
| `commercial` | text | optional | 1.0 | The commercial frame, its parts separated by "; ". |
| `art_lot` | whole number | optional | 1.0 | The LOT number of the album art. |
| `art_mime` | text | optional | 1.0 | The type of the album art. |
| `logo_port` | whole number | optional | 1.0 | The port of the station logo. |
| `art_port` | whole number | optional | 1.0 | The port of the album art. |

The column `name` names a row.

### Table `data_services`

Struct `HdradioDataServiceRow`. A data service component. The row key is its port.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the service. |
| `service` | whole number | always | 1.0 | The service number. |
| `port` | whole number | always | 1.0 | The port. |
| `kind` | whole number | always | 1.0 | The component type. |
| `mime` | text | always | 1.0 | The content type. |
| `packets` | whole number | always | 1.0 | Packets. |
| `bytes` | whole number | always | 1.0 | Bytes. |

The column `name` names a row.

### Table `files`

Struct `HdradioFileRow`. A file of the LOT transport. The row key is `<port>:<lot>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `port` | whole number | always | 1.0 | The port. |
| `lot` | whole number | always | 1.0 | The LOT number. |
| `name` | text | always | 1.0 | The file name. |
| `mime` | text | always | 1.0 | The content type. |
| `size` | whole number | always | 1.0 | The size in bytes. |
| `received` | whole number | always | 1.0 | Bytes received. |
| `complete` | yes or no | always | 1.0 | The file is complete. |
| `version` | whole number | always | 1.0 | The version. |
| `expiry_mjd` | whole number (MJD) | always | 1.0 | The expiry date. |
| `updated` | duration (s) | always | 1.0 | When it last changed, in signal time. |

The column `name` names a row.

### Table `label_tags`

Struct `LabelTagRow`. A tag of the radio text. The row key is the class name of the content type (`ITEM.TITLE`), or its code for a reserved type.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | whole number | always | 1.0 | The content type code (0 to 127). |
| `type` | text | optional | 1.0 | What people call the content type ("Title"). |
| `text` | text | always | 1.0 | The tagged text. |

### Table `guide_services`

Struct `GuideServiceRow`. A service of the service information. The row key is the SId in hexadecimal, or the place of a service without a bearer.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | optional | 1.0 | The name. |
| `short_name` | text | optional | 1.0 | The short name. |
| `description` | text | optional | 1.0 | The description. |
| `sid` | whole number | optional | 1.0 | The service identifier of the first bearer. |
| `eid` | whole number | optional | 1.0 | The ensemble identifier of the first bearer. |
| `genres` | text | optional | 1.0 | The genre links, separated by spaces. |
| `logo_url` | text | optional | 1.0 | The URL of the first logo. |

### Table `guide_programmes`

Struct `GuideProgrammeRow`. A programme of the programme information. The row key is `<crid>@<start>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `crid` | text | always | 1.0 | The programme id (CRID). |
| `name` | text | optional | 1.0 | The name. |
| `start` | time (s) | always | 1.0 | The start time. |
| `duration` | duration (s) | always | 1.0 | The duration. |
| `description` | text | optional | 1.0 | The description. |
| `genres` | text | optional | 1.0 | The genre links, separated by spaces. |

### Table `guide_groups`

Struct `GuideGroupRow`. A programme group. The row key is its name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name. |

### Table `slide_categories`

Struct `SlideCategoryRow`. A slide of a categorised slideshow. The row key is `<CategoryID>/<SlideID>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `category` | whole number | always | 1.0 | The CategoryID. |
| `title` | text | optional | 1.0 | The CategoryTitle. |
| `slide` | whole number | always | 1.0 | The SlideID. |
| `name` | text | always | 1.0 | The name of the slide. |

### Table `logos`

Struct `LogoRow`. A logo. The row key is its MOT ContentName. A row holds the picture in the member of its format.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `png` | bytes | optional | 1.0 | A PNG picture. |
| `jpeg` | bytes | optional | 1.0 | A JPEG picture. |

### Event `hdr.psd`

Struct `HdradioSongEvent`. The fields of the song event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | text | always | 1.0 | The program ("HD1"). |
| `title` | text | always | 1.0 | The song title. |
| `artist` | text | always | 1.0 | The artist. |
| `text` | text | always | 1.0 | "artist - title", or the one of them that is there. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `service.select` | `sdrkit_hdradio_service_select` | `sdrkit_broadcast_service_select_args` | none |
| `clear` | `sdrkit_hdradio_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_waveform` | `sdrkit_hdradio_gen_get_waveform` | `sdrkit_hdradio_gen_set_waveform` |
| `gen_analog_db` | `sdrkit_hdradio_gen_get_analog_db` | `sdrkit_hdradio_gen_set_analog_db` |

## Try it

```sh
sdrkit run hdradio --source synth:hdradio
sdrkit run hdradio --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
