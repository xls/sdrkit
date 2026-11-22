<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Mesh (LoRa)

Meshtastic, MeshCore, LoRa APRS, MeshCom and LoRaWAN frames on one band plan, with nodes, messages and maps.

Stack id: `mesh`. Group: IoT and mesh.

Signal types: LoRa.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 400 MHz to 2.5 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 250 kHz |
| Offset from the signal | 300 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `meshtastic` | Meshtastic | 0 Hz |
| `meshcore` | MeshCore | 0 Hz |
| `lorawan` | LoRaWAN | 0 Hz |
| `loraaprs` | LoRa APRS | 0 Hz |
| `meshcom` | MeshCom | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mesh` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `mt_preset` | Meshtastic preset | choice | `LongFast` | `LongFast`, `LongSlow`, `MediumSlow`, `MediumFast`, `ShortSlow`, `ShortFast`, `LongModerate`, `ShortTurbo`, `LongTurbo`, `EdgeFastLow` |  |
| `mt_channel_name` | Primary channel name | text | empty |  | The name decides the frequency slot. Empty takes the name of the preset. |
| `mt_long_name` | Long name | text | `sdrkit` |  |  |
| `mt_short_name` | Short name | text | `SDRK` |  |  |
| `mt_role` | Role | choice | `CLIENT_MUTE` | `CLIENT` (Client), `CLIENT_MUTE` (Client (mute)), `CLIENT_HIDDEN` (Client (hidden)) |  |
| `mt_position_mode` | Position | choice | `off` | `off` (Off), `station` (The station's) |  |
| `mc_name` | Name | text | `sdrkit` |  |  |
| `mc_share_location` | Send the station's position | yes or no | false |  |  |
| `aprs_ssid` | SSID | whole number | 0 | 0 to 15 | The SSID after the callsign of the station in own frames. On AX.25 the callsign must then carry no SSID of its own. |
| `aprs_symbol` | Symbol | text | `/-` |  | Two characters: the symbol table, then the symbol code. |
| `aprs_comment` | Comment | text | empty |  | The comment of the beacon. Without a position of the station the beacon sends this text as its status. |
| `duty_limit` | Duty-cycle limit | yes or no | true |  |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `dial_freq` | Dial | frequency | 0 Hz | 0 Hz to 6 GHz | The frequency of the stack's primary channel. A change after the start asks the host to tune there. |
| `protocols` | Protocols | whole number | 15 | 0 to 31 | Bits: 1 Meshtastic, 2 MeshCore, 4 LoRa APRS, 8 MeshCom, 16 LoRaWAN. Only the mesh stack reads it. |
| `preset_search` | Every Meshtastic preset | yes or no | false |  | Also every other preset of the band (more work). |
| `mt_slot_start` | Slot band start | frequency | 0 Hz | 0 Hz to 6 GHz | The lower edge of the band in which the name of the primary channel chooses the frequency slot. With both edges 0 each preset lies at the dial. From 2.4 GHz the presets take the wide bandwidths. |
| `mt_slot_end` | Slot band end | frequency | 0 Hz | 0 Hz to 6 GHz | The upper edge of the band of the frequency slots. |
| `channels` | Channels | text | empty |  | MeshCore, LoRa APRS and MeshCom networks: entries separated by ";", each a label, "=", the protocol (meshcore, loraaprs or meshcom) and, each after "/", the frequency in Hz, the bandwidth in Hz, the spreading factor, the coding rate and, when they differ from the protocol's, the preamble and the sync word. For example "Net=meshcore/100000000/62500/8/8". At most 256 entries. A protocol without a channel listens at the dial. |
| `lw_channels` | LoRaWAN channels | text | empty |  | Entries separated by ";", each the frequency in Hz, the bandwidth in Hz, the spreading factors and the flags, separated by "/": u devices send there, d gateways answer there, j a default channel. For example "100100000/125000/7-12/udj". At most 256 entries. Empty: one channel at the dial. |
| `mt_channel_num` | Channel number | whole number | 0 | 0 to 1024 | The frequency slot, from 1. 0 takes it from the primary channel's name. With a slot band, a number above the slots of the preset in the band is refused when it is set; after a later change of the band or the preset it counts on from slot 1 again (number minus 1, modulo the slots), as Meshtastic devices count. |
| `mt_override_freq` | Frequency override | frequency | 0 Hz | 0 Hz to 6 GHz |  |
| `mt_freq_offset` | Frequency offset | frequency | 0 Hz | -1 MHz to 1 MHz |  |
| `custom` | Custom LoRa setting | yes or no | false |  |  |
| `custom_freq` | Frequency | frequency | 0 Hz | 0 Hz to 6 GHz |  |
| `custom_bw` | Bandwidth | choice | `250000` | `7812.5` (7.8 kHz), `10417` (10.4 kHz), `15625` (15.6 kHz), `20833` (20.8 kHz), `31250` (31.25 kHz), `41667` (41.7 kHz), `62500` (62.5 kHz), `125000` (125 kHz), `250000` (250 kHz), `500000` (500 kHz), `812500` (812.5 kHz (2.4 GHz)), `1625000` (1625 kHz (2.4 GHz)) |  |
| `custom_sf` | Spreading factor | whole number | 11 | 5 to 12 |  |
| `custom_cr` | Coding rate | choice | `5` | `5` (4/5), `6` (4/6), `7` (4/7), `8` (4/8) |  |
| `custom_sync` | Sync word | whole number | 43 | 0 to 255 | 0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN. |
| `custom_preamble` | Preamble | whole number | 16 | 6 to 65535 |  |
| `jump_search` | Lost-sample search | yes or no | true |  |  |
| `jump_chips` | Expected jump | number (chips) | 11.7 | 0 to 64 |  |
| `raw` | Raw payload | yes or no | false |  |  |
| `clock0` | Computer clock at signal 0 | number (s) | 0 | 0 to 1000000000000 | Seconds since 1970 at the first sample. 0 when the times follow the signal. |
| `mt_node_id` | Node ID | text | empty |  | "!xxxxxxxx". Empty takes one from the callsign of the station, or a random one. |
| `mt_licensed` | Licensed (ham) mode | yes or no | false |  | No encryption, and the callsign of the station is the long name. |
| `mt_callsign_prefix` | Callsign prefix | yes or no | false |  |  |
| `mt_hop_limit` | Hop limit | whole number | 3 | 0 to 7 |  |
| `mt_nodeinfo_interval` | NodeInfo interval | number (s) | 10800 | 0 to 1000000 | 0 sends it only when asked. |
| `mt_position_interval` | Position interval | number (s) | 900 | 30 to 1000000 |  |
| `mt_ack_broadcast` | Want-ack on channel texts | yes or no | true |  |  |
| `mc_advert_interval` | Advert interval | number (s) | 0 | 0 to 1000000 | 0 sends an advert only when asked. |
| `aprs_tocall` | Destination | text | `APRS` |  |  |
| `aprs_path` | Path | text | `WIDE1-1` |  | Calls separated by commas, at most eight. Empty sends no path. |
| `duty_bands` | Duty-cycle bands | text | empty |  | Entries separated by ";", each the lower and the upper edge in Hz separated by "-", "=" and the limit in percent of the air time of the last hour, for example "100000000-100250000=10". Empty: no band has a limit. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `auto` | `auto` (The stack's), `texting` (Meshtastic and MeshCore), `amateur` (LoRa APRS and MeshCom), `lorawan` (LoRaWAN) |  |
| `gen_damaged_copy` | Damaged copy first | yes or no | false |  | The texting scene sends one more Meshtastic text: first with one bit of its payload flipped, so that its checksum fails, and 6 s later as a good copy. The text shows with crc_ok false, and the good copy corrects it. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `preambles` | Preambles | whole number |
| `header_failed` | Headers failed | whole number |
| `mt_nodes` | Meshtastic nodes | whole number |
| `mc_nodes` | MeshCore nodes | whole number |
| `messages` | Messages | whole number |
| `stations` | APRS stations | whole number |
| `devices` | LoRaWAN devices | whole number |
| `plan` | Band plan | text |
| `searching` | Searching | text |
| `dial` | Dial | frequency |
| `protocols` | Protocols | text |
| `custom` | Custom setting | yes or no |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `mt_node_id` | Meshtastic node | text |
| `mt_public_key` | Meshtastic public key | text |
| `mt_long_name` | Long name | text |
| `mt_short_name` | Short name | text |
| `mt_in_flight` | Waiting for an ack | whole number |
| `mt_rebroadcasts` | Rebroadcasts | whole number |
| `mt_warnings` | Warnings | text |
| `mt_freq` | Meshtastic frequency | frequency |
| `mc_public_key` | MeshCore public key | text |
| `mc_name` | MeshCore name | text |
| `mc_contacts` | Contacts | whole number |
| `mc_freq` | MeshCore frequency | frequency |
| `aprs_call` | APRS call | text |
| `aprs_freq` | APRS frequency | frequency |
| `tx_pending` | Waiting to send | whole number |
| `tx_sent` | Sent | whole number |
| `bad_samples` | Bad samples | whole number |
| `modulate.tx_bursts` | Bursts written | whole number |
| `modulate.tx_samples` | Samples written | whole number |
| `modulate.tx_dropped` | Frames dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `packets` | Packets | `time_s` (number (s)), `net` (choice), `type` (text), `source` (text), `target` (text), `bytes` (whole number), `content` (text), `hops` (whole number), `snr_db` (number (dB)), `sf` (whole number), `bw` (frequency), `summary` (text) |
| `nodes` | Nodes | `net` (choice), `name` (text), `short` (text), `id` (text), `hw` (text), `role` (text), `last_heard` (time), `heard` (whole number), `snr_db` (number (dB)), `hops` (whole number), `battery_pct` (number (%)), `voltage_v` (number (V)), `position` (position) |
| `messages` | Messages | `time_s` (number (s)), `net` (choice), `channel` (text), `source` (text), `sender` (text), `target` (text), `text` (text), `hops` (whole number), `state` (choice), `failure` (text), `relays` (whole number), `packet_id` (whole number), `utc` (time), `snr_db` (number (dB)), `level_dbfs` (number (dBFS)), `crc_ok` (yes or no), `copies` (whole number) |
| `readings` | Readings | `time_s` (number (s)), `node` (text), `battery_pct` (number (%)), `voltage_v` (number (V)), `ch_util_pct` (number (%)), `air_tx_pct` (number (%)), `temp_c` (number (°C)), `humidity_pct` (number (%)), `pressure_hpa` (number (hPa)), `position` (position), `alt_m` (number (m)) |
| `stations` | Stations | `net` (choice), `call` (text), `symbol` (text), `position` (position), `type` (text), `last_heard` (time), `heard` (whole number), `snr_db` (number (dB)), `path` (text), `comment` (text), `battery_pct` (number (%)) |
| `devices` | Devices | `id` (text), `network` (text), `vendor` (text), `last_heard` (time), `heard` (whole number), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `fcnt` (whole number), `missed` (whole number), `sf` (whole number), `freq` (frequency), `uplinks` (whole number), `downlinks` (whole number), `joins` (whole number), `type` (text), `fport` (whole number), `mac` (text) |
| `types` | Packet types | `net` (choice), `type` (text), `count` (whole number) |
| `decoders` | Decoders | `net` (choice), `preset` (text), `freq` (frequency), `offset` (frequency), `sf` (whole number), `bw` (frequency), `cr` (whole number), `sync` (whole number), `preamble` (whole number), `ldro` (yes or no), `in_band` (yes or no), `frames` (whole number), `errors` (whole number), `snr_db` (number (dB)), `cfo` (frequency), `last_s` (number (s)) |
| `contacts` | Contacts | `name` (text), `key` (text), `last_heard` (time), `snr_db` (number (dB)), `path` (text) |
| `channels` | Channels | `net` (choice), `channel` (text), `key_set` (yes or no) |
| `slots` | Meshtastic slots | `slot` (whole number), `freq_hz` (frequency), `current` (yes or no), `preset_default` (yes or no) |

## Map points

The stack puts points on a map: Nodes and stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `lora.packet` | LoRa packet | `net` (choice), `modulation` (choice), `snr_db` (number (dB)), `sf` (whole number), `payload` (bytes), `crc_ok` (yes or no), `source` (text), `target` (text), `devaddr` (text), `iq_inverted` (yes or no), `crc` (yes or no), `bw` (frequency), `cr` (whole number), `implicit` (yes or no), `sync` (whole number), `level_dbfs` (number (dBFS)), `family` (choice) |
| `mesh.text` | Mesh text message | `source` (text), `target` (text), `channel` (text), `text` (text), `net` (choice), `sender` (text), `hops` (whole number), `packet_id` (whole number), `reply_id` (whole number), `emoji` (whole number), `own` (yes or no), `state` (choice), `family` (choice), `snr_db` (number (dB)), `level_dbfs` (number (dBFS)), `crc_ok` (yes or no) |
| `mesh.node` | Mesh node heard | `source` (text), `name` (text), `position` (position), `net` (choice), `family` (choice) |
| `tx.request` | Frame to send | `id` (whole number), `what` (choice), `payload` (bytes), `freq_hz` (frequency), `not_before_s` (number (s)), `air_time` (duration (s)), `message_key` (whole number), `setting` (text), `afsk_baud` (whole number (bit/s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `mt.send` | Send a Meshtastic text | `text` (text, required), `channel` (text, default empty), `to` (text, default `^all`), `reply_id` (whole number, default 0), `emoji` (whole number, default 0), `want_ack` (choice, default `setting`), `hop_limit` (whole number, default 0) | `packet_id` (whole number) | To a node ("!xxxxxxxx") or to the channel ("^all"). The reply holds the packet id of the text; the column `state` of its row in the messages says whether it was delivered. |
| `mc.send` | Send a MeshCore text | `text` (text, required), `channel` (text, default empty), `to` (text, default empty) |  |  |
| `aprs.send` | Send an APRS message | `to` (text, required), `text` (text, required) | `packet_id` (whole number) | Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. |
| `mt.channel.add` | Add a Meshtastic channel | `name` (text, required), `key` (text, default empty) |  | The session reads the texts of the channel and can send on it. The key is kept and never shown. |
| `mc.channel.add` | Add a MeshCore channel | `name` (text, required), `key` (text, default empty) |  | The session reads the texts of the channel and can send on it. The key is kept and never shown. |
| `channel.remove` | Remove a channel | `row` (text, required) |  |  |
| `mc.advert` | Send a MeshCore advert | `scope` (choice, default `flood`) |  |  |
| `mc.contact.add` | Add a MeshCore contact | `key` (text, required), `name` (text, default empty) |  |  |
| `aprs.beacon` | Send an APRS beacon |  |  | The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. |
| `mt.request` | Ask a Meshtastic node | `node` (text, required), `what` (choice, default `nodeinfo`) |  |  |
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `reset` | Reset |  |  | Forgets the signal and makes the bank again. |
| `mt.channel` | Set a Meshtastic channel slot | `index` (whole number, default 0), `name` (text, required), `key` (text, default empty), `precision` (whole number (bits), default 13) |  | Sets the key and the position precision of one of the 8 channel slots. Slot 0 is the primary channel: its name comes from the setting `mt_channel_name`. |
| `mt.identity` | Set the Meshtastic key | `key` (text, required) |  |  |
| `mc.identity` | Set the MeshCore identity | `key` (text, required) |  |  |

## Typed interface

The stack is in the family `lora`, version 3.1. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Mesh messaging (`mesh`). Media: text.

### Result fields

Struct `MeshFields`. The result fields of the mesh decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames with a good CRC since the last reset. |
| `frames_bad` | whole number | always | 1.0 | Frames whose CRC failed since the last reset. |
| `preambles` | whole number | always | 1.0 | Frame preambles seen. |
| `header_failed` | whole number | always | 1.0 | Headers that failed their checksum. |
| `mt_nodes` | whole number | always | 1.0 | Meshtastic nodes. |
| `mc_nodes` | whole number | always | 1.0 | MeshCore nodes. |
| `messages` | whole number | always | 1.0 | Messages in the table. |
| `stations` | whole number | always | 1.0 | APRS stations. |
| `devices` | whole number | always | 1.0 | LoRaWAN devices. |
| `plan` | text | always | 1.0 | The band plan. |
| `searching` | text | always | 1.0 | What is searched. |
| `dial` | frequency | always | 1.0 | The dial. |
| `protocols` | text | always | 1.0 | The protocols searched, in words. |
| `custom` | yes or no | always | 1.0 | The custom LoRa setting is searched. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last good frame, while frames come. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last good frame, while frames come. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the bank reports. |

### Result fields

Struct `MeshMtFields`. The own Meshtastic node, once the Meshtastic client reports.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mt_node_id` | text | optional | 1.0 | The own node id. |
| `mt_public_key` | text | optional | 1.0 | The own public key, base64. |
| `mt_long_name` | text | optional | 1.0 | The own long name. |
| `mt_short_name` | text | optional | 1.0 | The own short name. |
| `mt_in_flight` | whole number | optional | 1.0 | Own packets waiting for an acknowledgement. |
| `mt_rebroadcasts` | whole number | optional | 1.0 | Packets of other nodes sent again. |
| `mt_warnings` | text | optional | 1.0 | Warnings about the settings of the own node. |
| `mt_freq` | frequency | optional | 1.0 | The channel frequency that the client sends on, while the session listens to a channel of the network. |

### Result fields

Struct `MeshMcFields`. The own MeshCore node, once the MeshCore client reports.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mc_public_key` | text | optional | 1.0 | The own public key, hex. |
| `mc_name` | text | optional | 1.0 | The own name. |
| `mc_contacts` | whole number | optional | 1.0 | The contacts. |
| `mc_freq` | frequency | optional | 1.0 | The channel frequency that the client sends on, while the session listens to a channel of the network. |

### Result fields

Struct `HamFields`. The own APRS station, once the amateur client reports.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aprs_call` | text | optional | 1.0 | The source call of own APRS frames: the callsign of the station with the SSID. Empty without a callsign. |
| `aprs_freq` | frequency | optional | 1.0 | The frequency that the client sends on, while the session listens to a channel of its link. |

### Result fields

Struct `MeshTxFields`. The state of the transmit scheduler.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_pending` | whole number | always | 1.0 | Frames waiting to be sent. |
| `tx_sent` | whole number | always | 1.0 | Frames sent. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `packets`

Struct `MeshPacketRow`. A row of the packet list. The row key is the number of the packet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `net` | choice | always | 1.0 | The protocol. |
| `type` | text | optional | 1.0 | The packet type, as the protocol names it. |
| `source` | text | optional | 1.0 | The sender. |
| `target` | text | optional | 1.0 | The addressee. |
| `bytes` | whole number | always | 1.0 | The payload length. |
| `content` | text | optional | 1.0 | The content, decoded or raw. |
| `hops` | whole number | optional | 1.0 | The hops taken. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `sf` | whole number | always | 1.0 | The spreading factor. |
| `bw` | frequency | always | 1.0 | The bandwidth. |
| `summary` | text | optional | 1.0 | More about the packet. |

The column `source` names a row.

### Table `nodes`

Struct `MeshNodeRow`. A row of the node list. The row key is `<net key>/<id>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol. |
| `name` | text | always | 1.0 | The node name. |
| `short` | text | optional | 1.0 | The short name. |
| `id` | text | always | 1.0 | The node id. |
| `hw` | text | optional | 1.0 | The hardware model. |
| `role` | text | optional | 1.0 | The role. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet. |
| `heard` | whole number | optional | 1.0 | The packets of the node. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last packet. |
| `hops` | whole number | optional | 1.0 | The hops of the last packet. |
| `battery_pct` | number (%) | optional | 1.0 | The battery charge; above 100 the node is powered from outside. |
| `voltage_v` | number (V) | optional | 1.0 | The battery voltage. |
| `position` | position | optional | 1.0 | The last position. |

The column `name` names a row. Place of a row: the column `position`.

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

The column `sender` names a row.

### Table `readings`

Struct `MeshReadingRow`. A row of the telemetry and position readings. The row key is the number of the reading.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `node` | text | always | 1.0 | The node of the reading. |
| `battery_pct` | number (%) | optional | 1.0 | The battery charge. |
| `voltage_v` | number (V) | optional | 1.0 | The battery voltage. |
| `ch_util_pct` | number (%) | optional | 1.0 | The channel use. |
| `air_tx_pct` | number (%) | optional | 1.0 | The own air time. |
| `temp_c` | number (°C) | optional | 1.0 | The temperature. |
| `humidity_pct` | number (%) | optional | 1.0 | The relative humidity. |
| `pressure_hpa` | number (hPa) | optional | 1.0 | The barometric pressure. |
| `position` | position | optional | 1.0 | The position. |
| `alt_m` | number (m) | optional | 1.0 | The altitude. |

Place of a row: the column `position`. Height: `alt_m`.

### Table `stations`

Struct `MeshStationRow`. A row of the APRS stations of LoRa APRS and MeshCom. The row key is `<net key>/<call>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol. |
| `call` | text | always | 1.0 | The call or the object name. |
| `symbol` | text | optional | 1.0 | The map symbol. |
| `position` | position | optional | 1.0 | The last position. |
| `type` | text | optional | 1.0 | The type of the last packet. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet. |
| `heard` | whole number | optional | 1.0 | The packets of the station. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the last packet. |
| `path` | text | optional | 1.0 | The relay path. |
| `comment` | text | optional | 1.0 | The comment. |
| `battery_pct` | number (%) | optional | 1.0 | The battery charge. |

The column `call` names a row. Place of a row: the column `position`.

### Table `devices`

Struct `MeshDeviceRow`. A row of the LoRaWAN devices. The row key is the device id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | text | always | 1.0 | The DevAddr or the DevEUI. |
| `network` | text | optional | 1.0 | The network operator. |
| `vendor` | text | optional | 1.0 | The maker, or the JoinEUI. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `heard` | whole number | optional | 1.0 | The frames of the device. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the last frame. |
| `fcnt` | whole number | optional | 1.0 | The last frame counter. |
| `missed` | whole number | always | 1.0 | The frames that the counter missed. |
| `sf` | whole number | optional | 1.0 | The spreading factor of the last frame, none for FSK. |
| `freq` | frequency | always | 1.0 | The frequency of the last frame. |
| `uplinks` | whole number | always | 1.0 | Uplinks. |
| `downlinks` | whole number | always | 1.0 | Downlinks. |
| `joins` | whole number | always | 1.0 | Join requests. |
| `type` | text | optional | 1.0 | The type of the last frame. |
| `fport` | whole number | optional | 1.0 | The last FPort. |
| `mac` | text | optional | 1.0 | The last MAC commands or status. |

The column `id` names a row.

### Table `types`

Struct `MeshTypeRow`. A row of the packet counts by protocol and type. The row key is `<net key>/<type>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol. |
| `type` | text | always | 1.0 | The packet type. |
| `count` | whole number | always | 1.0 | The packets of the type. |

### Table `decoders`

Struct `MeshDecoderRow`. A row of the decoders: one searched LoRa setting. The row key is the entry number of the plan.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol. |
| `preset` | text | always | 1.0 | The preset. |
| `freq` | frequency | always | 1.0 | The channel frequency. |
| `offset` | frequency | always | 1.0 | The channel offset from the dial. |
| `sf` | whole number | always | 1.0 | The spreading factor. |
| `bw` | frequency | always | 1.0 | The bandwidth. |
| `cr` | whole number | always | 1.0 | The coding rate denominator: 5 is 4/5, 8 is 4/8. |
| `sync` | whole number | always | 1.0 | The sync word. |
| `preamble` | whole number | always | 1.0 | The preamble length. |
| `ldro` | yes or no | always | 1.0 | Low data rate optimisation. |
| `in_band` | yes or no | always | 1.0 | The channel lies inside the captured band. |
| `frames` | whole number | always | 1.0 | Frames decoded. |
| `errors` | whole number | always | 1.0 | Frames lost to errors. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last good frame. |
| `cfo` | frequency | optional | 1.0 | The carrier offset of the last good frame. |
| `last_s` | number (s) | optional | 1.0 | The signal time of the last frame. |

### Table `contacts`

Struct `MeshContactRow`. A row of the MeshCore contacts. The row key is the hex of the public key.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name. |
| `key` | text | always | 1.0 | The first 12 hex digits of the public key. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet, when the clock is known. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the last packet. |
| `path` | text | always | 1.0 | The learned path, or "flood". |

The column `name` names a row.

### Table `channels`

Struct `MeshChannelRow`. A row of the channels with keys. The row key is `<net key>/<name>`. The key itself is never shown.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol. |
| `channel` | text | always | 1.0 | The name of the channel. |
| `key_set` | yes or no | always | 1.0 | The channel holds a key. |

### Table `slots`

Struct `MeshSlotRow`. A row of the Meshtastic frequency slots of the primary preset in the slot band. The row key is the slot number with four digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `slot` | whole number | always | 1.0 | The slot number, from 1, as the setting `mt_channel_num` counts. |
| `freq_hz` | frequency | always | 1.0 | The centre of the slot plus the setting `mt_freq_offset`. |
| `current` | yes or no | always | 1.0 | The slot that the stack listens on: that of `mt_channel_num`, else of the hash of `mt_channel_name`; with `mt_override_freq` the slot whose frequency it is, and no row for a frequency between slots. |
| `preset_default` | yes or no | always | 1.0 | The slot of the default channel name of the primary preset. |

### Event `lora.packet`

Struct `LoraPacketEvent`. The fields of one LoRa or FSK frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `net` | choice | always | 1.0 | The protocol of the frame. |
| `modulation` | choice | always | 1.0 | How the frame was modulated. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `sf` | whole number | optional | 1.0 | The spreading factor, none for FSK. |
| `payload` | bytes | always | 1.0 | The payload. |
| `crc_ok` | yes or no | always | 1.0 | The CRC is right, or the frame has none. |
| `source` | text | optional | 1.0 | The sender, if the protocol names one. |
| `target` | text | optional | 1.0 | The addressee, if the protocol names one. |
| `devaddr` | text | optional | 1.0 | LoRaWAN: the device address, eight hex digits. |
| `iq_inverted` | yes or no | optional | 1.0 | LoRaWAN: the IQ is inverted (a downlink). |
| `crc` | yes or no | optional | 1.0 | The frame has a CRC. |
| `bw` | frequency | optional | 1.0 | The bandwidth. |
| `cr` | whole number | optional | 1.0 | The coding rate denominator. |
| `implicit` | yes or no | optional | 1.0 | The header is implicit. |
| `sync` | whole number | optional | 1.0 | The sync word. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the frame. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `mesh.text`

Struct `MeshTextEvent`. The fields of a text message, sent or received.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender. |
| `target` | text | always | 1.0 | The addressee. |
| `channel` | text | always | 1.0 | The channel. |
| `text` | text | always | 1.0 | The text. |
| `net` | choice | always | 1.0 | The protocol. |
| `sender` | text | optional | 1.0 | The name of the sender, if known. |
| `hops` | whole number | optional | 1.0 | The hops taken, if known. |
| `packet_id` | whole number | always | 1.0 | The packet id. |
| `reply_id` | whole number | always | 1.0 | The message that the text replies to. |
| `emoji` | whole number | always | 1.0 | A reaction, as a Unicode code point. |
| `own` | yes or no | always | 1.0 | Sent by the own node. |
| `state` | choice | optional | 1.0 | The delivery state of an own message. |
| `family` | choice | always | 1.0 | The family of the event. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the frame of a received text. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the frame of a received text, relative to the full scale of the input (not an absolute power). |
| `crc_ok` | yes or no | optional | 1.0 | The text comes from a copy with a good CRC. False for a damaged copy shown after the wait; a later good copy sends the text again with true. None for an own message. |

### Event `mesh.node`

Struct `MeshNodeEvent`. The fields of a node heard for the first time or with news.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The node id. |
| `name` | text | optional | 1.0 | The node name, if known. |
| `position` | position | optional | 1.0 | The position, if known. |
| `net` | choice | always | 1.0 | The protocol. |
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
| `mt.send` | `sdrkit_mesh_mt_send` | `sdrkit_lora_mt_send_args` | `sdrkit_lora_mt_send_reply` |
| `mc.send` | `sdrkit_mesh_mc_send` | `sdrkit_lora_mc_send_args` | none |
| `aprs.send` | `sdrkit_mesh_aprs_send` | `sdrkit_aprs_send_args` | `sdrkit_aprs_send_reply` |
| `mt.channel.add` | `sdrkit_mesh_mt_channel_add` | `sdrkit_lora_mt_channel_add_args` | none |
| `mc.channel.add` | `sdrkit_mesh_mc_channel_add` | `sdrkit_lora_mc_channel_add_args` | none |
| `channel.remove` | `sdrkit_mesh_channel_remove` | `sdrkit_lora_channel_remove_args` | none |
| `mc.advert` | `sdrkit_mesh_mc_advert` | `sdrkit_lora_mc_advert_args` | none |
| `mc.contact.add` | `sdrkit_mesh_mc_contact_add` | `sdrkit_lora_mc_contact_add_args` | none |
| `aprs.beacon` | `sdrkit_mesh_aprs_beacon` | none | none |
| `mt.request` | `sdrkit_mesh_mt_request` | `sdrkit_lora_mt_request_args` | none |
| `clear` | `sdrkit_mesh_clear` | none | none |
| `remove` | `sdrkit_mesh_remove` | `sdrkit_mesh_remove_args` | none |
| `reset` | `sdrkit_mesh_reset` | none | none |
| `mt.channel` | `sdrkit_mesh_mt_channel` | `sdrkit_lora_mt_channel_args` | none |
| `mt.identity` | `sdrkit_mesh_mt_identity` | `sdrkit_lora_identity_args` | none |
| `mc.identity` | `sdrkit_mesh_mc_identity` | `sdrkit_lora_identity_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `dial_freq` | `sdrkit_mesh_get_dial_freq` | `sdrkit_mesh_set_dial_freq` |
| `protocols` | `sdrkit_mesh_get_protocols` | `sdrkit_mesh_set_protocols` |
| `preset_search` | `sdrkit_mesh_get_preset_search` | `sdrkit_mesh_set_preset_search` |
| `mt_slot_start` | `sdrkit_mesh_get_mt_slot_start` | `sdrkit_mesh_set_mt_slot_start` |
| `mt_slot_end` | `sdrkit_mesh_get_mt_slot_end` | `sdrkit_mesh_set_mt_slot_end` |
| `channels` | `sdrkit_mesh_get_channels` | `sdrkit_mesh_set_channels` |
| `lw_channels` | `sdrkit_mesh_get_lw_channels` | `sdrkit_mesh_set_lw_channels` |
| `mt_preset` | `sdrkit_mesh_get_mt_preset` | `sdrkit_mesh_set_mt_preset` |
| `mt_channel_name` | `sdrkit_mesh_get_mt_channel_name` | `sdrkit_mesh_set_mt_channel_name` |
| `mt_channel_num` | `sdrkit_mesh_get_mt_channel_num` | `sdrkit_mesh_set_mt_channel_num` |
| `mt_override_freq` | `sdrkit_mesh_get_mt_override_freq` | `sdrkit_mesh_set_mt_override_freq` |
| `mt_freq_offset` | `sdrkit_mesh_get_mt_freq_offset` | `sdrkit_mesh_set_mt_freq_offset` |
| `custom` | `sdrkit_mesh_get_custom` | `sdrkit_mesh_set_custom` |
| `custom_freq` | `sdrkit_mesh_get_custom_freq` | `sdrkit_mesh_set_custom_freq` |
| `custom_bw` | `sdrkit_mesh_get_custom_bw` | `sdrkit_mesh_set_custom_bw` |
| `custom_sf` | `sdrkit_mesh_get_custom_sf` | `sdrkit_mesh_set_custom_sf` |
| `custom_cr` | `sdrkit_mesh_get_custom_cr` | `sdrkit_mesh_set_custom_cr` |
| `custom_sync` | `sdrkit_mesh_get_custom_sync` | `sdrkit_mesh_set_custom_sync` |
| `custom_preamble` | `sdrkit_mesh_get_custom_preamble` | `sdrkit_mesh_set_custom_preamble` |
| `jump_search` | `sdrkit_mesh_get_jump_search` | `sdrkit_mesh_set_jump_search` |
| `jump_chips` | `sdrkit_mesh_get_jump_chips` | `sdrkit_mesh_set_jump_chips` |
| `raw` | `sdrkit_mesh_get_raw` | `sdrkit_mesh_set_raw` |
| `clock0` | `sdrkit_mesh_get_clock0` | `sdrkit_mesh_set_clock0` |
| `mt_node_id` | `sdrkit_mesh_get_mt_node_id` | `sdrkit_mesh_set_mt_node_id` |
| `mt_long_name` | `sdrkit_mesh_get_mt_long_name` | `sdrkit_mesh_set_mt_long_name` |
| `mt_short_name` | `sdrkit_mesh_get_mt_short_name` | `sdrkit_mesh_set_mt_short_name` |
| `mt_role` | `sdrkit_mesh_get_mt_role` | `sdrkit_mesh_set_mt_role` |
| `mt_licensed` | `sdrkit_mesh_get_mt_licensed` | `sdrkit_mesh_set_mt_licensed` |
| `mt_callsign_prefix` | `sdrkit_mesh_get_mt_callsign_prefix` | `sdrkit_mesh_set_mt_callsign_prefix` |
| `mt_hop_limit` | `sdrkit_mesh_get_mt_hop_limit` | `sdrkit_mesh_set_mt_hop_limit` |
| `mt_nodeinfo_interval` | `sdrkit_mesh_get_mt_nodeinfo_interval` | `sdrkit_mesh_set_mt_nodeinfo_interval` |
| `mt_position_mode` | `sdrkit_mesh_get_mt_position_mode` | `sdrkit_mesh_set_mt_position_mode` |
| `mt_position_interval` | `sdrkit_mesh_get_mt_position_interval` | `sdrkit_mesh_set_mt_position_interval` |
| `mt_ack_broadcast` | `sdrkit_mesh_get_mt_ack_broadcast` | `sdrkit_mesh_set_mt_ack_broadcast` |
| `mc_name` | `sdrkit_mesh_get_mc_name` | `sdrkit_mesh_set_mc_name` |
| `mc_share_location` | `sdrkit_mesh_get_mc_share_location` | `sdrkit_mesh_set_mc_share_location` |
| `mc_advert_interval` | `sdrkit_mesh_get_mc_advert_interval` | `sdrkit_mesh_set_mc_advert_interval` |
| `aprs_ssid` | `sdrkit_mesh_get_aprs_ssid` | `sdrkit_mesh_set_aprs_ssid` |
| `aprs_tocall` | `sdrkit_mesh_get_aprs_tocall` | `sdrkit_mesh_set_aprs_tocall` |
| `aprs_path` | `sdrkit_mesh_get_aprs_path` | `sdrkit_mesh_set_aprs_path` |
| `aprs_symbol` | `sdrkit_mesh_get_aprs_symbol` | `sdrkit_mesh_set_aprs_symbol` |
| `aprs_comment` | `sdrkit_mesh_get_aprs_comment` | `sdrkit_mesh_set_aprs_comment` |
| `duty_limit` | `sdrkit_mesh_get_duty_limit` | `sdrkit_mesh_set_duty_limit` |
| `duty_bands` | `sdrkit_mesh_get_duty_bands` | `sdrkit_mesh_set_duty_bands` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_mesh_gen_get_scene` | `sdrkit_mesh_gen_set_scene` |
| `gen_damaged_copy` | `sdrkit_mesh_gen_get_damaged_copy` | `sdrkit_mesh_gen_set_damaged_copy` |

## Try it

```sh
sdrkit run mesh --source synth:mesh
sdrkit run mesh --source synth:mesh --submode meshtastic
sdrkit run mesh --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/lora/lorawan_eu868_700k.sigmf-meta`
- `tests/vectors/lora/meshcom_eu_600k.sigmf-meta`
- `tests/vectors/lora/meshcore_eu_narrow_250k.sigmf-meta`
- `tests/vectors/lora/meshtastic_anz_tbeam_2m.sigmf-meta`
- `tests/vectors/lora/meshtastic_longfast_600k.sigmf-meta`
<!-- generated: end -->
