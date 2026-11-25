<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ACARS

Aircraft text messages on VHF: every ACARS channel in the capture at once, with positions and an aircraft list.

Stack id: `acars`. Group: Aviation.

Signal types: ACARS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 118 MHz to 137 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 2 MHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:acars` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | text | empty |  | Frequencies in MHz separated by spaces. Empty watches every carrier on the 25 kHz grid. An 8.33 kHz channel such as 131.5167 needs the list. The host fills it from a channel file (data/avdata/datalink_channels.csv). |
| `threshold_db` | Threshold | number (dB) | 8 | 3 to 30 | Carrier over the noise needed to start decoding a channel. |
| `join_blocks` | Join blocks | yes or no | true |  | Joins the blocks of a long message into one message. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_aircraft` | Test aircraft | whole number | 6 | 1 to 24 | Aircraft in the test signal. |
| `gen_rate_factor` | Test traffic | number | 1 | 0.5 to 4 | Scales how often the test aircraft send. |
| `gen_positions` | Test positions | yes or no | false |  | The first six test aircraft also send position reports. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `blocks_repaired` | Blocks repaired | whole number |
| `frames_started` | Frames started | whole number |
| `positions` | Positions | whole number |
| `channels_active` | Channels active | whole number |
| `aircraft` | Aircraft | whole number |
| `messages` | Messages | whole number |
| `good_pct` | Good blocks | number (%) |
| `watching` | Watching | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `time` (time), `signal_s` (number (s)), `frequency` (frequency), `level_db` (number (dB)), `mode` (text), `registration` (text), `aircraft` (text), `flight` (text), `label` (text), `label_meaning` (text), `block_id` (text), `msn` (text), `downlink` (yes or no), `ack` (text), `text` (text), `reading` (text), `reading_kind` (text), `reading_category` (text), `cpdlc_imi` (text), `cpdlc_ground` (text), `cpdlc_aircraft` (text), `cpdlc_crc_ok` (yes or no), `cpdlc_parsed` (yes or no), `cpdlc_text` (text), `blocks` (whole number), `bits_fixed` (whole number), `position` (position), `alt_ft` (number (ft)) |
| `aircraft` | Aircraft | `address` (text), `registration` (text), `country` (text), `flight` (text), `label` (text), `messages` (whole number), `position` (position), `alt_ft` (number (ft)), `track_deg` (number (°)), `gs_kt` (number (kt)), `vs_fpm` (number (ft/min)), `last_heard` (time), `frequency` (frequency), `level_db` (number (dB)) |
| `channels` | Channels | `frequency` (frequency), `level_db` (number (dB)), `snr_db` (number (dB)), `offset_hz` (frequency), `active` (yes or no), `good` (whole number), `bad` (whole number), `last_heard` (time) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `acars.message` | ACARS message | `source` (text), `target` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `flight` (text), `label` (text), `downlink` (yes or no), `msn` (text), `position` (position), `alt_ft` (number (ft)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `avdata`, version 2.0. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: text.

### Result fields

Struct `AcarsFields`. The result fields of ACARS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Good blocks. |
| `frames_bad` | whole number | always | 1.0 | Bad blocks. |
| `blocks_repaired` | whole number | always | 1.0 | Good blocks that needed a repair. |
| `frames_started` | whole number | always | 1.0 | Frame starts (SYN SYN SOH) seen. |
| `positions` | whole number | always | 1.0 | Messages with a position. |
| `channels_active` | whole number | always | 1.0 | Channels being decoded now. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the table. |
| `messages` | whole number | always | 1.0 | Messages in the list. |
| `good_pct` | number (%) | optional | 1.0 | Good blocks, percent of all blocks. No value before a block. |
| `watching` | text | always | 1.0 | What is watched: the channel list, or the 25 kHz grid. |

### Table `messages`

Struct `AcarsMessageRow`. A row of the messages. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The wall-clock time. No value for a recording. |
| `signal_s` | number (s) | always | 1.0 | The signal time. |
| `frequency` | frequency | always | 1.0 | The channel. |
| `level_db` | number (dB) | always | 1.0 | The level of the carrier. |
| `mode` | text | optional | 1.0 | The mode character. |
| `registration` | text | optional | 1.0 | The registration. |
| `aircraft` | text | optional | 1.0 | The row of the aircraft in the aircraft table. |
| `flight` | text | optional | 1.0 | The flight id of a downlink. |
| `label` | text | optional | 1.0 | The label. |
| `label_meaning` | text | optional | 1.0 | The meaning of the label. |
| `block_id` | text | optional | 1.0 | The block id of the first block. |
| `msn` | text | optional | 1.0 | The message sequence number of a downlink. |
| `downlink` | yes or no | always | 1.0 | Aircraft to ground. |
| `ack` | text | optional | 1.0 | The technical acknowledgement: `NAK`, or the block id acknowledged. |
| `text` | text | optional | 1.0 | The printable text. |
| `reading` | text | optional | 1.0 | The reading of the fixed format. |
| `reading_kind` | text | optional | 1.0 | The kind of the reading. |
| `reading_category` | text | optional | 1.0 | The category of the reading. |
| `cpdlc_imi` | text | optional | 1.0 | The CPDLC envelope: AT1, CR1, CC1 or DR1. |
| `cpdlc_ground` | text | optional | 1.0 | The CPDLC ground facility. |
| `cpdlc_aircraft` | text | optional | 1.0 | The aircraft named in the CPDLC envelope. |
| `cpdlc_crc_ok` | yes or no | optional | 1.0 | The CPDLC envelope check passed. |
| `cpdlc_parsed` | yes or no | optional | 1.0 | The CPDLC message body decoded completely. |
| `cpdlc_text` | text | optional | 1.0 | The decoded CPDLC message. |
| `blocks` | whole number | always | 1.0 | Blocks joined. |
| `bits_fixed` | whole number | always | 1.0 | Bits the repair changed. |
| `position` | position | optional | 1.0 | The position the message gives. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude the message gives. |

The rows are the messages of the stack. The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`. Sender: `registration`. Text: `text`. Time: `time`.

### Table `aircraft`

Struct `AcarsAircraftRow`. A row of the aircraft. The row key is the registration.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | optional | 1.0 | The 24-bit address, six hex digits. |
| `registration` | text | optional | 1.0 | The registration. |
| `country` | text | optional | 1.0 | The State the address is allocated to. |
| `flight` | text | optional | 1.0 | The flight id. |
| `label` | text | optional | 1.0 | The label of the last message. |
| `messages` | whole number | always | 1.0 | Messages of the aircraft. |
| `position` | position | optional | 1.0 | The last position. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude of the last position. |
| `track_deg` | number (°) | optional | 1.0 | The track. |
| `gs_kt` | number (kt) | optional | 1.0 | The ground speed. |
| `vs_fpm` | number (ft/min) | optional | 1.0 | The vertical rate. |
| `last_heard` | time | optional | 1.0 | When the aircraft was last heard. |
| `frequency` | frequency | always | 1.0 | The channel of the last message. |
| `level_db` | number (dB) | always | 1.0 | The level of the last downlink. |

The rows are the stations or nodes of the stack. The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`. Time: `last_heard`.

### Table `channels`

Struct `AcarsChannelRow`. A row of the channels. The row key is the frequency in hertz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The channel. |
| `level_db` | number (dB) | optional | 1.0 | The level now. No value without a carrier. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio in 7 kHz. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `active` | yes or no | always | 1.0 | A decoder is on the channel. |
| `good` | whole number | always | 1.0 | Good blocks. |
| `bad` | whole number | always | 1.0 | Bad blocks. |
| `last_heard` | time | optional | 1.0 | The wall-clock time of the last good block, when known. |

### Event `acars.message`

Struct `AcarsMessageEvent`. An ACARS message. The source is the registration of the aircraft, as the link names it. The link does not name the ground station, so the target has no value.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The registration. |
| `target` | text | optional | 1.0 | The receiver. No value: the link does not name it. |
| `text` | text | always | 1.0 | The text. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. No value: the stack does not measure it per message. |
| `family` | choice | always | 1.0 | What the event is about: an aircraft. |
| `flight` | text | always | 1.0 | The flight id of a downlink. |
| `label` | text | always | 1.0 | The label. |
| `downlink` | yes or no | always | 1.0 | Aircraft to ground. |
| `msn` | text | always | 1.0 | The message sequence number. |
| `position` | position | optional | 1.0 | The position the message gives. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude the message gives. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_acars_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_acars_get_channels` | `sdrkit_acars_set_channels` |
| `threshold_db` | `sdrkit_acars_get_threshold_db` | `sdrkit_acars_set_threshold_db` |
| `join_blocks` | `sdrkit_acars_get_join_blocks` | `sdrkit_acars_set_join_blocks` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_aircraft` | `sdrkit_acars_gen_get_aircraft` | `sdrkit_acars_gen_set_aircraft` |
| `gen_rate_factor` | `sdrkit_acars_gen_get_rate_factor` | `sdrkit_acars_gen_set_rate_factor` |
| `gen_positions` | `sdrkit_acars_gen_get_positions` | `sdrkit_acars_gen_set_positions` |

## Try it

```sh
sdrkit run acars --source synth:acars
sdrkit run acars --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
