<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HF packet 300 Bd

AX.25 frames of amateur packet radio on HF: 300 Bd AFSK with a 200 Hz shift on a sideband.

Stack id: `hfpacket`. Group: HF utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 1.5 MHz to 30 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hfpacket` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `sideband` | Sideband | choice | `usb` | `usb` (Upper sideband), `lsb` (Lower sideband) | Where the dial sits: below the tones (upper sideband) or above them (lower sideband). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_frames` | Test signal: frames | whole number | 4 | 1 to 64 |  |
| `gen_lead_s` | Test signal: lead | duration (s) | 0.3 s | 0 to 5 |  |
| `gen_tail_s` | Test signal: tail | duration (s) | 0.3 s | 0 to 5 |  |
| `gen_mark_hz` | Test signal: mark | frequency | 1.6 kHz | 300 Hz to 3 kHz |  |
| `gen_space_hz` | Test signal: space | frequency | 1.8 kHz | 300 Hz to 3 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `level_dbfs` | Level | number (dBFS) |
| `quality_db` | Quality | number (dB) |
| `signal_time_s` | Signal time | duration (s) |
| `input_rate_hz` | Input rate | frequency |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `tone_offset_hz` | Tone offset | frequency |
| `stations` | Stations | whole number |
| `last_frame` | Last frame | text |
| `guard.bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `time_utc` (time), `signal_time_s` (duration (s)), `text` (text) |
| `frames` | Frames | `time_utc` (time), `source` (text), `destination` (text), `path` (text), `type` (text), `pid` (whole number), `info` (text), `tone_offset_hz` (frequency) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfpacket.frame` | Frame | `text` (text), `source` (text), `destination` (text), `path` (text), `type` (text), `info` (text), `tone_offset_hz` (frequency) |
| `hfpacket.station` | Station | `text` (text), `source` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `voice`, version 1.3. The C header `sdrkit/voice.h`, the registry record `registry/voice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

### Result fields

Struct `DetectorFields`. The result fields that every single-channel stack shares.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | How far the detector is. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `quality_db` | number (dB) | optional | 1.0 | The quality measure of the stack. |
| `signal_time_s` | duration (s) | always | 1.0 | The signal time. |
| `input_rate_hz` | frequency | always | 1.0 | The rate of the input. |
| `good` | whole number | always | 1.0 | The count of good results. |
| `bad` | whole number | always | 1.0 | The count of bad results. |

### Result fields

Struct `HfpacketFields`. The result fields of HF packet.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tone_offset_hz` | frequency | always | 1.0 | The tuning correction. |
| `stations` | whole number | always | 1.0 | Different sources heard. |
| `last_frame` | text | optional | 1.0 | The newest frame in the monitor form. |

### Table `events`

Struct `DetectorEventRow`. An event of a single-channel stack. The row key is the number of the event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_utc` | time | optional | 1.0 | The wall-clock time of an event. |
| `signal_time_s` | duration (s) | always | 1.0 | The signal time. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Table `frames`

Struct `HfpacketFrameRow`. A frame with a good FCS. The row key is the number of the frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_utc` | time | optional | 1.0 | When a call started. |
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `destination` | text | always | 1.0 | The source of a frame. |
| `path` | text | always | 1.0 | The digipeater path of a frame. |
| `type` | text | always | 1.0 | The frame type. |
| `pid` | whole number | optional | 1.0 | The protocol identifier. |
| `info` | text | always | 1.0 | The information field as text. |
| `tone_offset_hz` | frequency | always | 1.0 | The tuning correction. |

The column `source` names a row.

### Event `hfpacket.frame`

Struct `HfpacketFrameEvent`. A frame with a good FCS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `destination` | text | always | 1.0 | The source of a frame. |
| `path` | text | always | 1.0 | The digipeater path of a frame. |
| `type` | text | always | 1.0 | The frame type. |
| `info` | text | always | 1.0 | The information field as text. |
| `tone_offset_hz` | frequency | always | 1.0 | The tuning correction. |

### Event `hfpacket.station`

Struct `HfpacketStationEvent`. A station heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `source` | text | always | 1.0 | The station. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hfpacket_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `sideband` | `sdrkit_hfpacket_get_sideband` | `sdrkit_hfpacket_set_sideband` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_frames` | `sdrkit_hfpacket_gen_get_frames` | `sdrkit_hfpacket_gen_set_frames` |
| `gen_lead_s` | `sdrkit_hfpacket_gen_get_lead_s` | `sdrkit_hfpacket_gen_set_lead_s` |
| `gen_tail_s` | `sdrkit_hfpacket_gen_get_tail_s` | `sdrkit_hfpacket_gen_set_tail_s` |
| `gen_mark_hz` | `sdrkit_hfpacket_gen_get_mark_hz` | `sdrkit_hfpacket_gen_set_mark_hz` |
| `gen_space_hz` | `sdrkit_hfpacket_gen_get_space_hz` | `sdrkit_hfpacket_gen_set_space_hz` |

## Try it

```sh
sdrkit run hfpacket --source synth:hfpacket
sdrkit run hfpacket --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
