<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LoRaWAN (all channels)

Every channel of a LoRaWAN plan at once, uplinks and downlinks, and the FSK channel.

Stack id: `lorawanwide`. Group: IoT and mesh.

Signal types: LoRa.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 137 MHz to 2.5 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 250 kHz |
| Offset from the signal | 100 kHz |
| Radio filter | 3.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:lorawanwide` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `downlinks` | Downlinks | yes or no | true |  |  |
| `defaults_only` | Default channels only | yes or no | false |  |  |
| `private_sync` | Private networks (0x12) | yes or no | false |  |  |
| `max_channels` | Channels at most | whole number | 24 | 1 to 72 |  |
| `wide_channels` | 250 and 500 kHz channels | yes or no | true |  |  |
| `fsk` | FSK channel | yes or no | true |  |  |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `dial_freq` | Dial | frequency | 0 Hz | 0 Hz to 6 GHz | The middle of the channels: the centre of the capture when the input does not state it, and the dial of the test signal. |
| `lw_channels` | LoRaWAN channels | text | empty |  | Entries separated by ";", each the frequency in Hz, the bandwidth in Hz, the spreading factors and the flags, separated by "/": u devices send there, d gateways answer there, j a default channel. For example "100100000/125000/7-12/udj". At most 256 entries. Empty: one channel at the dial. |
| `fsk_freq` | FSK frequency | frequency | 0 Hz | 0 Hz to 6 GHz |  |
| `clock0` | Computer clock at signal 0 | number (s) | 0 | 0 to 1000000000000 | Seconds since 1970 at the first sample. 0 when the times follow the signal. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `preambles` | Preambles | whole number |
| `header_failed` | Headers failed | whole number |
| `fsk_syncs` | FSK syncs | whole number |
| `fsk_frames` | FSK frames | whole number |
| `fsk_crc_errors` | FSK CRC errors | whole number |
| `demodulators` | Demodulators | whole number |
| `channels_listed` | Channels in the plan | whole number |
| `channels_decoded` | Channels decoded | whole number |
| `note` | Note | text |
| `plan` | Plan | text |
| `devices` | LoRaWAN devices | whole number |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `type` (text), `device` (text), `network` (text), `fcnt` (whole number), `fport` (whole number), `payload_bytes` (whole number (B)), `fctrl` (text), `mac_commands` (text), `mic` (text), `modulation` (choice), `sf` (whole number), `snr_db` (number (dB)), `freq` (frequency), `summary` (text) |
| `channels` | Channels | `freq` (frequency), `bw` (frequency), `sf_min` (whole number), `sf_max` (whole number), `in_band` (yes or no), `preambles` (whole number), `frames` (whole number), `crc_errors` (whole number) |
| `devices` | Devices | `id` (text), `network` (text), `vendor` (text), `last_heard` (time), `heard` (whole number), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `fcnt` (whole number), `missed` (whole number), `sf` (whole number), `freq` (frequency), `uplinks` (whole number), `downlinks` (whole number), `joins` (whole number), `type` (text), `fport` (whole number), `mac` (text) |
| `types` | Packet types | `net` (choice), `type` (text), `count` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `lora.packet` | LoRa packet | `net` (choice), `modulation` (choice), `snr_db` (number (dB)), `sf` (whole number), `payload` (bytes), `crc_ok` (yes or no), `source` (text), `target` (text), `devaddr` (text), `iq_inverted` (yes or no), `crc` (yes or no), `bw` (frequency), `cr` (whole number), `implicit` (yes or no), `sync` (whole number), `level_dbfs` (number (dBFS)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `reset` | Reset |  |  | Forgets the signal and makes the bank again. |

## Typed interface

The stack is in the family `lora`, version 5.0. The C header `sdrkit/lora.h`, the registry record `registry/lora.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `WideFields`. The result fields of the LoRaWAN stack of every channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, once the bank reports. |
| `frames_ok` | whole number | always | 1.0 | Frames with a good CRC. |
| `frames_bad` | whole number | always | 1.0 | Frames that are not LoRaWAN or whose CRC failed. |
| `preambles` | whole number | always | 1.0 | Frame preambles seen. |
| `header_failed` | whole number | always | 1.0 | Headers that failed their checksum. |
| `fsk_syncs` | whole number | always | 1.0 | FSK sync words found. |
| `fsk_frames` | whole number | always | 1.0 | FSK frames. |
| `fsk_crc_errors` | whole number | always | 1.0 | FSK frames whose CRC failed. |
| `demodulators` | whole number | always | 1.0 | LoRa demodulators running. |
| `channels_listed` | whole number | always | 1.0 | Channels in the plan. |
| `channels_decoded` | whole number | always | 1.0 | Channels decoded. |
| `note` | text | always | 1.0 | What is decoded, in words. |
| `plan` | text | always | 1.0 | The plan in words: its count of channels and their span. |
| `devices` | whole number | always | 1.0 | LoRaWAN devices. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `frames`

Struct `WideFrameRow`. A row of the LoRaWAN frame log. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `type` | text | optional | 1.0 | The frame type. |
| `device` | text | optional | 1.0 | The DevAddr or the DevEUI. |
| `network` | text | optional | 1.0 | The network operator. |
| `fcnt` | whole number | optional | 1.0 | The frame counter. |
| `fport` | whole number | optional | 1.0 | The FPort. |
| `payload_bytes` | whole number (B) | optional | 1.0 | The encrypted payload length of a LoRaWAN frame. |
| `fctrl` | text | optional | 1.0 | The FCtrl flags. |
| `mac_commands` | text | optional | 1.0 | The MAC commands. |
| `mic` | text | optional | 1.0 | The message integrity code, hex. |
| `modulation` | choice | always | 1.0 | How the frame was modulated. |
| `sf` | whole number | optional | 1.0 | The spreading factor, none for the FSK channel. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the frame. |
| `freq` | frequency | always | 1.0 | The frequency of the frame. |
| `summary` | text | always | 1.0 | More about the frame. |

The rows are a log: they arrive over time. The column `device` names a row. Time: `time_s`.

### Table `channels`

Struct `WideChannelRow`. A row of the LoRaWAN channels. The row key is the place of the channel in the plan.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq` | frequency | always | 1.0 | The channel frequency. |
| `bw` | frequency | always | 1.0 | The bandwidth. |
| `sf_min` | whole number | always | 1.0 | The least spreading factor searched. |
| `sf_max` | whole number | always | 1.0 | The greatest spreading factor searched. |
| `in_band` | yes or no | always | 1.0 | The channel lies inside the captured band. |
| `preambles` | whole number | always | 1.0 | Preambles. |
| `frames` | whole number | always | 1.0 | Frames decoded. |
| `crc_errors` | whole number | always | 1.0 | CRC errors. |

Default order: the column `freq`, lowest first.

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

The column `type` names a row.

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

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_lorawanwide_clear` | none | none |
| `reset` | `sdrkit_lorawanwide_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `dial_freq` | `sdrkit_lorawanwide_get_dial_freq` | `sdrkit_lorawanwide_set_dial_freq` |
| `lw_channels` | `sdrkit_lorawanwide_get_lw_channels` | `sdrkit_lorawanwide_set_lw_channels` |
| `downlinks` | `sdrkit_lorawanwide_get_downlinks` | `sdrkit_lorawanwide_set_downlinks` |
| `defaults_only` | `sdrkit_lorawanwide_get_defaults_only` | `sdrkit_lorawanwide_set_defaults_only` |
| `private_sync` | `sdrkit_lorawanwide_get_private_sync` | `sdrkit_lorawanwide_set_private_sync` |
| `max_channels` | `sdrkit_lorawanwide_get_max_channels` | `sdrkit_lorawanwide_set_max_channels` |
| `wide_channels` | `sdrkit_lorawanwide_get_wide_channels` | `sdrkit_lorawanwide_set_wide_channels` |
| `fsk` | `sdrkit_lorawanwide_get_fsk` | `sdrkit_lorawanwide_set_fsk` |
| `fsk_freq` | `sdrkit_lorawanwide_get_fsk_freq` | `sdrkit_lorawanwide_set_fsk_freq` |
| `clock0` | `sdrkit_lorawanwide_get_clock0` | `sdrkit_lorawanwide_set_clock0` |

## Try it

```sh
sdrkit run lorawanwide --source synth:lorawanwide
sdrkit run lorawanwide --source <driver>:<address> --freq 0
```
<!-- generated: end -->
