<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# MAVLink (SiK)

Telemetry radios of drones: SiK packets at every air rate, with and without error correction, the MAVLink messages in them and the position of each vehicle.

Stack id: `mavlink_sik`. Group: Aircraft and drones.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 433.92 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 8 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mavlink_sik` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `air_rate` | Air speed | choice | `auto` | `auto` (Auto), `2`, `4`, `8`, `16`, `19`, `24`, `32`, `48`, `64`, `96`, `128`, `192`, `250` | The air rate of the radios, kbit/s. Auto searches every rate that the sample rate allows (at least 2.5 samples a bit). |
| `threshold_db` | Burst threshold | number (dB) | 9 | 6 to 20 | How far above the noise floor of a frequency bin a burst must rise. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `bursts` | Bursts | whole number |
| `bursts_without_frame` | Bursts without a frame | whole number |
| `state` | State | whole number |
| `links_heard` | Links | whole number |
| `covered` | Searched | text |
| `snr_db` | SNR | number (dB) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `links` | Links | `link` (text), `id` (text), `frames` (whole number), `first_heard` (time), `last_heard` (time), `last_type` (text), `last_detail` (text), `position` (position), `alt_m` (number (m)) |
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `link` (text), `id` (text), `type` (text), `detail` (text), `hex` (text), `freq_hz` (number (Hz)), `snr_db` (number (dB)), `encrypted` (yes or no) |

## Map points

The stack puts points on a map: Vehicles.

## Events

| Kind | Label | Fields |
|---|---|---|
| `mavlink_sik.vehicle` | New MAVLink vehicle | `source` (text), `link` (text), `position` (position), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 1.0. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `LinkFields`. The result fields of the link stacks.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `bursts` | whole number | always | 1.0 | Bursts analysed. |
| `bursts_without_frame` | whole number | always | 1.0 | Bursts without a frame. |
| `state` | whole number | always | 1.0 | 0 nothing, 1 bursts seen, 2 frames decoded lately. |
| `links_heard` | whole number | always | 1.0 | Links in the table. |
| `covered` | text | always | 1.0 | The span of the capture that is searched. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `links`

Struct `HeardLinkRow`. A link heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | text | always | 1.0 | The link name. |
| `id` | text | always | 1.0 | The transmitter id. |
| `frames` | whole number | always | 1.0 | Frames of a link. |
| `first_heard` | time | optional | 1.0 | Wall-clock time when first heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `last_type` | text | always | 1.0 | The type of the last frame of a link. |
| `last_detail` | text | always | 1.0 | The fields of the last frame of a link. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | optional | 1.0 | The altitude of the last position. |

### Table `frames`

Struct `LinkFrameRow`. A frame in the frame log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The wall time of a frame. |
| `time_s` | number (s) | always | 1.0 | The signal time of a frame. |
| `link` | text | always | 1.0 | The link name. |
| `id` | text | always | 1.0 | The transmitter id. |
| `type` | text | always | 1.0 | What a frame is. |
| `detail` | text | always | 1.0 | The fields of a frame. |
| `hex` | text | always | 1.0 | The first bytes of a frame as hex. |
| `freq_hz` | number (Hz) | always | 1.0 | The frequency of a frame. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |

### Event `mavlink_sik.vehicle`

Struct `MavlinkSikVehicleEvent`. A new MAVLink vehicle.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `link` | text | always | 1.0 | The link name. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_mavlink_sik_clear` | none | none |
| `remove` | `sdrkit_mavlink_sik_remove` | `sdrkit_drones_link_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `air_rate` | `sdrkit_mavlink_sik_get_air_rate` | `sdrkit_mavlink_sik_set_air_rate` |
| `threshold_db` | `sdrkit_mavlink_sik_get_threshold_db` | `sdrkit_mavlink_sik_set_threshold_db` |

## Try it

```sh
sdrkit run mavlink_sik --source synth:mavlink_sik
sdrkit run mavlink_sik --source iiod:192.168.2.1 --freq 433.92
```
<!-- generated: end -->

## Description

The receiver finds bursts anywhere in the capture (the radios hop; the hops are not followed). For each air rate that the
sample rate allows (at least 2.5 samples a bit: 2, 4, 8, 16, 19, 24, 32, 48, 64, 96, 128, 192 and 250 kbit/s), the rates whose
GFSK width is nearest the burst's first, it mixes the burst to its centre, filters and decimates it to 8 samples a bit, and
searches the discriminator for AA AA 2D D4 (the end of the preamble and the sync word) with at most one bit error. The bytes
after it are read as a SiK packet: net id, length, data and CRC-16/CMS; or, with error correction, the length, the Golay
(23,12) words of the header, of the SiK CRC and of the data. The data end with a 2-byte trailer: the TDM window and the command,
bonus and resend bits.

The data of each net id are a stream in which MAVLink v1 and v2 frames are found across packets; bytes of a frame that is not
whole wait for the next packet, and more than 300 waiting bytes are dropped. A frame counts only with a good checksum, which
needs the CRC_EXTRA byte of its message: the stack knows HEARTBEAT, SYS_STATUS, GPS_RAW_INT, ATTITUDE, GLOBAL_POSITION_INT,
VFR_HUD and STATUSTEXT, and shows their fields. A signed v2 frame is marked; the signature is not checked. GPS_RAW_INT with a
fix and GLOBAL_POSITION_INT give the position of the link and a point on the map.

Not confirmed by a public document: the SiK packet layout and its CRC coverage (net id, length and data are assumed), the
error-corrected form and its CRC, and the trailer. Frames of other MAVLink messages than the seven above are not accepted. Not
tried on a real signal.

Standard: MAVLink Developer Guide (frames, checksum, CRC_EXTRA); message definitions `minimal.xml` and `common.xml`.

## Tuning: notes

The stack holds no band: the bands per region (433 MHz in Europe, 902 to 928 MHz in the Americas, 915 to 928 MHz in Australia)
are in `data/plans/drone_links.json`, for the host to offer.

## Result tables: notes

A link is "net N, sys S / C": the net id of the radios and the system and component ids of MAVLink. The type of a frame is the
MAVLink message name, or "SiK packet" or "SiK packet (ECC)" for a packet with no whole frame in it.
