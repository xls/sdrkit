<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Bluetooth LE

Bluetooth LE advertising: the devices around, their names, vendors and beacons, as a scanner app shows them.

Stack id: `ble`. Group: Data and IoT.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ble` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | choice | `37` | `37` (37 2402 MHz), `38` (38 2426 MHz), `39` (39 2480 MHz) | Asks the host to tune to the advertising channel. |
| `hop` | Hop | yes or no | false |  | Asks the host to step 37, 38, 39: some devices skip a channel, and interference differs between them. |
| `hop_seconds` | Dwell | duration (s) | 3 s | 1 to 30 | Time on each channel while hopping. |
| `filter_text` | Filter | text | empty |  | Shows only the devices whose name, address, vendor or beacon holds this text. |
| `min_level_dbfs` | Level above | number (dBFS) | -120 | -120 to 0 |  |
| `named_only` | Named only | yes or no | false |  |  |
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
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Packets sent | whole number |
| `tx_left` | Packets left | whole number |
| `adv.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Devices | `address` (text), `address_type` (choice), `name` (text), `level_dbfs` (number (dBFS)), `last_heard` (time), `heard` (whole number), `vendor` (text), `beacon` (text), `channels` (text), `connectable` (yes or no), `scanned` (yes or no), `manufacturer_data` (text), `first_heard` (time) |
| `packets` | Packets | `signal_s` (number (s)), `channel` (whole number), `pdu` (text), `address` (text), `level_dbfs` (number (dBFS)), `length` (whole number (bytes)), `details` (text) |
| `pdu_types` | PDU types | `pdu` (text), `packets` (whole number) |
| `device_levels` | Device levels | `device` (text), `sample` (whole number), `level_dbfs` (number (dBFS)) |

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

The stack is in the family `wlan`, version 1.2. The C header `sdrkit/wlan.h`, the registry record `registry/wlan.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

### Result fields

Struct `BleTxFields`. The result fields of the advertising sender of `ble` and `bletracker`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.2 | Whether a send is on air. |
| `tx_sent` | whole number | always | 1.2 | The packets of the send on air that went out. |
| `tx_left` | whole number | always | 1.2 | The packets still to go: the rest of the send on air and the sends that wait. |

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

### Table `packets`

Struct `BlePacketRow`. A row of the packet log of `ble`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_s` | number (s) | always | 1.0 | The signal time of the packet. |
| `channel` | whole number | always | 1.0 | The advertising channel of the packet. |
| `pdu` | text | always | 1.0 | The PDU type name. |
| `address` | text | optional | 1.0 | The address of the sender, if the PDU has one. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the packet. |
| `length` | whole number (bytes) | always | 1.0 | The payload length. |
| `details` | text | always | 1.0 | The one-line summary. |

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
| `state` | text | always | 1.2 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.2 | Why it changed: empty, halted, the transmitter is off, the format changed, the frequency changed, or why a send was refused. |
| `target` | text | always | 1.2 | The advertiser address. |
| `text` | text | always | 1.2 | The PDU type, the channel, the packets and the advertising data in hex; empty for finished. |
| `family` | choice | always | 1.2 | The family of the source. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ble_clear` | none | none |
| `remove` | `sdrkit_ble_remove` | `sdrkit_ble_device_args` | none |
| `send` | `sdrkit_ble_send` | `sdrkit_ble_send_args` | none |
| `halt` | `sdrkit_ble_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_ble_get_channel` | `sdrkit_ble_set_channel` |
| `hop` | `sdrkit_ble_get_hop` | `sdrkit_ble_set_hop` |
| `hop_seconds` | `sdrkit_ble_get_hop_seconds` | `sdrkit_ble_set_hop_seconds` |
| `filter_text` | `sdrkit_ble_get_filter_text` | `sdrkit_ble_set_filter_text` |
| `min_level_dbfs` | `sdrkit_ble_get_min_level_dbfs` | `sdrkit_ble_set_min_level_dbfs` |
| `named_only` | `sdrkit_ble_get_named_only` | `sdrkit_ble_set_named_only` |
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
