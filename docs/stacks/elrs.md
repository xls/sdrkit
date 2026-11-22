<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ExpressLRS

ExpressLRS radio-control links on the hop channels of one capture: link id, hop index and nonce from sync frames, frame counts for each link, and the stick values of your own link from its binding phrase. Experimental: the frame layout, the CRC start value, the rate rows and the hop plans are not confirmed by a public document.

Stack id: `elrs`. Group: IoT and mesh.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 137 MHz to 2.5 GHz |
| Sample rate | 3 MHz |
| Smallest sample rate | 1.1 MHz |
| Signal bandwidth | 2.3 MHz |
| Radio filter | 2.6 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:elrs` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `hop_channels` | Hop channels | whole number | 4 | 1 to 80 | The count of channels of the hop plan of your area. The tuned frequency is the middle of the plan. The default is the plan of the test signal. |
| `hop_step_hz` | Hop channel spacing | frequency | 600 kHz | 100 kHz to 2 MHz | The distance between two hop channels. The default is the plan of the test signal. |
| `packet_rate` | Packet rate | choice | `100` | `all` (All rates), `200` (200 Hz), `100` (100 Hz), `50` (50 Hz), `25` (25 Hz), `100full` (100 Hz Full) | The packet rate of the links. All rates needs five times the processor time. The default is the rate of the test signal. |
| `binding_phrase` | Binding phrase (your own link) | text | empty |  | Secret. With it the stick values of your own link are shown. The stack keeps it in memory only: it is not in the saved state, the log or the results. Empty forgets it. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_phrase` | Test signal binding phrase | text | `sdrkit test` |  |  |
| `gen_row` | Test signal rate row | whole number | 2 | 1 to 5 |  |
| `gen_inverted_iq` | Test signal with inverted IQ | yes or no | false |  |  |
| `gen_second_link` | Test signal with a second link without sync frames | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `preambles` | Preambles | whole number |
| `phrase_set` | Binding phrase set | yes or no |
| `hop_channels_inside` | Hop channels inside the capture | whole number |
| `hop_channels` | Hop channels | whole number |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `links_heard` | Links | whole number |
| `note` | Note | text |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `link` (text), `kind` (choice), `content` (text), `freq_hz` (frequency), `hop_channel` (whole number), `sf` (whole number), `bytes` (whole number), `inverted` (yes or no), `snr_db` (number (dB)), `encrypted` (yes or no), `hex` (text) |
| `links` | Links | `source` (text), `kind` (choice), `content` (text), `frames` (whole number), `sync_frames` (whole number), `rate` (text), `hop_index` (whole number), `heard_on` (text), `first_heard` (time), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `elrs.link` | ExpressLRS link | `source` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `lora`, version 3.2. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `ElrsFields`. The result fields of the ExpressLRS stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames whose CRC fits a link. |
| `frames_bad` | whole number | always | 1.0 | Frames whose CRC fits no link. |
| `preambles` | whole number | always | 1.0 | Preambles whose sync word followed. |
| `phrase_set` | yes or no | always | 1.0 | The binding phrase of the user's own link is set. |
| `hop_channels_inside` | whole number | always | 1.0 | The hop channels inside the capture. |
| `hop_channels` | whole number | always | 1.0 | The hop channels of the plan. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last frame, while frames come. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the bank reports. |
| `links_heard` | whole number | always | 1.0 | The links in the table. |
| `note` | text | always | 1.0 | What the decoder cannot confirm, and what it needs. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `frames`

Struct `ElrsFrameRow`. A row of the frame log. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the frame, when known. |
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `link` | text | always | 1.0 | The link of the frame. |
| `kind` | choice | always | 1.0 | What the frame is. |
| `content` | text | always | 1.0 | The frame in one line. |
| `freq_hz` | frequency | always | 1.0 | The hop channel plus the carrier offset. |
| `hop_channel` | whole number | always | 1.0 | The index of the hop channel. |
| `sf` | whole number | always | 1.0 | The spreading factor. |
| `bytes` | whole number | always | 1.0 | The length of the frame. |
| `inverted` | yes or no | always | 1.0 | The IQ was inverted. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |
| `hex` | text | always | 1.0 | The frame bytes as hex. |

The column `link` names a row.

### Table `links`

Struct `ElrsLinkRow`. A row of the links. The row key and `source` are the link, 4 hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The link. |
| `kind` | choice | always | 1.0 | What the last frame of the link was. |
| `content` | text | always | 1.0 | The last frame of the link in one line. |
| `frames` | whole number | always | 1.0 | The frames of the link. |
| `sync_frames` | whole number | always | 1.0 | The sync frames of the link. |
| `rate` | text | always | 1.0 | The rate of the last frame. |
| `hop_index` | whole number | optional | 1.0 | The hop index of the last sync frame, once one came. |
| `heard_on` | text | always | 1.0 | The hop channels on which the link was heard, in rising order. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The column `source` names a row.

### Event `elrs.link`

Struct `ElrsLinkEvent`. The fields of a link heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The link, 4 hex digits. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_elrs_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `hop_channels` | `sdrkit_elrs_get_hop_channels` | `sdrkit_elrs_set_hop_channels` |
| `hop_step_hz` | `sdrkit_elrs_get_hop_step_hz` | `sdrkit_elrs_set_hop_step_hz` |
| `packet_rate` | `sdrkit_elrs_get_packet_rate` | `sdrkit_elrs_set_packet_rate` |
| `binding_phrase` | `sdrkit_elrs_get_binding_phrase` | `sdrkit_elrs_set_binding_phrase` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_phrase` | `sdrkit_elrs_gen_get_phrase` | `sdrkit_elrs_gen_set_phrase` |
| `gen_row` | `sdrkit_elrs_gen_get_row` | `sdrkit_elrs_gen_set_row` |
| `gen_inverted_iq` | `sdrkit_elrs_gen_get_inverted_iq` | `sdrkit_elrs_gen_set_inverted_iq` |
| `gen_second_link` | `sdrkit_elrs_gen_get_second_link` | `sdrkit_elrs_gen_set_second_link` |

## Try it

```sh
sdrkit run elrs --source synth:elrs
sdrkit run elrs --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
