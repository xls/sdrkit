<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Wireless M-Bus meters

Water, heat, gas and electricity meters (EN 13757-4 S, T, C, R2 and N): ids and unencrypted readings; encrypted payloads shown as such. R2 and N1 have no real-signal confirmation yet.

Stack id: `wmbus`. Group: Meters, 868 / 169 MHz.

Wireless M-Bus meters : every channel of the plan in the capture is demodulated at once, frames are checked block by block, and unencrypted application data is decoded. A payload under a security mode is listed as encrypted and never decoded.

Standard: EN 13757-4 (radio link), EN 13757-3 (application layer)

Signal types: Wireless M-Bus.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.625 MHz |
| Tuning range | 150 MHz to 880 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1.6 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:wmbus` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_plan` | Test signal plan | choice | `868` | `868` (868 MHz), `169` (169 MHz) |  |
| `gen_phy` | Test signal mode | choice | `all` | `all` (All), `S1`, `T1`, `C1A` (C1 format A), `C1B` (C1 format B), `R2`, `N1` |  |
| `gen_encrypted` | Encrypted meter | yes or no | true |  |  |
| `gen_silence` | No meters (noise only) | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `meters` | Meters | whole number |
| `syncs` | Syncs | whole number |
| `false_syncs` | False syncs | whole number |
| `encrypted_frames` | Encrypted frames | whole number |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `meters` | Meters | `maker` (text), `meter_id` (text), `device_type` (text), `phy` (text), `frequency` (frequency), `heard` (whole number), `last_heard` (time), `level_dbfs` (number (dBFS)), `encrypted` (yes or no), `last_reading` (text), `status` (text) |
| `meter_records` | Meter records | `meter` (text), `index` (whole number), `quantity` (text), `value` (text), `notes` (text) |
| `frames` | Frames | `time_s` (number (s)), `time` (time), `phy` (text), `frequency` (frequency), `meter` (text), `level_dbfs` (number (dBFS)), `crc_ok` (yes or no), `encrypted` (yes or no), `result` (text), `frame` (text) |
| `channels` | Channels | `frequency` (frequency), `phy` (text), `level_dbfs` (number (dBFS)), `noise_dbfs` (number (dBFS)), `frames` (whole number), `crc_failures` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level_history` | Level of the strongest channel | s | dBFS |
| `trace` | Soft chips of the last frame | chip |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `wmbus.frame` | Wireless M-Bus frame | `source` (text), `text` (text), `family` (choice), `maker` (text), `meter_id` (text), `device_type` (text), `phy` (text), `encrypted` (yes or no), `level_dbfs` (number (dBFS)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `ism`, version 1.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `WmbusFields`. The result fields of the Wireless M-Bus stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames whose CRCs passed. |
| `frames_bad` | whole number | always | 1.0 | Frames with a failed CRC. |
| `meters` | whole number | always | 1.0 | The meters in the table. |
| `syncs` | whole number | always | 1.0 | Syncs found. |
| `false_syncs` | whole number | always | 1.0 | Syncs that did not give a frame. |
| `encrypted_frames` | whole number | always | 1.0 | Frames with an encrypted payload. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `meters`

Struct `WmbusMeterRow`. A row of the meters. The row key is the maker, the id and the type.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `maker` | text | always | 1.0 | The manufacturer letters. |
| `meter_id` | text | always | 1.0 | The meter id (8 digits). |
| `device_type` | text | always | 1.0 | The device type name. |
| `phy` | text | always | 1.0 | The physical mode of the last frame ("S1", "T1", "C1 (A)" ...). |
| `frequency` | frequency | always | 1.0 | The channel of the last frame. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |
| `last_reading` | text | always | 1.0 | The summary of the last frame. |
| `status` | text | always | 1.0 | The application status. |

### Table `meter_records`

Struct `WmbusRecordRow`. A row of the data records of the last frame of each meter. The row key is the row key of the meter, "/" and the index.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `meter` | text | always | 1.0 | The row key of the meter. |
| `index` | whole number | always | 1.0 | The place of the record in its frame, from 0. |
| `quantity` | text | always | 1.0 | The quantity ("Volume"). |
| `value` | text | always | 1.0 | The value with its unit ("123.456 m3"). |
| `notes` | text | optional | 1.0 | The notes ("storage 1"), if any. |

### Table `frames`

Struct `WmbusFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `time` | time | optional | 1.0 | The UTC of the frame, when the samples are live. |
| `phy` | text | always | 1.0 | The physical mode. |
| `frequency` | frequency | always | 1.0 | The channel. |
| `meter` | text | optional | 1.0 | The meter ("KAM 12345678"), if the frame names one. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |
| `crc_ok` | yes or no | always | 1.0 | Every CRC of the frame passed. |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |
| `result` | text | always | 1.0 | The summary of the frame. |
| `frame` | text | always | 1.0 | The frame bytes in hex. |

### Table `channels`

Struct `WmbusChannelRow`. A row of the channels. The row key is the frequency in MHz and the modes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The channel frequency. |
| `phy` | text | always | 1.0 | The physical modes on the channel. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The slow power of the channel. |
| `noise_dbfs` | number (dBFS) | always | 1.0 | The noise floor of the channel. |
| `frames` | whole number | always | 1.0 | Good frames on the channel. |
| `crc_failures` | whole number | always | 1.0 | Frames with a failed CRC on the channel. |

### Event `wmbus.frame`

Struct `WmbusFrameEvent`. The fields of a frame event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The meter: maker and id ("KAM 12345678"). |
| `text` | text | always | 1.0 | The text of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `maker` | text | always | 1.0 | The manufacturer letters. |
| `meter_id` | text | always | 1.0 | The meter id. |
| `device_type` | text | always | 1.0 | The device type name. |
| `phy` | text | always | 1.0 | The physical mode. |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the frame. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_wmbus_clear` | none | none |
| `remove` | `sdrkit_wmbus_remove` | `sdrkit_wmbus_meter_args` | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_plan` | `sdrkit_wmbus_gen_get_plan` | `sdrkit_wmbus_gen_set_plan` |
| `gen_phy` | `sdrkit_wmbus_gen_get_phy` | `sdrkit_wmbus_gen_set_phy` |
| `gen_encrypted` | `sdrkit_wmbus_gen_get_encrypted` | `sdrkit_wmbus_gen_set_encrypted` |
| `gen_silence` | `sdrkit_wmbus_gen_get_silence` | `sdrkit_wmbus_gen_set_silence` |

## Try it

```sh
sdrkit run wmbus --source synth:wmbus
sdrkit run wmbus --source iiod:192.168.2.1 --freq 868.625
```

## Test recordings

- `tests/vectors/ism/wmbus_868_625_1200k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
