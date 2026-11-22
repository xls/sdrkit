<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# VDES

VHF data exchange: ASM frames decoded, VDE-TER bursts measured and classified.

Stack id: `vdes`. Group: Marine.

Signal types: VDES.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 161.9 MHz |
| Tuning range | 150 MHz to 165 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 400 kHz |
| Signal bandwidth | 400 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:vdes` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `nmea_out` | Send ASM as NMEA | yes or no | true |  | AIS-like ASM frames go out as !AIVDM sentences. |
| `time_base` | Time base | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `bursts` | VDE-TER bursts | whole number |
| `reports` | Reports | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `asm` | ASM frames | `time` (time), `channel` (text), `bits` (whole number), `msg_type` (whole number), `mmsi` (text), `snr_db` (number (dB)), `offset_hz` (frequency), `position` (position), `speed_kt` (number (kt)), `course_deg` (number (°)), `hex` (text) |
| `bursts` | VDE-TER bursts | `time` (time), `centre_hz` (frequency), `bandwidth_hz` (frequency), `modulation` (choice), `rssi_dbfs` (number (dBFS)), `snr_db` (number (dB)), `duration` (duration (s)), `offset_hz` (frequency), `cv` (number), `psi` (number), `symbols` (whole number), `subchannels` (text) |
| `channels` | Channels | `name` (text), `freq_hz` (frequency), `in_view` (yes or no), `frames_ok` (whole number), `frames_bad` (whole number), `bursts` (whole number), `level_dbfs` (number (dBFS)), `noise_dbfs` (number (dBFS)) |
| `by_modulation` | Bursts by modulation | `modulation` (choice), `count` (whole number) |
| `by_bandwidth` | Bursts by bandwidth | `bandwidth_hz` (frequency), `count` (whole number) |

## Map points

The stack puts points on a map: ASM positions.

## Events

| Kind | Label | Fields |
|---|---|---|
| `vdes.asm` | ASM frame | `source` (text), `family` (choice), `channel` (text), `bits` (whole number), `hex` (text), `msg_type` (whole number) |
| `vdes.nmea` | ASM sentence | `source` (text), `family` (choice), `sentence` (text) |
| `vdes.burst` | VDE-TER burst | `family` (choice), `modulation` (choice), `bandwidth_hz` (frequency), `duration` (duration (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `marine`, version 1.3. The C header `sdrkit/marine.h`, the registry record `registry/marine.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `VdesFields`. The result fields of the VDES stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `bursts` | whole number | always | 1.0 | Bursts found on the VDE-TER channels. |
| `reports` | whole number | always | 1.0 | Reports made. |

### Table `asm`

Struct `VdesAsmRow`. A row of the ASM frame table. The row key is the place of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When (wall-clock time, when known). |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `bits` | whole number | always | 1.0 | The payload length. |
| `msg_type` | whole number | optional | 1.0 | The AIS message type of the payload. |
| `mmsi` | text | optional | 1.0 | The MMSI, nine digits. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `hex` | text | always | 1.0 | The payload in hexadecimal. |

The column `mmsi` names a row. Place of a row: the column `position`. Course: `course_deg`. Speed: `speed_kt`.

### Table `bursts`

Struct `VdesBurstRow`. A row of the VDE-TER burst table. The row key is the place of the burst in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When (wall-clock time, when known). |
| `centre_hz` | frequency | always | 1.0 | The centre of a burst's occupied sub-channels. |
| `bandwidth_hz` | frequency | always | 1.0 | The bandwidth of a burst. |
| `modulation` | choice | always | 1.0 | The modulation of a burst. |
| `rssi_dbfs` | number (dBFS) | always | 1.0 | The peak power of a burst. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `duration` | duration (s) | always | 1.0 | How long a burst lasted. |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `cv` | number | always | 1.0 | The spread of the symbol amplitudes. |
| `psi` | number | always | 1.0 | The mean of cos(4 x phase step). |
| `symbols` | whole number | always | 1.0 | The symbols used for the classification. |
| `subchannels` | text | always | 1.0 | The occupied sub-channels. |

### Table `channels`

Struct `VdesChannelRow`. A row of the channel table of the VDES stack. The row key is the letter of an ASM channel, or `TER` and the number of a VDE-TER band.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of a channel. |
| `freq_hz` | frequency | always | 1.0 | The frequency a station was heard on. |
| `in_view` | yes or no | always | 1.0 | The capture holds the channel. |
| `frames_ok` | whole number | optional | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | optional | 1.0 | Frames that failed their check. |
| `bursts` | whole number | optional | 1.0 | Bursts found on the VDE-TER channels. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | Noise floor, dBFS. |

The column `name` names a row. Default order: the column `freq_hz`, lowest first.

### Table `by_modulation`

Struct `VdesModulationRow`. A row of the count of bursts by modulation. The row key is the name of the modulation.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `modulation` | choice | always | 1.0 | The modulation of a burst. |
| `count` | whole number | always | 1.0 | A number of bursts. |

The column `modulation` names a row.

### Table `by_bandwidth`

Struct `VdesBandwidthRow`. A row of the count of bursts by bandwidth. The row key is the bandwidth in Hz.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bandwidth_hz` | frequency | always | 1.0 | The bandwidth of a burst. |
| `count` | whole number | always | 1.0 | A number of bursts. |

The column `bandwidth_hz` names a row. Default order: the column `bandwidth_hz`, lowest first.

### Event `vdes.asm`

Struct `VdesAsmEvent`. The fields of the event of an ASM frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `bits` | whole number | always | 1.0 | The payload length. |
| `hex` | text | always | 1.0 | The payload in hexadecimal. |
| `msg_type` | whole number | optional | 1.0 | The AIS message type of the payload. |

### Event `vdes.nmea`

Struct `VdesSentenceEvent`. The fields of the event of an NMEA sentence of an ASM frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `sentence` | text | always | 1.0 | An `!AIVDM` sentence. |

### Event `vdes.burst`

Struct `VdesBurstEvent`. The fields of the event of a VDE-TER burst.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `modulation` | choice | always | 1.0 | The modulation of a burst. |
| `bandwidth_hz` | frequency | always | 1.0 | The bandwidth of a burst. |
| `duration` | duration (s) | always | 1.0 | How long a burst lasted. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_vdes_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `nmea_out` | `sdrkit_vdes_get_nmea_out` | `sdrkit_vdes_set_nmea_out` |
| `time_base` | `sdrkit_vdes_get_time_base` | `sdrkit_vdes_set_time_base` |

## Try it

```sh
sdrkit run vdes --source synth:vdes
sdrkit run vdes --source iiod:192.168.2.1 --freq 161.9
```

## Test recordings

- `tests/vectors/marine/vdes_asm_vdeter_400k.sigmf-meta`
<!-- generated: end -->
