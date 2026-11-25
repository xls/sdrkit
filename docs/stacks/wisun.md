<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wi-SUN FAN

Wi-SUN FAN smart utility networks: PAN adverts and configuration, nodes, links and header IEs; secured frames shown as encrypted.

Stack id: `wisun`. Group: IoT, sub-GHz.

Wi-SUN FAN : SUN FSK frames on every channel of the band taken in, through a polyphase filter bank, bursts per band, demodulation per channel and rate, de-whitening, the FCS with least-reliable-bit correction, the MAC header and the Wi-SUN information elements. Secured frames are shown with their header IEs and lengths as encrypted.

Standard: IEEE 802.15.4-2020 clause 19 (SUN FSK) and clause 7 (MAC), Wi-SUN FAN 1.1.

Signal types: Wi-SUN.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 470 MHz to 2.484 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 18 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wisun` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `ranges` | Usable ranges | text | empty |  | Entries separated by ";", each the lower and the upper edge in Hz separated by "-", for example "902000000-907500000;915000000-928000000". At most 16 entries. Empty: every channel of the classes is usable. |
| `classes` | Operating classes | text | empty |  | Entries separated by ";", one for each operating class in the order of the class numbers: the first channel in Hz, the spacing in Hz, the number of channels and the FSK modes, separated by "/". The modes are separated by spaces, each the bit rate in kbit/s and the modulation index separated by ":", for example "915200000/200000/64/50:1;915400000/400000/32/100:0.5 150:0.5". At most 16 entries. Empty: class 1 every 200 kHz at 50 kbit/s and class 2 every 400 kHz at 100 and 150 kbit/s, on grids through the tuned frequency, without channel numbers. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_span` | Span of channels used | frequency | 4 MHz | 200 kHz to 13 MHz |  |
| `gen_centre_hz` | Test signal radio centre | frequency | 0 Hz |  | The frequency that the radio is tuned to: the test signal puts the channels of the plan around it. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `corrected_frames` | Corrected frames | whole number |
| `fec_frames` | FEC frames | whole number |
| `mode_switches` | Mode switches | whole number |
| `encrypted_frames` | Encrypted frames | whole number |
| `networks` | Networks | whole number |
| `nodes` | Nodes | whole number |
| `links` | Links | whole number |
| `bad_samples` | Bad samples | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Frames sent | whole number |
| `tx_waiting` | Frames waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `networks` | Networks | `name` (text), `border_router` (text), `size` (whole number), `nodes` (whole number), `level_dbfs` (number (dBFS)), `heard` (whole number), `last_heard` (time), `schedule` (text), `pan_id` (text) |
| `nodes` | Nodes | `vendor` (text), `role` (text), `pan` (text), `cost` (whole number), `level_dbfs` (number (dBFS)), `frames` (whole number), `last_heard` (time), `freq` (frequency), `authenticator` (text), `eap` (text), `last` (text), `address` (text) |
| `links` | Links | `from` (text), `to` (text), `frames` (whole number), `acks` (whole number), `rsl_dbm` (number (dBm)), `level_dbfs` (number (dBFS)), `last_heard` (time) |
| `frames` | Frames | `time_s` (number (s)), `freq` (frequency), `ch` (text), `kbps` (whole number), `type` (text), `route` (text), `level_dbfs` (number (dBFS)), `details` (text), `mpdu` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `hops` | Hop chart | s | MHz |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wisun.pan` | New Wi-SUN PAN | `source` (text), `text` (text), `family` (choice), `pan` (text) |
| `wisun.tx` | Wi-SUN transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send a frame | `frame_type` (choice, default `data`), `pan_id` (text, required), `source` (text, required), `destination` (text, required), `payload` (text, required), `header_ies` (text, default empty), `sequence` (whole number, default 0), `kbps` (whole number (kbit/s), default 0), `ufsi` (whole number, default 0) |  | Sends an IEEE 802.15.4 frame of version 2015 with the UTT-IE of the frame type, the header IEs, a header termination IE and the payload IEs, whitened with a 4-octet FCS, on the channel of the plan nearest the tuned frequency. Frames sent during a frame wait for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the frames waiting and ends the frame on air with the fall of the carrier. |

## Typed interface

The stack is in the family `ism`, version 4.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `WisunFields`. The result fields of the Wi-SUN FAN stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Good frames. |
| `frames_bad` | whole number | always | 1.0 | Bad frames. |
| `corrected_frames` | whole number | always | 1.0 | Frames corrected by the least-reliable-bit search. |
| `fec_frames` | whole number | always | 1.0 | Frames with an FEC SFD, counted and not decoded. |
| `mode_switches` | whole number | always | 1.0 | Mode switches. |
| `encrypted_frames` | whole number | always | 1.0 | Secured frames. |
| `networks` | whole number | always | 1.0 | The networks in the table. |
| `nodes` | whole number | always | 1.0 | The nodes in the table. |
| `links` | whole number | always | 1.0 | The links in the table. |

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

Struct `WisunNetworkRow`. A row of the Wi-SUN networks. The row key is the PAN id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | optional | 1.0 | The network name, once heard. |
| `border_router` | text | optional | 1.0 | The border router EUI-64, once known. |
| `size` | whole number | optional | 1.0 | The PAN size, once heard. |
| `nodes` | whole number | always | 1.0 | The nodes in the PAN. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `schedule` | text | optional | 1.0 | The unicast schedule, once heard. |
| `pan_id` | text | always | 1.0 | The PAN id, as `send` takes it. |

The column `name` names a row. Default order: the column `level_dbfs`, highest first.

### Table `nodes`

Struct `WisunNodeRow`. A row of the Wi-SUN nodes. The row key is the EUI-64.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vendor` | text | optional | 1.0 | The vendor of the EUI-64. |
| `role` | text | always | 1.0 | The role: node, LFN, joining, router or border router. |
| `pan` | text | optional | 1.0 | The row key of the network of the node, once known. |
| `cost` | whole number | optional | 1.0 | The routing cost, once heard. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `frames` | whole number | always | 1.0 | Frames sent. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `freq` | frequency | always | 1.0 | The last channel centre. |
| `authenticator` | text | optional | 1.0 | The authenticator EUI-64, once heard. |
| `eap` | text | optional | 1.0 | The EAP state, once heard. |
| `last` | text | always | 1.0 | The summary of the last frame. |
| `address` | text | always | 1.0 | The EUI-64, as `send` takes it. |

### Table `links`

Struct `WisunLinkRow`. A row of the Wi-SUN links. The row key is the two ends.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `from` | text | always | 1.0 | The row key of the sender. |
| `to` | text | always | 1.0 | The row key of the receiver. |
| `frames` | whole number | always | 1.0 | Frames from the sender. |
| `acks` | whole number | always | 1.0 | Acks from the receiver. |
| `rsl_dbm` | number (dBm) | optional | 1.0 | The level the receiver reported for the frames of the sender, once reported. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the sender here. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The column `from` names a row.

### Table `frames`

Struct `WisunFrameRow`. A row of the Wi-SUN frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `freq` | frequency | always | 1.0 | The channel centre. |
| `ch` | text | optional | 1.0 | The operating class and channel number in the plan of the bit rate, if the channel is in the plan. |
| `kbps` | whole number | always | 1.0 | The bit rate. |
| `type` | text | always | 1.0 | The UTT or 802.15.4 frame type name. |
| `route` | text | always | 1.0 | "source > destination". |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `details` | text | always | 1.0 | The summary of the frame. |
| `mpdu` | text | always | 1.0 | The MPDU without the FCS in hex. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `wisun.pan`

Struct `WisunPanEvent`. The fields of a new Wi-SUN PAN event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The PAN id. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `pan` | text | always | 1.0 | The PAN id. |

### Event `wisun.tx`

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
| `clear` | `sdrkit_wisun_clear` | none | none |
| `remove` | `sdrkit_wisun_remove` | `sdrkit_wisun_node_args` | none |
| `send` | `sdrkit_wisun_send` | `sdrkit_wisun_send_args` | none |
| `halt` | `sdrkit_wisun_halt` | none | none |

The argument `pan_id` of `send` takes the column `pan_id` of a row of the table `networks`. The argument `source` of `send` takes the column `address` of a row of the table `nodes`. The argument `destination` of `send` takes the column `address` of a row of the table `nodes`.

### Settings

| Key | C getter | C setter |
|---|---|---|
| `ranges` | `sdrkit_wisun_get_ranges` | `sdrkit_wisun_set_ranges` |
| `classes` | `sdrkit_wisun_get_classes` | `sdrkit_wisun_set_classes` |
| `tx_level` | `sdrkit_wisun_get_tx_level` | `sdrkit_wisun_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_span` | `sdrkit_wisun_gen_get_span` | `sdrkit_wisun_gen_set_span` |
| `gen_centre_hz` | `sdrkit_wisun_gen_get_centre_hz` | `sdrkit_wisun_gen_set_centre_hz` |

## Try it

```sh
sdrkit run wisun --source synth:wisun
sdrkit run wisun --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
