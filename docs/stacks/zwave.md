<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Z-Wave

Z-Wave home automation (ITU-T G.9959, R1 R2 R3): networks, nodes and commands; Security S0 and S2 payloads shown as encrypted.

Stack id: `zwave`. Group: IoT, sub-GHz.

Z-Wave : every channel of the `channels` setting in the capture, MPDUs at R1, R2 and R3, and tables of networks, nodes and commands. Security S0 and S2 payloads are shown as encrypted and never decoded.

Standard: ITU-T G.9959 (PHY and MAC)

Signal types: Z-Wave.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 860 MHz to 930 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1.8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:zwave` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | text | empty |  | Entries separated by ";", each the frequency in Hz and the rates on it (1, 2, 3) separated by "/", for example "868420000/1;868400000/2;869850000/3". At most 64 entries. Empty: one channel at the tuned frequency with the three rates. |
| `sof_f0_r3` | Accept 0xF0 as the R3 start of frame | yes or no | false |  | For recordings made with the ISM-mode start of frame at every rate. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_centre_hz` | Test signal radio centre | frequency | 0 Hz |  | The frequency that the radio is tuned to: the test signal puts the channels of the setting channels around it. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `networks` | Networks | whole number |
| `nodes` | Nodes | whole number |
| `frames_r1` | R1 frames | whole number |
| `frames_r2` | R2 frames | whole number |
| `frames_r3` | R3 frames | whole number |
| `s0_frames` | S0 frames | whole number |
| `s2_frames` | S2 frames | whole number |
| `acks` | Acks | whole number |
| `bad_samples` | Bad samples | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Frames sent | whole number |
| `tx_waiting` | Frames waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `time` (time), `frequency` (frequency), `rate` (text), `home_id` (text), `route` (text), `frame_type` (text), `seq` (whole number), `ack_requested` (yes or no), `encryption` (text), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `command` (text), `mpdu` (text) |
| `nodes` | Nodes | `home_id` (text), `node` (whole number), `role` (text), `sent` (whole number), `addressed` (whole number), `encrypted` (whole number), `rate` (text), `level_dbfs` (number (dBFS)), `last_heard` (time), `last_class` (text), `beaming` (yes or no), `low_power` (yes or no) |
| `networks` | Networks | `home_id` (text), `nodes` (whole number), `controller` (yes or no), `heard` (whole number), `encrypted` (whole number), `level_dbfs` (number (dBFS)), `first_heard` (time), `last_heard` (time) |
| `channels` | Channels | `frequency` (frequency), `rate` (text), `covered` (yes or no), `frames` (whole number), `failed_checks` (whole number) |
| `classes` | Command classes | `class` (text), `count` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `zwave.frame` | Z-Wave frame | `source` (text), `target` (text), `text` (text), `family` (choice), `home_id` (text), `route` (text), `rate` (text), `frame_type` (text), `encryption` (text), `command` (text), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)) |
| `zwave.state` | Z-Wave activity | `text` (text), `family` (choice) |
| `zwave.tx` | Z-Wave transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send a frame | `home_id` (text, required), `source` (whole number, required), `destination` (whole number, required), `payload` (text, required), `rate` (choice, default `r2`), `ack` (yes or no, default false), `sequence` (whole number, default 0) |  | Sends a singlecast MPDU on the first channel of the rate in the setting channels (the tuned frequency when the setting is empty). Frames sent during a frame wait for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the frames waiting and ends the frame on air with the fall of the carrier. |

## Typed interface

The stack is in the family `ism`, version 1.1. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `ZwaveFields`. The result fields of the Z-Wave stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames decoded at every rate. |
| `frames_bad` | whole number | always | 1.0 | Bursts whose check failed. |
| `networks` | whole number | always | 1.0 | The networks in the table. |
| `nodes` | whole number | always | 1.0 | The nodes in the table. |
| `frames_r1` | whole number | always | 1.0 | Frames at R1. |
| `frames_r2` | whole number | always | 1.0 | Frames at R2. |
| `frames_r3` | whole number | always | 1.0 | Frames at R3. |
| `s0_frames` | whole number | always | 1.0 | Security S0 frames. |
| `s2_frames` | whole number | always | 1.0 | Security S2 frames. |
| `acks` | whole number | always | 1.0 | Acknowledgements. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Result fields

Struct `IsmTxFields`. The result fields of the sender of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.1 | Whether a frame is on air. |
| `tx_sent` | whole number | always | 1.1 | The frames sent since the stack started. |
| `tx_waiting` | whole number | always | 1.1 | The frames waiting for the transmitter. |

### Table `frames`

Struct `ZwaveFrameRow`. A row of the Z-Wave frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `time` | time | optional | 1.0 | The UTC of the frame, when the samples are live. |
| `frequency` | frequency | always | 1.0 | The channel frequency. |
| `rate` | text | always | 1.0 | The rate ("R1", "R2", "R3"). |
| `home_id` | text | always | 1.0 | The home id (8 hex digits). |
| `route` | text | always | 1.0 | Source and destination ("2 > 1"). |
| `frame_type` | text | always | 1.0 | The header type. |
| `seq` | whole number | always | 1.0 | The sequence number. |
| `ack_requested` | yes or no | always | 1.0 | An acknowledgement is requested. |
| `encryption` | text | always | 1.0 | The encryption ("none", "S0", "S2"). |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the frame. |
| `command` | text | always | 1.0 | The one-line description. |
| `mpdu` | text | always | 1.0 | The MPDU in hex. |

### Table `nodes`

Struct `ZwaveNodeRow`. A row of the Z-Wave nodes. The row key is the home id and the node id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `home_id` | text | always | 1.0 | The row key of the network of the node. |
| `node` | whole number | always | 1.0 | The node id. |
| `role` | text | always | 1.0 | "controller" or "node". |
| `sent` | whole number | always | 1.0 | Frames the node sent. |
| `addressed` | whole number | always | 1.0 | Frames addressed to the node. |
| `encrypted` | whole number | always | 1.0 | Encrypted frames the node sent. |
| `rate` | text | always | 1.0 | The rate of the last frame. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `last_class` | text | always | 1.0 | The last command class the node sent. |
| `beaming` | yes or no | always | 1.0 | The node uses beaming. |
| `low_power` | yes or no | always | 1.0 | The node sent at low power. |

### Table `networks`

Struct `ZwaveNetworkRow`. A row of the Z-Wave networks. The row key is the home id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `home_id` | text | always | 1.0 | The home id. |
| `nodes` | whole number | always | 1.0 | The nodes heard. |
| `controller` | yes or no | always | 1.0 | Node 1 of the network has sent. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `encrypted` | whole number | always | 1.0 | Encrypted frames. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

### Table `channels`

Struct `ZwaveChannelRow`. A row of the Z-Wave channels in use. The row key is the frequency and the rate.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The channel frequency. |
| `rate` | text | always | 1.0 | The rate. |
| `covered` | yes or no | always | 1.0 | The channel is in the band taken in. |
| `frames` | whole number | always | 1.0 | Frames decoded on the channel. |
| `failed_checks` | whole number | always | 1.0 | Bursts whose check failed on the channel. |

### Table `classes`

Struct `ZwaveClassRow`. A row of the command class counts. The row key is the name of the class.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `class` | text | always | 1.0 | The command class. |
| `count` | whole number | always | 1.0 | How often it was sent. |

### Event `zwave.frame`

Struct `ZwaveFrameEvent`. The fields of a Z-Wave frame event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender: home id and node ("C0FFEE01:2"). |
| `target` | text | optional | 1.0 | The addressee, for a frame to one node. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `home_id` | text | always | 1.0 | The home id. |
| `route` | text | always | 1.0 | Source and destination. |
| `rate` | text | always | 1.0 | The rate. |
| `frame_type` | text | always | 1.0 | The header type. |
| `encryption` | text | always | 1.0 | The encryption. |
| `command` | text | always | 1.0 | The one-line description. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the frame. |

### Event `zwave.state`

Struct `ZwaveStateEvent`. The fields of the event that Z-Wave frames start or stop coming in.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |

### Event `zwave.tx`

Struct `IsmTxEvent`. The fields of a change of a transmission of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.1 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.1 | Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused. |
| `target` | text | always | 1.1 | The addressee or the identity of the frame. |
| `text` | text | always | 1.1 | What the frame carries, for started and queued; empty otherwise. |
| `family` | choice | always | 1.1 | The family of the source: a sensor. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_zwave_clear` | none | none |
| `remove` | `sdrkit_zwave_remove` | `sdrkit_zwave_node_args` | none |
| `send` | `sdrkit_zwave_send` | `sdrkit_zwave_send_args` | none |
| `halt` | `sdrkit_zwave_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_zwave_get_channels` | `sdrkit_zwave_set_channels` |
| `sof_f0_r3` | `sdrkit_zwave_get_sof_f0_r3` | `sdrkit_zwave_set_sof_f0_r3` |
| `tx_level` | `sdrkit_zwave_get_tx_level` | `sdrkit_zwave_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_centre_hz` | `sdrkit_zwave_gen_get_centre_hz` | `sdrkit_zwave_gen_set_centre_hz` |

## Try it

```sh
sdrkit run zwave --source synth:zwave
sdrkit run zwave --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/ism/zwave_868_4_1msps.sigmf-meta`
<!-- generated: end -->
