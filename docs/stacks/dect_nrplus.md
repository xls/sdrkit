<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DECT NR+

DECT-2020 NR packets on a 1.728 MHz channel: the physical layer control field (network id, transmitter and receiver ids, length, MCS, HARQ); the data field is not decoded.

Stack id: `dect_nrplus`. Group: IoT, 1.9 GHz.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.881792 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 4 MHz |
| Smallest sample rate | 1.728 MHz |
| Signal bandwidth | 1.728 MHz |
| Automatic gain | Radio AGC |

## Settings

The stack has no settings.

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `stf_detections` | STF detections | whole number |
| `devices` | Devices | whole number |
| `snr_db` | SNR | number (dB) |
| `cfo_hz` | Carrier offset | number (Hz) |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `packets` | Packets | `utc` (time), `time_s` (number (s)), `id` (text), `type` (text), `text` (text), `cfo_hz` (number (Hz)), `snr_db` (number (dB)) |
| `devices` | Devices | `transmitter` (text), `id` (text), `packets` (whole number), `text` (text), `snr_db` (number (dB)), `first_heard` (time), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `dect_nrplus.packet` | DECT NR+ packet | `id` (text), `type` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `ism`, version 4.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `NrPlusFields`. The result fields of the DECT NR+ stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Packets read with a good check. |
| `stf_detections` | whole number | always | 1.0 | Short training field detections. |
| `devices` | whole number | always | 1.0 | The devices in the table. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last packet. It has no value before the first packet. |
| `cfo_hz` | number (Hz) | optional | 1.0 | The carrier offset of the last packet. It has no value before the first packet. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. It has no value before the first input. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `packets`

Struct `NrPlusPacketRow`. A row of the packet log. The row key is the number of the packet in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the packet, when the samples are live. |
| `time_s` | number (s) | always | 1.0 | The signal time of the packet. |
| `id` | text | always | 1.0 | The device. |
| `type` | text | always | 1.0 | The type of the packet. |
| `text` | text | always | 1.0 | The fields of the packet. |
| `cfo_hz` | number (Hz) | always | 1.0 | The carrier offset. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |

The column `id` names a row.

### Table `devices`

Struct `NrPlusDeviceRow`. A row of the devices. The row key is the device number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitter` | text | always | 1.0 | The transmitter. |
| `id` | text | always | 1.0 | The device. |
| `packets` | whole number | always | 1.0 | The packets heard. |
| `text` | text | always | 1.0 | The fields of the last packet. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the last packet. |
| `first_heard` | time | optional | 1.0 | The UTC of the first packet. |
| `last_heard` | time | optional | 1.0 | The UTC of the last packet. |

The column `id` names a row.

### Event `dect_nrplus.packet`

Struct `NrPlusPacketEvent`. The fields of a packet event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | text | always | 1.0 | The device. |
| `type` | text | always | 1.0 | The type of the packet. |
| `text` | text | always | 1.0 | The fields of the packet. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dect_nrplus_clear` | none | none |

## Try it

```sh
sdrkit run dect_nrplus --source synth:dect_nrplus
sdrkit run dect_nrplus --source iiod:192.168.2.1 --freq 1881.792
```
<!-- generated: end -->
