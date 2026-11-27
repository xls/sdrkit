<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# BLE tracker detector

Finds item trackers (AirTag and Find My accessories, Tile, Chipolo, SmartTag, Google tags) that stay with you.

Stack id: `bletracker`. Group: Data and IoT.

Signal types: Bluetooth LE.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.402 GHz |
| Tuning range | 2.4 GHz to 2.484 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 2 MHz |
| Offset from the signal | 1 MHz |
| Radio filter | 2 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:bletracker` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | choice | `37` | `37` (37 2402 MHz), `38` (38 2426 MHz), `39` (39 2480 MHz) | Trackers advertise on all three channels in turn, so any one of them hears every tracker. |
| `alert_after_min` | Alert after | number (min) | 10 | 1 to 60 | How long a tracker must stay with the radio before the alert. |
| `near_dbfs` | Near above | number (dBFS) | -60 | -100 to -20 | The averaged level from which a tracker counts as near. The radio's level, not a calibrated distance. |
| `alert_after_s` | Alert after (seconds, for tests) | duration (s) | 0 s | 0 to 7200 | When above 0, replaces the alert time in minutes. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_schedule` | Schedule | choice | `all` | `all` (Every tracker and a phone), `find_my` (Only the Find My frame) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `following_now` | Following now | whole number |
| `tracker_packets` | Tracker packets | whole number |
| `trackers_now` | Trackers now | whole number |
| `trackers` | Trackers | whole number |
| `alert_after` | Alert after | duration (s) |
| `near_dbfs` | Near above | number (dBFS) |
| `now_s` | Signal time | number (s) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `channel` | Channel | whole number |
| `limits` | Limits | text |
| `privacy` | Privacy | text |
| `quiet_reason` | Why nothing shows | text |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Packets sent | whole number |
| `tx_left` | Packets left | whole number |
| `adv.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `trackers` | Trackers | `tracker` (text), `kind` (choice), `state` (choice), `stays` (duration (s)), `near_pct` (number (%)), `level_dbfs` (number (dBFS)), `peak_dbfs` (number (dBFS)), `trend` (choice), `addresses` (whole number), `last_s` (number (s)), `last_heard` (time), `packets` (whole number), `separated` (yes or no), `status` (whole number), `following` (yes or no), `mine` (yes or no), `id` (whole number), `mode` (choice), `tag_state` (choice), `tag_state_code` (whole number), `aging_counter` (whole number), `region` (whole number), `e2e` (yes or no), `uwb` (yes or no), `battery` (whole number), `identifier` (text), `service_data` (text) |
| `alerts` | Alerts | `time` (time), `signal_s` (number (s)), `level_dbfs` (number (dBFS)), `alert` (text) |
| `tracker_kinds` | Tracker kinds | `kind` (choice), `last_minute` (whole number), `last_30_min` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ble.tracker` | Unwanted tracking alert | `source` (text), `kind` (choice), `text` (text), `stays` (duration (s)), `level_dbfs` (number (dBFS)), `urgency` (choice), `family` (choice) |
| `ble.tx` | Bluetooth LE transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `mine` | Mark as mine | `row` (text, required), `mine` (yes or no, default true) |  | A tracker marked as the user's own raises no alert. |
| `send` | Send a tracker advertisement | `tracker` (choice, default `find_my`), `address` (text, required), `address_type` (choice, default `random`), `pdu_type` (choice, default `adv_nonconn_ind`), `status` (whole number, default 0), `protection` (yes or no, default false), `payload` (text, default empty), `count` (whole number, default 1), `interval_ms` (whole number (ms), default 100), `shipping` (yes or no, default false), `service_data` (text, default empty), `tag_state` (choice, default `lost`), `aging_counter` (whole number, default 0), `privacy_id` (text, default empty), `region` (whole number, default 0), `e2e` (yes or no, default false), `uwb` (yes or no, default false), `battery` (whole number, default 3), `signature` (text, default empty) |  | Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: Find My manufacturer data of company 0x004C (type 0x12, length 25, the status, the payload); Google service data of UUID 0xFEAA (frame type 0x40 or 0x41, the payload); Tile, a UUID list of 0xFEED and the service data under 0xFEED, or in shipping mode a UUID list of 0xFEEC alone; Chipolo, a UUID list of 0xFE33 and the service data under 0xFE33; SmartTag, the 20-byte service data of UUID 0xFD5A from the tag state, the aging counter, the privacy ID, the region, the E2E and UWB flags, the battery code and the signature. No flags structure goes out. A send during another waits for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the packets of the send on air that are still to go, and the sends that wait. |

## Typed interface

The stack is in the family `wlan`, version 4.7. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `trackers_now`, `trackers`.

### Result fields

Struct `BletrackerFields`. The result fields of `bletracker`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `following_now` | whole number | always | 1.0 | Trackers that follow the radio now. |
| `tracker_packets` | whole number | always | 1.0 | Tracker advertisements taken since the last clear. |
| `trackers_now` | whole number | always | 1.0 | Trackers heard in the last minute. |
| `trackers` | whole number | always | 1.0 | Trackers in the table. |
| `alert_after` | duration (s) | always | 1.0 | How long a tracker must stay before the alert. |
| `near_dbfs` | number (dBFS) | always | 1.0 | The averaged level from which a tracker counts as near. |
| `now_s` | number (s) | always | 1.0 | The signal time of the book. |
| `frames_ok` | whole number | always | 1.0 | Good packets. |
| `frames_bad` | whole number | always | 1.0 | Exact access-address matches whose CRC failed on every channel. |
| `snr_db` | number (dB) | optional | 1.0 | The running mean of the eye SNR. No value before the first good packet. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last good packet. No value before it. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the channel. No value before the first input. |
| `channel` | whole number | optional | 1.0 | The tuned advertising channel. No value when it is not known, as for a recording. |
| `limits` | text | always | 1.0 | What the detector cannot tell. |
| `privacy` | text | always | 1.0 | The privacy note. |
| `quiet_reason` | text | optional | 4.6 | Why nothing can be heard: the capture holds no primary advertising channel. None while one is inside, and when the centre of the input is not known. |

### Result fields

Struct `BleTxFields`. The result fields of the advertising sender of `ble` and `bletracker`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.0 | Whether a send is on air. |
| `tx_sent` | whole number | always | 1.0 | The packets of the send on air that went out. |
| `tx_left` | whole number | always | 1.0 | The packets still to go: the rest of the send on air and the sends that wait. |

### Table `trackers`

Struct `BletrackerTrackerRow`. A row of the trackers of `bletracker`. The row key is the tracker id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tracker` | text | always | 1.0 | The label of the tracker: its kind and the last two address bytes. |
| `kind` | choice | optional | 1.0 | The kind, if recognised. |
| `state` | choice | always | 1.0 | What the tracker does now. |
| `stays` | duration (s) | always | 1.0 | How long it stayed with the radio. |
| `near_pct` | number (%) | always | 1.0 | The share of the time it was near. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The averaged level. |
| `peak_dbfs` | number (dBFS) | always | 1.0 | The highest level. |
| `trend` | choice | always | 1.0 | Whether the level rises or falls. |
| `addresses` | whole number | always | 1.0 | The addresses it used. |
| `last_s` | number (s) | always | 1.0 | The signal time it was last heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last sighting, for a live source. |
| `packets` | whole number | always | 1.0 | Packets heard. |
| `separated` | yes or no | always | 1.0 | It says it is away from its owner. |
| `status` | whole number | optional | 1.0 | The status byte, if the advertisement has one. |
| `following` | yes or no | always | 1.0 | It follows the radio. |
| `mine` | yes or no | always | 1.0 | It is marked as the user's own. |
| `id` | whole number | always | 1.0 | The tracker id. |
| `mode` | choice | optional | 1.0 | The mode that the advertised UUID tells: Tile activated or shipping, SmartTag registered or unregistered, Chipolo disconnected from its owner. No value for other families. |
| `tag_state` | choice | optional | 1.0 | The SmartTag tag state. No value for the codes 0 and 7, which have no name, and for other families. |
| `tag_state_code` | whole number | optional | 1.0 | The SmartTag tag state code, 0 to 7. |
| `aging_counter` | whole number | optional | 1.0 | The SmartTag aging counter: 15-minute steps of the tag's clock from 2020-07-02 00:00 UTC. |
| `region` | whole number | optional | 1.0 | The SmartTag region ID, 0 to 15. |
| `e2e` | yes or no | optional | 1.0 | The SmartTag E2E flag: finders must encrypt their location reports. |
| `uwb` | yes or no | optional | 1.0 | The SmartTag UWB flag. |
| `battery` | whole number | optional | 1.0 | The SmartTag battery code, 0 to 3. |
| `identifier` | text | optional | 1.0 | The rolling identifier in hex, opaque: the SmartTag privacy ID. |
| `service_data` | text | optional | 1.0 | The service data value under the family's UUID in hex, as sent (Tile, Chipolo, SmartTag). |

The rows are the stations or nodes of the stack. The column `identifier` names a row. Default order: the column `level_dbfs`, highest first. Time: `last_heard`.

### Table `alerts`

Struct `BletrackerAlertRow`. A row of the alerts of `bletracker`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The UTC of the alert, for a live source. |
| `signal_s` | number (s) | always | 1.0 | The signal time of the alert. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the tracker. |
| `alert` | text | always | 1.0 | The text of the alert. |

### Table `tracker_kinds`

Struct `BletrackerKindRow`. A row of the trackers heard of each kind of `bletracker`. The row key is the key of the kind.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | The kind. |
| `last_minute` | whole number | always | 1.0 | Trackers of the kind heard in the last minute. |
| `last_30_min` | whole number | always | 1.0 | Trackers of the kind heard in the last 30 minutes. |

### Event `ble.tracker`

Struct `BletrackerAlertEvent`. The fields of an unwanted tracking alert.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The label of the tracker. |
| `kind` | choice | optional | 1.0 | The kind, if recognised. |
| `text` | text | always | 1.0 | The text of the alert. |
| `stays` | duration (s) | always | 1.0 | How long it stayed with the radio. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the tracker. |
| `urgency` | choice | always | 1.0 | Urgent for a tracker away from its owner, else a notice. |
| `family` | choice | always | 1.0 | The family of the source: an alert. |

### Event `ble.tx`

Struct `BleTxEvent`. The fields of a change of an advertising send.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.0 | Why it changed: empty, halted, the transmitter is off, the format changed, the frequency changed, or why a send was refused. |
| `target` | text | always | 1.0 | The advertiser address. |
| `text` | text | always | 1.0 | The PDU type, the channel, the packets and the advertising data in hex; empty for finished. |
| `family` | choice | always | 1.0 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_bletracker_clear` | none | none |
| `mine` | `sdrkit_bletracker_mine` | `sdrkit_bletracker_mine_args` | none |
| `send` | `sdrkit_bletracker_send` | `sdrkit_bletracker_send_args` | none |
| `halt` | `sdrkit_bletracker_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_bletracker_get_channel` | `sdrkit_bletracker_set_channel` |
| `alert_after_min` | `sdrkit_bletracker_get_alert_after_min` | `sdrkit_bletracker_set_alert_after_min` |
| `near_dbfs` | `sdrkit_bletracker_get_near_dbfs` | `sdrkit_bletracker_set_near_dbfs` |
| `alert_after_s` | `sdrkit_bletracker_get_alert_after_s` | `sdrkit_bletracker_set_alert_after_s` |
| `tx_level` | `sdrkit_bletracker_get_tx_level` | `sdrkit_bletracker_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_schedule` | `sdrkit_bletracker_gen_get_schedule` | `sdrkit_bletracker_gen_set_schedule` |

## Try it

```sh
sdrkit run bletracker --source synth:bletracker
sdrkit run bletracker --source iiod:192.168.2.1 --freq 2402
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
