<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FPV video (analogue)

Analogue FPV video transmitters on the 5.8 GHz and 1.2 GHz channels: a channel table, the picture and its sound. Transmits a picture and the sound that the host gives.

Stack id: `fpv`. Group: Video.

Signal types: FM video.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 900 MHz to 6 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 16 MHz |
| Signal bandwidth | 18 MHz |
| Radio filter | 18 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fpv` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `system` | System | choice | `auto` | `auto` (Automatic), `pal` (PAL (625)), `ntsc` (NTSC (525)) |  |
| `channels` | Channels | text | empty |  | The channels: entries separated by ";", each a label, "=" and the frequency in Hz, for example "E21=474000000" or "R5=5806000000". At most 256 entries for atv and 64 for fpv. The host fills it from a channel plan file. The table channels lists them and channel.tune takes a label. Empty: no channel. |
| `sound_subcarrier` | Sound subcarrier | choice | `auto` | `auto` (Automatic), `6.0` (6.0 MHz), `6.5` (6.5 MHz) |  |
| `colour_on` | Colour | yes or no | true |  |  |
| `saturation` | Saturation | number | 1 | 0 to 2 |  |
| `deinterlace` | Fields | choice | `weave` | `weave` (Weave), `bob` (Bob) |  |
| `audio` | Audio | yes or no | true |  |  |
| `tx_system` | System | choice | `pal` | `pal` (PAL (625)), `ntsc` (NTSC (525)) |  |
| `tx_picture` | Test picture | choice | `card` | `card` (Test card), `bars` (Colour bars) | What is sent until the host gives a picture with the command frame. |
| `tx_sound` | Sound | yes or no | true |  | On: the host's audio (mono) on the sound carrier, an unmodulated carrier while it gives none. Off: no sound carrier. |
| `tx_subcarrier` | Sound subcarrier | choice | `6.5` | `6.5` (6.5 MHz), `6.0` (6.0 MHz) | The subcarrier of the sound in the composite: FM, 25 kHz deviation at full scale. |
| `tx_deviation_mhz` | Deviation | number (MHz) | 8 | 1 to 12 | The swing from the middle to the sync tip and to peak white. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tx_sync_high` | Sync tip at the highest frequency | yes or no | false |  |  |
| `tx_ac_ms` | AC coupling time constant | number (ms) | 0 | 0 to 100 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_system` | System | choice | `pal` | `pal` (PAL (625)), `ntsc` (NTSC (525)) |  |
| `gen_picture` | Picture | choice | `card` | `card` (Test card), `bars` (Colour bars) |  |
| `gen_sound` | Sound | choice | `6.5` | `6.5` (Subcarrier 6.5 MHz), `6.0` (6.0 MHz), `none` (None) |  |
| `gen_none` | No transmitter | yes or no | false |  |  |
| `gen_centre_mhz` | Capture centre from the transmitter | number (MHz) | 0 | -10 to 10 |  |
| `gen_deviation_mhz` | Deviation | number (MHz) | 8 | 1 to 12 |  |
| `gen_sync_high` | Sync tip at the highest frequency | yes or no | false |  |  |
| `gen_ac_ms` | AC coupling time constant | number (ms) | 0 | 0 to 100 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `channel` | Channel | text |
| `level_dbfs` | Level | number (dBFS) |
| `over_noise_db` | Over the noise | number (dB) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `system` | System | text |
| `colour_system` | Colour system | choice |
| `lines` | Lines | whole number |
| `field_rate` | Field rate | frequency |
| `sync_found_pct` | Sync found | number (%) |
| `pictures` | Pictures | whole number |
| `fields_ok` | Fields good | whole number |
| `fields_bad` | Fields damaged | whole number |
| `polarity` | Polarity | choice |
| `snr_db` | SNR | number (dB) |
| `sound_present` | Sound carrier | yes or no |
| `sound_subcarrier` | Sound subcarrier | frequency |
| `subcarrier_60_db` | 6.0 MHz | number (dB) |
| `subcarrier_65_db` | 6.5 MHz | number (dB) |
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
| `channels` | Channels | `name` (text), `band` (text), `number` (whole number), `freq` (frequency), `level_dbfs` (number (dBFS)), `over_noise_db` (number (dB)), `present` (yes or no), `inside` (yes or no), `heard` (duration (s)) |
| `teletext_pages` | Teletext pages | `page` (whole number), `subpages` (whole number) |
| `teletext_subpages` | Teletext subpages | `page_row` (text), `subcode` (whole number), `version` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Spectrum | MHz | dB |
| `history` | Level history | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fpv.state` | State | `state` (choice), `text` (text), `family` (choice) |
| `teletext.page` | Teletext page | `page` (whole number), `subcode` (whole number), `version` (whole number), `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear the channel levels |  |  | Forgets the channel levels, the noise floor and the level history. |
| `channel.tune` | Tune to a channel | `row` (text, required) |  | Asks the host to tune to an FPV channel. |
| `scan` | Scan | `plan` (choice, required) |  | Asks the host to visit every channel of a plan for 0.7 s each. |
| `teletext.select` | Select a teletext page | `row` (text, required) |  | Shows a teletext page. |
| `start` | Start sending |  |  | Sends the television signal continuously from the start of a field: the picture of the command frame, or the test picture, and the host's audio. The transmit settings apply at each start. |
| `stop` | Stop sending |  |  | Ends the transmission at the end of a field, with a fade of 5 ms. |
| `frame` | Set the picture | `pixels` (bytes, required), `width` (whole number (px), required), `height` (whole number (px), required), `pixel_format` (choice, required) |  | Sends this picture from the start of the next field until the next one: a still picture, or video when the host gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and below or at the sides. |

## Typed interface

The stack is in the family `analogtv`, version 2.0. The C header `sdrkit/analogtv.h`, the registry record `registry/analogtv.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Television and video (`television`). Media: sound (programme), video.

### Result fields

Struct `FpvFields`. The result fields of the `fpv` receiver, which its display writes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The state of the receiver. |
| `channel` | text | optional | 1.0 | The names of the channels at the tuned frequency. Absent off every channel. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel. |
| `over_noise_db` | number (dB) | always | 1.0 | The level of the channel above the noise floor. |
| `noise_dbfs` | number (dBFS) | always | 1.0 | The noise floor. |
| `offset_hz` | frequency | always | 1.0 | The offset of the transmitter from the channel. |
| `system` | text | optional | 1.0 | The system and its raster. Absent while the receiver knows none. |
| `colour_system` | choice | optional | 1.0 | The colour system in use. |
| `lines` | whole number | always | 1.0 | Lines a frame. |
| `field_rate` | frequency | always | 1.0 | Fields a second. |
| `sync_found_pct` | number (%) | always | 1.0 | The share of lines whose sync was found. |
| `pictures` | whole number | always | 1.0 | Pictures sent. |
| `fields_ok` | whole number | always | 1.0 | Fields decoded completely. |
| `fields_bad` | whole number | always | 1.0 | Fields damaged. |
| `polarity` | choice | always | 1.0 | The polarity of the FM video. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the picture. |
| `sound_present` | yes or no | always | 1.0 | A sound subcarrier is there. |
| `sound_subcarrier` | frequency | optional | 1.0 | The sound subcarrier, from the vision carrier. Absent while none is found. |
| `subcarrier_60_db` | number (dB) | always | 1.0 | The 6.0 MHz subcarrier against the noise beside it. |
| `subcarrier_65_db` | number (dB) | always | 1.0 | The 6.5 MHz subcarrier against the noise beside it. |

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
| `tx_sending` | yes or no | always | 1.0 | Whether the sender sends. |
| `tx_system` | text | optional | 1.0 | The system sent, "PAL B/G 625/50". No value while nothing is sent. |
| `tx_frames` | whole number | always | 1.0 | The pictures of the host put on the air in the last transmission: one for each field that started with a new one. |
| `tx_fields` | whole number | always | 1.0 | The fields sent in the last transmission. |

### Table `channels`

Struct `FpvChannelRow`. A channel of the FPV meter. The row key is the channel name ("R5").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the channel ("R5"). |
| `band` | text | always | 1.0 | The band letter. |
| `number` | whole number | always | 1.0 | The number of the channel in its band. |
| `freq` | frequency | always | 1.0 | The frequency of the channel. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel. |
| `over_noise_db` | number (dB) | always | 1.0 | The level of the channel above the noise floor. |
| `present` | yes or no | always | 1.0 | A transmitter is on the channel. |
| `inside` | yes or no | always | 1.0 | The channel is inside the capture. |
| `heard` | duration (s) | optional | 1.0 | The stream time at which a transmitter was last heard on the channel. |

The column `name` names a row.

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

### Event `fpv.state`

Struct `FpvStateEvent`. A change of the state of the `fpv` receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The new state. |
| `text` | text | always | 1.0 | The names of the channels at the tuned frequency. |
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
| `clear` | `sdrkit_fpv_clear` | none | none |
| `channel.tune` | `sdrkit_fpv_channel_tune` | `sdrkit_fpv_channel_args` | none |
| `scan` | `sdrkit_fpv_scan` | `sdrkit_fpv_scan_args` | none |
| `teletext.select` | `sdrkit_fpv_teletext_select` | `sdrkit_analogtv_teletext_select_args` | none |
| `start` | `sdrkit_fpv_start` | none | none |
| `stop` | `sdrkit_fpv_stop` | none | none |
| `frame` | `sdrkit_fpv_frame` | `sdrkit_analogtv_tv_frame_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `system` | `sdrkit_fpv_get_system` | `sdrkit_fpv_set_system` |
| `channels` | `sdrkit_fpv_get_channels` | `sdrkit_fpv_set_channels` |
| `sound_subcarrier` | `sdrkit_fpv_get_sound_subcarrier` | `sdrkit_fpv_set_sound_subcarrier` |
| `colour_on` | `sdrkit_fpv_get_colour_on` | `sdrkit_fpv_set_colour_on` |
| `saturation` | `sdrkit_fpv_get_saturation` | `sdrkit_fpv_set_saturation` |
| `deinterlace` | `sdrkit_fpv_get_deinterlace` | `sdrkit_fpv_set_deinterlace` |
| `audio` | `sdrkit_fpv_get_audio` | `sdrkit_fpv_set_audio` |
| `tx_system` | `sdrkit_fpv_get_tx_system` | `sdrkit_fpv_set_tx_system` |
| `tx_picture` | `sdrkit_fpv_get_tx_picture` | `sdrkit_fpv_set_tx_picture` |
| `tx_sound` | `sdrkit_fpv_get_tx_sound` | `sdrkit_fpv_set_tx_sound` |
| `tx_subcarrier` | `sdrkit_fpv_get_tx_subcarrier` | `sdrkit_fpv_set_tx_subcarrier` |
| `tx_deviation_mhz` | `sdrkit_fpv_get_tx_deviation_mhz` | `sdrkit_fpv_set_tx_deviation_mhz` |
| `tx_sync_high` | `sdrkit_fpv_get_tx_sync_high` | `sdrkit_fpv_set_tx_sync_high` |
| `tx_ac_ms` | `sdrkit_fpv_get_tx_ac_ms` | `sdrkit_fpv_set_tx_ac_ms` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_system` | `sdrkit_fpv_gen_get_system` | `sdrkit_fpv_gen_set_system` |
| `gen_picture` | `sdrkit_fpv_gen_get_picture` | `sdrkit_fpv_gen_set_picture` |
| `gen_sound` | `sdrkit_fpv_gen_get_sound` | `sdrkit_fpv_gen_set_sound` |
| `gen_none` | `sdrkit_fpv_gen_get_none` | `sdrkit_fpv_gen_set_none` |
| `gen_centre_mhz` | `sdrkit_fpv_gen_get_centre_mhz` | `sdrkit_fpv_gen_set_centre_mhz` |
| `gen_deviation_mhz` | `sdrkit_fpv_gen_get_deviation_mhz` | `sdrkit_fpv_gen_set_deviation_mhz` |
| `gen_sync_high` | `sdrkit_fpv_gen_get_sync_high` | `sdrkit_fpv_gen_set_sync_high` |
| `gen_ac_ms` | `sdrkit_fpv_gen_get_ac_ms` | `sdrkit_fpv_gen_set_ac_ms` |

## Try it

```sh
sdrkit run fpv --source synth:fpv
sdrkit run fpv --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends an analogue FPV video signal continuously: the composite video of a picture that the host gives, or of the
test picture, with the host's sound on a subcarrier, frequency-modulating the carrier. The engine setting
`transmit_allowed` is the only switch: with it off, `start` is refused and nothing is sent.

In most countries 5725 to 5875 MHz is open without a licence at a low power (for example 25 mW e.i.r.p. for video in the
European Union); more power, and the channels outside that band, need an amateur licence (amateur allocation 5650 to 5925
MHz). These are facts for the host and the plan data, not checks of the library.

### Commands

`start`, `stop` and `frame` work as in `atv` (see its page): `frame` takes pixels, width, height and a pixel format, the
picture is fitted to 768 x 576 (PAL) or 640 x 480 (NTSC) with its shape kept and taken at the start of each field; `start`
begins at the start of a field with a rise of 5 ms; `stop` ends at the end of a field with a fall of 5 ms and the end mark.

The settings of the sender (section "Transmit"):

| Key | Meaning | Default |
|---|---|---|
| `tx_system` | PAL (625 lines) or NTSC (525 lines) | PAL |
| `tx_picture` | What is sent while the host gives no picture: test card or colour bars | test card |
| `tx_sound` | The host's audio on the sound subcarrier; off: no subcarrier | on |
| `tx_subcarrier` | The sound subcarrier: 6.5 or 6.0 MHz | 6.5 MHz |
| `tx_deviation_mhz` | The swing from the carrier to the sync tip and to peak white, 1 to 12 MHz | 8 |
| `tx_sync_high` | The sync tip at the highest frequency | off |
| `tx_ac_ms` | AC coupling time constant of the video, ms (0: DC coupled) | 0 |

The session's rate must be at least 16 Msps; 20 Msps is usual.

### The signal

The composite video is that of `atv` (ITU-R BT.470 and BT.1700; PAL B/G or NTSC M timing and colour), brought to the session's
rate. The sound subcarrier is added to it: FM with 25 kHz deviation at full scale of the host's audio (mono), at 0.12 of the
video swing. The sum frequency-modulates the carrier: the sync tip at minus the deviation, peak white at plus it (on R1 at
5658 MHz with 8 MHz: the sync tip at 5650 MHz, white at 5666 MHz). The envelope is constant at 0.9 of full scale. The carrier
is on the dial.

Result fields: `tx_sending`, `tx_system`, `tx_frames`, `tx_fields`, as in `atv`.

### Checks

The test sends the test card of the generator as a picture of the host on R5 at 20 Msps with the host's 1 kHz tone at half
of full scale; the stack's own receiver locks to 625 lines, names "PAL B/G 625/50" with PAL colour, finds the sound on 6.5 MHz
and reads: bars error 0.006, luma difference 4.0, tone 0.250 (12.5 kHz deviation against the receiver's 50 kHz full scale). A
picture given by `frame` while sending is in the received picture after 80 ms. On the samples of a white picture without
sound, the frequency of the sync tip is −8.000 MHz and of white +8.013 MHz from the carrier. The envelope is 0.9.

### Open points

- Mono only, on one subcarrier.
- The processor share of the sender has no benchmark of its own; it shares the composite video of `atv` and adds one
  FM oscillator at 20 Msps.
- Nothing was sent on a real radio.
