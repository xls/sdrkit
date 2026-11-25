<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Zigbee / 802.15.4

IEEE 802.15.4 frames on 2.4 GHz channels 11 to 26: MAC headers, beacons, Zigbee NWK and APS headers and device announcements; secured MAC frames opened with the keys of the setting `keys`, else shown as encrypted.

Stack id: `zigbee`. Group: IoT, 2.4 GHz.

Zigbee and IEEE 802.15.4 on 2.4 GHz : O-QPSK chips, frames with their FCS, the MAC, Zigbee NWK and APS headers, beacons and device announcements. A secured MAC frame opens with a key of the setting `keys` ; without one its payload is shown as encrypted.

Standard: IEEE 802.15.4-2020 clause 12 (2450 MHz O-QPSK) and clause 7 (MAC frames), Zigbee specification R22 (NWK, APS).

Signal types: IEEE 802.15.4.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.405 GHz |
| Tuning range | 2.405 GHz to 2.48 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 2 MHz |
| Offset from the signal | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:zigbee` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | whole number | 11 | 11 to 26 | Asks the host to tune 2405 + 5 (k - 11) MHz. |
| `scan` | Scan channels 11 to 26 | yes or no | false |  | Asks the host to visit each channel in turn. |
| `dwell` | Scan dwell | duration (s) | 5 s | 1 to 60 |  |
| `keys` | Keys (kept in memory only) | text | empty |  | AES-128 keys of 32 hex digits, separated by ';', tried in turn on each secured MAC frame (IEEE 802.15.4 CCM*). Empty: secured payloads are shown as encrypted. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bad_fcs` | Every frame with a bad FCS | yes or no | false |  |  |
| `gen_interferer_db` | Adjacent transmitter level (0 off) | number (dB) | 0 | 0 to 50 |  |
| `gen_interferer_hz` | Adjacent transmitter offset | frequency | 5 MHz | -10 MHz to 10 MHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `candidates` | Preamble candidates | whole number |
| `encrypted_frames` | Encrypted frames | whole number |
| `networks` | Networks | whole number |
| `devices` | Devices | whole number |
| `channel` | Channel | whole number |
| `frames_per_s` | Frames in the last second | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `bad_samples` | Bad samples | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Frames sent | whole number |
| `tx_waiting` | Frames waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `networks` | Networks | `pan` (text), `ext_pan` (text), `ch` (whole number), `coordinator` (text), `devices` (whole number), `level_dbfs` (number (dBFS)), `heard` (whole number), `last_heard` (time), `name` (text) |
| `devices` | Devices | `short` (text), `extended` (text), `vendor` (text), `pan` (text), `ch` (whole number), `role` (text), `level_dbfs` (number (dBFS)), `heard` (whole number), `last_heard` (time), `last_frame` (text), `name` (text) |
| `frames` | Frames | `time_s` (number (s)), `ch` (whole number), `type` (text), `route` (text), `pan` (text), `len` (whole number), `encrypted` (yes or no), `level_dbfs` (number (dBFS)), `details` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `frames_per_second` | Frames per second | s | frames |

## Events

| Kind | Label | Fields |
|---|---|---|
| `zigbee.pan` | New Zigbee PAN | `source` (text), `text` (text), `family` (choice), `pan` (text), `ch` (whole number) |
| `zigbee.tx` | Zigbee transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send a frame | `pan_id` (text, required), `destination` (text, required), `source` (text, required), `payload` (text, required), `frame_type` (choice, default `data`), `sequence` (whole number, default 0), `ack` (yes or no, default false), `security_level` (whole number, default 0), `key` (text, default empty), `frame_counter` (whole number, default 0), `key_id_mode` (whole number, default 1), `key_index` (whole number, default 1), `key_source` (text, default empty) |  | Sends an IEEE 802.15.4 MAC frame on the tuned frequency: frame version 2003 without security, frame version 2006 with the auxiliary security header and CCM* at the security level. With both addresses the source PAN id is left out. Frames sent during a frame wait for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the frames waiting and ends the frame on air with the fall of the carrier. |

## Typed interface

The stack is in the family `ism`, version 4.2. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

Key readings: `devices`.

### Result fields

Struct `ZigbeeFields`. The result fields of the Zigbee stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Good frames. |
| `frames_bad` | whole number | always | 1.0 | Frames whose SFD was found and whose FCS failed. |
| `candidates` | whole number | always | 1.0 | Preamble candidates. |
| `encrypted_frames` | whole number | always | 1.0 | Frames with an encrypted payload. |
| `networks` | whole number | always | 1.0 | The networks in the table. |
| `devices` | whole number | always | 1.0 | The devices in the table. |
| `channel` | whole number | always | 1.0 | The channel listened to (0 when not known). |
| `frames_per_s` | whole number | always | 1.0 | Good frames in the last second. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last frame since the last change of frequency. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame since the last change of frequency. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Result fields

Struct `IsmTxFields`. The result fields of the sender of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.0 | Whether a frame is on air. |
| `tx_sent` | whole number | always | 1.0 | The frames sent since the stack started. |
| `tx_waiting` | whole number | always | 1.0 | The frames waiting for the transmitter. |

### Table `networks`

Struct `ZigbeeNetworkRow`. A row of the Zigbee networks. The row key is the PAN and the channel as one number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pan` | text | always | 1.0 | The PAN id. |
| `ext_pan` | text | optional | 1.0 | The extended PAN id, if a beacon gave it. |
| `ch` | whole number | always | 1.0 | The channel. |
| `coordinator` | text | optional | 1.0 | The coordinator, if known. |
| `devices` | whole number | always | 1.0 | The devices in the PAN. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `name` | text | always | 4.2 | The name of the network for people: the extended PAN id when known, else the PAN id. |

The column `name` names a row. Default order: the column `level_dbfs`, highest first.

### Table `devices`

Struct `PanDeviceRow`. A row of the devices of an IEEE 802.15.4 stack. The row key is the number of the device.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `short` | text | optional | 1.0 | The short address, if heard. |
| `extended` | text | optional | 1.0 | The extended address, if heard. |
| `vendor` | text | optional | 1.0 | The vendor of the extended address. |
| `pan` | text | optional | 1.0 | The PAN id, if known. |
| `ch` | whole number | always | 1.0 | The channel. |
| `role` | text | always | 1.0 | "coordinator", "router" or "device". |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `last_frame` | text | always | 1.0 | The description of the last frame. |
| `name` | text | always | 4.1 | The name of the device for people: the vendor and the short address, else the extended address. |

The rows are the stations or nodes of the stack. The column `name` names a row. Default order: the column `last_heard`, highest first. Time: `last_heard`.

### Table `frames`

Struct `ZigbeeFrameRow`. A row of the Zigbee frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `ch` | whole number | always | 1.0 | The channel. |
| `type` | text | always | 1.0 | The MAC frame type. |
| `route` | text | always | 1.0 | "source > destination". |
| `pan` | text | optional | 1.0 | The PAN id, if known. |
| `len` | whole number | always | 1.0 | The PSDU length with the FCS. |
| `encrypted` | yes or no | always | 1.0 | Some payload is encrypted. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `details` | text | always | 1.0 | The description. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `zigbee.pan`

Struct `ZigbeePanEvent`. The fields of a new PAN event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The PAN id. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `pan` | text | always | 1.0 | The PAN id. |
| `ch` | whole number | always | 1.0 | The channel. |

### Event `zigbee.tx`

Struct `IsmTxEvent`. The fields of a change of a transmission of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.0 | Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused. |
| `target` | text | always | 1.0 | The addressee or the identity of the frame. |
| `text` | text | always | 1.0 | What the frame carries, for started and queued; empty otherwise. |
| `family` | choice | always | 1.0 | The family of the source: a sensor. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_zigbee_clear` | none | none |
| `remove` | `sdrkit_zigbee_remove` | `sdrkit_ism_pan_device_args` | none |
| `send` | `sdrkit_zigbee_send` | `sdrkit_zigbee_send_args` | none |
| `halt` | `sdrkit_zigbee_halt` | none | none |

The argument `pan_id` of `send` takes the column `pan` of a row of the table `networks`. The argument `destination` of `send` takes the column `short` of a row of the table `devices`. The argument `source` of `send` takes the column `short` of a row of the table `devices`.

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_zigbee_get_channel` | `sdrkit_zigbee_set_channel` |
| `scan` | `sdrkit_zigbee_get_scan` | `sdrkit_zigbee_set_scan` |
| `dwell` | `sdrkit_zigbee_get_dwell` | `sdrkit_zigbee_set_dwell` |
| `keys` | `sdrkit_zigbee_get_keys` | `sdrkit_zigbee_set_keys` |
| `tx_level` | `sdrkit_zigbee_get_tx_level` | `sdrkit_zigbee_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bad_fcs` | `sdrkit_zigbee_gen_get_bad_fcs` | `sdrkit_zigbee_gen_set_bad_fcs` |
| `gen_interferer_db` | `sdrkit_zigbee_gen_get_interferer_db` | `sdrkit_zigbee_gen_set_interferer_db` |
| `gen_interferer_hz` | `sdrkit_zigbee_gen_get_interferer_hz` | `sdrkit_zigbee_gen_set_interferer_hz` |

## Try it

```sh
sdrkit run zigbee --source synth:zigbee
sdrkit run zigbee --source iiod:192.168.2.1 --freq 2405
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
