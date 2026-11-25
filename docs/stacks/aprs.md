<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# APRS (AFSK)

AX.25 packet radio and APRS at 1200 bit/s AFSK and 9600 bit/s G3RUH: stations, positions, weather and messages.

Stack id: `aprs`. Group: Amateur.

Signal types: Packet (1200 bit/s AFSK).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 25 MHz to 1 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 240 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:aprs` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `narrow` | 12.5 kHz channel | yes or no | false |  |  |
| `emphasis` | Emphasis | choice | `auto` | `auto` (None), `de` (De-emphasis), `pre` (Pre-emphasis) |  |
| `audio_input` | Audio input | yes or no | false |  | The input is FM-demodulated audio: no minimum rate. |
| `listen` | Listen | yes or no | false |  |  |
| `aprs_ssid` | SSID | whole number | 0 | 0 to 15 | The SSID after the callsign of the station in own frames. On AX.25 the callsign must then carry no SSID of its own. |
| `aprs_symbol` | Symbol | text | `/-` |  | Two characters: the symbol table, then the symbol code. |
| `aprs_comment` | Comment | text | empty |  | The comment of the beacon. Without a position of the station the beacon sends this text as its status. |
| `tx_baud` | Speed | choice | `1200` | `1200` (1200 bit/s), `9600` (9600 bit/s) |  |
| `beacon_interval` | Beacon interval | number (s) | 0 | 0 to 1000000 | At least 60 s. |
| `match_ssid` | Match the SSID | yes or no | false |  | Off: a message to the callsign with any SSID is for the station. |
| `wide_input` | Wide input | yes or no | false |  | One carrier at 192 ksps from a channel bank: no minimum rate. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `aprs_tocall` | Destination | text | `APRS` |  |  |
| `aprs_path` | Path | text | `WIDE1-1` |  | Calls separated by commas, at most eight. Empty sends no path. |
| `tx_delay` | TX delay | number (s) | 0.3 | 0.01 to 2 | Flags before each frame, so that the receivers open their squelch and find the bit clock. |
| `tx_pre_emphasis` | Pre-emphasis | yes or no | false |  | Lifts the 2200 Hz tone over the 1200 Hz tone (6 dB an octave above 300 Hz), as the microphone path of an FM radio does, for receivers with de-emphasis. Off sends both tones at the same level. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_option` | Test speeds | choice | `both` | `both` (1200 and 9600 bit/s), `1200` (Only 1200 bit/s), `9600` (Only 9600 bit/s) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `frames_1200` | 1200 bit/s frames | whole number |
| `frames_9600` | 9600 bit/s frames | whole number |
| `aprs_frames` | APRS frames | whole number |
| `copies` | Digipeated copies | whole number |
| `not_ax25` | Not AX.25 | whole number |
| `stations` | Stations | whole number |
| `carrier` | Carrier | yes or no |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `aprs_call` | APRS call | text |
| `aprs_freq` | APRS frequency | frequency |
| `bad_samples` | Bad samples | whole number |
| `modulate.tx_bursts` | Bursts written | whole number |
| `modulate.tx_samples` | Samples written | whole number |
| `modulate.tx_dropped` | Frames dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `source` (text), `target` (text), `path` (text), `type` (text), `text` (text), `bit_rate` (frequency) |
| `stations` | Stations | `call` (text), `symbol` (text), `position` (position), `last_heard` (time), `heard` (whole number), `course_deg` (number (°)), `speed_kt` (number (kt)), `alt_m` (number (m)), `comment` (text), `via` (text) |
| `messages` | Messages | `time_s` (number (s)), `net` (choice), `channel` (text), `source` (text), `sender` (text), `target` (text), `text` (text), `hops` (whole number), `state` (choice), `failure` (text), `relays` (whole number), `packet_id` (whole number), `utc` (time), `snr_db` (number (dB)), `level_dbfs` (number (dBFS)), `crc_ok` (yes or no), `copies` (whole number) |

## Map points

The stack puts points on a map: Stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `aprs.packet` | APRS packet | `source` (text), `target` (text), `path` (text), `info` (text), `type` (text), `text` (text), `position` (position), `tnc2` (text), `family` (choice) |
| `tx.request` | Frame to send | `id` (whole number), `what` (choice), `payload` (bytes), `freq_hz` (frequency), `not_before_s` (number (s)), `air_time` (duration (s)), `message_key` (whole number), `setting` (text), `afsk_baud` (whole number (bit/s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `aprs.send` | Send an APRS message | `to` (text, required), `text` (text, required) | `packet_id` (whole number) | Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. |
| `aprs.beacon` | Send an APRS beacon |  |  | The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. |
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `lora`, version 4.0. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Mesh messaging (`mesh`). Media: sound (monitor), text.

### Result fields

Struct `AprsFields`. The result fields of the APRS stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames with a good CRC. |
| `frames_bad` | whole number | always | 1.0 | Frames whose CRC failed. |
| `frames_1200` | whole number | always | 1.0 | Frames at 1200 bit/s. |
| `frames_9600` | whole number | always | 1.0 | Frames at 9600 bit/s. |
| `aprs_frames` | whole number | always | 1.0 | APRS frames, without the digipeated copies. |
| `copies` | whole number | always | 1.0 | Digipeated copies. |
| `not_ax25` | whole number | always | 1.0 | Frames that are not AX.25. |
| `stations` | whole number | always | 1.0 | Stations, objects and items in the table. |
| `carrier` | yes or no | always | 1.0 | A carrier is present. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio, once the demodulator reports. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the demodulator reports. |

### Result fields

Struct `HamFields`. The own APRS station, once the amateur client reports.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aprs_call` | text | optional | 1.0 | The source call of own APRS frames: the callsign of the station with the SSID. Empty without a callsign. |
| `aprs_freq` | frequency | optional | 1.0 | The frequency that the client sends on, while the session listens to a channel of its link. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `frames`

Struct `AprsFrameRow`. A row of the APRS frame log. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `source` | text | always | 1.0 | The sender. |
| `target` | text | always | 1.0 | The destination. |
| `path` | text | always | 1.0 | The digipeater path. |
| `type` | text | always | 1.0 | The APRS kind or the AX.25 frame type. |
| `text` | text | always | 1.0 | The frame in one line. |
| `bit_rate` | frequency | always | 1.0 | The bit rate. |

The rows are a log: they arrive over time. The column `source` names a row. Sender: `source`. Addressee: `target`. Text: `text`. Time: `time_s`.

### Table `stations`

Struct `AprsStationRow`. A row of the APRS stations. The row key is the callsign with its SSID, or the name of an object or item.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `call` | text | always | 1.0 | The callsign with its SSID, or the name of an object or item. |
| `symbol` | text | always | 1.0 | The map symbol. |
| `position` | position | optional | 1.0 | The last position. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `heard` | whole number | optional | 1.0 | The frames of the station. |
| `course_deg` | number (°) | optional | 1.0 | The course. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed. |
| `alt_m` | number (m) | optional | 1.0 | The altitude. |
| `comment` | text | always | 1.0 | The comment or status. |
| `via` | text | always | 1.0 | The station that sent an object, an item or a third-party packet. |

The rows are the stations or nodes of the stack. The column `call` names a row. Place of a row: the column `position`. Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`. Time: `last_heard`.

### Table `messages`

Struct `MeshMessageRow`. A row of the messages. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `net` | choice | always | 1.0 | The protocol. |
| `channel` | text | optional | 1.0 | The channel. |
| `source` | text | optional | 1.0 | The sender. |
| `sender` | text | optional | 1.0 | The name of the sender. |
| `target` | text | optional | 1.0 | The addressee. |
| `text` | text | always | 1.0 | The text. |
| `hops` | whole number | optional | 1.0 | The hops taken. |
| `state` | choice | optional | 1.0 | The delivery state of an own message. |
| `failure` | text | optional | 1.0 | Why the delivery failed. |
| `relays` | whole number | optional | 1.0 | The distinct relays heard for an own broadcast. |
| `packet_id` | whole number | optional | 1.0 | The packet id: the reply of `mt.send` for an own Meshtastic text, the message number of an APRS message (the reply of `aprs.send` for an own one). None for a protocol without packet ids. |
| `utc` | time | optional | 1.0 | The UTC when the text was received or sent, if the clock of the session is known. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the frame of a received text. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the frame of a received text, relative to the full scale of the input (not an absolute power). |
| `crc_ok` | yes or no | optional | 1.0 | The text comes from a copy with a good CRC. False for a damaged copy that was shown after the wait without a good copy; a later good copy replaces the text and sets it. None for an own message. |
| `copies` | whole number | optional | 1.0 | The copies of the packet heard while the message is remembered (10 minutes), damaged ones included. None for an own message. |

The rows are the messages of the stack. The column `sender` names a row. Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`. Delivery state: `state`. Failure reason: `failure`.

### Event `aprs.packet`

Struct `AprsPacketEvent`. The fields of one AX.25 frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender. |
| `target` | text | always | 1.0 | The destination. |
| `path` | text | always | 1.0 | The digipeater path. |
| `info` | text | always | 1.0 | The information field as received. |
| `type` | text | always | 1.0 | The APRS kind or the AX.25 frame type. |
| `text` | text | always | 1.0 | The frame in one line. |
| `position` | position | optional | 1.0 | The position that the frame gives, if it gives one. |
| `tnc2` | text | always | 1.0 | The frame in TNC2 text. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `tx.request`

Struct `TxRequestEvent`. The fields of a frame that the stack requests the host to send.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | A number for the reply. |
| `what` | choice | always | 1.0 | What the frame is. |
| `payload` | bytes | always | 1.0 | The payload. |
| `freq_hz` | frequency | always | 1.0 | The frequency to send on. |
| `not_before_s` | number (s) | always | 1.0 | The earliest send time, signal seconds. |
| `air_time` | duration (s) | optional | 1.0 | The time on air of a LoRa frame. |
| `message_key` | whole number | optional | 1.0 | The own message that the frame carries. |
| `setting` | text | optional | 1.0 | The LoRa setting in words. |
| `afsk_baud` | whole number (bit/s) | optional | 1.0 | The packet speed of an AX.25 frame. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `aprs.send` | `sdrkit_aprs_send` | `sdrkit_aprs_send_args` | `sdrkit_aprs_send_reply` |
| `aprs.beacon` | `sdrkit_aprs_beacon` | none | none |
| `clear` | `sdrkit_aprs_clear` | none | none |
| `remove` | `sdrkit_aprs_remove` | `sdrkit_aprs_remove_args` | none |

The argument `to` of `aprs.send` takes the column `call` of a row of the table `stations`.

### Settings

| Key | C getter | C setter |
|---|---|---|
| `narrow` | `sdrkit_aprs_get_narrow` | `sdrkit_aprs_set_narrow` |
| `emphasis` | `sdrkit_aprs_get_emphasis` | `sdrkit_aprs_set_emphasis` |
| `audio_input` | `sdrkit_aprs_get_audio_input` | `sdrkit_aprs_set_audio_input` |
| `listen` | `sdrkit_aprs_get_listen` | `sdrkit_aprs_set_listen` |
| `aprs_ssid` | `sdrkit_aprs_get_aprs_ssid` | `sdrkit_aprs_set_aprs_ssid` |
| `aprs_tocall` | `sdrkit_aprs_get_aprs_tocall` | `sdrkit_aprs_set_aprs_tocall` |
| `aprs_path` | `sdrkit_aprs_get_aprs_path` | `sdrkit_aprs_set_aprs_path` |
| `aprs_symbol` | `sdrkit_aprs_get_aprs_symbol` | `sdrkit_aprs_set_aprs_symbol` |
| `aprs_comment` | `sdrkit_aprs_get_aprs_comment` | `sdrkit_aprs_set_aprs_comment` |
| `tx_baud` | `sdrkit_aprs_get_tx_baud` | `sdrkit_aprs_set_tx_baud` |
| `tx_delay` | `sdrkit_aprs_get_tx_delay` | `sdrkit_aprs_set_tx_delay` |
| `tx_pre_emphasis` | `sdrkit_aprs_get_tx_pre_emphasis` | `sdrkit_aprs_set_tx_pre_emphasis` |
| `beacon_interval` | `sdrkit_aprs_get_beacon_interval` | `sdrkit_aprs_set_beacon_interval` |
| `match_ssid` | `sdrkit_aprs_get_match_ssid` | `sdrkit_aprs_set_match_ssid` |
| `wide_input` | `sdrkit_aprs_get_wide_input` | `sdrkit_aprs_set_wide_input` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_option` | `sdrkit_aprs_gen_get_option` | `sdrkit_aprs_gen_set_option` |

## Try it

```sh
sdrkit run aprs --source synth:aprs
sdrkit run aprs --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/lora/aprs_afsk1200_240k.sigmf-meta`
- `tests/vectors/lora/lora_aprs_eu_300k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
