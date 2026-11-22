<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Analog TV

Analogue television: PAL, NTSC and SECAM pictures with FM, AM, NICAM or A2 sound and teletext; also FM video and NBTV. Transmits a picture and the sound that the host gives.

Stack id: `atv`. Group: Video.

Signal types: Analog TV.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 45 MHz to 6 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 8 MHz |
| Signal bandwidth | 8 MHz |
| Radio filter | 9 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:atv` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `modulation` | Modulation | choice | `am` | `am` (AM (broadcast)), `fm` (FM video (FPV)), `nbtv_am` (NBTV / mechanical on AM), `nbtv_fm` (NBTV / mechanical on FM) | Broadcast television is AM. FM video needs 20 Msps. |
| `channels` | Channels | text | empty |  | The channels: entries separated by ";", each a label, "=" and the frequency in Hz, for example "E21=474000000" or "R5=5806000000". At most 256 entries for atv and 64 for fpv. The host fills it from a channel plan file. The table channels lists them and channel.tune takes a label. Empty: no channel. |
| `system` | System | choice | `auto` | `auto` (Automatic), `b` (B (7 MHz)), `g` (G (8 MHz)), `i` (I), `dk` (D/K), `m` (M), `n` (N), `a` (A (405 lines)), `c` (C), `e` (E (819 lines)), `h` (H), `kp` (K'), `l` (L), `lp` (L' (band I)) | Automatic finds the system from the signal. E needs 20 Msps for its 14 MHz channel. |
| `colour` | Colour system | choice | `auto` | `auto` (Automatic), `mono` (Mono), `pal` (PAL), `ntsc` (NTSC), `secam` (SECAM) |  |
| `colour_on` | Colour | yes or no | true |  |  |
| `saturation` | Saturation | number | 1 | 0 to 2 |  |
| `tint` | Tint | number (°) | 0 | -30 to 30 | NTSC only. PAL and SECAM correct themselves. |
| `detector` | Envelope detector | choice | `auto` | `auto` (Automatic (synchronous when the carrier is locked)), `envelope` (Envelope only) |  |
| `deinterlace` | Fields | choice | `weave` | `weave` (Weave), `bob` (Bob) |  |
| `noise_reduction` | Noise reduction | choice | `auto` | `auto` (Automatic), `off` (Off), `light` (Light), `medium` (Medium), `strong` (Strong) |  |
| `black_setup` | Black setup | choice | `system` | `system` (By system), `none` (None), `75` (7.5 IRE) |  |
| `chroma_delay` | Chroma delay | number (ns) | -1 | -1 to 500 | -1 is the standard's 170 ns. |
| `channel_width` | Channel width | choice | `8` | `6` (6 MHz), `7` (7 MHz), `8` (8 MHz) | Only matters for the carrier offset with 5.5 MHz sound. |
| `snow` | Show snow when there is no picture | yes or no | false |  |  |
| `sound_system` | Sound | choice | `auto` | `auto` (Automatic (NICAM or A2 when there)), `carrier` (FM / AM carrier only) |  |
| `dual_sound` | Two languages | choice | `first` | `first` (First), `second` (Second) |  |
| `nbtv_lines` | Lines | whole number | 32 | 24 to 240 |  |
| `nbtv_rate` | Pictures/s | number | 12.5 | 1 to 60 |  |
| `nbtv_deviation` | FM deviation | number (kHz) | 5 | 0.5 to 100 |  |
| `nbtv_vertical` | Vertical scan | yes or no | true |  |  |
| `tx_system` | System | choice | `g` | `g` (G (8 MHz)), `b` (B (7 MHz)), `i` (I), `dk` (D/K), `m` (M), `n` (N), `a` (A (405 lines)), `c` (C), `e` (E (819 lines)), `h` (H), `kp` (K'), `l` (L), `lp` (L' (band I)) |  |
| `tx_colour` | Colour | choice | `system` | `system` (By system), `pal` (PAL), `ntsc` (NTSC), `secam` (SECAM), `none` (None) | By system: NTSC on M, SECAM on L, L' and K', none on A, C and E, PAL on the others. |
| `tx_picture` | Test picture | choice | `card` | `card` (Test card), `bars` (Colour bars), `ramp` (Grey ramp) | What is sent until the host gives a picture with the command frame. |
| `tx_sound` | Sound | yes or no | true |  | On: the host's audio (mono) on the sound carrier, an unmodulated carrier while it gives none. Off: no sound carrier. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tx_compression` | Sync compression | number (%) | 0 | 0 to 80 |  |
| `tx_group_delay` | Group-delay pre-correction | yes or no | true |  | The chrominance 170 ns early, as BT.470 asks of a PAL or NTSC transmitter. |
| `tx_nyquist` | Nyquist slope | yes or no | false |  |  |
| `tx_setup` | Black setup in 525 lines | choice | `75` | `75` (7.5 IRE), `none` (None (Japan)) |  |
| `tx_secam_id` | SECAM identification | choice | `both` | `both` (Both), `porch` (Back porch only), `lines` (Field lines only) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_system` | System | choice | `g` | `g` (B/G (8 MHz)), `b` (B (7 MHz)), `i` (I), `dk` (D/K), `m` (M), `n` (N), `a` (A (405)), `c` (C), `e` (E (819)), `h` (H), `kp` (K'), `l` (L), `lp` (L'), `nbtv_am` (NBTV on AM), `nbtv_fm` (NBTV on narrow FM) |  |
| `gen_colour` | Colour | choice | `system` | `system` (By system), `pal` (PAL), `ntsc` (NTSC), `secam` (SECAM), `none` (None) |  |
| `gen_picture` | Picture | choice | `card` | `card` (Test card), `bars` (Colour bars), `ramp` (Grey ramp) |  |
| `gen_sound` | Sound | choice | `gaps` | `gaps` (1 kHz tone with gaps), `melody` (Melody), `carrier` (Unmodulated carrier), `none` (No sound carrier), `tone` (Continuous 1 kHz) |  |
| `gen_second` | Second sound | choice | `none` | `none` (None), `a2_stereo` (A2 stereo), `a2_dual` (A2 two languages), `a2_mono` (A2 mono (pilot)), `nicam_stereo` (NICAM stereo), `nicam_dual` (NICAM two languages) |  |
| `gen_compression` | Sync compression | number (%) | 0 | 0 to 80 |  |
| `gen_group_delay` | Group-delay pre-correction | yes or no | true |  |  |
| `gen_nyquist` | Nyquist-slope transmitter | yes or no | false |  |  |
| `gen_setup` | Black setup in 525 lines | choice | `75` | `75` (7.5 IRE), `none` (None (Japan)) |  |
| `gen_secam_id` | SECAM identification | choice | `both` | `both` (Both), `porch` (Back porch only), `lines` (Field lines only) |  |
| `gen_ident` | Ident text | text | `TEST CARD` |  |  |
| `gen_teletext` | Teletext | yes or no | false |  | Three pages of teletext in the field blanking (625 lines). |
| `gen_start` | Clock start time | number (s) | 0 | 0 to 86400 |  |
| `gen_nbtv_lines` | NBTV lines | whole number | 0 | 0 to 240 |  |
| `gen_nbtv_rate` | NBTV pictures/s | number | 0 | 0 to 60 |  |
| `gen_nbtv_pattern` | NBTV picture | whole number | 0 | 0 to 2 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `system` | System | text |
| `colour_system` | Colour system | choice |
| `colour` | Colour | yes or no |
| `colour_killer` | Colour killer | yes or no |
| `lines` | Lines | whole number |
| `field_rate` | Field rate | frequency |
| `line_rate` | Line rate | frequency |
| `line_error_ppm` | Line rate error | number (ppm) |
| `detector` | Detector | choice |
| `modulation` | Modulation | choice |
| `vision_carrier` | Vision carrier | frequency |
| `sound_carrier` | Sound carrier | frequency |
| `sound_spacing` | Sound spacing | frequency |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `carrier_to_noise_db` | Carrier to noise | number (dB) |
| `snr_db` | SNR | number (dB) |
| `sound_present` | Sound carrier | yes or no |
| `sound_deviation` | Deviation | frequency |
| `audio_level_dbfs` | Audio level | number (dBFS) |
| `sound_system` | Sound | choice |
| `sync_found_pct` | Sync found | number (%) |
| `line_count` | Lines decoded | whole number |
| `field_count` | Fields | whole number |
| `pictures` | Pictures | whole number |
| `snow_pictures` | Snow pictures | whole number |
| `last_field` | Last field | whole number |
| `fields_ok` | Fields good | whole number |
| `fields_bad` | Fields damaged | whole number |
| `sync_depth_pct` | Sync depth | number (%) |
| `sync_compression_pct` | Sync compression | number (%) |
| `white_peak` | White peak | number |
| `burst_pct` | Burst | number (%) |
| `burst_phase_error_deg` | Burst phase error | number (°) |
| `chroma_level_pct` | Chroma level | number (%) |
| `secam_identification` | SECAM identification | text |
| `fm_polarity` | FM polarity | choice |
| `nbtv_line_locked` | Line lock | yes or no |
| `nbtv_picture_locked` | Picture lock | yes or no |
| `nicam_locked` | NICAM | yes or no |
| `nicam_valid` | NICAM sound | yes or no |
| `nicam_mode` | NICAM mode | choice |
| `nicam_parity_errors` | NICAM parity errors | whole number |
| `nicam_error_pct` | NICAM errors | number (%) |
| `a2_mode` | A2 | choice |
| `teletext_header` | Teletext | text |
| `teletext_pages` | Teletext pages | whole number |
| `teletext_packets` | Teletext packets | whole number |
| `teletext_errors` | Teletext errors | whole number |
| `teletext_parity_errors` | Teletext parity errors | whole number |
| `teletext_page` | Teletext page | whole number |
| `page_text` | Page text | text |
| `tx_sending` | Sending | yes or no |
| `tx_system` | System sent | text |
| `tx_frames` | Frames shown | whole number |
| `tx_fields` | Fields sent | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Channels | `label` (text), `vision` (frequency) |
| `teletext_pages` | Teletext pages | `page` (whole number), `subpages` (whole number) |
| `teletext_subpages` | Teletext subpages | `page_row` (text), `subcode` (whole number), `version` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `line` | Line | µs |  |
| `vbi` | Field blanking line | µs |  |
| `spectrum` | Spectrum | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `atv.state` | State | `state` (choice), `text` (text), `family` (choice) |
| `teletext.page` | Teletext page | `page` (whole number), `subcode` (whole number), `version` (whole number), `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `channel.tune` | Tune to a channel | `row` (text, required) |  | Asks the host to tune to a channel of the setting channels. |
| `start` | Start sending |  |  | Sends the television signal continuously from the start of a field: the picture of the command frame, or the test picture, and the host's audio. The transmit settings apply at each start. |
| `stop` | Stop sending |  |  | Ends the transmission at the end of a field, with a fade of 5 ms. |
| `frame` | Set the picture | `pixels` (bytes, required), `width` (whole number (px), required), `height` (whole number (px), required), `pixel_format` (choice, required) |  | Sends this picture from the start of the next field until the next one: a still picture, or video when the host gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and below or at the sides. |
| `teletext.select` | Select a teletext page | `row` (text, required) |  | Shows a teletext page. |

## Typed interface

The stack is in the family `analogtv`, version 1.1. The C header `sdrkit/analogtv.h`, the registry record `registry/analogtv.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `AtvFields`. The result fields of the `atv` receiver, which its status component writes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The state of the receiver. |
| `system` | text | optional | 1.0 | The system and its raster, "PAL B/G 625/50". Absent while the receiver knows none. |
| `colour_system` | choice | optional | 1.0 | The colour system in use. |
| `colour` | yes or no | always | 1.0 | The picture has colour now. |
| `colour_killer` | yes or no | always | 1.0 | The colour killer is on (the burst is too weak). |
| `lines` | whole number | always | 1.0 | Lines a frame. |
| `field_rate` | frequency | always | 1.0 | Fields a second. |
| `line_rate` | frequency | always | 1.0 | The line rate. |
| `line_error_ppm` | number (ppm) | always | 1.0 | The line rate against its nominal value. |
| `detector` | choice | always | 1.0 | The detector in use. |
| `modulation` | choice | always | 1.0 | The modulation the receiver runs. |
| `vision_carrier` | frequency | always | 1.0 | The vision carrier, from the centre of the input. |
| `sound_carrier` | frequency | optional | 1.0 | The sound carrier, from the centre of the input. Absent while none is found. |
| `sound_spacing` | frequency | always | 1.0 | The sound carrier above the vision carrier (negative: below it). |
| `offset_hz` | frequency | always | 1.0 | The offset of the vision carrier. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the vision carrier. |
| `carrier_to_noise_db` | number (dB) | always | 1.0 | The peak vision carrier against the noise in 5 MHz. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the picture. |
| `sound_present` | yes or no | always | 1.0 | A sound carrier is there. |
| `sound_deviation` | frequency | always | 1.0 | The peak deviation of the FM sound carrier. |
| `audio_level_dbfs` | number (dBFS) | always | 1.0 | The audio level. |
| `sound_system` | choice | always | 1.0 | The sound that plays. |
| `sync_found_pct` | number (%) | always | 1.0 | The share of lines whose sync was found. |
| `line_count` | whole number | always | 1.0 | Lines since the line loop started. |
| `field_count` | whole number | always | 1.0 | Field syncs found. |
| `pictures` | whole number | always | 1.0 | Pictures sent. |
| `snow_pictures` | whole number | always | 1.0 | Snow pictures sent. |
| `last_field` | whole number | always | 1.0 | The field completed last (1 or 2). |
| `fields_ok` | whole number | always | 1.0 | Fields decoded completely. |
| `fields_bad` | whole number | always | 1.0 | Fields damaged. |
| `sync_depth_pct` | number (%) | always | 1.0 | The sync depth, a share of the peak carrier. |
| `sync_compression_pct` | number (%) | always | 1.0 | The sync compression measured. |
| `white_peak` | number | always | 1.0 | The white peak (1 is the nominal white). |
| `burst_pct` | number (%) | always | 1.0 | The colour burst, a share of its nominal level. |
| `burst_phase_error_deg` | number (°) | always | 1.0 | The phase error of the burst. |
| `chroma_level_pct` | number (%) | always | 1.0 | The SECAM colour subcarrier level, a share of its nominal level. |
| `secam_identification` | text | optional | 1.0 | How SECAM was found. |
| `fm_polarity` | choice | optional | 1.0 | The polarity of FM video. |
| `nbtv_line_locked` | yes or no | always | 1.0 | The NBTV line loop is locked. |
| `nbtv_picture_locked` | yes or no | always | 1.0 | The NBTV picture is locked. |
| `nicam_locked` | yes or no | optional | 1.0 | The NICAM frame alignment holds. Absent without a NICAM carrier. |
| `nicam_valid` | yes or no | optional | 1.0 | The NICAM sound is valid and can play. |
| `nicam_mode` | choice | optional | 1.0 | The NICAM sound mode. |
| `nicam_parity_errors` | whole number | optional | 1.0 | NICAM sound words whose parity failed, since the lock. |
| `nicam_error_pct` | number (%) | optional | 1.0 | The share of NICAM sound words whose parity failed. |
| `a2_mode` | choice | optional | 1.0 | The A2 identification found. |

### Result fields

Struct `TeletextFields`. The result fields of the teletext decoder of the `atv` and `fpv` stacks. The counts have no value while no teletext line arrives.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `teletext_header` | text | optional | 1.0 | The header of the last teletext page. |
| `teletext_pages` | whole number | optional | 1.0 | Teletext pages held. |
| `teletext_packets` | whole number | optional | 1.0 | Teletext packets received. |
| `teletext_errors` | whole number | optional | 1.0 | Teletext lines whose framing code or address failed. |
| `teletext_parity_errors` | whole number | optional | 1.0 | Teletext characters whose parity failed. |
| `teletext_page` | whole number | always | 1.0 | The teletext page shown. |
| `page_text` | text | optional | 1.0 | The text of the teletext page shown, its rows joined with line feeds. |

### Result fields

Struct `TvTxFields`. The result fields of the senders of `atv` and `fpv`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.1 | Whether the sender sends. |
| `tx_system` | text | optional | 1.1 | The system sent, "PAL B/G 625/50". No value while nothing is sent. |
| `tx_frames` | whole number | always | 1.1 | The pictures of the host put on the air in the last transmission: one for each field that started with a new one. |
| `tx_fields` | whole number | always | 1.1 | The fields sent in the last transmission. |

### Table `channels`

Struct `AtvChannelRow`. A channel of the setting `channels`. The row key is the label.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `label` | text | always | 1.0 | The channel label. |
| `vision` | frequency | always | 1.0 | The vision carrier of the channel. |

### Table `teletext_pages`

Struct `TeletextPageRow`. A teletext page held. The row key is the page number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `page` | whole number | always | 1.0 | The page number. |
| `subpages` | whole number | always | 1.0 | The number of subpages held. |

### Table `teletext_subpages`

Struct `TeletextSubpageRow`. A subpage of a teletext page. The row key is `<page>/<subcode>`, the subcode as four hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `page_row` | text | always | 1.0 | The row key of the page in `teletext_pages`. |
| `subcode` | whole number | always | 1.0 | The subcode, in nibble form. |
| `version` | whole number | always | 1.0 | The version, raised on each change. |

### Event `atv.state`

Struct `AtvStateEvent`. A change of the state of the `atv` receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The new state. |
| `text` | text | always | 1.0 | The system of the receiver. |
| `family` | choice | always | 1.0 | What the event is about: the receiver. |

### Event `teletext.page`

Struct `TeletextPageEvent`. A teletext page that arrived.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `page` | whole number | always | 1.0 | The page number. |
| `subcode` | whole number | always | 1.0 | The subcode, in nibble form. |
| `version` | whole number | always | 1.0 | The version, raised on each change. |
| `text` | text | always | 1.0 | The text of the page, its 25 rows joined with line feeds. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `channel.tune` | `sdrkit_atv_channel_tune` | `sdrkit_atv_channel_args` | none |
| `start` | `sdrkit_atv_start` | none | none |
| `stop` | `sdrkit_atv_stop` | none | none |
| `frame` | `sdrkit_atv_frame` | `sdrkit_analogtv_tv_frame_args` | none |
| `teletext.select` | `sdrkit_atv_teletext_select` | `sdrkit_analogtv_teletext_select_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `modulation` | `sdrkit_atv_get_modulation` | `sdrkit_atv_set_modulation` |
| `channels` | `sdrkit_atv_get_channels` | `sdrkit_atv_set_channels` |
| `system` | `sdrkit_atv_get_system` | `sdrkit_atv_set_system` |
| `colour` | `sdrkit_atv_get_colour` | `sdrkit_atv_set_colour` |
| `colour_on` | `sdrkit_atv_get_colour_on` | `sdrkit_atv_set_colour_on` |
| `saturation` | `sdrkit_atv_get_saturation` | `sdrkit_atv_set_saturation` |
| `tint` | `sdrkit_atv_get_tint` | `sdrkit_atv_set_tint` |
| `detector` | `sdrkit_atv_get_detector` | `sdrkit_atv_set_detector` |
| `deinterlace` | `sdrkit_atv_get_deinterlace` | `sdrkit_atv_set_deinterlace` |
| `noise_reduction` | `sdrkit_atv_get_noise_reduction` | `sdrkit_atv_set_noise_reduction` |
| `black_setup` | `sdrkit_atv_get_black_setup` | `sdrkit_atv_set_black_setup` |
| `chroma_delay` | `sdrkit_atv_get_chroma_delay` | `sdrkit_atv_set_chroma_delay` |
| `channel_width` | `sdrkit_atv_get_channel_width` | `sdrkit_atv_set_channel_width` |
| `snow` | `sdrkit_atv_get_snow` | `sdrkit_atv_set_snow` |
| `sound_system` | `sdrkit_atv_get_sound_system` | `sdrkit_atv_set_sound_system` |
| `dual_sound` | `sdrkit_atv_get_dual_sound` | `sdrkit_atv_set_dual_sound` |
| `nbtv_lines` | `sdrkit_atv_get_nbtv_lines` | `sdrkit_atv_set_nbtv_lines` |
| `nbtv_rate` | `sdrkit_atv_get_nbtv_rate` | `sdrkit_atv_set_nbtv_rate` |
| `nbtv_deviation` | `sdrkit_atv_get_nbtv_deviation` | `sdrkit_atv_set_nbtv_deviation` |
| `nbtv_vertical` | `sdrkit_atv_get_nbtv_vertical` | `sdrkit_atv_set_nbtv_vertical` |
| `tx_system` | `sdrkit_atv_get_tx_system` | `sdrkit_atv_set_tx_system` |
| `tx_colour` | `sdrkit_atv_get_tx_colour` | `sdrkit_atv_set_tx_colour` |
| `tx_picture` | `sdrkit_atv_get_tx_picture` | `sdrkit_atv_set_tx_picture` |
| `tx_sound` | `sdrkit_atv_get_tx_sound` | `sdrkit_atv_set_tx_sound` |
| `tx_compression` | `sdrkit_atv_get_tx_compression` | `sdrkit_atv_set_tx_compression` |
| `tx_group_delay` | `sdrkit_atv_get_tx_group_delay` | `sdrkit_atv_set_tx_group_delay` |
| `tx_nyquist` | `sdrkit_atv_get_tx_nyquist` | `sdrkit_atv_set_tx_nyquist` |
| `tx_setup` | `sdrkit_atv_get_tx_setup` | `sdrkit_atv_set_tx_setup` |
| `tx_secam_id` | `sdrkit_atv_get_tx_secam_id` | `sdrkit_atv_set_tx_secam_id` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_system` | `sdrkit_atv_gen_get_system` | `sdrkit_atv_gen_set_system` |
| `gen_colour` | `sdrkit_atv_gen_get_colour` | `sdrkit_atv_gen_set_colour` |
| `gen_picture` | `sdrkit_atv_gen_get_picture` | `sdrkit_atv_gen_set_picture` |
| `gen_sound` | `sdrkit_atv_gen_get_sound` | `sdrkit_atv_gen_set_sound` |
| `gen_second` | `sdrkit_atv_gen_get_second` | `sdrkit_atv_gen_set_second` |
| `gen_compression` | `sdrkit_atv_gen_get_compression` | `sdrkit_atv_gen_set_compression` |
| `gen_group_delay` | `sdrkit_atv_gen_get_group_delay` | `sdrkit_atv_gen_set_group_delay` |
| `gen_nyquist` | `sdrkit_atv_gen_get_nyquist` | `sdrkit_atv_gen_set_nyquist` |
| `gen_setup` | `sdrkit_atv_gen_get_setup` | `sdrkit_atv_gen_set_setup` |
| `gen_secam_id` | `sdrkit_atv_gen_get_secam_id` | `sdrkit_atv_gen_set_secam_id` |
| `gen_ident` | `sdrkit_atv_gen_get_ident` | `sdrkit_atv_gen_set_ident` |
| `gen_teletext` | `sdrkit_atv_gen_get_teletext` | `sdrkit_atv_gen_set_teletext` |
| `gen_start` | `sdrkit_atv_gen_get_start` | `sdrkit_atv_gen_set_start` |
| `gen_nbtv_lines` | `sdrkit_atv_gen_get_nbtv_lines` | `sdrkit_atv_gen_set_nbtv_lines` |
| `gen_nbtv_rate` | `sdrkit_atv_gen_get_nbtv_rate` | `sdrkit_atv_gen_set_nbtv_rate` |
| `gen_nbtv_pattern` | `sdrkit_atv_gen_get_nbtv_pattern` | `sdrkit_atv_gen_set_nbtv_pattern` |

## Try it

```sh
sdrkit run atv --source synth:atv
sdrkit run atv --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/analogtv/atv_pal_i_nicam_10m.sigmf-meta`
- `tests/vectors/analogtv/atv_secam_dk_10m.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends analogue television continuously: the composite video of a picture that the host gives, or of the test
picture, with the host's sound, on the transmit sink. The engine setting `transmit_allowed` is the only switch: with it off,
`start` is refused and nothing is sent.

Broadcasting television needs a licence nearly everywhere. Amateur television is allowed to licensed amateurs by national
rules, for example on 70 cm (430 to 440 MHz in Region 1, vision at 434.25 MHz; 420 to 450 MHz in Region 2) and 23 cm (1240
to 1300 MHz); an amateur station shows its call sign in the picture or says it in the sound as its licence asks. These are
facts for the host and the plan data, not checks of the library.

### Commands

| Command | Arguments | What it does |
|---|---|---|
| `start` | none | Sends from the start of a field with a rise of 5 ms; while sending, it does nothing. The transmit settings apply at each start. |
| `stop` | none | Ends at the end of a field, with a fall of 5 ms; the last sample carries the end mark. |
| `frame` | `pixels`, `width`, `height`, `pixel_format` | Sets the picture sent from the start of the next field until the next `frame`. |

`frame` takes the picture as pixels, not as a file, as the SSTV `send` does:

| Argument | Kind | Meaning |
|---|---|---|
| `pixels` | bytes | The pixel bytes, rows top first, no padding between rows |
| `width` | integer | 1 to 8192 pixels |
| `height` | integer | 1 to 8192 pixels |
| `pixel_format` | choice | `rgb8` (3 bytes a pixel), `bgra` (4 bytes, alpha not used) or `grey8` (1 byte) |

The picture is fitted to the picture of the system (768 x 576 for 625 lines, 640 x 480 for 525, 504 x 376 for 405, 1024 x 768
for 819) with its shape kept: scaled by the smaller of the two size ratios and centred, with black bars above and below a wider
picture and at the sides of a narrower one. A larger picture is reduced by the area mean of the pixels each place covers; a
smaller one is enlarged by repeating pixels. A picture is taken at the start of each field: the last one given before it. A
still picture is given once; video is given a picture a field (50 or 59.94 a second) or a frame. A picture at the size of the
system is not scaled, which costs least. `frame` with width and height 0 and no pixels returns to the test picture. The picture
is kept for the next `start`.

The settings of the sender (section "Transmit"):

| Key | Meaning | Default |
|---|---|---|
| `tx_system` | The system, by the names of the receiver: B, G, I, D/K, M, N, A, C, E, H, K', L, L' | G |
| `tx_colour` | By system (NTSC on M, SECAM on L, L', K', none on A, C, E, PAL on the others), PAL, NTSC, SECAM, none | by system |
| `tx_picture` | What is sent while the host gives no picture: test card, colour bars, grey ramp | test card |
| `tx_sound` | The host's audio on the sound carrier; off: no sound carrier | on |
| `tx_compression` | Sync compression of the transmitter, 0 to 80 % | 0 |
| `tx_group_delay` | Chrominance 170 ns early (BT.470 group-delay pre-correction) | on |
| `tx_nyquist` | The modulator has the Nyquist slope | off |
| `tx_setup` | Black setup in 525 lines: 7.5 IRE or none | 7.5 IRE |
| `tx_secam_id` | SECAM identification: lead-in and field lines, lead-in only, field lines only | both |

A combination that never existed (colour on A, C or E; NTSC on a positive system) is refused by `start`.

### The signal

The sender is the modulator of the test signal, fed with the host's picture and sound; the two share every table. The
composite video is made line by line at 12 Msps (24 Msps for E) on the half-line grid of ITU-R BT.470: line and field
syncs, equalising and broad pulses, blanking with raised-cosine edges of the system's build-up time, the colour burst and
PAL, NTSC or SECAM chrominance, interlaced fields. The luminance is limited to the video band of the system, the colour
differences to about 1.3 MHz.

The carrier amplitude follows the system: negative modulation with the sync tip at the peak and peak white at 12.5 % of it
(20 % in I), or positive modulation with the sync tip at 0, blanking at 30 % (25 % in E) and white at 100 %. The vestigial
sideband filter keeps the main sideband to the video band and the vestige to its width (0.75 or 1.25 MHz; the main sideband
below the carrier in A and L'), the lower sideband more than 20 dB down beyond it. The vision carrier sits 1.25 MHz above the
lower edge of the channel (−2.75 MHz from the centre of an 8 MHz channel); the channel is centred on the dial.

The sound is mono: the host's audio, at any rate, on the sound carrier at the spacing of the system. FM sound has the
pre-emphasis of the system (50 or 75 µs) and its deviation (50 kHz, 25 kHz in M and N) at full scale, limited to full
deviation after the pre-emphasis; AM sound (A, C, E, L, L') has a depth of 0.8 at full scale. The sound carrier is 13 dB
(10 dB in I, M, N and the positive systems) below the peak vision carrier. While the host gives no audio, the carrier is
unmodulated. SECAM on M and N has no sound carrier.

Level: the sum of the peaks of the vision carrier and of the sound carrier is 0.9 of full scale. In B/G the sync tip is at
0.718 and the sound carrier at 0.161; the measured peak is 0.885, with no sample at the bound. The vision peak includes 3 % for the overshoot of the vestigial-sideband filter at the sync edges.

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at the session's rate, which
must be at least the channel width (8 Msps for an 8 MHz channel, 14 Msps for E; 10 Msps is usual). On a live radio whose host
gives a transmit lead, the first sample states its time on air, as every sender of the library does. A change of format
(a retune) and transmit going off end the transmission at once, with the end mark on the next pass. While the sender is
keyed, the receive audio of the stack is muted.

Result fields: `tx_sending`, `tx_system` (the name of the system sent, "PAL B/G 625/50"), `tx_frames` (the pictures of the
host put on the air: one for each field that started with a new one) and `tx_fields` (the fields sent).

### Checks

The tests send the test card of the generator as a picture of the host through the stack and read it back with the stack's
own receiver, with the host's 1 kHz tone at half of full scale:

| System | Bars error (0 to 1) | Luma difference (0 to 255) | Tone (0.5 sent) |
|---|---|---|---|
| PAL B/G | 0.004 | 3.4 | 0.500 |
| PAL I | 0.017 | 3.9 | 0.500 |
| NTSC M | 0.012 | 3.3 | 0.501 |
| SECAM L (AM sound) | 0.022 | 4.2 | 0.500 |
| SECAM D/K | 0.015 | 3.5 | 0.500 |
| A, 405 lines, mono | 0.002 (luminance) | 2.4 | 0.500 |

The bars error is the worst channel difference of the mean colour of the middle of each EBU bar; the luma difference is the
mean absolute luminance difference to the picture sent. Each system is named by the receiver as sent ("PAL B/G 625/50",
"SECAM L 625/50", "A 405/50 mono").

A picture given by `frame` while sending is in the received picture within two frames (80 ms at 625 lines, 35 ms at 525, 85 ms at 405, read in pieces of 5 ms). On the samples, against the
numbers of the receive specification: line period 64.0000 and 63.5556 µs (single periods within 0.003 and 0.008 µs), line sync, equalising and broad pulse widths within 0.014 µs of 4.7, 2.35 (2.3) and 27.3 (27.1) µs, five (six) equalising, broad and equalising pulses a field, fields half a frame apart within 0.01 µs, the active picture from 10.50 (9.40) µs and 51.95 (52.66) µs long. The burst of PAL B/G: from 5.60 µs after the line sync, 10.0 cycles, 0.215 of the span from blanking to white (3/14 is 0.214), the phase turning by 90° from line to line within 0.1°, the frequency within 0.01 Hz of 4 433 618.75 Hz. The sound carrier of B/G: 0.2 Hz from 5.5 MHz above the vision carrier, 26.20 kHz deviation for 26.21 kHz wanted (0.5 x 50 kHz x the 50 µs pre-emphasis at 1 kHz), 13.1 dB below the sync tip. The samples do not
depend on the size of the passes, `stop` ends at the end of a field, and with transmit off nothing is sent.

Cost: one core of this 8-core machine makes 10 Msps of PAL B/G with a new picture each field and sound in 0.58 of real time (bench `analogtv.atv_tx_10m`, load average 12 from other builds). The stack starts no thread of its own.

### Open points

- Mono only: the shared microphone input gives one channel, and the A2 and NICAM second sounds of the generator carry test
  tones, not the host's audio. Stereo needs a two-channel host audio input.
- Wide screen signalling (16:9 on line 23) is not sent: its timing is not settled and the receiver does not read it. Pictures
  go out as 4:3.
- The ident text, teletext, the time-out, the level trim and the mirror of the specification's list are not settings of the
  sender.
- Nothing was sent on a real radio.
