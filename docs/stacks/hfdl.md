<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HFDL

The HF data link of aircraft over oceans and remote areas: squitters, logons, positions and ACARS from ground stations worldwide.

Stack id: `hfdl`. Group: Aviation.

Signal types: HFDL.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 8.9 MHz |
| Tuning range | 2 MHz to 30 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 400 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hfdl` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | text | empty |  | Dial frequencies in kHz separated by spaces. Empty watches every frequency of the ground station table inside the capture. |
| `whole_span` | Whole span | yes or no | false |  | Every table frequency inside the capture, and the list. |
| `shared_channeliser` | Shared channeliser | yes or no | true |  | One polyphase bank feeds every channel (else one channeliser each). |
| `channel_filter_hz` | Channel filter | frequency | 3.4 kHz | 2 kHz to 5 kHz | The width of each channel's filter. |
| `follow_best_band` | Follow the best band | yes or no | false |  | Asks the host to retune to the HFDL band with the most traffic. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `pdus` | PDUs | whole number |
| `bursts_bad` | Bad bursts | whole number |
| `squitters` | Squitters | whole number |
| `lpdus` | LPDUs | whole number |
| `acars` | ACARS | whole number |
| `positions` | Positions | whole number |
| `bursts_300` | Bursts at 300 bit/s | whole number |
| `bursts_600` | Bursts at 600 bit/s | whole number |
| `bursts_1200` | Bursts at 1200 bit/s | whole number |
| `bursts_1800` | Bursts at 1800 bit/s | whole number |
| `lost_preamble` | Preambles refused | whole number |
| `lost_short` | Bursts cut short | whole number |
| `lost_squitter_fcs` | Bad squitter FCS | whole number |
| `lost_header_fcs` | Bad header FCS | whole number |
| `lost_lpdu_fcs` | Bad LPDU FCS | whole number |
| `lost_lpdu_type` | Unknown LPDU type | whole number |
| `hopper` | Band hopper | text |
| `aircraft` | Aircraft | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `time` (time), `signal_s` (number (s)), `dial` (frequency), `level_db` (number (dB)), `snr_db` (number (dB)), `offset_hz` (frequency), `bps` (whole number (bit/s)), `gs_id` (whole number), `ground_station` (text), `ac_id` (whole number), `kind` (text), `hfnpdu` (text), `uplink` (yes or no), `summary` (text), `registration` (text), `flight` (text), `label` (text), `block_id` (text), `msn` (text), `text` (text), `acars_ok` (yes or no), `reading_kind` (text), `reading_category` (text), `cpdlc_imi` (text), `cpdlc_ground` (text), `cpdlc_aircraft` (text), `cpdlc_crc_ok` (yes or no), `cpdlc_parsed` (yes or no), `cpdlc_text` (text), `position` (position), `detail` (text) |
| `aircraft` | Aircraft | `key` (text), `address` (text), `registration` (text), `flight` (text), `gs_id` (whole number), `ground_station` (text), `count` (whole number), `dial` (frequency), `last_heard` (time), `logged_on` (yes or no), `position` (position), `alt_ft` (number (ft)) |
| `ground_stations` | Ground stations | `gs_id` (whole number), `name` (text), `heard_direct` (yes or no), `utc_sync` (yes or no), `squitters` (whole number), `count` (whole number), `dial` (frequency), `last_heard` (time), `frame_index` (whole number), `active_khz` (text) |
| `channels` | Channels | `dial` (frequency), `name` (text), `ground_station` (text), `in_band` (yes or no), `found` (whole number), `good` (whole number), `bad` (whole number), `snr_db` (number (dB)), `offset_hz` (frequency), `bps` (whole number (bit/s)) |

## Map points

The stack puts points on a map: Aircraft and ground stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfdl.message` | HFDL message | `source` (text), `target` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `position` (position), `kind` (text), `uplink` (yes or no), `gs_id` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `avdata`, version 1.4. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: text.

### Result fields

Struct `HfdlFields`. The result fields of HFDL.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pdus` | whole number | always | 1.0 | Good PDUs: squitters and MPDUs, one a burst. |
| `bursts_bad` | whole number | always | 1.0 | Bursts that failed. |
| `squitters` | whole number | always | 1.0 | Squitters. |
| `lpdus` | whole number | always | 1.0 | LPDUs. |
| `acars` | whole number | always | 1.0 | ACARS messages. |
| `positions` | whole number | always | 1.0 | Messages with a position. |
| `bursts_300` | whole number | always | 1.0 | Good bursts at 300 bit/s. |
| `bursts_600` | whole number | always | 1.0 | Good bursts at 600 bit/s. |
| `bursts_1200` | whole number | always | 1.0 | Good bursts at 1200 bit/s. |
| `bursts_1800` | whole number | always | 1.0 | Good bursts at 1800 bit/s. |
| `lost_preamble` | whole number | always | 1.0 | Bursts lost: the preamble was refused. |
| `lost_short` | whole number | always | 1.0 | Bursts lost: cut short. |
| `lost_squitter_fcs` | whole number | always | 1.0 | Squitters lost to a bad FCS. |
| `lost_header_fcs` | whole number | always | 1.0 | Bursts lost to a bad header FCS. |
| `lost_lpdu_fcs` | whole number | always | 1.0 | LPDUs lost to a bad FCS. |
| `lost_lpdu_type` | whole number | always | 1.0 | LPDUs of an unknown type. |
| `hopper` | text | always | 1.0 | What the band hopper does. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the table. |

### Table `messages`

Struct `HfdlMessageRow`. A row of the messages. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The wall-clock time. No value for a recording. |
| `signal_s` | number (s) | always | 1.0 | The signal time. |
| `dial` | frequency | always | 1.0 | The dial frequency of the channel. |
| `level_db` | number (dB) | always | 1.0 | The level of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the burst. |
| `offset_hz` | frequency | always | 1.0 | The offset of the burst. |
| `bps` | whole number (bit/s) | always | 1.0 | The bit rate of the burst. |
| `gs_id` | whole number | always | 1.0 | The ground station id. |
| `ground_station` | text | always | 1.0 | The ground station as its row key. |
| `ac_id` | whole number | optional | 1.0 | The aircraft id the ground station gave. |
| `kind` | text | optional | 1.0 | The kind of the PDU. |
| `hfnpdu` | text | optional | 1.0 | The kind of the HFNPDU. |
| `uplink` | yes or no | always | 1.0 | Ground to aircraft. |
| `summary` | text | optional | 1.0 | One line. |
| `registration` | text | optional | 1.0 | The registration of an ACARS message. |
| `flight` | text | optional | 1.0 | The flight id. |
| `label` | text | optional | 1.0 | The label of an ACARS message. |
| `block_id` | text | optional | 1.0 | The block id of an ACARS message. |
| `msn` | text | optional | 1.0 | The message sequence number of an ACARS message. |
| `text` | text | optional | 1.0 | The text of an ACARS message. |
| `acars_ok` | yes or no | always | 1.0 | The ACARS check sequence passed. |
| `reading_kind` | text | optional | 1.0 | The kind of the reading. |
| `reading_category` | text | optional | 1.0 | The category of the reading. |
| `cpdlc_imi` | text | optional | 1.0 | The CPDLC envelope: AT1, CR1, CC1 or DR1. |
| `cpdlc_ground` | text | optional | 1.0 | The CPDLC ground facility. |
| `cpdlc_aircraft` | text | optional | 1.0 | The aircraft named in the CPDLC envelope. |
| `cpdlc_crc_ok` | yes or no | optional | 1.0 | The CPDLC envelope check passed. |
| `cpdlc_parsed` | yes or no | optional | 1.0 | The CPDLC message body decoded completely. |
| `cpdlc_text` | text | optional | 1.0 | The decoded CPDLC message. |
| `position` | position | optional | 1.0 | The position the message gives. |
| `detail` | text | optional | 1.0 | Every decoded layer, joined by " \| ". |

The rows are the messages of the stack. The column `flight` names a row. Place of a row: the column `position`. Text: `text`. Time: `time`.

### Table `aircraft`

Struct `HfdlAircraftRow`. A row of the aircraft. The row key is the address, or the ground station and the aircraft id while the address is not known.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `key` | text | always | 1.0 | The row key. |
| `address` | text | optional | 1.0 | The 24-bit address, six hex digits. |
| `registration` | text | optional | 1.0 | The registration. |
| `flight` | text | optional | 1.0 | The flight id. |
| `gs_id` | whole number | always | 1.0 | The ground station id. |
| `ground_station` | text | always | 1.0 | The ground station as its row key. |
| `count` | whole number | always | 1.0 | Messages of the aircraft. |
| `dial` | frequency | always | 1.0 | The dial frequency of the last message. |
| `last_heard` | time | optional | 1.0 | When the aircraft was last heard. |
| `logged_on` | yes or no | always | 1.0 | The aircraft is logged on. |
| `position` | position | optional | 1.0 | The last position. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude of the last position. |

The rows are the stations or nodes of the stack. The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`. Time: `last_heard`.

### Table `ground_stations`

Struct `HfdlStationRow`. A row of the ground stations. The row key is the station id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `gs_id` | whole number | always | 1.0 | The station id. |
| `name` | text | always | 1.0 | The name of the station. |
| `heard_direct` | yes or no | always | 1.0 | Heard directly: a squitter or an uplink. |
| `utc_sync` | yes or no | always | 1.0 | The station is synchronised to UTC. |
| `squitters` | whole number | always | 1.0 | Squitters. |
| `count` | whole number | always | 1.0 | Messages that name the station. |
| `dial` | frequency | always | 1.0 | The dial frequency it was heard on. |
| `last_heard` | time | optional | 1.0 | When the station was last heard. |
| `frame_index` | whole number | optional | 1.0 | The frame index of the last squitter. |
| `active_khz` | text | optional | 1.0 | The frequencies in use by the last squitter, kHz, separated by spaces. |

The column `name` names a row.

### Table `channels`

Struct `HfdlChannelRow`. A row of the channels. The row key is the dial frequency in hertz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dial` | frequency | always | 1.0 | The dial frequency. |
| `name` | text | optional | 1.0 | The name of the ground station of the channel. |
| `ground_station` | text | optional | 1.0 | The ground station as its row key. |
| `in_band` | yes or no | always | 1.0 | The channel is inside the band of the input. |
| `found` | whole number | always | 1.0 | Preambles found. |
| `good` | whole number | always | 1.0 | Good bursts. |
| `bad` | whole number | always | 1.0 | Bad bursts. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the channel. |
| `offset_hz` | frequency | always | 1.0 | The offset of the last burst. |
| `bps` | whole number (bit/s) | always | 1.0 | The bit rate of the last burst. |

The column `name` names a row.

### Event `hfdl.message`

Struct `HfdlMessageEvent`. An HFDL message. The source is the sender (the address or registration of the aircraft, or the name of the ground station), the target the receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender. |
| `target` | text | optional | 1.0 | The receiver. No value for a squitter. |
| `text` | text | always | 1.0 | The summary of the message. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the burst. |
| `family` | choice | always | 1.0 | What the event is about: an aircraft. |
| `position` | position | optional | 1.0 | The position the message gives. |
| `kind` | text | always | 1.0 | The kind of the PDU. |
| `uplink` | yes or no | always | 1.0 | Ground to aircraft. |
| `gs_id` | whole number | always | 1.0 | The ground station id. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hfdl_clear` | none | none |
| `remove` | `sdrkit_hfdl_remove` | `sdrkit_hfdl_aircraft_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_hfdl_get_channels` | `sdrkit_hfdl_set_channels` |
| `whole_span` | `sdrkit_hfdl_get_whole_span` | `sdrkit_hfdl_set_whole_span` |
| `shared_channeliser` | `sdrkit_hfdl_get_shared_channeliser` | `sdrkit_hfdl_set_shared_channeliser` |
| `channel_filter_hz` | `sdrkit_hfdl_get_channel_filter_hz` | `sdrkit_hfdl_set_channel_filter_hz` |
| `follow_best_band` | `sdrkit_hfdl_get_follow_best_band` | `sdrkit_hfdl_set_follow_best_band` |

## Try it

```sh
sdrkit run hfdl --source synth:hfdl
sdrkit run hfdl --source <driver>:<address> --freq 8.9
```
<!-- generated: end -->
