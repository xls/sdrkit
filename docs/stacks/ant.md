<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ANT+ sensors

ANT+ heart rate straps, bike speed, cadence and power sensors, footpods and environment sensors on 2457 MHz; sends one ANT+ frame.

Stack id: `ant`. Group: IoT, 2.4 GHz.

ANT+ sensors : 1 Mbit/s GFSK frames on one channel, the ANT+ network marker and the frames of other ANT networks, the public device profiles, and a table of sensors with the history of their main value. `send` sends one ANT+ frame.

Standard: ANT+ device profiles (heart rate, bike speed and cadence, bike power, stride, environment, common pages).

Signal types: ANT / ANT+.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.457 GHz |
| Tuning range | 2.4 GHz to 2.524 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 2 MHz |
| Offset from the signal | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ant` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `other_networks` | Other ANT networks | yes or no | true |  | Also decodes frames of networks other than ANT+. |
| `wheel_mm` | Wheel circumference | number (mm) | 2096 | 1000 to 3000 |  |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_sensors` | Test signal sensors | choice | `all` | `all` (All three), `strap` (Heart rate strap alone), `none` (No sensor) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `sensors` | Sensors | whole number |
| `heart_rate_bpm` | Heart rate | number (bpm) |
| `deviation` | Deviation | frequency |
| `channel` | Channel | frequency |
| `frames_bad` | Frames bad | whole number |
| `syncs` | Sync hits | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Frames sent | whole number |
| `tx_waiting` | Frames waiting | whole number |
| `demod.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sensors` | Sensors | `device` (whole number), `type` (text), `marker` (text), `manufacturer` (text), `serial` (whole number), `value` (text), `battery` (text), `period` (duration (s)), `level_dbfs` (number (dBFS)), `frequency` (frequency), `heard` (whole number), `last_heard` (time) |
| `frames` | Frames | `time_s` (number (s)), `device` (whole number), `type` (text), `tx_type` (whole number), `marker` (text), `level_dbfs` (number (dBFS)), `payload` (text), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `history` | Main value of the newest sensor | s |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ant.sensor` | New ANT sensor | `source` (text), `text` (text), `family` (choice), `device` (whole number), `type` (text), `marker` (text) |
| `ant.tx` | ANT+ transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send a frame | `device` (whole number, required), `device_type` (whole number, required), `tx_type` (whole number, required), `payload` (text, required), `pairing` (yes or no, default false) |  | Sends one ANT+ broadcast frame (the ANT+ network marker, the channel id, the data page and the CRC) on the tuned frequency. Frames sent during a frame wait for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the frames waiting and ends the frame on air with the fall of the carrier. |

## Typed interface

The stack is in the family `ism`, version 4.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `AntFields`. The result fields of the sensor table of the ANT+ stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames with a good CRC. |
| `sensors` | whole number | always | 1.0 | The sensors in the table. |
| `heart_rate_bpm` | number (bpm) | optional | 1.0 | The newest heart rate of any strap, if a strap sent one. |
| `deviation` | frequency | optional | 1.0 | The deviation of the last frame. It has no value before the first frame. |
| `channel` | frequency | always | 1.0 | The channel the receiver is tuned to. |

### Result fields

Struct `AntDemodFields`. The counts of the ANT+ demodulator.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_bad` | whole number | always | 1.0 | Frames whose CRC failed. |
| `syncs` | whole number | always | 1.0 | Sync words found. |

### Result fields

Struct `IsmTxFields`. The result fields of the sender of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.0 | Whether a frame is on air. |
| `tx_sent` | whole number | always | 1.0 | The frames sent since the stack started. |
| `tx_waiting` | whole number | always | 1.0 | The frames waiting for the transmitter. |

### Table `sensors`

Struct `AntSensorRow`. A row of the sensors. The row key is the device number, type and transmission type as one number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `device` | whole number | always | 1.0 | The device number. |
| `type` | text | always | 1.0 | The device type name. |
| `marker` | text | always | 1.0 | The network marker as hex. |
| `manufacturer` | text | optional | 1.0 | The manufacturer, once a page names it. |
| `serial` | whole number | optional | 1.0 | The serial number, once a page gives it. |
| `value` | text | always | 1.0 | The main value of the sensor in words. |
| `battery` | text | optional | 1.0 | The battery status, once a page gives it. |
| `period` | duration (s) | optional | 1.0 | The measured message period. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `frequency` | frequency | always | 1.0 | The carrier frequency. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

### Table `frames`

Struct `AntFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `device` | whole number | always | 1.0 | The device number. |
| `type` | text | always | 1.0 | The device type name. |
| `tx_type` | whole number | always | 1.0 | The transmission type. |
| `marker` | text | always | 1.0 | The network marker as hex. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `payload` | text | always | 1.0 | The payload as hex. |
| `text` | text | always | 1.0 | The page and its decoded text. |

### Event `ant.sensor`

Struct `AntSensorEvent`. The fields of a new sensor event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The device number. |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `device` | whole number | always | 1.0 | The device number. |
| `type` | text | always | 1.0 | The device type name. |
| `marker` | text | always | 1.0 | The network marker as hex. |

### Event `ant.tx`

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
| `clear` | `sdrkit_ant_clear` | none | none |
| `remove` | `sdrkit_ant_remove` | `sdrkit_ant_sensor_args` | none |
| `send` | `sdrkit_ant_send` | `sdrkit_ant_send_args` | none |
| `halt` | `sdrkit_ant_halt` | none | none |

The argument `device` of `send` takes the column `device` of a row of the table `sensors`.

### Settings

| Key | C getter | C setter |
|---|---|---|
| `other_networks` | `sdrkit_ant_get_other_networks` | `sdrkit_ant_set_other_networks` |
| `wheel_mm` | `sdrkit_ant_get_wheel_mm` | `sdrkit_ant_set_wheel_mm` |
| `tx_level` | `sdrkit_ant_get_tx_level` | `sdrkit_ant_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_sensors` | `sdrkit_ant_gen_get_sensors` | `sdrkit_ant_gen_set_sensors` |

## Try it

```sh
sdrkit run ant --source synth:ant
sdrkit run ant --source iiod:192.168.2.1 --freq 2457
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
