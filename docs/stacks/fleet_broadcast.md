<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Fleet Broadcast

The 16-channel 1200 Bd DBPSK multiplex of UHF Fleet Broadcast: carrier, clock, frame alignment and the activity of each channel. Nothing is decoded. Experimental: the sync channel's pattern is not public. Receive only.

Stack id: `fleet_broadcast`. Group: Satellite.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 250 MHz |
| Tuning range | 240 MHz to 270 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 72 kHz |
| Signal bandwidth | 250 kHz |
| Offset from the signal | 30 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fleet_broadcast` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `invert` | Invert bits | yes or no | false |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scenario` | Scenario | choice | `0` | `0` (Sync, reversals, idle and random channels), `1` (Sync, idle channels), `2` (Noise only), `3` (A plain carrier), `4` (Random bits everywhere) |  |
| `gen_carrier_hz` | Carrier from the dial | number (Hz) | 1500 | -4500 to 4500 |  |
| `gen_fade_db` | Fading depth | number (dB) | 0 | 0 to 30 | 0 for none; else the depth of a flat fading at 0.5 Hz. |
| `gen_invert` | Invert bits | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | whole number |
| `offset_hz` | Offset | frequency |
| `freq_hz` | Frequency | frequency |
| `line_db` | Carrier line | number (dB) |
| `clock_ppm` | Clock error | number (ppm) |
| `eye` | Eye | number |
| `snr_db` | SNR | number (dB) |
| `symbols` | Symbols | whole number |
| `framed` | Framed | yes or no |
| `ambiguous` | Ambiguous | yes or no |
| `sync_period` | Sync repeat | whole number (frames) |
| `sync_agreement` | Sync agreement | number (%) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `classes` | Channels | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Channels | `channel` (whole number), `class` (text), `activity_pct` (number (%)), `ones_pct` (number (%)), `sync_period` (whole number (frames)), `last_bits` (text) |
| `frame_log` | Frame log | `word` (text), `bits` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fleet_broadcast.state` | Fleet Broadcast state | `text` (text), `state` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.0. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FleetBroadcastFields`. The result fields of Fleet Broadcast.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | whole number | always | 1.0 | The state: 0 nothing, 1 carrier, 2 bits, 3 framed. |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `freq_hz` | frequency | optional | 1.0 | The frequency a station was heard on. |
| `line_db` | number (dB) | always | 1.0 | The line of the carrier search over the floor. |
| `clock_ppm` | number (ppm) | always | 1.0 | Sample-clock or symbol-clock error, parts per million. |
| `eye` | number | always | 1.0 | The eye: 0 clean, towards 1 noisy. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `symbols` | whole number | always | 1.0 | Symbols so far. |
| `framed` | yes or no | always | 1.0 | The frames are aligned. |
| `ambiguous` | yes or no | always | 1.0 | Several columns repeat. |
| `sync_period` | whole number (frames) | optional | 1.0 | The repeat of the sync channel, frames. |
| `sync_agreement` | number (%) | optional | 1.0 | The agreement of the sync channel with itself one period later. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `classes` | text | always | 1.0 | The class letters, channel 0 first. |
| `note` | text | always | 1.0 | What is not confirmed by a public document. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `channels`

Struct `FleetBroadcastChannelRow`. One of the 16 channels. The row key is its place, `000` first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | whole number | always | 1.0 | The channel (or column, when not framed). |
| `class` | text | always | 1.0 | The class of a channel. |
| `activity_pct` | number (%) | always | 1.0 | The share of bit changes. |
| `ones_pct` | number (%) | always | 1.0 | The share of ones. |
| `sync_period` | whole number (frames) | optional | 1.0 | The repeat of the sync channel, frames. |
| `last_bits` | text | always | 1.0 | The last 64 bits as hex, oldest first. |

### Table `frame_log`

Struct `FleetBroadcastFrameRow`. A frame of the frame log. The row key is its place, `000` first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `word` | text | always | 1.0 | A frame as 4 hex digits, channel 0 the most significant bit. |
| `bits` | text | always | 1.0 | A frame as 16 bits, channel 0 first. |

### Event `fleet_broadcast.state`

Struct `FleetBroadcastStateEvent`. A change of the state of the receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `state` | whole number | always | 1.0 | The state: 0 nothing, 1 carrier, 2 bits, 3 framed. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fleet_broadcast_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `invert` | `sdrkit_fleet_broadcast_get_invert` | `sdrkit_fleet_broadcast_set_invert` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scenario` | `sdrkit_fleet_broadcast_gen_get_scenario` | `sdrkit_fleet_broadcast_gen_set_scenario` |
| `gen_carrier_hz` | `sdrkit_fleet_broadcast_gen_get_carrier_hz` | `sdrkit_fleet_broadcast_gen_set_carrier_hz` |
| `gen_fade_db` | `sdrkit_fleet_broadcast_gen_get_fade_db` | `sdrkit_fleet_broadcast_gen_set_fade_db` |
| `gen_invert` | `sdrkit_fleet_broadcast_gen_get_invert` | `sdrkit_fleet_broadcast_gen_set_invert` |

## Try it

```sh
sdrkit run fleet_broadcast --source synth:fleet_broadcast
sdrkit run fleet_broadcast --source iiod:192.168.2.1 --freq 250
```
<!-- generated: end -->

## Description

The stack finds the carrier from the line of the squared signal, the 1200 Bd clock from the line of the signal energy, and reads the bits by differential detection. It cuts the bit stream into 16 columns, one bit a channel, and classes each column: sync (repeats with a period of 3 to 64 frames), reversals, idle, random-looking or unknown. Exactly one sync column is channel 0, and the frames are aligned on it. The test signal's channel 0 carries a 31-bit m-sequence of its own, not the pattern of a real broadcast. The field `note` says what is not confirmed by a public document.

## Tuning: notes

The default frequency is the dial of the test signal, not a carrier of a real broadcast.

## Settings: notes

Advanced settings. The owner works without any of them set.
