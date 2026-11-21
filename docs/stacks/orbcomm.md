<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Orbcomm

Orbcomm 137 MHz downlinks: packets, satellite positions, orbital elements, channel lists and messages. Receive only.

Stack id: `orbcomm`. Group: Satellite.

Signal types: Orbcomm.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 137.25 MHz |
| Tuning range | 136 MHz to 139 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 600 kHz |
| Offset from the signal | 25 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:orbcomm` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `decoders` | Decoders | choice | `2` | `1`, `2` | Downlinks decoded at once. |
| `follow` | Follow satellite | yes or no | false |  | The map follows the selected satellite. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_downlinks` | Downlinks | whole number | 1 | 1 to 2 |  |
| `gen_satellite_doppler_hz` | Satellite Doppler | frequency | 1.2 kHz | -10 kHz to 10 kHz | The first downlink's Doppler. The second has the opposite sign. |
| `gen_ramp_hz` | Doppler ramp (+-) | frequency | 0 Hz | 0 Hz to 10 kHz | 0: the fixed Doppler. Else a triangular ramp between + and - this. |
| `gen_ramp_s` | Ramp leg | duration (s) | 20 s | 1 to 600 |  |
| `gen_broken` | Broken checks | number | 0 | 0 to 1 | The share of packets sent with a broken Fletcher check. |
| `gen_centre_hz` | Radio centre | frequency | 137.275 MHz | 136 MHz to 139 MHz | Where the radio is tuned, so the generator puts the downlinks in the right place. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `messages_joined` | Messages joined | whole number |
| `satellites` | Satellites | whole number |
| `dropped` | Dropped | whole number |
| `receiver.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `satellites` | Satellites | `satellite` (whole number), `frequency` (frequency), `doppler` (frequency), `snr_db` (number (dB)), `frames` (whole number), `heard` (whole number), `bad` (whole number), `position` (position), `height_m` (number (m)), `inclination_deg` (number (deg)), `period` (duration (s)), `last_heard` (time) |
| `satellite_channels` | Satellite channels | `satellite` (whole number), `uplink` (yes or no), `frequency` (frequency) |
| `decoders` | Decoders | `decoder` (whole number), `state` (choice), `frequency` (frequency), `offset` (frequency), `doppler` (frequency), `snr_db` (number (dB)), `level_db` (number (dB)), `satellite` (whole number), `clock_ppm` (number (ppm)), `frames` (whole number), `frames_ok` (whole number), `frames_bad` (whole number), `sync_losses` (whole number) |
| `packets` | Packets | `at` (duration (s)), `wall` (time), `satellite` (whole number), `decoder` (whole number), `type` (choice), `bytes` (bytes), `text` (text) |
| `packet_types` | Packet types | `type` (choice), `count` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |

## Map points

The stack puts points on a map: Satellites.

## Events

| Kind | Label | Fields |
|---|---|---|
| `orbcomm.message` | Message | `source` (text), `text` (text), `family` (choice), `message_id` (whole number), `complete` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `preset.apply` | Tune to channel | `channel` (choice, default `137.2500`) |  | Asks the host to tune the dial to the channel. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `satcom`, version 1.0. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `OrbcommFields`. The result fields of Orbcomm.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `messages_joined` | whole number | always | 1.0 | Messages joined from their parts. |
| `satellites` | whole number | always | 1.0 | Satellites heard. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `satellites`

Struct `OrbcommSatelliteRow`. A satellite heard. The row key is the satellite number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `satellite` | whole number | always | 1.0 | The satellite. |
| `frequency` | frequency | optional | 1.0 | The frequency. |
| `doppler` | frequency | optional | 1.0 | The doppler. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `frames` | whole number | always | 1.0 | The frames. |
| `heard` | whole number | optional | 1.0 | How many times it was heard. |
| `bad` | whole number | always | 1.0 | The bad packets. |
| `position` | position | optional | 1.0 | The position. |
| `height_m` | number (m) | optional | 1.0 | The height. |
| `inclination_deg` | number (deg) | optional | 1.0 | The inclination. |
| `period` | duration (s) | optional | 1.0 | The period. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |

### Table `satellite_channels`

Struct `OrbcommSatelliteChannelRow`. A channel that a satellite announces. The row key is `<satellite>/<u or d><n>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `satellite` | whole number | always | 1.0 | The satellite. |
| `uplink` | yes or no | always | 1.0 | The channel is an uplink (else a downlink). |
| `frequency` | frequency | always | 1.0 | The frequency. |

### Table `decoders`

Struct `OrbcommDecoderRow`. A decoder. The row key is the decoder number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `decoder` | whole number | always | 1.0 | The decoder. |
| `state` | choice | always | 1.0 | The state. |
| `frequency` | frequency | optional | 1.0 | The frequency. |
| `offset` | frequency | optional | 1.0 | The offset. |
| `doppler` | frequency | optional | 1.0 | The doppler. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `level_db` | number (dB) | optional | 1.0 | The level. |
| `satellite` | whole number | optional | 1.0 | The satellite. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The clock error. |
| `frames` | whole number | optional | 1.0 | The frames. |
| `frames_ok` | whole number | optional | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | optional | 1.0 | Frames that failed their check. |
| `sync_losses` | whole number | optional | 1.0 | The sync losses. |

### Table `packets`

Struct `OrbcommPacketRow`. A packet of the log. The row key is the sequence number of the packet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The at (signal time). |
| `wall` | time | optional | 1.0 | The time. |
| `satellite` | whole number | optional | 1.0 | The satellite. |
| `decoder` | whole number | always | 1.0 | The decoder. |
| `type` | choice | always | 1.0 | The type. |
| `bytes` | bytes | optional | 1.0 | The bytes. |
| `text` | text | always | 1.0 | What the packet says. |

### Table `packet_types`

Struct `OrbcommPacketTypeRow`. The packets of one type. The row key is the key of the type.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `type` | choice | always | 1.0 | The type. |
| `count` | whole number | always | 1.0 | The count. |

### Event `orbcomm.message`

Struct `OrbcommMessageEvent`. A message joined from its parts. The source is the satellite ("satellite 12").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The satellite ("satellite 12"). |
| `text` | text | always | 1.0 | The text of the message. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `message_id` | whole number | always | 1.0 | The message number of a joined message. |
| `complete` | yes or no | always | 1.0 | Whether every part of a joined message came. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_orbcomm_clear` | none | none |
| `preset.apply` | `sdrkit_orbcomm_preset_apply` | `sdrkit_orbcomm_channel_args` | none |
| `remove` | `sdrkit_orbcomm_remove` | `sdrkit_orbcomm_satellite_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `decoders` | `sdrkit_orbcomm_get_decoders` | `sdrkit_orbcomm_set_decoders` |
| `follow` | `sdrkit_orbcomm_get_follow` | `sdrkit_orbcomm_set_follow` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_downlinks` | `sdrkit_orbcomm_gen_get_downlinks` | `sdrkit_orbcomm_gen_set_downlinks` |
| `gen_satellite_doppler_hz` | `sdrkit_orbcomm_gen_get_satellite_doppler_hz` | `sdrkit_orbcomm_gen_set_satellite_doppler_hz` |
| `gen_ramp_hz` | `sdrkit_orbcomm_gen_get_ramp_hz` | `sdrkit_orbcomm_gen_set_ramp_hz` |
| `gen_ramp_s` | `sdrkit_orbcomm_gen_get_ramp_s` | `sdrkit_orbcomm_gen_set_ramp_s` |
| `gen_broken` | `sdrkit_orbcomm_gen_get_broken` | `sdrkit_orbcomm_gen_set_broken` |
| `gen_centre_hz` | `sdrkit_orbcomm_gen_get_centre_hz` | `sdrkit_orbcomm_gen_set_centre_hz` |

## Try it

```sh
sdrkit run orbcomm --source synth:orbcomm
sdrkit run orbcomm --source iiod:192.168.2.1 --freq 137.25
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
