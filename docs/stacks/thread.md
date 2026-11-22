<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Thread

Thread networks on the 802.15.4 channels 11 to 26: beacons with the network name, devices, 6LoWPAN headers, and MLE and CoAP where they are sent in the clear. Encrypted frames are marked.

Stack id: `thread`. Group: IoT, 2.4 GHz.

Thread on 2.4 GHz : the 802.15.4 receiver of `zigbee` with a reader for Thread above the MAC. It shows what a listener sees without the network key: beacons with the network name, devices, 6LoWPAN headers, and MLE and CoAP where they are sent in the clear. A frame that the link layer encrypts is marked.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:thread` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Channel | whole number | 11 | 11 to 26 | Asks the host to tune 2405 + 5 (k - 11) MHz. |
| `scan` | Scan channels 11 to 26 | yes or no | false |  | Asks a live radio to visit each channel in turn. A network is found on the channel that its commissioner selected. |
| `dwell` | Scan dwell | duration (s) | 5 s | 1 to 60 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_network_name` | Test signal network name | text | `HomeThread` |  |  |
| `gen_bad_fcs` | Every frame with a bad FCS | yes or no | false |  |  |
| `gen_interferer_db` | Adjacent transmitter level (0 off) | number (dB) | 0 | 0 to 50 |  |
| `gen_interferer_hz` | Adjacent transmitter offset | frequency | 5 MHz | -10 MHz to 10 MHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `encrypted_frames` | Encrypted frames | whole number |
| `beacons` | Beacon frames | whole number |
| `data_frames` | Data frames | whole number |
| `acks` | Acknowledgements | whole number |
| `commands` | Command frames | whole number |
| `ch11` | Channel 11 frames | whole number |
| `ch12` | Channel 12 frames | whole number |
| `ch13` | Channel 13 frames | whole number |
| `ch14` | Channel 14 frames | whole number |
| `ch15` | Channel 15 frames | whole number |
| `ch16` | Channel 16 frames | whole number |
| `ch17` | Channel 17 frames | whole number |
| `ch18` | Channel 18 frames | whole number |
| `ch19` | Channel 19 frames | whole number |
| `ch20` | Channel 20 frames | whole number |
| `ch21` | Channel 21 frames | whole number |
| `ch22` | Channel 22 frames | whole number |
| `ch23` | Channel 23 frames | whole number |
| `ch24` | Channel 24 frames | whole number |
| `ch25` | Channel 25 frames | whole number |
| `ch26` | Channel 26 frames | whole number |
| `frames_per_s` | Frames in the last second | whole number |
| `channel` | Channel | whole number |
| `scanning` | Scanning | yes or no |
| `networks` | Networks | whole number |
| `devices` | Devices | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `networks` | Networks | `pan` (text), `ch` (whole number), `network_name` (text), `ext_pan` (text), `version` (whole number), `joining_open` (yes or no), `thread_seen` (yes or no), `coordinator` (text), `level_dbfs` (number (dBFS)), `heard` (whole number), `last_heard` (time) |
| `devices` | Devices | `short` (text), `extended` (text), `vendor` (text), `pan` (text), `ch` (whole number), `role` (text), `level_dbfs` (number (dBFS)), `heard` (whole number), `last_heard` (time), `last_frame` (text) |
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `ch` (whole number), `type` (text), `source` (text), `target` (text), `pan` (text), `len` (whole number), `encrypted` (yes or no), `level_dbfs` (number (dBFS)), `seq` (whole number), `ack_request` (yes or no), `details` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `thread.network` | Thread network | `source` (text), `text` (text), `network_name` (text), `ext_pan` (text), `ch` (whole number), `family` (choice) |
| `thread.device` | Thread device | `source` (text), `pan` (text), `ch` (whole number), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `ism`, version 3.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `ThreadFields`. The result fields of the Thread stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Good frames. |
| `frames_bad` | whole number | always | 1.0 | Candidates whose check failed. |
| `encrypted_frames` | whole number | always | 1.0 | Frames that the link layer encrypts. |
| `beacons` | whole number | always | 1.0 | Beacon frames. |
| `data_frames` | whole number | always | 1.0 | Data frames. |
| `acks` | whole number | always | 1.0 | Acknowledgements. |
| `commands` | whole number | always | 1.0 | Command frames. |
| `ch11` | whole number | always | 1.0 | Good frames on channel 11. |
| `ch12` | whole number | always | 1.0 | Good frames on channel 12. |
| `ch13` | whole number | always | 1.0 | Good frames on channel 13. |
| `ch14` | whole number | always | 1.0 | Good frames on channel 14. |
| `ch15` | whole number | always | 1.0 | Good frames on channel 15. |
| `ch16` | whole number | always | 1.0 | Good frames on channel 16. |
| `ch17` | whole number | always | 1.0 | Good frames on channel 17. |
| `ch18` | whole number | always | 1.0 | Good frames on channel 18. |
| `ch19` | whole number | always | 1.0 | Good frames on channel 19. |
| `ch20` | whole number | always | 1.0 | Good frames on channel 20. |
| `ch21` | whole number | always | 1.0 | Good frames on channel 21. |
| `ch22` | whole number | always | 1.0 | Good frames on channel 22. |
| `ch23` | whole number | always | 1.0 | Good frames on channel 23. |
| `ch24` | whole number | always | 1.0 | Good frames on channel 24. |
| `ch25` | whole number | always | 1.0 | Good frames on channel 25. |
| `ch26` | whole number | always | 1.0 | Good frames on channel 26. |
| `frames_per_s` | whole number | always | 1.0 | Good frames in the last second. |
| `channel` | whole number | always | 1.0 | The channel listened to (0 when not known). |
| `scanning` | yes or no | always | 1.0 | The stack steps through the channels. |
| `networks` | whole number | always | 1.0 | The networks in the table. |
| `devices` | whole number | always | 1.0 | The devices in the table. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the last frame since the last change of frequency. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last frame since the last change of frequency. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last frame since the last change of frequency. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `networks`

Struct `ThreadNetworkRow`. A row of the Thread networks. The row key is the PAN and the channel as one number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pan` | text | always | 1.0 | The PAN id. |
| `ch` | whole number | always | 1.0 | The channel where it was heard (0 when not known). |
| `network_name` | text | optional | 1.0 | The network name of a Thread beacon. |
| `ext_pan` | text | optional | 1.0 | The extended PAN id. |
| `version` | whole number | optional | 1.0 | The Thread version of the beacon. |
| `joining_open` | yes or no | always | 1.0 | The last beacon permitted joining. |
| `thread_seen` | yes or no | always | 1.0 | A Thread beacon or a 6LoWPAN header was seen in the PAN. |
| `coordinator` | text | optional | 1.0 | The sender of the beacon. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The column `network_name` names a row. Default order: the column `level_dbfs`, highest first.

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

### Table `frames`

Struct `ThreadFrameRow`. A row of the Thread frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the frame, when the samples are live. |
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `ch` | whole number | always | 1.0 | The channel. |
| `type` | text | always | 1.0 | The MAC frame type. |
| `source` | text | always | 1.0 | The source address. |
| `target` | text | always | 1.0 | The destination address. |
| `pan` | text | optional | 1.0 | The PAN id, if known. |
| `len` | whole number | always | 1.0 | The PSDU length with the FCS. |
| `encrypted` | yes or no | always | 1.0 | The link layer encrypts the payload. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `seq` | whole number | optional | 1.0 | The sequence number, if the frame has one. |
| `ack_request` | yes or no | always | 1.0 | The frame requests an acknowledgement. |
| `details` | text | always | 1.0 | The description. |

The column `source` names a row.

### Event `thread.network`

Struct `ThreadNetworkEvent`. The fields of a new Thread network event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The PAN id. |
| `text` | text | always | 1.0 | The text of the event. |
| `network_name` | text | always | 1.0 | The network name. |
| `ext_pan` | text | always | 1.0 | The extended PAN id. |
| `ch` | whole number | always | 1.0 | The channel. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `thread.device`

Struct `ThreadDeviceEvent`. The fields of a new Thread device event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address of the device. |
| `pan` | text | optional | 1.0 | The PAN id, if known. |
| `ch` | whole number | always | 1.0 | The channel. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_thread_clear` | none | none |
| `remove` | `sdrkit_thread_remove` | `sdrkit_ism_pan_device_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_thread_get_channel` | `sdrkit_thread_set_channel` |
| `scan` | `sdrkit_thread_get_scan` | `sdrkit_thread_set_scan` |
| `dwell` | `sdrkit_thread_get_dwell` | `sdrkit_thread_set_dwell` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_network_name` | `sdrkit_thread_gen_get_network_name` | `sdrkit_thread_gen_set_network_name` |
| `gen_bad_fcs` | `sdrkit_thread_gen_get_bad_fcs` | `sdrkit_thread_gen_set_bad_fcs` |
| `gen_interferer_db` | `sdrkit_thread_gen_get_interferer_db` | `sdrkit_thread_gen_set_interferer_db` |
| `gen_interferer_hz` | `sdrkit_thread_gen_get_interferer_hz` | `sdrkit_thread_gen_set_interferer_hz` |

## Try it

```sh
sdrkit run thread --source synth:thread
sdrkit run thread --source iiod:192.168.2.1 --freq 2405
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
