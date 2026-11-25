<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# mioty

mioty (TS-UNB) uplinks: the radio bursts of each frame found by their TSMA pattern, the frame decoded and checked, and the MAC fields shown; payloads stay encrypted.

Stack id: `mioty`. Group: IoT, 868 / 902 MHz.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.18 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 1 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 150 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mioty` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `min_bursts` | Bursts needed to try a pattern | whole number (count) | 5 | 3 to 24 | A TSMA pattern is tried when this many of its 24 bursts were detected where it puts them. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `bursts_detected` | Bursts detected | whole number |
| `failed_hypotheses` | Failed hypotheses | whole number |
| `devices` | Devices | whole number |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `id` (text), `pattern` (text), `psi` (whole number), `mmode` (whole number), `mac` (text), `mpdu` (text), `freq_hz` (frequency), `snr_db` (number (dB)), `bursts_matched` (whole number (of 24)) |
| `devices` | Devices | `id` (text), `frames` (whole number), `counter` (whole number), `snr_db` (number (dB)), `freq_hz` (frequency), `first_heard` (time), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `mioty.frame` | mioty frame | `id` (text), `pattern` (text), `psi` (whole number), `mac` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `ism`, version 4.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `MiotyFields`. The result fields of the mioty stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames read. |
| `bursts_detected` | whole number | always | 1.0 | Bursts detected. |
| `failed_hypotheses` | whole number | always | 1.0 | Patterns that did not decode. |
| `devices` | whole number | always | 1.0 | The devices in the table. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the last frame. It has no value before the first frame. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. It has no value before the first input. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `frames`

Struct `MiotyFrameRow`. A row of the frame log. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The UTC of the frame, when the samples are live. |
| `time_s` | number (s) | always | 1.0 | The signal time of the frame. |
| `id` | text | always | 1.0 | The device. |
| `pattern` | text | always | 1.0 | The pattern of the bursts. |
| `psi` | whole number | always | 1.0 | The bytes of the MPDU. |
| `mmode` | whole number | always | 1.0 | The MAC mode. |
| `mac` | text | always | 1.0 | The MAC. |
| `mpdu` | text | always | 1.0 | The first bytes of the MPDU as hex. |
| `freq_hz` | frequency | always | 1.0 | The frequency. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `bursts_matched` | whole number (of 24) | always | 1.0 | The bursts of the pattern that matched. |

The column `id` names a row.

### Table `devices`

Struct `MiotyDeviceRow`. A row of the devices. The row key is the device.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | text | always | 1.0 | The device. |
| `frames` | whole number | always | 1.0 | The frames heard. |
| `counter` | whole number | always | 1.0 | The last packet counter. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the last frame. |
| `freq_hz` | frequency | always | 1.0 | The frequency of the last frame. |
| `first_heard` | time | optional | 1.0 | The UTC of the first frame. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |

The column `id` names a row.

### Event `mioty.frame`

Struct `MiotyFrameEvent`. The fields of a frame event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | text | always | 1.0 | The device. |
| `pattern` | text | always | 1.0 | The pattern of the bursts. |
| `psi` | whole number | always | 1.0 | The bytes of the MPDU. |
| `mac` | text | always | 1.0 | The MAC. |
| `family` | choice | always | 1.0 | The family of the event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_mioty_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `min_bursts` | `sdrkit_mioty_get_min_bursts` | `sdrkit_mioty_set_min_bursts` |

## Try it

```sh
sdrkit run mioty --source synth:mioty
sdrkit run mioty --source iiod:192.168.2.1 --freq 868.18
```
<!-- generated: end -->
