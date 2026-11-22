<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# JS8

Keyboard chat in FT8-style frames: heartbeats, directed messages and free text.

Stack id: `js8`. Group: Weak-signal digital.

Signal types: JS8.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 3.5 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `js8normal` | JS8 Normal | 0 Hz |
| `js8fast` | JS8 Fast | 0 Hz |
| `js8turbo` | JS8 Turbo | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:js8` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `js8_normal` | JS8 Normal (15 s) | yes or no | true |  |  |
| `js8_fast` | JS8 Fast (10 s) | yes or no | true |  |  |
| `js8_turbo` | JS8 Turbo (6 s) | yes or no | true |  |  |
| `deep` | Deep decoding | yes or no | true |  | A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of candidates that failed in earlier slots, and assume the calls heard on the band. |
| `demod` | Demodulator | choice | `usb` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) |  |
| `listen` | Listen | yes or no | false |  | Copies the 8 kHz audio to the speaker. Decoding is the same either way. |
| `time_source` | Slot clock | choice | `system` | `system` (System clock), `start` (Recording start), `search` (Slot search) |  |
| `time_start` | Recording start (UTC) | text | empty |  | An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given. |
| `band_edges` | Band edges | text | empty |  | The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by ";", for example "20m=14000000-14350000". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band. |
| `spots` | Spot events | yes or no | false |  | Makes a spot event of every decode with a calling station, for the host to forward. |
| `js8_speed` | Speed | choice | `normal` | `normal` (Normal), `fast` (Fast), `turbo` (Turbo) |  |
| `js8_hz` | Audio frequency | frequency | 1.5 kHz | 200 Hz to 3 kHz |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mirrored` | Test signal upside down | yes or no | false |  |  |
| `gen_live` | Test signal in step with the clock | yes or no | false |  |  |
| `gen_single` | Single station message | text | empty |  |  |
| `gen_hz` | Single station frequency | frequency | 1.5 kHz | 200 Hz to 3.5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `time_source` | Slot clock | choice |
| `utc` | UTC | time |
| `clock_error_s` | Clock error | number (s) |
| `clock_spread_s` | Clock spread | number (s) |
| `clock_decodes` | Clock decodes | whole number |
| `clock_warning` | Clock warning | yes or no |
| `js8_frames` | JS8 frames | whole number |
| `js8_failed` | JS8 candidates failed | whole number |
| `js8_state` | JS8 | choice |
| `js8_waterfall_seq` | Waterfall line | whole number |
| `dec_js8.slots` | Slots decoded | whole number |
| `dec_js8.decodes` | Decodes | whole number |
| `dec_js8.decode_ms` | Decoding time | number (ms) |
| `dec_js8.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `modes` | Modes | `mode` (text), `period` (duration (s)), `slots` (whole number), `decodes` (whole number), `last_count` (whole number), `utc` (time), `decode_ms` (number (ms)), `state` (choice) |
| `stations` | Stations | `callsign` (text), `locator` (text), `mode` (choice), `snr_db` (number (dB)), `freq_hz` (frequency), `heard` (whole number), `last_heard` (time), `position` (position) |
| `js8_frames` | JS8 frames | `utc` (time), `submode` (choice), `snr_db` (number (dB)), `dt_s` (number (s)), `audio_hz` (frequency), `frame` (text) |
| `js8_messages` | JS8 messages | `utc` (time), `snr_db` (number (dB)), `audio_hz` (frequency), `message` (text) |
| `js8_stations` | JS8 stations | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `audio_hz` (frequency), `utc` (time), `last_text` (text), `last_heard` (time), `position` (position) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `js8_waterfall` | JS8 waterfall | Hz | dB |

## Map points

The stack puts points on a map: Stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `wsjt.spot` | Station heard | `callsign` (text), `locator` (text), `snr_db` (number (dB)), `freq_hz` (frequency), `mode` (text), `source` (text), `family` (choice) |
| `js8.frame` | JS8 frame | `text` (text), `chars` (text), `snr_db` (number (dB)), `submode` (choice), `source` (text), `target` (text), `cq` (yes or no), `family` (choice) |
| `js8.message` | JS8 message | `text` (text), `complete` (yes or no), `source` (text), `target` (text), `cq` (yes or no), `family` (choice) |
| `wsjt.tx` | Message to send | `mode` (choice), `text` (text), `audio_hz` (frequency), `slot` (time), `period` (duration (s)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send | `text` (text, required) |  | Queues a JS8 message. My call and locator come from the station. |
| `halt` | Halt |  |  | Stops transmitting at once. |

## Typed interface

The stack is in the family `wsjt`, version 5.0. The C header `sdrkit/wsjt.h`, the registry record `registry/wsjt.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Weak-signal messages (`weak_signal`). Media: sound (monitor), text.

### Result fields

Struct `WsjtFields`. The result fields of a slot stack: the slot clock and the clock estimate from the decodes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_source` | choice | optional | 1.0 | The time source of the slot clock. It has no value before the clock reports. |
| `utc` | time | always | 1.0 | The UTC of the newest audio. |
| `clock_error_s` | number (s) | optional | 1.0 | The median DT of the recent decodes. It has no value without a recent decode. |
| `clock_spread_s` | number (s) | optional | 1.0 | The spread of the recent DTs. It has no value without a recent decode. |
| `clock_decodes` | whole number | always | 1.0 | The decodes that the clock estimate uses. |
| `clock_warning` | yes or no | always | 1.0 | The clock is more than 1 s off. |

### Result fields

Struct `Js8Fields`. The result fields of a slot stack with JS8.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `js8_frames` | whole number | always | 1.0 | JS8 frames since Clear. |
| `js8_failed` | whole number | always | 1.0 | JS8 candidates that did not decode since Clear. |
| `js8_state` | choice | always | 1.0 | What JS8 has heard. |
| `js8_waterfall_seq` | whole number | optional | 1.0 | The sequence number of the newest line of the JS8 waterfall. It has no value before the first line. |

### Table `modes`

Struct `ModeRow`. A row of the modes table: the counters and the state of one mode heard, or of one JS8 speed. The row key is the mode id, or `js8n`, `js8f` and `js8t`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | text | always | 1.0 | The mode with its sub-mode and period. |
| `period` | duration (s) | always | 1.0 | The T/R period. |
| `slots` | whole number | always | 1.0 | The slots decoded. |
| `decodes` | whole number | always | 1.0 | Decodes since Clear. |
| `last_count` | whole number | always | 1.0 | The decodes of the last slot. |
| `utc` | time | optional | 1.0 | The UTC of the last slot decoded. It has no value before the first slot. |
| `decode_ms` | number (ms) | always | 1.0 | The time that the last slot took to decode. |
| `state` | choice | always | 1.0 | Whether the mode decodes. |

### Table `stations`

Struct `StationRow`. A row of the station table: a station heard in any mode. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `mode` | choice | always | 1.0 | The mode of the newest decode. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the newest decode. |
| `freq_hz` | frequency | always | 1.0 | The radio frequency of the newest decode. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last decode. |
| `position` | position | optional | 1.0 | The centre of the locator, if one was heard. |

The column `callsign` names a row. Default order: the column `last_heard`, highest first. Place of a row: the column `position`.

### Table `js8_frames`

Struct `Js8FrameRow`. A row of the JS8 frames. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the slot. |
| `submode` | choice | optional | 1.0 | The speed of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `dt_s` | number (s) | always | 1.0 | The time offset from the slot start. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `frame` | text | always | 1.0 | The text of the frame. |

Default order: the column `utc`, highest first.

### Table `js8_messages`

Struct `Js8MessageRow`. A row of the JS8 messages. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | always | 1.0 | The UTC of the first frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `message` | text | always | 1.0 | The message. |

Default order: the column `utc`, highest first.

### Table `js8_stations`

Struct `Js8StationRow`. A row of the JS8 stations. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the newest frame. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency of the newest frame. |
| `utc` | time | always | 1.0 | The UTC of the newest frame. |
| `last_text` | text | always | 1.0 | The newest text of the station. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `position` | position | optional | 1.0 | The centre of the locator, if one was heard. |

The column `callsign` names a row. Default order: the column `last_heard`, highest first. Place of a row: the column `position`.

### Event `wsjt.spot`

Struct `SpotEvent`. The fields of a station heard, with the standard spot fields.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `freq_hz` | frequency | always | 1.0 | The radio frequency. |
| `mode` | text | always | 1.0 | The emission as spot networks name it. |
| `source` | text | always | 1.0 | The callsign. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `js8.frame`

Struct `Js8FrameEvent`. The fields of a JS8 frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the frame. |
| `chars` | text | always | 1.0 | The twelve characters of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR in 2500 Hz. |
| `submode` | choice | optional | 1.0 | The speed of the frame. |
| `source` | text | optional | 1.0 | The sending station, if the frame names one. |
| `target` | text | optional | 1.0 | The station or group addressed, if the frame names one. |
| `cq` | yes or no | always | 1.0 | The frame is a CQ: a heartbeat to all with a CQ. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `js8.message`

Struct `Js8MessageEvent`. The fields of a JS8 message, sent when its last frame came or when it stopped.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The message. |
| `complete` | yes or no | always | 1.0 | The last frame of the message came. |
| `source` | text | optional | 1.0 | The sending station, if it is known. |
| `target` | text | optional | 1.0 | The station or group addressed, if it is known. |
| `cq` | yes or no | always | 1.0 | The message is a CQ. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Event `wsjt.tx`

Struct `TxEvent`. The fields of a message to send.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | choice | always | 1.0 | The mode. |
| `text` | text | always | 1.0 | The message. |
| `audio_hz` | frequency | always | 1.0 | The audio frequency. |
| `slot` | time | always | 1.0 | The UTC of the start of the transmit slot. |
| `period` | duration (s) | always | 1.0 | The T/R period. |
| `family` | choice | always | 1.0 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_js8_clear` | none | none |
| `remove` | `sdrkit_js8_remove` | `sdrkit_wsjt_station_args` | none |
| `send` | `sdrkit_js8_send` | `sdrkit_wsjt_send_args` | none |
| `halt` | `sdrkit_js8_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `js8_normal` | `sdrkit_js8_get_js8_normal` | `sdrkit_js8_set_js8_normal` |
| `js8_fast` | `sdrkit_js8_get_js8_fast` | `sdrkit_js8_set_js8_fast` |
| `js8_turbo` | `sdrkit_js8_get_js8_turbo` | `sdrkit_js8_set_js8_turbo` |
| `deep` | `sdrkit_js8_get_deep` | `sdrkit_js8_set_deep` |
| `demod` | `sdrkit_js8_get_demod` | `sdrkit_js8_set_demod` |
| `listen` | `sdrkit_js8_get_listen` | `sdrkit_js8_set_listen` |
| `time_source` | `sdrkit_js8_get_time_source` | `sdrkit_js8_set_time_source` |
| `time_start` | `sdrkit_js8_get_time_start` | `sdrkit_js8_set_time_start` |
| `band_edges` | `sdrkit_js8_get_band_edges` | `sdrkit_js8_set_band_edges` |
| `spots` | `sdrkit_js8_get_spots` | `sdrkit_js8_set_spots` |
| `js8_speed` | `sdrkit_js8_get_js8_speed` | `sdrkit_js8_set_js8_speed` |
| `js8_hz` | `sdrkit_js8_get_js8_hz` | `sdrkit_js8_set_js8_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mirrored` | `sdrkit_js8_gen_get_mirrored` | `sdrkit_js8_gen_set_mirrored` |
| `gen_live` | `sdrkit_js8_gen_get_live` | `sdrkit_js8_gen_set_live` |
| `gen_single` | `sdrkit_js8_gen_get_single` | `sdrkit_js8_gen_set_single` |
| `gen_hz` | `sdrkit_js8_gen_get_hz` | `sdrkit_js8_gen_set_hz` |

## Try it

```sh
sdrkit run js8 --source synth:js8
sdrkit run js8 --source synth:js8 --submode js8normal
sdrkit run js8 --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
