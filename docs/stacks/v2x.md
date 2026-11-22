<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# 802.11p V2X

Road vehicles and roadside units on a 10 MHz 802.11p channel: basic safety messages (SAE J2735) and cooperative awareness messages (ETSI) with their positions on the map; hazard warnings counted.

Stack id: `v2x`. Group: Road.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 5.9 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 10 MHz |
| Smallest sample rate | 10 MHz |
| Signal bandwidth | 10 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:v2x` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_rate` | Test signal data rate | choice | `6` | `3` (3 Mbit/s (BPSK 1/2)), `6` (6 Mbit/s (QPSK 1/2)) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `safety_messages` | Safety messages | whole number |
| `denm` | Hazard warnings | whole number |
| `vehicles` | Vehicles | whole number |
| `preambles` | Preambles | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `text` | Text | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `vehicles` | Vehicles | `kind` (choice), `source` (text), `position` (position), `speed_kt` (number (kt)), `course_deg` (number (°)), `text` (text), `frames` (whole number), `level_dbfs` (number (dBFS)), `first_heard` (time), `last_heard` (time) |
| `frames` | Frames | `time_s` (number (s)), `kind` (choice), `source` (text), `text` (text), `level_dbfs` (number (dBFS)) |

## Map points

The stack puts points on a map: Vehicles.

## Events

| Kind | Label | Fields |
|---|---|---|
| `v2x.vehicle` | V2X vehicle | `kind` (choice), `source` (text), `position` (position), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `road`, version 1.1. The C header `sdrkit/road.h`, the registry record `registry/road.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `V2xFields`. The result fields of V2X.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `safety_messages` | whole number | always | 1.0 | Safety messages (BSM and CAM). |
| `denm` | whole number | always | 1.0 | DENM frames, counted. |
| `vehicles` | whole number | always | 1.0 | Vehicles in the table. |
| `preambles` | whole number | always | 1.0 | Short training fields detected. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | optional | 1.0 | Carrier offset from where it was expected, Hz. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `vehicles`

Struct `V2xVehicleRow`. A vehicle heard. The source is the temporary id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | The kind of a row. |
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `frames` | whole number | always | 1.0 | Messages from a vehicle. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `first_heard` | time | optional | 1.0 | Wall-clock time when first heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

The column `source` names a row. Place of a row: the column `position`. Course: `course_deg`. Speed: `speed_kt`.

### Table `frames`

Struct `V2xFrameRow`. A frame of the frame log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of a frame. |
| `kind` | choice | always | 1.0 | The kind of a row. |
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |

The column `source` names a row.

### Event `v2x.vehicle`

Struct `V2xVehicleEvent`. A vehicle heard for the first time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | The kind of a row. |
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_v2x_clear` | none | none |
| `remove` | `sdrkit_v2x_remove` | `sdrkit_v2x_vehicle_args` | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_rate` | `sdrkit_v2x_gen_get_rate` | `sdrkit_v2x_gen_set_rate` |

## Try it

```sh
sdrkit run v2x --source synth:v2x
sdrkit run v2x --source iiod:192.168.2.1 --freq 5900
```
<!-- generated: end -->

## Description

The channel is resampled to 10 Msps and fed to the 20 MHz 802.11 OFDM receiver as if it were 20 Msps: every time doubles and the carrier spacing halves to 156.25 kHz, which is clause 17 at half clock. All eight rates are decoded. Data frames are read for SAE J2735 basic safety messages after the WSMP EtherType and ETSI cooperative awareness messages after the GeoNetworking EtherType; their positions go on the map. Only the temporary id that a message carries is shown. Security headers are not checked.

Standard: IEEE Std 802.11-2020 clause 17 (10 MHz channel spacing); SAE J2735; ETSI EN 302 637-2; IEEE 1609.3.

## Settings: notes

Advanced settings. The owner works without any of them set.
