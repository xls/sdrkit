<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Bluetooth LE

Bluetooth LE advertising on LE 1M, LE 2M and LE Coded, legacy and extended, on every channel inside the capture: the devices around, their names, vendors and beacons, as a scanner app shows them.

Stack id: `ble`. Group: Data and IoT.

Signal types: Bluetooth LE.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.402 GHz |
| Tuning range | 2.4 GHz to 2.484 GHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 2 MHz |
| Offset from the signal | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ble` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | choice | `37` | `37` (37 2402 MHz), `38` (38 2426 MHz), `39` (39 2480 MHz) | Asks the host to tune to the advertising channel. |
| `hop` | Hop | yes or no | false |  | Asks the host to step 37, 38, 39: some devices skip a channel, and interference differs between them. |
| `hop_seconds` | Dwell | duration (s) | 3 s | 1 to 30 | Time on each channel while hopping. |
| `filter_text` | Filter | text | empty |  | Shows only the devices whose name, address, vendor or beacon holds this text. |
| `min_level_dbfs` | Level above | number (dBFS) | -120 | -120 to 0 |  |
| `named_only` | Named only | yes or no | false |  |  |
| `att_values` | ATT values | yes or no | true |  | Keeps the value bytes of the attribute messages read on a connection. Off, the ATT log holds the handle, the type and the length only. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_channel` | Whitening | choice | `round` | `round` (Round the three channels), `37` (All on 37), `38` (All on 38), `39` (All on 39) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `devices` | Devices | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `aa_hits` | Access address hits | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `deviation_hz` | Deviation | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `channel` | Channel | whole number |
| `last_channel` | Last channel | whole number |
| `packets_37` | Packets on 37 | whole number |
| `packets_38` | Packets on 38 | whole number |
| `packets_39` | Packets on 39 | whole number |
| `hopping` | Hopping | yes or no |
| `privacy` | Privacy | text |
| `packets_secondary` | Packets on secondary channels | whole number |
| `packets_2m` | Packets on LE 2M | whole number |
| `packets_coded` | Packets on LE Coded | whole number |
| `channels_in_capture` | Channels in the capture | whole number |
| `aux_followed` | Auxiliary packets followed | whole number |
| `aux_outside` | Pointers outside the capture | whole number |
| `periodic_trains` | Periodic trains | whole number |
| `connections_followed` | Connections followed | whole number |
| `connection_packets` | Connection packets | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Packets sent | whole number |
| `tx_left` | Packets left | whole number |
| `adv.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Devices | `address` (text), `address_type` (choice), `name` (text), `level_dbfs` (number (dBFS)), `last_heard` (time), `heard` (whole number), `vendor` (text), `beacon` (text), `channels` (text), `connectable` (yes or no), `scanned` (yes or no), `manufacturer_data` (text), `first_heard` (time), `phys` (text), `extended` (yes or no), `data_outside` (yes or no), `set_id` (whole number), `periodic_interval_ms` (number (ms)), `device_kind` (choice), `tracker` (yes or no) |
| `packets` | Packets | `signal_s` (number (s)), `channel` (whole number), `pdu` (text), `address` (text), `level_dbfs` (number (dBFS)), `length` (whole number (B)), `details` (text), `phy` (text) |
| `pdu_types` | PDU types | `pdu` (text), `packets` (whole number) |
| `device_levels` | Device levels | `device` (text), `sample` (whole number), `level_dbfs` (number (dBFS)) |
| `connections` | Connections | `access_address` (text), `central` (text), `peripheral` (text), `interval_ms` (number (ms)), `latency` (whole number), `timeout_ms` (number (ms)), `phy` (text), `csa` (whole number), `channels_used` (whole number), `packets` (whole number), `encrypted` (yes or no), `procedures` (text), `att` (whole number), `ended` (yes or no), `first_heard` (time), `last_heard` (time) |
| `att` | ATT | `signal_s` (number (s)), `connection` (text), `from_central` (yes or no), `opcode` (text), `handle` (whole number), `service` (text), `value` (text), `length` (whole number (B)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ble.device` | Bluetooth LE device | `source` (text), `text` (text), `level_dbfs` (number (dBFS)), `family` (choice) |
| `ble.tx` | Bluetooth LE transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send an advertisement | `address` (text, required), `address_type` (choice, default `random`), `pdu_type` (choice, default `adv_nonconn_ind`), `data` (text, default empty), `name` (text, default empty), `uuid16` (text, default empty), `company` (text, default empty), `manufacturer_data` (text, default empty), `count` (whole number, default 1), `interval_ms` (whole number (ms), default 100) |  | Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: 2402, 2426 or 2480 MHz. A send during another waits for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the packets of the send on air that are still to go, and the sends that wait. |

## Typed interface

The stack is in the family `wlan`, version 4.3. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

Key readings: `devices`.

### Result fields

Struct `BleFields`. The result fields of `ble`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `devices` | whole number | always | 1.0 | Devices in the table. |
| `frames_ok` | whole number | always | 1.0 | Good packets. |
| `frames_bad` | whole number | always | 1.0 | Exact access-address matches whose CRC failed on every channel. |
| `aa_hits` | whole number | always | 1.0 | Access-address candidates. |
| `snr_db` | number (dB) | optional | 1.0 | The running mean of the eye SNR. No value before the first good packet. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last good packet. No value before it. |
| `deviation_hz` | frequency | optional | 1.0 | The deviation of the last good packet. No value before it. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the channel. No value before the first input. |
| `channel` | whole number | optional | 1.0 | The tuned advertising channel. No value when it is not known, as for a recording. |
| `last_channel` | whole number | optional | 1.0 | The channel of the last good packet. No value before it. |
| `packets_37` | whole number | always | 1.0 | Good packets on channel 37. |
| `packets_38` | whole number | always | 1.0 | Good packets on channel 38. |
| `packets_39` | whole number | always | 1.0 | Good packets on channel 39. |
| `hopping` | yes or no | always | 1.0 | The host steps through the advertising channels. |
| `privacy` | text | always | 1.0 | The privacy note. |
| `packets_secondary` | whole number | always | 1.0 | Good packets on the secondary advertising channels and the periodic trains (channels 0 to 36). |
| `packets_2m` | whole number | always | 1.0 | Good packets on the LE 2M PHY. |
| `packets_coded` | whole number | always | 1.0 | Good packets on the LE Coded PHY. |
| `channels_in_capture` | whole number | always | 1.0 | The BLE channels inside the capture: the primary ones are received all the time, a secondary one while a packet points to it. 0 when the centre of the input is not known. |
| `aux_followed` | whole number | always | 1.0 | Auxiliary packets reached through the AuxPtr that pointed to them. |
| `aux_outside` | whole number | always | 1.0 | AuxPtr fields that point to a channel outside the capture: their data cannot be received here. |
| `periodic_trains` | whole number | always | 1.0 | Periodic advertising trains whose packets the receivers take. |
| `connections_followed` | whole number | always | 4.2 | Connections whose events the receivers follow now (from a CONNECT_IND or AUX_CONNECT_REQ heard). |
| `connection_packets` | whole number | always | 4.2 | Good packets of followed connections. |

### Result fields

Struct `BleTxFields`. The result fields of the advertising sender of `ble` and `bletracker`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.0 | Whether a send is on air. |
| `tx_sent` | whole number | always | 1.0 | The packets of the send on air that went out. |
| `tx_left` | whole number | always | 1.0 | The packets still to go: the rest of the send on air and the sends that wait. |

### Table `devices`

Struct `BleDeviceRow`. A row of the devices of `ble`. The row key is the address and "r" (random) or "p" (public).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address. |
| `address_type` | choice | always | 1.0 | The kind of the address. |
| `name` | text | optional | 1.0 | The name, if a packet gave one. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last packet. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet. |
| `heard` | whole number | optional | 1.0 | Packets heard. |
| `vendor` | text | optional | 1.0 | The vendor, if known. |
| `beacon` | text | optional | 1.0 | The beacon, the services and the TX power, if a packet gave them. |
| `channels` | text | always | 1.0 | The channels the device was heard on, as "37 38 39". |
| `connectable` | yes or no | always | 1.0 | The device takes connections. |
| `scanned` | yes or no | always | 1.0 | Another device sent a scan request to it. |
| `manufacturer_data` | text | optional | 1.0 | The first 32 bytes of manufacturer data after the company identifier, hex, if a packet gave them. |
| `first_heard` | time | optional | 1.0 | The UTC of the first packet. |
| `phys` | text | always | 1.0 | The PHYs the device was heard on, as "LE 1M, LE Coded". |
| `extended` | yes or no | always | 1.0 | The device uses extended advertising. |
| `data_outside` | yes or no | always | 1.0 | An AuxPtr of the device points to a channel outside the capture: the row holds what the primary packets gave. |
| `set_id` | whole number | optional | 1.0 | The advertising set id (SID) of the last extended packet, if one had it. |
| `periodic_interval_ms` | number (ms) | optional | 1.0 | The interval of the device's periodic advertising train, if a SyncInfo gave one. |
| `device_kind` | choice | always | 1.0 | The kind of device its Appearance or beacon format declares. |
| `tracker` | yes or no | always | 1.0 | Its advertising data matches a rule of the tracker detector of `bletracker` (Find My, Tile, Chipolo, SmartTag, Google Find Hub). |

The rows are the stations or nodes of the stack. The column `name` names a row. Default order: the column `level_dbfs`, highest first. Time: `last_heard`.

### Table `packets`

Struct `BlePacketRow`. A row of the packet log of `ble`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_s` | number (s) | always | 1.0 | The signal time of the packet. |
| `channel` | whole number | always | 1.0 | The advertising channel of the packet. |
| `pdu` | text | always | 1.0 | The PDU type name. |
| `address` | text | optional | 1.0 | The address of the sender, if the PDU has one. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the packet. |
| `length` | whole number (B) | always | 1.0 | The payload length. |
| `details` | text | always | 1.0 | The one-line summary. |
| `phy` | text | always | 1.0 | The PHY of the packet ("LE 1M", "LE 2M", "LE Coded S=8" ...). |

The rows are a log: they arrive over time. The column `address` names a row. Time: `signal_s`.

### Table `pdu_types`

Struct `BlePduTypeRow`. A row of the good packets of each PDU type of `ble`. The row key is the PDU type name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pdu` | text | always | 1.0 | The PDU type name. |
| `packets` | whole number | always | 1.0 | Good packets of the type. |

### Table `device_levels`

Struct `BleDeviceLevelRow`. A row of the level history of the devices of `ble`. The row key is `<device row>/<slot>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `device` | text | always | 1.0 | The row of the device. |
| `sample` | whole number | always | 1.0 | The number of the level in the history of its device, counted from the first one heard. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |

The column `device` names a row.

### Table `connections`

Struct `BleConnectionRow`. A row of the connections of `ble`, followed from their CONNECT_IND or AUX_CONNECT_REQ. The row key is the access address as eight hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `access_address` | text | always | 4.2 | The access address of the connection, eight hex digits. |
| `central` | text | always | 4.2 | The central (the initiator of the connection). |
| `peripheral` | text | always | 4.2 | The peripheral (the advertiser it connected to). |
| `interval_ms` | number (ms) | always | 4.2 | The connection interval. |
| `latency` | whole number | always | 4.2 | The peripheral latency, events. |
| `timeout_ms` | number (ms) | always | 4.2 | The supervision timeout. |
| `phy` | text | always | 4.2 | The PHY ("LE 1M", "LE 2M", "LE Coded"). |
| `csa` | whole number | optional | 4.2 | The channel selection algorithm (1 or 2). No value until a packet tells. |
| `channels_used` | whole number | always | 4.2 | The data channels its channel map uses. |
| `packets` | whole number | always | 4.2 | Packets of the connection heard. |
| `encrypted` | yes or no | always | 4.2 | The connection is encrypted: LL_ENC_REQ or LL_START_ENC_REQ was heard. Its payloads are then not read. |
| `procedures` | text | always | 4.2 | The LL control procedures heard, joined with ", " ("LL_VERSION_IND, LL_ENC_REQ"). |
| `att` | whole number | always | 4.2 | ATT PDUs read in the clear. |
| `ended` | yes or no | always | 4.2 | LL_TERMINATE_IND was heard. |
| `first_heard` | time | optional | 4.2 | The UTC of the CONNECT_IND. |
| `last_heard` | time | optional | 4.2 | The UTC of the last packet. |

The column `access_address` names a row. Default order: the column `last_heard`, highest first. Time: `last_heard`.

### Table `att`

Struct `BleAttRow`. A row of the Attribute Protocol log of `ble`: an ATT PDU sent in the clear on a followed connection.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_s` | number (s) | always | 4.2 | The signal time of the packet. |
| `connection` | text | always | 4.2 | The connection. |
| `from_central` | yes or no | always | 4.2 | The central sent it (the first packet of its connection event); false for the peripheral. |
| `opcode` | text | always | 4.2 | The ATT opcode name ("ATT_READ_REQ"). |
| `handle` | whole number | optional | 4.2 | The attribute handle the PDU names. No value when it names none. |
| `service` | text | optional | 4.2 | The GATT service that holds the handle, by its assigned name, when a service discovery on the connection was heard. No value otherwise. |
| `value` | text | always | 4.2 | The bytes after the opcode and the handle as hex, at most 32 bytes; empty when the setting `att_values` is off. |
| `length` | whole number (B) | always | 4.3 | The number of bytes after the opcode and the handle. |

The rows are a log: they arrive over time. The column `opcode` names a row. Time: `signal_s`.

### Event `ble.device`

Struct `BleDeviceEvent`. The fields of a good packet with an address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address. |
| `text` | text | optional | 1.0 | The name of the device, if known. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the packet. |
| `family` | choice | always | 1.0 | The family of the source. |

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
| `clear` | `sdrkit_ble_clear` | none | none |
| `remove` | `sdrkit_ble_remove` | `sdrkit_ble_device_args` | none |
| `send` | `sdrkit_ble_send` | `sdrkit_ble_send_args` | none |
| `halt` | `sdrkit_ble_halt` | none | none |

The argument `address` of `send` takes the column `address` of a row of the table `devices`.

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_ble_get_channel` | `sdrkit_ble_set_channel` |
| `hop` | `sdrkit_ble_get_hop` | `sdrkit_ble_set_hop` |
| `hop_seconds` | `sdrkit_ble_get_hop_seconds` | `sdrkit_ble_set_hop_seconds` |
| `filter_text` | `sdrkit_ble_get_filter_text` | `sdrkit_ble_set_filter_text` |
| `min_level_dbfs` | `sdrkit_ble_get_min_level_dbfs` | `sdrkit_ble_set_min_level_dbfs` |
| `named_only` | `sdrkit_ble_get_named_only` | `sdrkit_ble_set_named_only` |
| `att_values` | `sdrkit_ble_get_att_values` | `sdrkit_ble_set_att_values` |
| `tx_level` | `sdrkit_ble_get_tx_level` | `sdrkit_ble_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_channel` | `sdrkit_ble_gen_get_channel` | `sdrkit_ble_gen_set_channel` |

## Try it

```sh
sdrkit run ble --source synth:ble
sdrkit run ble --source iiod:192.168.2.1 --freq 2402
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
