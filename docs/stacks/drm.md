<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DRM

Digital Radio Mondiale on short wave (DRM30) and VHF (DRM+): services, text messages, slide shows and Journaline.

Stack id: `drm`. Group: Digital radio.

Signal types: DRM.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 240 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:drm` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_robustness` | Robustness mode | choice | `B` | `A`, `B`, `C`, `D`, `E` |  |
| `gen_occupancy` | Occupancy | choice | `usual` | `usual` (Usual), `0` (4.5 kHz), `1` (5 kHz), `2` (9 kHz), `3` (10 kHz), `4` (18 kHz), `5` (20 kHz) |  |
| `gen_msc_qam` | MSC | choice | `high` | `high` (64-QAM (E: 16-QAM)), `low` (16-QAM (E: 4-QAM)) |  |
| `gen_protection_b` | Protection | choice | `usual` | `usual` (Usual), `0`, `1`, `2`, `3` |  |
| `gen_interleaver` | Interleaver | choice | `long` | `long` (Long), `short` (Short) |  |
| `gen_audio` | Audio | choice | `melody` | `melody` (Melody), `tone` (1 kHz tone), `silence` (Silence), `noise` (Noise-like), `not_aac` (Not AAC) |  |
| `gen_text_message` | Text message | yes or no | true |  |  |
| `gen_data` | Data service | choice | `none` | `none` (None), `2` (MOT slide show), `3` (MOT website), `4` (TPEG), `7` (EPG), `1098` (Journaline) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `robustness_mode` | Robustness mode | choice |
| `occupancy` | Occupancy | whole number |
| `bandwidth` | Bandwidth | frequency |
| `interleaver` | Interleaver | text |
| `msc` | MSC mapping | text |
| `sdc` | SDC mapping | text |
| `protection` | Protection | text |
| `protection_b` | Protection B | whole number |
| `time` | Time | text |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `delay_ms` | Strongest path | number (ms) |
| `guard_correlation` | Guard correlation | number |
| `quality` | Quality | number |
| `fac_ok` | FAC good | whole number |
| `fac_bad` | FAC bad | whole number |
| `sdc_ok` | SDC good | whole number |
| `sdc_bad` | SDC bad | whole number |
| `mux_ok` | Multiplex frames good | whole number |
| `mux_bad` | Multiplex frames bad | whole number |
| `audio_ok` | Audio frames good | whole number |
| `audio_bad` | Audio frames bad | whole number |
| `fades` | Fades held | whole number |
| `resyncs` | Re-synchronisations | whole number |
| `audio_state` | Audio | choice |
| `audio_info` | Audio | text |
| `text_ok` | Text segments good | whole number |
| `text_bad` | Text segments bad | whole number |
| `signal_time` | Signal time | duration (s) |
| `first_audio` | First audio | duration (s) |
| `holding` | Holding through a fade | yes or no |
| `mirrored` | Mirrored | yes or no |
| `playing_service` | Service playing | text |
| `text` | Text | text |
| `item_running` | Item running | yes or no |
| `slide_name` | Slide | text |
| `slide_category` | Slide category | text |
| `slide_link` | Slide link | text |
| `slide_alert` | Emergency slide | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `services` | Services | `short_id` (whole number), `service_id` (text), `kind` (text), `label` (text), `language` (text), `country` (text), `programme` (text), `ca` (yes or no), `codec` (text), `core_rate` (frequency), `audio_mode` (whole number), `sbr` (yes or no), `text` (yes or no), `stream` (whole number), `bit_rate_bps` (number (bit/s)), `application` (text), `mode_text` (text), `decoded` (yes or no) |
| `streams` | Streams | `part_a` (whole number (B)), `part_b` (whole number (B)) |
| `objects` | Objects | `short_id` (whole number), `transport_id` (whole number), `name` (text), `content_type` (text), `size` (whole number), `received` (whole number), `order` (whole number), `note` (text) |
| `slides` | Slides | `short_id` (whole number), `order` (whole number), `name` (text), `category` (text), `category_id` (whole number), `slide_id` (whole number), `click_url` (text), `alt_url` (text), `trigger` (text), `content_type` (text), `size` (whole number), `width` (whole number), `height` (whole number) |
| `journaline` | Journaline | `id` (whole number), `kind` (text), `title` (text), `content` (text) |
| `packets` | Packets | `short_id` (whole number), `packets` (whole number), `bad_crc` (whole number), `lost` (whole number), `orphans` (whole number), `empty` (whole number), `other_id` (whole number), `units_ok` (whole number), `units_dropped` (whole number), `unit_bytes` (whole number), `tpeg_frames` (whole number) |
| `notes` | Notes | `note` (text) |
| `label_tags` | Text tags | `code` (whole number), `type` (text), `text` (text) |
| `guide_services` | Guide services | `name` (text), `short_name` (text), `description` (text), `sid` (whole number), `eid` (whole number), `genres` (text), `logo_url` (text) |
| `guide_programmes` | Programmes | `crid` (text), `name` (text), `start` (time (s)), `duration` (duration (s)), `description` (text), `genres` (text) |
| `guide_groups` | Programme groups | `name` (text) |
| `slide_categories` | Slide categories | `category` (whole number), `title` (text), `slide` (whole number), `name` (text) |
| `logos` | Logos | `png` (bytes), `jpeg` (bytes) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `channel` | Channel | carrier | dB |
| `impulse_response` | Impulse response | ms | dB |
| `fac_constellation` | FAC |  |  |
| `sdc_constellation` | SDC |  |  |
| `msc_constellation` | MSC |  |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `drm.text` | Text message | `text` (text), `family` (choice) |
| `drm.object` | Object received | `transport_id` (whole number), `text` (text), `content_type` (text), `size` (whole number), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |
| `clear` | Clear |  |  | Forgets the signal and starts again. |

## Typed interface

The stack is in the family `broadcast`, version 2.1. The C header `sdrkit/broadcast.h`, the registry record `registry/broadcast.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Broadcast radio (`broadcast`). Media: sound (programme), pictures.

### Result fields

Struct `DrmFields`. The results of the DRM stack. No FAC values before the first FAC, no protection before the multiplex description, no time before SDC type 8, no measurement before the first frame and no first audio before it came.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `robustness_mode` | choice | optional | 1.0 | The robustness mode, A to E. |
| `occupancy` | whole number | optional | 1.0 | The spectrum occupancy code. |
| `bandwidth` | frequency | optional | 1.0 | The channel bandwidth. |
| `interleaver` | text | optional | 1.0 | The interleaver as text. |
| `msc` | text | optional | 1.0 | The MSC mapping, with the hierarchical flag. |
| `sdc` | text | optional | 1.0 | The SDC mapping. |
| `protection` | text | optional | 1.0 | The protection levels and their code rates. |
| `protection_b` | whole number | optional | 1.0 | The protection level of part B. |
| `time` | text | optional | 1.0 | The time of SDC type 8 as text. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The sample clock error. |
| `delay_ms` | number (ms) | optional | 1.0 | The delay of the strongest path from the symbol start. |
| `guard_correlation` | number | optional | 1.0 | The guard correlation at the symbol timing. |
| `quality` | number | optional | 1.0 | The quality, 0 to 1. |
| `fac_ok` | whole number | always | 1.0 | FAC blocks good. |
| `fac_bad` | whole number | always | 1.0 | FAC blocks bad. |
| `sdc_ok` | whole number | always | 1.0 | SDC blocks good. |
| `sdc_bad` | whole number | always | 1.0 | SDC blocks bad. |
| `mux_ok` | whole number | always | 1.0 | Multiplex frames good. |
| `mux_bad` | whole number | always | 1.0 | Multiplex frames bad. |
| `audio_ok` | whole number | always | 1.0 | Audio frames good. |
| `audio_bad` | whole number | always | 1.0 | Audio frames bad. |
| `fades` | whole number | always | 1.0 | Fades held. |
| `resyncs` | whole number | always | 1.0 | Re-synchronisations. |
| `audio_state` | choice | always | 1.0 | The audio state. |
| `audio_info` | text | always | 1.0 | What plays, or why nothing can. |
| `text_ok` | whole number | always | 1.0 | Text segments good. |
| `text_bad` | whole number | always | 1.0 | Text segments bad. |
| `signal_time` | duration (s) | always | 1.0 | Seconds of signal. |
| `first_audio` | duration (s) | optional | 1.0 | When the first audio frame came. |
| `holding` | yes or no | always | 1.0 | Holding through a fade. |
| `mirrored` | yes or no | always | 1.0 | The input is mirrored (I and Q swapped). |
| `playing_service` | text | optional | 2.1 | The row key of the service playing in the services table. No value before its first audio frame. |

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

### Table `services`

Struct `DrmServiceRow`. A service. The row key is its short id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `short_id` | whole number | always | 1.0 | The short id. |
| `service_id` | text | always | 1.0 | The 24-bit service id, six hex digits. |
| `kind` | text | always | 1.0 | "audio" or "data". |
| `label` | text | optional | 1.0 | The label, once the SDC gave it. |
| `language` | text | always | 1.0 | The language. |
| `country` | text | optional | 1.0 | The country, once the SDC gave it. |
| `programme` | text | always | 1.0 | The programme type or application id. |
| `ca` | yes or no | always | 1.0 | Conditional access is flagged. |
| `codec` | text | optional | 1.0 | The audio coding as text, once the SDC gave it. |
| `core_rate` | frequency | optional | 1.0 | The core sampling rate, once the SDC gave it. |
| `audio_mode` | whole number | always | 1.0 | The audio mode code. |
| `sbr` | yes or no | always | 1.0 | SBR. |
| `text` | yes or no | always | 1.0 | Text messages carried. |
| `stream` | whole number | always | 1.0 | The stream id. |
| `bit_rate_bps` | number (bit/s) | always | 1.0 | The net bit rate of the stream. |
| `application` | text | optional | 1.0 | The application name of a data service. |
| `mode_text` | text | optional | 1.0 | The mode text of a data service. |
| `decoded` | yes or no | always | 1.0 | Whether the service is decoded. |

The column `label` names a row.

### Table `streams`

Struct `DrmStreamRow`. The bytes per logical frame of a stream. The row key is the stream number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `part_a` | whole number (B) | always | 1.0 | The bytes of part A. |
| `part_b` | whole number (B) | always | 1.0 | The bytes of part B. |

### Table `objects`

Struct `DrmObjectRow`. A MOT object of a data service. The row key is `<short id>:<transport id>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `short_id` | whole number | always | 1.0 | The short id of the service. |
| `transport_id` | whole number | always | 1.0 | The MOT transport id. |
| `name` | text | always | 1.0 | The name, or the transport id for an object without one. |
| `content_type` | text | always | 1.0 | The content type. |
| `size` | whole number | always | 1.0 | The size in bytes. |
| `received` | whole number | always | 1.0 | Times received. |
| `order` | whole number | always | 1.0 | The order of arrival. |
| `note` | text | optional | 1.0 | What the object is, where the stack can say. |

The column `name` names a row.

### Table `slides`

Struct `DrmSlideRow`. The latest slide of a service. The row key is the short id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `short_id` | whole number | always | 1.0 | The short id of the service. |
| `order` | whole number | always | 1.0 | The order number of the slide. |
| `name` | text | always | 1.0 | The name. |
| `category` | text | optional | 1.0 | The category title. |
| `category_id` | whole number | optional | 1.0 | The category id. |
| `slide_id` | whole number | optional | 1.0 | The slide id. |
| `click_url` | text | optional | 1.0 | The click-through URL. |
| `alt_url` | text | optional | 1.0 | The alternative location URL. |
| `trigger` | text | optional | 1.0 | The trigger time as text. |
| `content_type` | text | always | 1.0 | The content type. |
| `size` | whole number | always | 1.0 | The size in bytes. |
| `width` | whole number | always | 1.0 | The width of the picture. |
| `height` | whole number | always | 1.0 | The height of the picture. |

The column `name` names a row.

### Table `journaline`

Struct `DrmPageRow`. A Journaline object. The row key is its id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The object id. |
| `kind` | text | always | 1.0 | The object type. |
| `title` | text | always | 1.0 | The title. |
| `content` | text | always | 1.0 | The content: links (id and text), body or list items, one per line. |

The column `title` names a row.

### Table `packets`

Struct `DrmPacketRow`. The packet counters of a packet-mode service. The row key is its short id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `short_id` | whole number | always | 1.0 | The short id of the service. |
| `packets` | whole number | always | 1.0 | Packets. |
| `bad_crc` | whole number | always | 1.0 | Packets with a bad CRC. |
| `lost` | whole number | always | 1.0 | Packets lost. |
| `orphans` | whole number | always | 1.0 | Continuation packets without their first. |
| `empty` | whole number | always | 1.0 | Padding packets. |
| `other_id` | whole number | always | 1.0 | Packets of another packet id. |
| `units_ok` | whole number | always | 1.0 | Data units complete. |
| `units_dropped` | whole number | always | 1.0 | Data units dropped. |
| `unit_bytes` | whole number | always | 1.0 | Bytes of the data units. |
| `tpeg_frames` | whole number | always | 1.0 | TPEG frames counted. |

### Table `notes`

Struct `DrmNoteRow`. A note on what the receiver cannot decode. The row key is its order.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | The note. |

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

### Event `drm.text`

Struct `DrmTextEvent`. The fields of the event of a new text message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text message. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Event `drm.object`

Struct `DrmObjectEvent`. The fields of the event of a received MOT object.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transport_id` | whole number | always | 1.0 | The MOT transport id. |
| `text` | text | always | 1.0 | The name of the object. |
| `content_type` | text | always | 1.0 | The content type. |
| `size` | whole number | always | 1.0 | The size in bytes. |
| `family` | choice | always | 1.0 | The family of the sender: broadcast. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `service.select` | `sdrkit_drm_service_select` | `sdrkit_broadcast_service_select_args` | none |
| `clear` | `sdrkit_drm_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_robustness` | `sdrkit_drm_gen_get_robustness` | `sdrkit_drm_gen_set_robustness` |
| `gen_occupancy` | `sdrkit_drm_gen_get_occupancy` | `sdrkit_drm_gen_set_occupancy` |
| `gen_msc_qam` | `sdrkit_drm_gen_get_msc_qam` | `sdrkit_drm_gen_set_msc_qam` |
| `gen_protection_b` | `sdrkit_drm_gen_get_protection_b` | `sdrkit_drm_gen_set_protection_b` |
| `gen_interleaver` | `sdrkit_drm_gen_get_interleaver` | `sdrkit_drm_gen_set_interleaver` |
| `gen_audio` | `sdrkit_drm_gen_get_audio` | `sdrkit_drm_gen_set_audio` |
| `gen_text_message` | `sdrkit_drm_gen_get_text_message` | `sdrkit_drm_gen_set_text_message` |
| `gen_data` | `sdrkit_drm_gen_get_data` | `sdrkit_drm_gen_set_data` |

## Try it

```sh
sdrkit run drm --source synth:drm
sdrkit run drm --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
